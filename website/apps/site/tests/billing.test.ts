import { describe, expect, test } from "bun:test";
import {
  createHmac,
  generateKeyPairSync,
  sign,
  verify,
} from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { loadConfig } from "../config.ts";
import {
  CHECKOUT_CLAIM_SCHEMA,
  CHECKOUT_ENVELOPE_SCHEMA,
  FakeBillingProvider,
  createBillingRouter,
  verifyCheckoutClaim,
  verifyStripeWebhook,
} from "../src/billing.ts";

function canonical(value: Record<string, unknown>): Buffer {
  return Buffer.from(`{${Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
    .map(([key, item]) => `${JSON.stringify(key)}:${JSON.stringify(item)}`).join(",")}}`);
}

function fixture(now = new Date("2027-01-01T00:00:00.000Z")) {
  const ed = generateKeyPairSync("ed25519");
  const raw = ed.publicKey.export({ format: "der", type: "spki" }).subarray(-32);
  const claim = {
    schema: CHECKOUT_CLAIM_SCHEMA,
    claim_id: "claim_fixture_1",
    installation_binding: "installation_fixture_1",
    signing_public_key_base64url: raw.toString("base64url"),
    qualified_successful_days: 5,
    plan: "yearly",
    issued_at: now.toISOString(),
    expires_at: new Date(now.getTime() + 120_000).toISOString(),
  } as const;
  const payload = canonical(claim);
  const signature = sign(null, Buffer.concat([
    Buffer.from("tohseno.billing.checkout-claim.v1\0"), payload,
  ]), ed.privateKey);
  return {
    now,
    envelope: {
      schema: CHECKOUT_ENVELOPE_SCHEMA,
      payload_base64url: payload.toString("base64url"),
      signature_base64url: signature.toString("base64url"),
    },
  };
}

describe("private billing boundary", () => {
  test("checkout claims are short-lived and signed by the installation", () => {
    const { envelope, now } = fixture();
    expect(verifyCheckoutClaim(envelope, now).plan).toBe("yearly");
    const tampered = structuredClone(envelope);
    const signature = Buffer.from(tampered.signature_base64url, "base64url");
    signature[0] ^= 1;
    tampered.signature_base64url = signature.toString("base64url");
    expect(() => verifyCheckoutClaim(tampered, now)).toThrow("signature");
    expect(() => verifyCheckoutClaim(envelope, new Date(now.getTime() + 300_000))).toThrow("expired");
  });

  test("Stripe webhook verification is content-bound, current, and fail-closed", () => {
    const body = Buffer.from('{"id":"evt_fixture"}');
    const timestamp = 1_800_000_000;
    const digest = createHmac("sha256", "whsec_fixture")
      .update(`${timestamp}.`).update(body).digest("hex");
    const header = `t=${timestamp},v1=${"0".repeat(64)},v1=${digest}`;
    expect(verifyStripeWebhook(body, header, "whsec_fixture", timestamp)).toContain("evt_fixture");
    expect(() => verifyStripeWebhook(body, header, "wrong", timestamp)).toThrow("signature");
    expect(() => verifyStripeWebhook(body, header, "whsec_fixture", timestamp + 301)).toThrow("stale");
  });

  test("fake completion is idempotent and produces a P-256 receipt", () => {
    const { privateKey, publicKey } = generateKeyPairSync("ec", { namedCurve: "prime256v1" });
    const config = loadConfig({
      NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000",
      BILLING_ENABLED: "true", BILLING_PROVIDER: "fake", BILLING_ROOT: "/tmp/tohseno-billing-fixture",
      BILLING_MONTHLY_PRICE_ID: "price_monthly_fixture", BILLING_YEARLY_PRICE_ID: "price_yearly_fixture",
      BILLING_RECEIPT_SIGNING_PKCS8_BASE64URL: privateKey.export({ format: "der", type: "pkcs8" }).toString("base64url"),
    }).billing;
    const { envelope, now } = fixture();
    const claim = verifyCheckoutClaim(envelope, now);
    const provider = new FakeBillingProvider(config);
    const first = provider.complete("evt_fixture_1", claim, now);
    const second = provider.complete("evt_fixture_1", claim, now);
    expect(second).toEqual(first);
    const payload = Buffer.from(first.payload_base64url, "base64url");
    expect(verify("sha256", payload, { key: publicKey, dsaEncoding: "ieee-p1363" }, Buffer.from(first.signature_base64url, "base64url"))).toBe(true);
  });

  test("one signed checkout claim has one durable hosted session", async () => {
    const root = mkdtempSync(join(tmpdir(), "tohseno-billing-checkout-"));
    try {
      const signing = generateKeyPairSync("ec", { namedCurve: "prime256v1" });
      const config = loadConfig({
        NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000",
        BILLING_ENABLED: "true", BILLING_PROVIDER: "fake", BILLING_ROOT: root,
        BILLING_MONTHLY_PRICE_ID: "price_monthly_fixture", BILLING_YEARLY_PRICE_ID: "price_yearly_fixture",
        BILLING_RECEIPT_SIGNING_PKCS8_BASE64URL: signing.privateKey.export({ format: "der", type: "pkcs8" }).toString("base64url"),
      });
      const router = await createBillingRouter(config);
      const { envelope } = fixture(new Date());
      const request = () => new Request("https://tohseno.com/api/billing/v1/checkout", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ claim: envelope }),
      });
      const first = await router.fetch(request());
      const firstBody = await first.json();
      const replay = await router.fetch(request());
      expect(first.status).toBe(201);
      expect(replay.status).toBe(200);
      expect(await replay.json()).toEqual(firstBody);
      const restarted = await createBillingRouter(config);
      expect(await (await restarted.fetch(request())).json()).toEqual(firstBody);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("production configuration cannot use fake or incomplete Stripe billing", () => {
    expect(() => loadConfig({
      NODE_ENV: "production", PORT: "3000", BASE_URL: "https://tohseno.com",
      BILLING_ENABLED: "true", BILLING_PROVIDER: "fake", BILLING_ROOT: "/srv/tohseno/billing",
    })).toThrow("fake billing");
    expect(() => loadConfig({
      NODE_ENV: "production", PORT: "3000", BASE_URL: "https://tohseno.com",
      BILLING_ENABLED: "true", BILLING_ROOT: "/srv/tohseno/billing",
      BILLING_MONTHLY_PRICE_ID: "price_monthly", BILLING_YEARLY_PRICE_ID: "price_yearly",
      BILLING_RECEIPT_SIGNING_PKCS8_BASE64URL: "fixture",
    })).toThrow("Stripe credentials");
  });

  test("signed webhook receipts persist and stale delivery cannot roll them back", async () => {
    const root = mkdtempSync(join(tmpdir(), "tohseno-billing-router-"));
    try {
      const signing = generateKeyPairSync("ec", { namedCurve: "prime256v1" });
      const secret = "whsec_router_fixture";
      const config = loadConfig({
        NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000",
        BILLING_ENABLED: "true", BILLING_PROVIDER: "stripe", BILLING_ROOT: root,
        BILLING_MONTHLY_PRICE_ID: "price_monthly_fixture", BILLING_YEARLY_PRICE_ID: "price_yearly_fixture",
        BILLING_RECEIPT_SIGNING_PKCS8_BASE64URL: signing.privateKey.export({ format: "der", type: "pkcs8" }).toString("base64url"),
        STRIPE_SECRET_KEY: "sk_test_fixture", STRIPE_WEBHOOK_SECRET: secret,
      });
      const router = await createBillingRouter(config);
      const { envelope } = fixture(new Date());
      const binding = verifyCheckoutClaim(envelope).installation_binding;
      const nowSeconds = Math.floor(Date.now() / 1000);
      const deliver = async (eventId: string, created: number, paidThrough: number) => {
        const body = Buffer.from(JSON.stringify({
          id: eventId,
          created,
          type: "customer.subscription.updated",
          data: { object: {
            id: "sub_fixture",
            customer: "cus_fixture",
            metadata: { installation_binding: binding },
            current_period_start: nowSeconds - 60,
            current_period_end: paidThrough,
            cancel_at_period_end: false,
            items: { data: [{ price: { id: "price_yearly_fixture" } }] },
          } },
        }));
        const digest = createHmac("sha256", secret)
          .update(`${created}.`).update(body).digest("hex");
        return router.fetch(new Request("https://tohseno.com/api/billing/v1/webhook", {
          method: "POST",
          headers: { "stripe-signature": `t=${created},v1=${digest}` },
          body,
        }));
      };
      expect((await deliver("evt_current", nowSeconds, nowSeconds + 86_400)).status).toBe(200);
      expect((await deliver("evt_stale", nowSeconds - 1, nowSeconds + 3_600)).status).toBe(200);
      const refresh = async () => router.fetch(new Request("https://tohseno.com/api/billing/v1/refresh", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ claim: envelope }),
      }));
      const first = await (await refresh()).json() as { payload_base64url: string };
      const payload = JSON.parse(Buffer.from(first.payload_base64url, "base64url").toString("utf8"));
      expect(payload.receipt_id).toBe("receipt_evt_current");
      expect(payload.paid_through).toBe(new Date((nowSeconds + 86_400) * 1000).toISOString());
      expect((await deliver("evt_current", nowSeconds, nowSeconds + 86_400)).status).toBe(200);
      expect(await (await refresh()).json()).toEqual(first);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
