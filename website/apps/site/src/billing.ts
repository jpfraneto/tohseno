import {
  createHmac,
  createHash,
  createPrivateKey,
  createPublicKey,
  sign,
  timingSafeEqual,
  verify,
} from "node:crypto";
import { lstat, mkdir, readFile, readdir, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type { BillingConfig } from "../config.ts";
import type { AppConfig } from "../config.ts";
import { withSecurityHeaders } from "./security.ts";

export const CHECKOUT_CLAIM_SCHEMA = "tohseno.private-checkout-claim/1";
export const CHECKOUT_ENVELOPE_SCHEMA = "tohseno.private-checkout-envelope/1";
export const RECEIPT_SCHEMA = "tohseno.private-entitlement-receipt/1";
export const RECEIPT_ENVELOPE_SCHEMA = "tohseno.private-entitlement-envelope/1";
const CLAIM_DOMAIN = Buffer.from("tohseno.billing.checkout-claim.v1\0");
const MAX_LIVE_CHECKOUT_RECORDS = 4_096;
const MAX_CHECKOUT_CLEANUP_SCAN = 8_192;

export type BillingPlan = "monthly" | "yearly";

export interface CheckoutClaim {
  schema: typeof CHECKOUT_CLAIM_SCHEMA;
  claim_id: string;
  installation_binding: string;
  signing_public_key_base64url: string;
  qualified_successful_days: 5;
  plan: BillingPlan;
  issued_at: string;
  expires_at: string;
}

export interface CheckoutClaimEnvelope {
  schema: typeof CHECKOUT_ENVELOPE_SCHEMA;
  payload_base64url: string;
  signature_base64url: string;
}

export interface EntitlementReceiptPayload {
  schema: typeof RECEIPT_SCHEMA;
  receipt_id: string;
  entitlement_id: string;
  installation_binding: string;
  plan: BillingPlan;
  issued_at: string;
  paid_through: string;
  cancellation_at_period_end: boolean;
  provider_revision: number;
}

export interface SignedEntitlementReceipt {
  schema: typeof RECEIPT_ENVELOPE_SCHEMA;
  payload_base64url: string;
  signature_base64url: string;
}

function decode(value: string, maximum = 32_768): Buffer {
  if (!/^[A-Za-z0-9_-]+$/.test(value)) throw new Error("billing value is not base64url");
  const bytes = Buffer.from(value, "base64url");
  if (!bytes.length || bytes.length > maximum) throw new Error("billing value is empty or oversized");
  return bytes;
}

function exactKeys(value: unknown, keys: string[], label: string): asserts value is Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${label} is invalid`);
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    throw new Error(`${label} contains missing or unexpected fields`);
  }
}

function boundedIdentifier(value: unknown, label: string): asserts value is string {
  if (typeof value !== "string" || !/^[A-Za-z0-9_-]{1,160}$/.test(value)) {
    throw new Error(`${label} is invalid`);
  }
}

function canonical(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  return `{${Object.entries(value as Record<string, unknown>)
    .sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)
    .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
    .join(",")}}`;
}

function ed25519PublicKey(raw: Buffer) {
  if (raw.length !== 32) throw new Error("checkout public key is invalid");
  return createPublicKey({
    key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), raw]),
    format: "der",
    type: "spki",
  });
}

export function verifyCheckoutClaim(
  envelopeValue: unknown,
  now = new Date(),
): CheckoutClaim {
  exactKeys(envelopeValue, ["schema", "payload_base64url", "signature_base64url"], "checkout envelope");
  if (envelopeValue.schema !== CHECKOUT_ENVELOPE_SCHEMA) throw new Error("checkout envelope schema is unsupported");
  if (typeof envelopeValue.payload_base64url !== "string" || typeof envelopeValue.signature_base64url !== "string") {
    throw new Error("checkout envelope encoding is invalid");
  }
  const payload = decode(envelopeValue.payload_base64url);
  const signature = decode(envelopeValue.signature_base64url, 64);
  if (signature.length !== 64) throw new Error("checkout signature is invalid");
  let claim: unknown;
  try { claim = JSON.parse(payload.toString("utf8")); } catch { throw new Error("checkout claim is invalid JSON"); }
  exactKeys(claim, [
    "schema", "claim_id", "installation_binding", "signing_public_key_base64url",
    "plan", "issued_at", "expires_at", "qualified_successful_days",
  ], "checkout claim");
  if (claim.schema !== CHECKOUT_CLAIM_SCHEMA) throw new Error("checkout claim schema is unsupported");
  if (canonical(claim) !== payload.toString("utf8")) throw new Error("checkout claim is noncanonical");
  boundedIdentifier(claim.claim_id, "checkout claim identifier");
  boundedIdentifier(claim.installation_binding, "installation binding");
  if (claim.plan !== "monthly" && claim.plan !== "yearly") throw new Error("checkout plan is invalid");
  if (claim.qualified_successful_days !== 5) throw new Error("checkout qualification is invalid");
  if (typeof claim.issued_at !== "string" || typeof claim.expires_at !== "string") throw new Error("checkout timestamps are invalid");
  const issued = Date.parse(claim.issued_at);
  const expires = Date.parse(claim.expires_at);
  if (!Number.isFinite(issued) || !Number.isFinite(expires) || issued > now.getTime() + 30_000
      || expires <= now.getTime() || expires - issued > 5 * 60_000) {
    throw new Error("checkout claim has expired or has an invalid lifetime");
  }
  if (typeof claim.signing_public_key_base64url !== "string") throw new Error("checkout public key is invalid");
  const publicKey = ed25519PublicKey(decode(claim.signing_public_key_base64url, 32));
  if (!verify(null, Buffer.concat([CLAIM_DOMAIN, payload]), publicKey, signature)) {
    throw new Error("checkout claim signature is invalid");
  }
  return claim as unknown as CheckoutClaim;
}

export function verifyStripeWebhook(
  body: Uint8Array,
  signatureHeader: string,
  secret: string,
  nowSeconds = Math.floor(Date.now() / 1000),
): string {
  if (!body.length || body.length > 256 * 1024) throw new Error("billing webhook is empty or oversized");
  const parts = signatureHeader.split(",").map((part) => part.split("=", 2));
  const timestamps = parts.filter(([key]) => key === "t").map(([, value]) => value);
  const signatures = parts.filter(([key]) => key === "v1").map(([, value]) => value);
  const timestampText = timestamps[0];
  if (timestamps.length !== 1 || !timestampText || !/^\d+$/.test(timestampText)
      || !signatures.length || signatures.some((value) => !value || !/^[0-9a-f]{64}$/.test(value))) {
    throw new Error("billing webhook signature is invalid");
  }
  const timestamp = Number(timestampText);
  if (Math.abs(nowSeconds - timestamp) > 300) throw new Error("billing webhook signature is stale");
  const expected = createHmac("sha256", secret)
    .update(`${timestamp}.`)
    .update(body)
    .digest();
  if (!signatures.some((presented) => timingSafeEqual(expected, Buffer.from(presented, "hex")))) {
    throw new Error("billing webhook signature is invalid");
  }
  return new TextDecoder("utf-8", { fatal: true }).decode(body);
}

export function signEntitlementReceipt(
  payload: EntitlementReceiptPayload,
  privateKeyPkcs8Base64url: string,
): SignedEntitlementReceipt {
  exactKeys(payload, [
    "schema", "receipt_id", "entitlement_id", "installation_binding", "plan",
    "issued_at", "paid_through", "cancellation_at_period_end", "provider_revision",
  ], "entitlement receipt");
  if (payload.schema !== RECEIPT_SCHEMA) throw new Error("entitlement receipt schema is unsupported");
  boundedIdentifier(payload.receipt_id, "receipt identifier");
  boundedIdentifier(payload.entitlement_id, "entitlement identifier");
  boundedIdentifier(payload.installation_binding, "installation binding");
  if (payload.plan !== "monthly" && payload.plan !== "yearly") throw new Error("entitlement plan is invalid");
  if (!Number.isSafeInteger(payload.provider_revision) || payload.provider_revision < 1) throw new Error("provider revision is invalid");
  const bytes = Buffer.from(canonical(payload));
  const privateKey = createPrivateKey({
    key: decode(privateKeyPkcs8Base64url),
    format: "der",
    type: "pkcs8",
  });
  const signature = sign("sha256", bytes, { key: privateKey, dsaEncoding: "ieee-p1363" });
  return {
    schema: RECEIPT_ENVELOPE_SCHEMA,
    payload_base64url: bytes.toString("base64url"),
    signature_base64url: signature.toString("base64url"),
  };
}

export class FakeBillingProvider {
  readonly receipts = new Map<string, SignedEntitlementReceipt>();
  readonly events = new Set<string>();
  private readonly signingKey: string;

  constructor(private readonly config: BillingConfig) {
    if (config.provider !== "fake" || !config.receiptSigningPrivateKey) {
      throw new Error("fake billing provider is not configured");
    }
    this.signingKey = config.receiptSigningPrivateKey;
  }

  complete(eventId: string, claim: CheckoutClaim, now: Date): SignedEntitlementReceipt {
    boundedIdentifier(eventId, "provider event identifier");
    const existing = this.receipts.get(claim.installation_binding);
    if (this.events.has(eventId) && existing) return existing;
    const paidThrough = new Date(now);
    paidThrough.setUTCDate(paidThrough.getUTCDate() + (claim.plan === "yearly" ? 365 : 31));
    const receipt = signEntitlementReceipt({
      schema: RECEIPT_SCHEMA,
      receipt_id: `receipt_${eventId}`,
      entitlement_id: `entitlement_${claim.installation_binding}`,
      installation_binding: claim.installation_binding,
      plan: claim.plan,
      issued_at: now.toISOString(),
      paid_through: paidThrough.toISOString(),
      cancellation_at_period_end: false,
      provider_revision: 1,
    }, this.signingKey);
    this.events.add(eventId);
    this.receipts.set(claim.installation_binding, receipt);
    return receipt;
  }
}

export interface BillingRouter {
  handles(pathname: string): boolean;
  fetch(request: Request): Promise<Response>;
}

type StoredEntitlement = {
  schema: "tohseno.private-billing-record/1";
  installation_binding: string;
  customer_id: string;
  subscription_id: string;
  provider_event_id: string;
  provider_revision: number;
  receipt: SignedEntitlementReceipt;
};

type StoredCheckout = {
  schema: "tohseno.private-checkout-record/1";
  claim_id: string;
  installation_binding: string;
  plan: BillingPlan;
  expires_at: string;
  checkout_url: string;
};

function response(value: unknown, status = 200): Response {
  return withSecurityHeaders(new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" },
  }));
}

async function boundedBytes(message: Request | Response, maximum: number): Promise<Uint8Array> {
  const contentLength = message.headers.get("content-length");
  if (contentLength !== null) {
    const declared = Number(contentLength);
    if (!Number.isSafeInteger(declared) || declared < 0 || declared > maximum) {
      throw new Error("billing body is oversized");
    }
  }
  if (!message.body) throw new Error("billing body is empty");
  const reader = message.body.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    length += value.byteLength;
    if (length > maximum) {
      await reader.cancel();
      throw new Error("billing body is oversized");
    }
    chunks.push(value);
  }
  if (!length) throw new Error("billing body is empty");
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
}

async function boundedJson(request: Request): Promise<unknown> {
  const bytes = await boundedBytes(request, 32 * 1024);
  try { return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); }
  catch { throw new Error("billing request is invalid JSON"); }
}

function storageName(binding: string): string {
  return createHash("sha256").update("tohseno.billing.storage.v1\0").update(binding).digest("hex");
}

function checkoutStorageName(claimId: string): string {
  return createHash("sha256").update("tohseno.billing.checkout.v1\0").update(claimId).digest("hex");
}

async function ensureStore(root: string): Promise<void> {
  await mkdir(root, { recursive: true, mode: 0o700 });
  const metadata = await lstat(root);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) throw new Error("billing storage root is unsafe");
}

async function storeRecord(root: string, record: StoredEntitlement): Promise<void> {
  const path = join(root, `${storageName(record.installation_binding)}.json`);
  const stage = `${path}.${crypto.randomUUID()}.tmp`;
  await writeFile(stage, JSON.stringify(record), { flag: "wx", mode: 0o600 });
  await rename(stage, path);
}

async function cleanupCheckouts(root: string, now: Date): Promise<void> {
  const entries = await readdir(root, { withFileTypes: true });
  const checkoutEntries = entries.filter((entry) => /^checkout-[0-9a-f]{64}\.json$/.test(entry.name));
  if (checkoutEntries.length > MAX_CHECKOUT_CLEANUP_SCAN) {
    throw new Error("checkout record capacity is exceeded");
  }
  let live = 0;
  for (const entry of checkoutEntries) {
    if (!entry.isFile() || entry.isSymbolicLink()) throw new Error("checkout record is unsafe");
    const path = join(root, entry.name);
    const metadata = await lstat(path);
    if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 8 * 1024) {
      throw new Error("checkout record is unsafe");
    }
    const value = JSON.parse(await readFile(path, "utf8")) as Record<string, unknown>;
    if (typeof value.expires_at !== "string") throw new Error("checkout record is invalid");
    const expires = Date.parse(value.expires_at);
    if (!Number.isFinite(expires)) throw new Error("checkout record is invalid");
    if (expires <= now.getTime()) await rm(path);
    else live += 1;
  }
  if (live >= MAX_LIVE_CHECKOUT_RECORDS) throw new Error("checkout record capacity is exceeded");
}

async function storeCheckout(root: string, record: StoredCheckout, now: Date): Promise<void> {
  await cleanupCheckouts(root, now);
  const path = join(root, `checkout-${checkoutStorageName(record.claim_id)}.json`);
  const stage = `${path}.${crypto.randomUUID()}.tmp`;
  await writeFile(stage, JSON.stringify(record), { flag: "wx", mode: 0o600 });
  await rename(stage, path);
}

async function loadCheckout(
  root: string,
  claim: CheckoutClaim,
  config: AppConfig,
): Promise<StoredCheckout | null> {
  try {
    const path = join(root, `checkout-${checkoutStorageName(claim.claim_id)}.json`);
    const metadata = await lstat(path);
    if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 8 * 1024) {
      throw new Error("checkout record is unsafe");
    }
    const value = JSON.parse(await readFile(path, "utf8")) as StoredCheckout;
    exactKeys(value, [
      "schema", "claim_id", "installation_binding", "plan", "expires_at", "checkout_url",
    ], "checkout record");
    if (value.schema !== "tohseno.private-checkout-record/1"
        || value.claim_id !== claim.claim_id
        || value.installation_binding !== claim.installation_binding
        || value.plan !== claim.plan
        || value.expires_at !== claim.expires_at) {
      throw new Error("checkout record conflicts with the signed claim");
    }
    const expectedFakeUrl = `${config.baseUrl}/api/billing/v1/test/checkout/${claim.claim_id}`;
    if (config.billing.provider === "fake") {
      if (value.checkout_url !== expectedFakeUrl) throw new Error("checkout record URL is invalid");
    } else if (hostedUrl(value.checkout_url, "checkout.stripe.com") !== value.checkout_url) {
      throw new Error("checkout record URL is invalid");
    }
    return value;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return null;
    throw error;
  }
}

async function loadRecord(root: string, binding: string): Promise<StoredEntitlement | null> {
  try {
    const path = join(root, `${storageName(binding)}.json`);
    const metadata = await lstat(path);
    if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 64 * 1024) throw new Error("billing record is unsafe");
    const value = JSON.parse(await readFile(path, "utf8")) as StoredEntitlement;
    exactKeys(value, [
      "schema", "installation_binding", "customer_id", "subscription_id",
      "provider_event_id", "provider_revision", "receipt",
    ], "billing record");
    if (value.schema !== "tohseno.private-billing-record/1" || value.installation_binding !== binding) {
      throw new Error("billing record is invalid");
    }
    stripeIdentifier(value.customer_id, "cus");
    stripeIdentifier(value.subscription_id, "sub");
    stripeIdentifier(value.provider_event_id, "evt");
    if (!Number.isSafeInteger(value.provider_revision) || value.provider_revision < 1) {
      throw new Error("billing record revision is invalid");
    }
    exactKeys(value.receipt, ["schema", "payload_base64url", "signature_base64url"], "billing receipt");
    if (value.receipt.schema !== RECEIPT_ENVELOPE_SCHEMA) throw new Error("billing receipt is invalid");
    return value;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return null;
    throw error;
  }
}

async function stripePost(
  config: BillingConfig,
  pathname: string,
  values: URLSearchParams,
  idempotencyKey?: string,
): Promise<Record<string, unknown>> {
  if (!config.stripeSecretKey) throw new Error("Stripe is not configured");
  const result = await fetch(`https://api.stripe.com/v1/${pathname}`, {
    method: "POST",
    redirect: "manual",
    headers: new Headers([
      ["authorization", `Bearer ${config.stripeSecretKey}`],
      ["content-type", "application/x-www-form-urlencoded"],
      ...(idempotencyKey ? [["idempotency-key", idempotencyKey] as [string, string]] : []),
    ]),
    body: values,
  });
  if (!result.ok || result.status >= 300) throw new Error("Stripe refused the billing request");
  const bytes = await boundedBytes(result, 256 * 1024);
  const value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Stripe response is invalid");
  return value as Record<string, unknown>;
}

function claimFromBody(value: unknown, now = new Date()): CheckoutClaim {
  exactKeys(value, ["claim"], "billing request");
  return verifyCheckoutClaim(value.claim, now);
}

function stripeIdentifier(value: unknown, prefix: string): string {
  if (typeof value !== "string" || !new RegExp(`^${prefix}_[A-Za-z0-9_]{1,160}$`).test(value)) {
    throw new Error("Stripe response identifier is invalid");
  }
  return value;
}

function hostedUrl(value: unknown, hostname: string): string {
  if (typeof value !== "string") throw new Error("hosted billing URL is invalid");
  const url = new URL(value);
  if (url.protocol !== "https:" || url.hostname !== hostname || url.port
      || url.username || url.password) {
    throw new Error("hosted billing URL is invalid");
  }
  return url.href;
}

function providerRevision(eventId: string, created: unknown): number {
  const seconds = Number(created);
  if (!Number.isSafeInteger(seconds) || seconds < 1) {
    throw new Error("Stripe event revision is invalid");
  }
  const tieBreaker = Number.parseInt(
    createHash("sha256").update(eventId).digest("hex").slice(0, 5),
    16,
  );
  const revision = seconds * 1_048_576 + tieBreaker;
  if (!Number.isSafeInteger(revision)) throw new Error("Stripe event revision is invalid");
  return revision;
}

function planForPrice(config: BillingConfig, price: unknown): BillingPlan {
  if (price === config.monthlyPriceId) return "monthly";
  if (price === config.yearlyPriceId) return "yearly";
  throw new Error("Stripe subscription uses an unknown price");
}

export async function createBillingRouter(config: AppConfig): Promise<BillingRouter> {
  const prefix = "/api/billing/v1/";
  const billing = config.billing;
  if (billing.enabled && billing.root) await ensureStore(billing.root);
  const fake = billing.enabled && billing.provider === "fake" ? new FakeBillingProvider(billing) : null;

  return {
    handles: (pathname) => pathname.startsWith(prefix),
    async fetch(request): Promise<Response> {
      if (!billing.enabled || !billing.root) return response({ error: "TOHSENO billing is not active" }, 503);
      const url = new URL(request.url);
      try {
        if (url.pathname === `${prefix}checkout` && request.method === "POST") {
          const claim = claimFromBody(await boundedJson(request));
          const replay = await loadCheckout(billing.root, claim, config);
          if (replay) {
            return response({
              schema: "tohseno.private-checkout-session/1",
              checkout_url: replay.checkout_url,
            }, 200);
          }
          let checkoutUrl: string;
          if (fake) {
            checkoutUrl = `${config.baseUrl}/api/billing/v1/test/checkout/${claim.claim_id}`;
          } else {
            const price = claim.plan === "monthly" ? billing.monthlyPriceId : billing.yearlyPriceId;
            const values = new URLSearchParams({
              mode: "subscription",
              "line_items[0][price]": price ?? "",
              "line_items[0][quantity]": "1",
              success_url: `${config.baseUrl}/?billing=complete`,
              cancel_url: `${config.baseUrl}/?billing=cancelled`,
              client_reference_id: claim.claim_id,
              "subscription_data[metadata][installation_binding]": claim.installation_binding,
            });
            const session = await stripePost(billing, "checkout/sessions", values, claim.claim_id);
            checkoutUrl = hostedUrl(session.url, "checkout.stripe.com");
          }
          await storeCheckout(billing.root, {
            schema: "tohseno.private-checkout-record/1",
            claim_id: claim.claim_id,
            installation_binding: claim.installation_binding,
            plan: claim.plan,
            expires_at: claim.expires_at,
            checkout_url: checkoutUrl,
          }, new Date());
          return response({
            schema: "tohseno.private-checkout-session/1",
            checkout_url: checkoutUrl,
          }, 201);
        }

        if (url.pathname === `${prefix}refresh` && request.method === "POST") {
          const claim = claimFromBody(await boundedJson(request));
          const record = await loadRecord(billing.root, claim.installation_binding);
          return record ? response(record.receipt) : response({ error: "No verified entitlement is available" }, 404);
        }

        if (url.pathname === `${prefix}portal` && request.method === "POST") {
          const claim = claimFromBody(await boundedJson(request));
          const record = await loadRecord(billing.root, claim.installation_binding);
          if (!record) return response({ error: "No billing account is available" }, 404);
          if (fake) return response({ schema: "tohseno.private-billing-portal/1", portal_url: `${config.baseUrl}/` });
          const portal = await stripePost(billing, "billing_portal/sessions", new URLSearchParams({
            customer: record.customer_id,
            return_url: config.baseUrl,
          }));
          return response({
            schema: "tohseno.private-billing-portal/1",
            portal_url: hostedUrl(portal.url, "billing.stripe.com"),
          }, 201);
        }

        if (url.pathname === `${prefix}webhook` && request.method === "POST" && !fake) {
          const bytes = await boundedBytes(request, 256 * 1024);
          const text = verifyStripeWebhook(bytes, request.headers.get("stripe-signature") ?? "", billing.stripeWebhookSecret ?? "");
          const event = JSON.parse(text) as Record<string, any>;
          const eventId = stripeIdentifier(event.id, "evt");
          const revision = providerRevision(eventId, event.created);
          const subscription = event.data?.object;
          if (!String(event.type).startsWith("customer.subscription.")) return response({ received: true });
          const binding = subscription?.metadata?.installation_binding;
          boundedIdentifier(binding, "installation binding");
          const existing = await loadRecord(billing.root, binding);
          if (existing) {
            const existingPayload = JSON.parse(
              Buffer.from(existing.receipt.payload_base64url, "base64url").toString("utf8"),
            ) as Record<string, unknown>;
            const existingRevision = Number(existingPayload.provider_revision);
            if (existingPayload.receipt_id === `receipt_${eventId}`
                || (Number.isSafeInteger(existingRevision) && existingRevision >= revision)) {
              return response({ received: true });
            }
          }
          const customerId = stripeIdentifier(subscription.customer, "cus");
          const subscriptionId = stripeIdentifier(subscription.id, "sub");
          const price = subscription.items?.data?.[0]?.price?.id;
          const plan = planForPrice(billing, price);
          const startSeconds = Number(subscription.current_period_start);
          const endSeconds = Number(subscription.current_period_end);
          if (!Number.isSafeInteger(startSeconds) || !Number.isSafeInteger(endSeconds) || endSeconds <= startSeconds) {
            throw new Error("Stripe subscription period is invalid");
          }
          const receipt = signEntitlementReceipt({
            schema: RECEIPT_SCHEMA,
            receipt_id: `receipt_${eventId}`,
            entitlement_id: `entitlement_${subscriptionId}`,
            installation_binding: binding,
            plan,
            issued_at: new Date(startSeconds * 1000).toISOString(),
            paid_through: new Date(endSeconds * 1000).toISOString(),
            cancellation_at_period_end: subscription.cancel_at_period_end === true || event.type === "customer.subscription.deleted",
            provider_revision: revision,
          }, billing.receiptSigningPrivateKey ?? "");
          await storeRecord(billing.root, {
            schema: "tohseno.private-billing-record/1",
            installation_binding: binding,
            customer_id: customerId,
            subscription_id: subscriptionId,
            provider_event_id: eventId,
            provider_revision: revision,
            receipt,
          });
          return response({ received: true });
        }

        if (url.pathname === `${prefix}test/complete` && request.method === "POST" && fake) {
          const value = await boundedJson(request);
          exactKeys(value, ["claim", "event_id"], "fake billing completion");
          const claim = verifyCheckoutClaim(value.claim);
          boundedIdentifier(value.event_id, "provider event identifier");
          const receipt = fake.complete(value.event_id, claim, new Date());
          await storeRecord(billing.root, {
            schema: "tohseno.private-billing-record/1",
            installation_binding: claim.installation_binding,
            customer_id: "cus_fake",
            subscription_id: "sub_fake",
            provider_event_id: value.event_id,
            provider_revision: 1,
            receipt,
          });
          return response(receipt, 201);
        }
        return response({ error: "Not found" }, 404);
      } catch (error) {
        return response({ error: error instanceof Error ? error.message : "Billing request failed" }, 400);
      }
    },
  };
}
