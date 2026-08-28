import { afterEach, describe, expect, test } from "bun:test";
import {
  createHmac,
  createHash,
  generateKeyPairSync,
  sign,
} from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { loadConfig } from "../config.ts";
import {
  balanceProjection,
  createManagedRouter,
  installationBinding,
  ManagedAuthority,
  type ManagedProvider,
} from "../src/managed.ts";

const roots: string[] = [];
afterEach(() => { while (roots.length) rmSync(roots.pop()!, { recursive: true, force: true }); });

function root(): string {
  const value = mkdtempSync(join(tmpdir(), "tohseno-managed-")); roots.push(value); return value;
}

function config(storage = root()) {
  const operator = "operator-fixture-secret";
  return {
    operator,
    config: loadConfig({
      NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000",
      MANAGED_COMPUTE_ENABLED: "true", MANAGED_COMPUTE_PROVIDER: "fake",
      MANAGED_COMPUTE_ROOT: storage,
      STRIPE_SECRET_KEY: "sk_test_fixture", STRIPE_WEBHOOK_SECRET: "whsec_fixture",
      STRIPE_BALANCE_PRICE_10: "price_10", STRIPE_BALANCE_PRICE_25: "price_25", STRIPE_BALANCE_PRICE_50: "price_50",
      MANAGED_CHECKOUT_SUCCESS_URL: "https://tohseno.com/billing/success",
      MANAGED_CHECKOUT_CANCEL_URL: "https://tohseno.com/billing/cancel",
      BANKR_MODEL_ALLOWLIST: "qwen3-coder,private-coder",
      TOHSENO_OPERATOR_TOKEN_SHA256: createHash("sha256").update(operator).digest("hex"),
    }),
  };
}

class FakeProvider implements ManagedProvider {
  status = 200;
  malformed = false;
  streaming = false;
  throws = false;
  calls = 0;
  async models() {
    return { data: [
      { id: "qwen3-coder", pricing: { input_microusd_per_million: 100_000, output_microusd_per_million: 300_000 }, zdr: true },
      { id: "private-coder", pricing: { input_microusd_per_million: 200_000, output_microusd_per_million: 500_000 }, private: true },
      { id: "not-allowed", pricing: { input_microusd_per_million: 1, output_microusd_per_million: 1 } },
    ] };
  }
  async credits() { return { effectiveBalanceUsd: 12.5 }; }
  async usage() { return { totals: { totalCost: 1.25 } }; }
  async completion() {
    this.calls += 1;
    if (this.throws) throw new Error("timeout");
    if (this.status !== 200) return new Response(JSON.stringify({ error: { type: "fixture" } }), { status: this.status });
    if (this.malformed) return new Response(JSON.stringify({ id: "chatcmpl_bad", choices: [] }), { status: 200 });
    const completion = { id: "chatcmpl_fixture", choices: [{ message: { content: "{}" } }], usage: { prompt_tokens: 20, completion_tokens: 10, total_tokens: 30, cost: 0.001 } };
    if (!this.streaming) return new Response(JSON.stringify(completion), { status: 200, headers: { "content-type": "application/json" } });
    return new Response(`data: ${JSON.stringify({ id: completion.id, choices: completion.choices })}\n\ndata: ${JSON.stringify({ id: completion.id, choices: [], usage: completion.usage })}\n\ndata: [DONE]\n\n`, { status: 200, headers: { "content-type": "text/event-stream" } });
  }
}

function requestBody(model = "qwen3-coder", privacy = "standard", stream = false) {
  return new TextEncoder().encode(JSON.stringify({ model, privacy, messages: [{ role: "user", content: "Build it" }], max_tokens: 100, stream }));
}

function priced<T extends { command_id: string; execution_id: string; model: string; privacy: "standard" | "zdr" | "private"; maximum_microusd: number }>(request: T) {
  return { ...request, pricing_snapshot_at: new Date().toISOString(),
    input_microusd_per_million: 120_000, output_microusd_per_million: 360_000 };
}

async function fundedAuthority(amount = 5_000_000) {
  const fixture = config(); const provider = new FakeProvider();
  const authority = new ManagedAuthority(fixture.config.managed, provider); await authority.initialize();
  const binding = "binding_fixture";
  await authority.grant(binding, amount, "welcome grant after direct onboarding", "grant_fixture", "jp");
  return { ...fixture, provider, authority, binding };
}

describe("append-only managed balance and reservations", () => {
  test("promotional grants are audited, idempotent, and distinct", async () => {
    const { authority, config: app, binding } = await fundedAuthority();
    await authority.grant(binding, 5_000_000, "welcome grant after direct onboarding", "grant_fixture", "jp");
    const balance = await balanceProjection(app.managed.root!, binding);
    expect(balance.promotional_microusd).toBe(5_000_000);
    expect(balance.paid_microusd).toBe(0);
    expect(balance.transactions).toHaveLength(1);
    expect(balance.transactions[0]?.private_operator_metadata).toBeUndefined();
    const account = readdirSync(app.managed.root!).find((name) => name.startsWith("account-"))!;
    const entry = readdirSync(join(app.managed.root!, account, "entries"))[0]!;
    expect(JSON.parse(readFileSync(join(app.managed.root!, account, "entries", entry), "utf8")).private_operator_metadata)
      .toEqual({ operator: "jp", reason: "welcome grant after direct onboarding" });
  });

  test("a stale account lock left by a crashed server is reclaimed", async () => {
    const { authority, config: app, binding } = await fundedAuthority();
    const account = readdirSync(app.managed.root!).find((name) => name.startsWith("account-"))!;
    const lock = join(app.managed.root!, account, ".lock");
    mkdirSync(lock, { mode: 0o700 });
    const stale = new Date(Date.now() - 3 * 60_000);
    utimesSync(lock, stale, stale);
    await authority.grant(binding, 1_000_000, "second audited allocation", "grant_after_crash", "jp");
    expect((await balanceProjection(app.managed.root!, binding)).promotional_microusd).toBe(6_000_000);
  });

  test("promotional revocation is a bounded compensating entry", async () => {
    const { authority, config: app, binding } = await fundedAuthority();
    await authority.revoke(binding, 2_000_000, "unused welcome allocation", "revoke_fixture", "jp");
    const balance = await balanceProjection(app.managed.root!, binding);
    expect(balance.promotional_microusd).toBe(3_000_000);
    expect(balance.transactions.some((entry) => entry.entry_type === "promotional_revocation")).toBeTrue();
    expect(balance.transactions.every((entry) => entry.private_operator_metadata === undefined)).toBeTrue();
    await expect(authority.revoke(binding, 4_000_000, "too much", "revoke_excess", "jp"))
      .rejects.toThrow("available promotional balance");
  });

  test("concurrent reservations cannot double-spend", async () => {
    const { authority, binding } = await fundedAuthority(5_000_000);
    const reserve = (id: string) => authority.reserve(binding, priced({
      command_id: id, execution_id: `execution_${id}`, model: "qwen3-coder", privacy: "standard", maximum_microusd: 4_000_000,
    }));
    const outcomes = await Promise.allSettled([reserve("one"), reserve("two")]);
    expect(outcomes.filter((value) => value.status === "fulfilled")).toHaveLength(1);
    expect(outcomes.filter((value) => value.status === "rejected")).toHaveLength(1);
  });

  test("actual usage charges once and releases the unused maximum", async () => {
    const { authority, provider, config: app, binding } = await fundedAuthority();
    const admitted = await authority.reserve(binding, priced({ command_id: "build", execution_id: "execution_build", model: "qwen3-coder", privacy: "standard", maximum_microusd: 1_000_000 }));
    const response = await authority.complete(admitted.capability, requestBody());
    expect(response.status).toBe(200); expect(provider.calls).toBe(1);
    const balance = await balanceProjection(app.managed.root!, binding);
    expect(balance.reserved_microusd).toBe(0);
    expect(balance.transactions.filter((entry) => entry.entry_type === "inference_charge")).toHaveLength(1);
    await expect(authority.complete(admitted.capability, requestBody())).rejects.toThrow("already used");
  });

  test("one implementation and one repair share one approved maximum", async () => {
    const { authority, provider, config: app, binding } = await fundedAuthority();
    const request = priced({ command_id: "bounded_repair", execution_id: "execution_bounded_repair", model: "qwen3-coder", privacy: "standard", maximum_microusd: 1_000_000 });
    const implementation = await authority.reserve(binding, request);
    expect((await authority.complete(implementation.capability, requestBody())).status).toBe(200);
    const repair = await authority.reserve(binding, request);
    expect(repair.reservation.maximum_microusd).toBeLessThan(1_000_000);
    expect((await authority.complete(repair.capability, requestBody())).status).toBe(200);
    await expect(authority.reserve(binding, request)).rejects.toThrow("invocation count");
    expect(provider.calls).toBe(2);
    const balance = await balanceProjection(app.managed.root!, binding);
    expect(balance.reserved_microusd).toBe(0);
    const charged = -balance.transactions.filter((entry) => entry.entry_type === "inference_charge").reduce((sum, entry) => sum + entry.amount_microusd, 0);
    expect(charged).toBeLessThanOrEqual(1_000_000);
  });

  test("maximum spend is enforced before provider use", async () => {
    const { authority, provider, binding } = await fundedAuthority();
    const admitted = await authority.reserve(binding, priced({ command_id: "capped", execution_id: "execution_capped", model: "qwen3-coder", privacy: "standard", maximum_microusd: 1 }));
    await expect(authority.complete(admitted.capability, requestBody())).rejects.toThrow("approved maximum");
    expect(provider.calls).toBe(0);
    await expect(authority.complete(admitted.capability, requestBody())).rejects.toThrow("approved maximum");
    expect(provider.calls).toBe(0);
  });

  test("expired capabilities recover unused holds and flag used crash windows", async () => {
    const unused = await fundedAuthority();
    const unusedRequest = priced({ command_id: "expired_unused", execution_id: "execution_expired_unused", model: "qwen3-coder", privacy: "standard" as const, maximum_microusd: 1_000_000 });
    const first = await unused.authority.reserve(unused.binding, unusedRequest);
    expireCapability(unused.config.managed.root!, first.capability, false);
    const retry = await unused.authority.reserve(unused.binding, unusedRequest);
    expect(retry.reservation.call_index).toBe(2);
    const unusedBalance = await balanceProjection(unused.config.managed.root!, unused.binding);
    expect(unusedBalance.transactions.some((entry) => entry.entry_type === "reservation_release")).toBeTrue();

    const interrupted = await fundedAuthority();
    const interruptedRequest = priced({ command_id: "expired_used", execution_id: "execution_expired_used", model: "qwen3-coder", privacy: "standard" as const, maximum_microusd: 1_000_000 });
    const admitted = await interrupted.authority.reserve(interrupted.binding, interruptedRequest);
    expireCapability(interrupted.config.managed.root!, admitted.capability, true);
    await expect(interrupted.authority.reserve(interrupted.binding, interruptedRequest))
      .rejects.toThrow("provider reconciliation");
    const interruptedBalance = await balanceProjection(interrupted.config.managed.root!, interrupted.binding);
    expect(interruptedBalance.reserved_microusd).toBe(1_000_000);
    expect(interruptedBalance.transactions.some((entry) =>
      entry.entry_type === "provider_reconciliation" && entry.reconciliation_status === "pending")).toBeTrue();
  });
});

function expireCapability(storage: string, capability: string, used: boolean): void {
  const account = readdirSync(storage).find((name) => name.startsWith("account-"))!;
  const hash = createHash("sha256").update(capability).digest("hex");
  const directory = join(storage, account, "capabilities");
  const path = join(directory, `capability-${hash}.json`);
  const record = JSON.parse(readFileSync(path, "utf8"));
  record.expires_at = "2000-01-01T00:00:00.000Z";
  writeFileSync(path, JSON.stringify(record));
  if (used) writeFileSync(join(directory, `used-${hash}`), record.reservation_id);
}

describe("Bankr-compatible failure and response handling", () => {
  test("TOHSENO rate limits per installation before forwarding and releases the hold", async () => {
    const fixture = config(); fixture.config.managed.rateLimitPerMinute = 1;
    const provider = new FakeProvider();
    const authority = new ManagedAuthority(fixture.config.managed, provider); await authority.initialize();
    const binding = "binding_rate_limit";
    await authority.grant(binding, 5_000_000, "welcome grant after direct onboarding", "grant_rate", "jp");
    const first = await authority.reserve(binding, priced({ command_id: "rate_one", execution_id: "execution_rate_one", model: "qwen3-coder", privacy: "standard", maximum_microusd: 1_000_000 }));
    expect((await authority.complete(first.capability, requestBody())).status).toBe(200);
    const second = await authority.reserve(binding, priced({ command_id: "rate_two", execution_id: "execution_rate_two", model: "qwen3-coder", privacy: "standard", maximum_microusd: 1_000_000 }));
    expect((await authority.complete(second.capability, requestBody())).status).toBe(429);
    expect(provider.calls).toBe(1);
    expect((await balanceProjection(fixture.config.managed.root!, binding)).reserved_microusd).toBe(0);
  });

  test.each([401, 402, 429])("HTTP %s is recoverable and releases held value", async (status) => {
    const { authority, provider, config: app, binding } = await fundedAuthority(); provider.status = status;
    const admitted = await authority.reserve(binding, priced({ command_id: `status_${status}`, execution_id: `execution_${status}`, model: "qwen3-coder", privacy: "standard", maximum_microusd: 1_000_000 }));
    const response = await authority.complete(admitted.capability, requestBody());
    expect([429, 503]).toContain(response.status);
    expect((await balanceProjection(app.managed.root!, binding)).reserved_microusd).toBe(0);
  });

  test("5xx, timeout, and malformed usage hold an ambiguous reservation", async () => {
    for (const mode of ["500", "timeout", "malformed"] as const) {
      const { authority, provider, config: app, binding } = await fundedAuthority();
      if (mode === "500") provider.status = 500;
      if (mode === "timeout") provider.throws = true;
      if (mode === "malformed") provider.malformed = true;
      const admitted = await authority.reserve(binding, priced({ command_id: mode, execution_id: `execution_${mode}`, model: "qwen3-coder", privacy: "standard", maximum_microusd: 1_000_000 }));
      const response = await authority.complete(admitted.capability, requestBody());
      expect(response.status).toBeGreaterThanOrEqual(500);
      expect((await balanceProjection(app.managed.root!, binding)).reserved_microusd).toBe(1_000_000);
    }
  });

  test("operator reconciliation explicitly releases an ambiguous provider outcome", async () => {
    const { authority, provider, config: app, binding } = await fundedAuthority(); provider.throws = true;
    const admitted = await authority.reserve(binding, priced({ command_id: "reconcile_release", execution_id: "execution_reconcile_release", model: "qwen3-coder", privacy: "standard", maximum_microusd: 1_000_000 }));
    expect((await authority.complete(admitted.capability, requestBody())).status).toBe(503);
    expect((await balanceProjection(app.managed.root!, binding)).reserved_microusd).toBe(1_000_000);
    await authority.reconcile(binding, admitted.reservation.reservation_id, "release", 0,
      "Bankr usage confirms no provider charge", "operator_release_fixture", "jp");
    const balance = await balanceProjection(app.managed.root!, binding);
    expect(balance.reserved_microusd).toBe(0);
    expect(balance.transactions.some((entry) => entry.reconciliation_status === "settled")).toBeTrue();
  });

  test("allowlist, server pricing, privacy, and streaming are enforced", async () => {
    const { authority, provider, binding } = await fundedAuthority();
    const catalog = await authority.catalog();
    expect(catalog.map((model) => model.model)).toEqual(["qwen3-coder", "private-coder"]);
    expect(catalog[0]?.input_microusd_per_million).toBe(120_000);
    await expect(authority.reserve(binding, { ...priced({ command_id: "price_bad", execution_id: "execution_price_bad", model: "qwen3-coder", privacy: "standard", maximum_microusd: 1_000_000 }), input_microusd_per_million: 1 }))
      .rejects.toThrow("server price");
    await expect(authority.reserve(binding, priced({ command_id: "private_bad", execution_id: "execution_private_bad", model: "qwen3-coder", privacy: "private", maximum_microusd: 1_000_000 }))).rejects.toThrow("privacy");
    provider.streaming = true;
    const admitted = await authority.reserve(binding, priced({ command_id: "stream", execution_id: "execution_stream", model: "qwen3-coder", privacy: "zdr", maximum_microusd: 1_000_000 }));
    const response = await authority.complete(admitted.capability, requestBody("qwen3-coder", "zdr", true));
    expect(response.status).toBe(200); expect(response.headers.get("content-type")).toContain("text/event-stream");
  });
});

function canonical(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  return `{${Object.entries(value as Record<string, unknown>).sort(([a], [b]) => a.localeCompare(b)).map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`).join(",")}}`;
}

function signer() {
  const keys = generateKeyPairSync("ed25519");
  const raw = (keys.publicKey.export({ format: "der", type: "spki" }) as Buffer).subarray(-32).toString("base64url");
  const binding = installationBinding(raw);
  return {
    binding,
    claim(action: string, request: unknown) {
      const now = new Date();
      const payload = canonical({
        action, claim_id: `claim_${crypto.randomUUID().replaceAll("-", "")}`, expires_at: new Date(now.getTime() + 120_000).toISOString(),
        installation_binding: binding, issued_at: now.toISOString(), request_digest: createHash("sha256").update(canonical(request)).digest("base64url"),
        schema: "tohseno.private-managed-claim/1", signing_public_key_base64url: raw,
      });
      return { schema: "tohseno.private-managed-claim-envelope/1", payload_base64url: Buffer.from(payload).toString("base64url"),
        signature_base64url: sign(null, Buffer.concat([Buffer.from("tohseno.managed.claim.v1\0"), Buffer.from(payload)]), keys.privateKey).toString("base64url") };
    },
  };
}

function webhook(event: unknown, created = Math.floor(Date.now() / 1000)): Request {
  const body = JSON.stringify(event); const signature = createHmac("sha256", "whsec_fixture").update(`${created}.${body}`).digest("hex");
  return new Request("http://localhost:3000/api/managed/v1/stripe/webhook", { method: "POST", body, headers: { "stripe-signature": `t=${created},v1=${signature}` } });
}

describe("Stripe and operator HTTP boundaries", () => {
  test("reordered checkout success credits once; refunds and disputes compensate", async () => {
    const fixture = config(); const provider = new FakeProvider(); const owner = signer();
    const session = { id: "cs_fixture", payment_status: "paid", currency: "usd", amount_total: 1000, payment_intent: "pi_fixture",
      metadata: { installation_binding: owner.binding, pack_id: "usd_10" }, line_items: { data: [{ price: { id: "price_10" } }] } };
    const fetchStub = async () => new Response(JSON.stringify(session), { status: 200, headers: { "content-type": "application/json" } });
    const router = await createManagedRouter(fixture.config, provider, fetchStub as unknown as typeof fetch);
    for (const [id, type] of [["evt_first", "checkout.session.async_payment_succeeded"], ["evt_second", "checkout.session.completed"], ["evt_first", "checkout.session.async_payment_succeeded"]]) {
      expect((await router.fetch(webhook({ id, created: 1, type, data: { object: { id: "cs_fixture" } } }))).status).toBe(200);
    }
    expect((await balanceProjection(fixture.config.managed.root!, owner.binding)).paid_microusd).toBe(10_000_000);
    await router.fetch(webhook({ id: "evt_refund", created: 2, type: "charge.refunded", data: { object: { payment_intent: "pi_fixture", amount_refunded: 200 } } }));
    await router.fetch(webhook({ id: "evt_refund_more", created: 3, type: "charge.refunded", data: { object: { payment_intent: "pi_fixture", amount_refunded: 300 } } }));
    await router.fetch(webhook({ id: "evt_dispute", created: 4, type: "charge.dispute.created", data: { object: { id: "dp_fixture", payment_intent: "pi_fixture", amount: 100 } } }));
    await router.fetch(webhook({ id: "evt_won", created: 5, type: "charge.dispute.closed", data: { object: { id: "dp_fixture", payment_intent: "pi_fixture", amount: 100, status: "won" } } }));
    await router.fetch(webhook({ id: "evt_won_first", created: 6, type: "charge.dispute.closed", data: { object: { id: "dp_reordered", payment_intent: "pi_fixture", amount: 100, status: "won" } } }));
    await router.fetch(webhook({ id: "evt_created_late", created: 7, type: "charge.dispute.created", data: { object: { id: "dp_reordered", payment_intent: "pi_fixture", amount: 100 } } }));
    expect((await balanceProjection(fixture.config.managed.root!, owner.binding)).paid_microusd).toBe(7_000_000);
  });

  test("operator grants require authentication and provider secrets never enter projections", async () => {
    const fixture = config(); const provider = new FakeProvider(); const router = await createManagedRouter(fixture.config, provider);
    const body = JSON.stringify({ installation_binding: "binding_operator", amount_microusd: 5_000_000, reason: "welcome grant after direct onboarding", idempotency_key: "operator_grant", operator: "jp" });
    expect((await router.fetch(new Request("http://localhost:3000/api/managed/v1/operator/grants", { method: "POST", body }))).status).toBe(401);
    expect((await router.fetch(new Request("http://localhost:3000/api/managed/v1/operator/grants", { method: "POST", body, headers: { "x-tohseno-operator-token": fixture.operator } }))).status).toBe(201);
    const revoke = JSON.stringify({ installation_binding: "binding_operator", amount_microusd: 1_000_000, reason: "corrected welcome allocation", idempotency_key: "operator_revoke", operator: "jp" });
    expect((await router.fetch(new Request("http://localhost:3000/api/managed/v1/operator/revocations", { method: "POST", body: revoke }))).status).toBe(401);
    expect((await router.fetch(new Request("http://localhost:3000/api/managed/v1/operator/revocations", { method: "POST", body: revoke, headers: { "x-tohseno-operator-token": fixture.operator } }))).status).toBe(201);
    expect((await balanceProjection(fixture.config.managed.root!, "binding_operator")).promotional_microusd).toBe(4_000_000);
    const serialized = JSON.stringify(await balanceProjection(fixture.config.managed.root!, "binding_operator"));
    expect(serialized).not.toContain("sk_test_fixture"); expect(serialized).not.toContain("bk_"); expect(serialized).not.toContain(fixture.operator);
  });
});
