import {
  createHash,
  createPublicKey,
  randomBytes,
  timingSafeEqual,
  verify,
} from "node:crypto";
import {
  lstat,
  mkdir,
  readFile,
  readdir,
  rename,
  rmdir,
  unlink,
  writeFile,
} from "node:fs/promises";
import { join } from "node:path";
import type { AppConfig, ManagedConfig } from "../config.ts";
import { verifyStripeWebhook } from "./billing.ts";
import { withSecurityHeaders } from "./security.ts";

const CLAIM_SCHEMA = "tohseno.private-managed-claim/1";
const CLAIM_ENVELOPE_SCHEMA = "tohseno.private-managed-claim-envelope/1";
const CLAIM_DOMAIN = Buffer.from("tohseno.managed.claim.v1\0");
const INSTALLATION_DOMAIN = Buffer.from("tohseno.managed.installation.v1\0");
const ENTRY_SCHEMA = "tohseno.private-balance-ledger-entry/1";
const MAX_REQUEST_BYTES = 8 * 1024 * 1024;
const MAX_PROVIDER_BYTES = 16 * 1024 * 1024;
const MICROS_PER_USD = 1_000_000;
const PACKS = Object.freeze({
  usd_10: 10 * MICROS_PER_USD,
  usd_25: 25 * MICROS_PER_USD,
  usd_50: 50 * MICROS_PER_USD,
});

export type BalanceBucket = "paid" | "promotional" | "none";
export type LedgerEntryType =
  | "purchase_credit" | "promotional_grant" | "promotional_revocation"
  | "reservation_hold" | "reservation_release" | "inference_charge"
  | "refund_adjustment" | "dispute_adjustment" | "provider_reconciliation"
  | "checkout_failed" | "capability_issued";

export interface BalanceLedgerEntry {
  schema: typeof ENTRY_SCHEMA;
  entry_id: string;
  installation_binding: string;
  amount_microusd: number;
  currency: "USD";
  entry_type: LedgerEntryType;
  bucket: BalanceBucket;
  related_checkout_id?: string;
  related_payment_id?: string;
  related_execution_id?: string;
  related_reservation_id?: string;
  related_provider_id?: string;
  related_model?: string;
  privacy_tier?: "standard" | "zdr" | "private";
  reconciliation_status?: "pending" | "settled";
  idempotency_key: string;
  created_at: string;
  description: string;
  private_operator_metadata?: Record<string, string>;
}

export interface BalanceProjection {
  schema: "tohseno.managed-balance/1";
  installation_binding: string;
  paid_microusd: number;
  promotional_microusd: number;
  reserved_microusd: number;
  spendable_microusd: number;
  currency: "USD";
  transactions: BalanceLedgerEntry[];
}

export interface ModelPrice {
  model: string;
  input_microusd_per_million: number;
  output_microusd_per_million: number;
  privacy_tiers: readonly ("standard" | "zdr" | "private")[];
  snapshot_at: string;
}

type ManagedClaim = {
  schema: typeof CLAIM_SCHEMA;
  claim_id: string;
  installation_binding: string;
  signing_public_key_base64url: string;
  action: string;
  request_digest: string;
  issued_at: string;
  expires_at: string;
};

type ManagedEnvelope = {
  schema: typeof CLAIM_ENVELOPE_SCHEMA;
  payload_base64url: string;
  signature_base64url: string;
};

type ReservationRequest = {
  command_id: string;
  execution_id: string;
  model: string;
  privacy: "standard" | "zdr" | "private";
  maximum_microusd: number;
  pricing_snapshot_at: string;
  input_microusd_per_million: number;
  output_microusd_per_million: number;
};

type Reservation = {
  reservation_id: string;
  binding: string;
  command_id: string;
  execution_id: string;
  model: string;
  privacy: "standard" | "zdr" | "private";
  maximum_microusd: number;
  pricing_snapshot_at: string;
  input_microusd_per_million: number;
  output_microusd_per_million: number;
  promotional_hold: number;
  paid_hold: number;
  call_index: number;
};

type CapabilityRecord = Reservation & {
  schema: "tohseno.private-managed-capability/1";
  capability_hash: string;
  expires_at: string;
};

export interface ManagedProvider {
  models(): Promise<unknown>;
  credits(): Promise<unknown>;
  usage(): Promise<unknown>;
  completion(body: Uint8Array): Promise<Response>;
}

function canonical(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  return `{${Object.entries(value as Record<string, unknown>)
    .filter(([, item]) => item !== undefined)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`).join(",")}}`;
}

function identifier(value: unknown, label: string): asserts value is string {
  if (typeof value !== "string" || !/^[A-Za-z0-9_-]{1,160}$/.test(value)) {
    throw new Error(`${label} is invalid`);
  }
}

function integer(value: unknown, label: string, maximum = Number.MAX_SAFE_INTEGER): asserts value is number {
  if (!Number.isSafeInteger(value) || Number(value) < 0 || Number(value) > maximum) {
    throw new Error(`${label} is invalid`);
  }
}

function digest(value: unknown): string {
  return createHash("sha256").update(canonical(value)).digest("base64url");
}

export function installationBinding(publicKeyBase64url: string): string {
  const key = Buffer.from(publicKeyBase64url, "base64url");
  if (key.length !== 32) throw new Error("installation public key is invalid");
  return createHash("sha256").update(INSTALLATION_DOMAIN).update(key).digest("base64url");
}

export function verifyManagedClaim(envelope: unknown, request: unknown, action: string, now = new Date()): ManagedClaim {
  if (!envelope || typeof envelope !== "object" || Array.isArray(envelope)) throw new Error("managed claim is invalid");
  const value = envelope as Record<string, unknown>;
  if (Object.keys(value).sort().join(",") !== "payload_base64url,schema,signature_base64url"
      || value.schema !== CLAIM_ENVELOPE_SCHEMA
      || typeof value.payload_base64url !== "string"
      || typeof value.signature_base64url !== "string") throw new Error("managed claim envelope is invalid");
  const payload = Buffer.from(value.payload_base64url, "base64url");
  const signature = Buffer.from(value.signature_base64url, "base64url");
  if (!payload.length || payload.length > 32 * 1024 || signature.length !== 64) throw new Error("managed claim encoding is invalid");
  const claim = JSON.parse(payload.toString("utf8")) as ManagedClaim;
  const keys = ["action", "claim_id", "expires_at", "installation_binding", "issued_at", "request_digest", "schema", "signing_public_key_base64url"];
  if (Object.keys(claim).sort().join(",") !== keys.join(",") || canonical(claim) !== payload.toString("utf8") || claim.schema !== CLAIM_SCHEMA) {
    throw new Error("managed claim payload is invalid");
  }
  identifier(claim.claim_id, "managed claim identifier");
  identifier(claim.action, "managed claim action");
  if (claim.action !== action || claim.request_digest !== digest(request)) throw new Error("managed claim is bound to different work");
  if (claim.installation_binding !== installationBinding(claim.signing_public_key_base64url)) throw new Error("managed claim installation binding is invalid");
  const issued = Date.parse(claim.issued_at), expires = Date.parse(claim.expires_at);
  if (!Number.isFinite(issued) || !Number.isFinite(expires) || issued > now.getTime() + 30_000
      || expires <= now.getTime() || expires - issued > 5 * 60_000) throw new Error("managed claim lifetime is invalid");
  const raw = Buffer.from(claim.signing_public_key_base64url, "base64url");
  const publicKey = createPublicKey({ key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), raw]), format: "der", type: "spki" });
  if (!verify(null, Buffer.concat([CLAIM_DOMAIN, payload]), publicKey, signature)) throw new Error("managed claim signature is invalid");
  return claim;
}

async function boundedBytes(message: Request | Response, maximum: number): Promise<Uint8Array> {
  const declared = message.headers.get("content-length");
  if (declared !== null && (!/^\d+$/.test(declared) || Number(declared) > maximum)) throw new Error("request is oversized");
  if (!message.body) throw new Error("request is empty");
  const reader = message.body.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    length += value.byteLength;
    if (length > maximum) { await reader.cancel(); throw new Error("request is oversized"); }
    chunks.push(value);
  }
  const output = new Uint8Array(length); let offset = 0;
  for (const chunk of chunks) { output.set(chunk, offset); offset += chunk.length; }
  if (!output.length) throw new Error("request is empty");
  return output;
}

async function jsonBody(request: Request, maximum = 64 * 1024): Promise<any> {
  return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(await boundedBytes(request, maximum)));
}

function json(value: unknown, status = 200): Response {
  return withSecurityHeaders(new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" },
  }));
}

async function safeRoot(root: string): Promise<void> {
  await mkdir(root, { recursive: true, mode: 0o700 });
  const metadata = await lstat(root);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) throw new Error("managed storage root is unsafe");
}

function accountName(binding: string): string {
  return createHash("sha256").update("tohseno.managed.account.v1\0").update(binding).digest("hex");
}

async function withAccountLock<T>(root: string, binding: string, operation: () => Promise<T>): Promise<T> {
  const account = join(root, `account-${accountName(binding)}`);
  await mkdir(account, { recursive: true, mode: 0o700 });
  const lock = join(account, ".lock");
  const owner = randomBytes(16).toString("hex");
  for (let attempt = 0; ; attempt += 1) {
    try {
      await mkdir(lock, { mode: 0o700 });
      try { await writeFile(join(lock, "owner"), owner, { flag: "wx", mode: 0o600 }); }
      catch (error) { await rmdir(lock).catch(() => {}); throw error; }
      break;
    }
    catch (error) {
      if (!(error instanceof Error && "code" in error && error.code === "EEXIST") || attempt >= 1_000) throw error;
      if (await reclaimStaleAccountLock(lock)) continue;
      await Bun.sleep(10);
    }
  }
  try { return await operation(); }
  finally {
    try {
      if (await readFile(join(lock, "owner"), "utf8") === owner) {
        await unlink(join(lock, "owner"));
        await rmdir(lock);
      }
    } catch (error) {
      if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) throw error;
    }
  }
}

async function reclaimStaleAccountLock(lock: string): Promise<boolean> {
  const metadata = await lstat(lock);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) throw new Error("managed account lock is unsafe");
  if (Date.now() - metadata.mtimeMs < 2 * 60_000) return false;
  const stale = `${lock}.stale-${randomBytes(8).toString("hex")}`;
  try { await rename(lock, stale); }
  catch (error) {
    if (error instanceof Error && "code" in error && ["ENOENT", "EEXIST"].includes(String(error.code))) return false;
    throw error;
  }
  await unlink(join(stale, "owner")).catch((error) => {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) throw error;
  });
  await rmdir(stale);
  return true;
}

async function entries(root: string, binding: string): Promise<BalanceLedgerEntry[]> {
  const directory = join(root, `account-${accountName(binding)}`, "entries");
  try {
    const names = (await readdir(directory)).filter((name) => /^entry_[a-f0-9]{64}\.json$/.test(name)).sort();
    if (names.length > 100_000) throw new Error("balance ledger is at capacity");
    const output: BalanceLedgerEntry[] = [];
    for (const name of names) {
      const path = join(directory, name); const metadata = await lstat(path);
      if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 64 * 1024) throw new Error("balance ledger entry is unsafe");
      output.push(JSON.parse(await readFile(path, "utf8")) as BalanceLedgerEntry);
    }
    output.sort((left, right) => left.created_at.localeCompare(right.created_at)
      || left.entry_id.localeCompare(right.entry_id));
    return output;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return [];
    throw error;
  }
}

function entryID(binding: string, idempotency: string): string {
  return `entry_${createHash("sha256").update("tohseno.managed.entry.v1\0").update(binding).update("\0").update(idempotency).digest("hex")}`;
}

async function append(root: string, input: Omit<BalanceLedgerEntry, "schema" | "entry_id" | "currency" | "created_at">): Promise<BalanceLedgerEntry> {
  identifier(input.installation_binding, "installation binding");
  identifier(input.idempotency_key, "idempotency key");
  if (!Number.isSafeInteger(input.amount_microusd)) throw new Error("ledger amount is invalid");
  const entry: BalanceLedgerEntry = {
    schema: ENTRY_SCHEMA,
    entry_id: entryID(input.installation_binding, input.idempotency_key),
    currency: "USD",
    created_at: new Date().toISOString(),
    ...input,
  };
  const directory = join(root, `account-${accountName(input.installation_binding)}`, "entries");
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const path = join(directory, `${entry.entry_id}.json`);
  try { await writeFile(path, canonical(entry), { flag: "wx", mode: 0o600 }); return entry; }
  catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "EEXIST")) throw error;
    const existing = JSON.parse(await readFile(path, "utf8")) as BalanceLedgerEntry;
    const comparable = { ...entry, created_at: existing.created_at };
    if (canonical(existing) !== canonical(comparable)) throw new Error("idempotency key conflicts with an existing ledger entry");
    return existing;
  }
}

export async function balanceProjection(root: string, binding: string): Promise<BalanceProjection> {
  const all = await entries(root, binding);
  const paid = all.filter((entry) => entry.bucket === "paid").reduce((sum, entry) => sum + entry.amount_microusd, 0);
  const promotional = all.filter((entry) => entry.bucket === "promotional").reduce((sum, entry) => sum + entry.amount_microusd, 0);
  const reserved = -all.filter((entry) => entry.entry_type === "reservation_hold").reduce((sum, entry) => sum + entry.amount_microusd, 0)
    - all.filter((entry) => entry.entry_type === "reservation_release").reduce((sum, entry) => sum + entry.amount_microusd, 0);
  return {
    schema: "tohseno.managed-balance/1", installation_binding: binding,
    paid_microusd: paid, promotional_microusd: promotional, reserved_microusd: Math.max(0, reserved),
    spendable_microusd: paid + promotional, currency: "USD",
    transactions: all.slice(-200).reverse().map(({ private_operator_metadata: _, ...entry }) => entry),
  };
}

export class ManagedAuthority {
  private catalogCache?: { expires: number; prices: ModelPrice[] };
  private readonly inferenceWindows = new Map<string, number[]>();

  constructor(
    readonly config: ManagedConfig,
    readonly provider: ManagedProvider,
    readonly fetchImpl: typeof fetch = fetch,
  ) {}

  async initialize(): Promise<void> {
    if (!this.config.enabled || !this.config.root) return;
    await safeRoot(this.config.root);
  }

  async catalog(now = new Date()): Promise<ModelPrice[]> {
    if (this.catalogCache && this.catalogCache.expires > now.getTime()) return this.catalogCache.prices;
    const value = await this.provider.models() as any;
    if (!Array.isArray(value?.data)) throw new Error("Bankr model catalog is invalid");
    const prices = value.data.filter((item: any) => this.config.modelAllowlist.includes(item?.id)).map((item: any) => {
      const providerInput = providerMicrosPerMillion(item?.pricing?.prompt ?? item?.pricing?.input, item?.pricing?.input_microusd_per_million);
      const providerOutput = providerMicrosPerMillion(item?.pricing?.completion ?? item?.pricing?.output, item?.pricing?.output_microusd_per_million);
      const tiers: ("standard" | "zdr" | "private")[] = ["standard"];
      if (item?.zdr === true || item?.privacy_tiers?.includes?.("zdr")) tiers.push("zdr");
      if (item?.private === true) tiers.push("private");
      return {
        model: item.id,
        input_microusd_per_million: Math.ceil(providerInput * 1.2),
        output_microusd_per_million: Math.ceil(providerOutput * 1.2),
        privacy_tiers: tiers,
        snapshot_at: now.toISOString(),
      } satisfies ModelPrice;
    });
    if (!prices.length) throw new Error("no allowlisted Bankr model has authoritative pricing");
    this.catalogCache = { expires: now.getTime() + 5 * 60_000, prices };
    return prices;
  }

  async grant(binding: string, amount: number, reason: string, idempotency: string, operator: string): Promise<BalanceLedgerEntry> {
    if (!this.config.root) throw new Error("managed balance is not configured");
    identifier(binding, "installation binding"); identifier(idempotency, "idempotency key");
    integer(amount, "grant amount", 100 * MICROS_PER_USD);
    if (!amount || reason.trim().length < 3 || reason.length > 500) throw new Error("grant reason is invalid");
    return withAccountLock(this.config.root, binding, () => append(this.config.root!, {
      installation_binding: binding, amount_microusd: amount, entry_type: "promotional_grant",
      bucket: "promotional", idempotency_key: idempotency, description: "Promotional managed-compute grant",
      private_operator_metadata: { operator, reason: reason.trim() },
    }));
  }

  async revoke(binding: string, amount: number, reason: string, idempotency: string, operator: string): Promise<BalanceLedgerEntry> {
    if (!this.config.root) throw new Error("managed balance is not configured");
    identifier(binding, "installation binding"); identifier(idempotency, "idempotency key");
    integer(amount, "revocation amount", 100 * MICROS_PER_USD);
    if (!amount || reason.trim().length < 3 || reason.length > 500) throw new Error("revocation reason is invalid");
    return withAccountLock(this.config.root, binding, async () => {
      const projection = await balanceProjection(this.config.root!, binding);
      if (amount > projection.promotional_microusd) throw new Error("revocation exceeds available promotional balance");
      return append(this.config.root!, {
        installation_binding: binding, amount_microusd: -amount, entry_type: "promotional_revocation",
        bucket: "promotional", idempotency_key: idempotency, description: "Promotional managed-compute balance revoked",
        private_operator_metadata: { operator, reason: reason.trim() },
      });
    });
  }

  async reconcile(
    binding: string,
    reservationID: string,
    action: "release" | "charge",
    retailCharge: number,
    reason: string,
    idempotency: string,
    operator: string,
    providerID?: string,
  ): Promise<BalanceLedgerEntry> {
    if (!this.config.root) throw new Error("managed balance is not configured");
    identifier(binding, "installation binding"); identifier(reservationID, "reservation identifier");
    identifier(idempotency, "idempotency key");
    integer(retailCharge, "reconciled retail charge", 100 * MICROS_PER_USD);
    if (!matchesReconciliationAction(action) || (action === "release" && retailCharge !== 0)
        || reason.trim().length < 3 || reason.length > 500) throw new Error("reconciliation decision is invalid");
    if (providerID !== undefined) identifier(providerID, "provider request identifier");
    return withAccountLock(this.config.root, binding, async () => {
      const related = (await entries(this.config.root!, binding)).filter((entry) => entry.related_reservation_id === reservationID);
      const pending = related.some((entry) => entry.entry_type === "provider_reconciliation" && entry.reconciliation_status === "pending");
      const settled = related.some((entry) => entry.entry_type === "provider_reconciliation" && entry.reconciliation_status === "settled");
      if (!pending || settled) throw new Error("reservation is not awaiting reconciliation");
      const outstanding = (bucket: Exclude<BalanceBucket, "none">) => -related
        .filter((entry) => entry.bucket === bucket && ["reservation_hold", "reservation_release"].includes(entry.entry_type))
        .reduce((sum, entry) => sum + entry.amount_microusd, 0);
      const promo = outstanding("promotional"), paid = outstanding("paid"), total = promo + paid;
      if (total <= 0 || retailCharge > total || (action === "charge" && retailCharge <= 0)) {
        throw new Error("reconciliation charge is outside the outstanding reservation");
      }
      const records = await reservationCapabilities(this.config.root!, binding, reservationID);
      const record = records.map((value) => value.record).sort((a, b) => b.call_index - a.call_index)[0];
      if (!record) throw new Error("reservation capability record is unavailable for reconciliation");
      if (promo) await append(this.config.root!, operatorReleaseEntry(record, "promotional", promo, idempotency));
      if (paid) await append(this.config.root!, operatorReleaseEntry(record, "paid", paid, idempotency));
      if (action === "charge") {
        const promoCharge = Math.min(promo, retailCharge), paidCharge = retailCharge - promoCharge;
        if (promoCharge) await append(this.config.root!, operatorChargeEntry(record, "promotional", promoCharge, providerID, idempotency));
        if (paidCharge) await append(this.config.root!, operatorChargeEntry(record, "paid", paidCharge, providerID, idempotency));
      }
      return append(this.config.root!, {
        installation_binding: binding, amount_microusd: 0, entry_type: "provider_reconciliation", bucket: "none",
        related_execution_id: record.execution_id, related_reservation_id: reservationID,
        related_provider_id: providerID, related_model: record.model, privacy_tier: record.privacy,
        reconciliation_status: "settled", idempotency_key: `reconcile_settled_${idempotency}`,
        description: action === "charge" ? "Ambiguous managed usage reconciled and charged" : "Ambiguous managed usage reconciled and released",
        private_operator_metadata: { operator, reason: reason.trim(), action, retail_charge_microusd: String(retailCharge) },
      });
    });
  }

  async reserve(binding: string, request: ReservationRequest): Promise<{ reservation: Reservation; capability: string; expires_at: string }> {
    if (!this.config.root) throw new Error("managed balance is not configured");
    identifier(request.command_id, "command identifier"); identifier(request.execution_id, "execution identifier");
    integer(request.maximum_microusd, "maximum managed spend", 100 * MICROS_PER_USD);
    integer(request.input_microusd_per_million, "managed input price", 100 * MICROS_PER_USD);
    integer(request.output_microusd_per_million, "managed output price", 100 * MICROS_PER_USD);
    if (!request.maximum_microusd) throw new Error("maximum managed spend is required");
    const price = (await this.catalog()).find((entry) => entry.model === request.model);
    if (!price || !price.privacy_tiers.includes(request.privacy)
        || typeof request.pricing_snapshot_at !== "string" || request.pricing_snapshot_at.length > 64
        || !Number.isFinite(Date.parse(request.pricing_snapshot_at))
        || price.input_microusd_per_million !== request.input_microusd_per_million
        || price.output_microusd_per_million !== request.output_microusd_per_million) {
      throw new Error("model, privacy tier, or accepted server price is no longer available");
    }
    const reservationID = `reservation_${createHash("sha256").update("tohseno.reservation.v1\0").update(binding).update(request.command_id).digest("hex").slice(0, 40)}`;
    const capability = randomBytes(32).toString("base64url");
    const capabilityHash = createHash("sha256").update(capability).digest("hex");
    const expiresAt = new Date(Date.now() + 15 * 60_000).toISOString();
    const reservation = await withAccountLock(this.config.root, binding, async () => {
      await this.recoverInterruptedReservation(binding, reservationID, request);
      const related = (await entries(this.config.root!, binding)).filter((entry) => entry.related_reservation_id === reservationID);
      const holds = related.filter((entry) => entry.entry_type === "reservation_hold");
      let callIndex = 1, amount = request.maximum_microusd;
      if (holds.length) {
        const initialHeld = -holds.filter((entry) => entry.idempotency_key.endsWith("_call_1")).reduce((sum, entry) => sum + entry.amount_microusd, 0);
        if (initialHeld !== request.maximum_microusd || holds.some((entry) => entry.related_execution_id !== request.execution_id)) throw new Error("reservation identity conflicts with existing work");
        const capabilities = await reservationCapabilities(this.config.root!, binding, reservationID);
        if (capabilities.some(({ record }) => record.model !== request.model
            || record.privacy !== request.privacy
            || record.pricing_snapshot_at !== request.pricing_snapshot_at
            || record.input_microusd_per_million !== request.input_microusd_per_million
            || record.output_microusd_per_million !== request.output_microusd_per_million)) {
          throw new Error("reservation pricing or route conflicts with existing work");
        }
        const pendingReconciliation = related.some((entry) => entry.entry_type === "provider_reconciliation" && entry.reconciliation_status === "pending")
          && !related.some((entry) => entry.entry_type === "provider_reconciliation" && entry.reconciliation_status === "settled");
        if (pendingReconciliation) throw new Error("reservation is awaiting provider reconciliation");
        const outstanding = -related.filter((entry) => ["reservation_hold", "reservation_release"].includes(entry.entry_type)).reduce((sum, entry) => sum + entry.amount_microusd, 0);
        if (outstanding > 0) throw new Error("reservation already has an active capability");
        const calls = capabilities.filter((record) => record.used).length;
        if (calls >= 2) throw new Error("reservation exhausted its bounded invocation count");
        const charged = -related.filter((entry) => entry.entry_type === "inference_charge").reduce((sum, entry) => sum + entry.amount_microusd, 0);
        amount = request.maximum_microusd - charged;
        if (amount <= 0) throw new Error("reservation approved maximum is exhausted");
        callIndex = Math.max(
          related.filter((entry) => entry.entry_type === "capability_issued").length,
          ...capabilities.map(({ record }) => record.call_index),
        ) + 1;
      }
      const projection = await balanceProjection(this.config.root!, binding);
      if (projection.spendable_microusd < amount) throw new Error("insufficient managed creation balance");
      const promo = Math.min(projection.promotional_microusd, amount);
      const paid = amount - promo;
      if (promo) await append(this.config.root!, holdEntry(binding, request, reservationID, "promotional", promo, callIndex));
      if (paid) await append(this.config.root!, holdEntry(binding, request, reservationID, "paid", paid, callIndex));
      const admitted = { reservation_id: reservationID, binding, ...request, maximum_microusd: amount, promotional_hold: promo, paid_hold: paid, call_index: callIndex };
      const directory = join(this.config.root!, `account-${accountName(binding)}`, "capabilities");
      await mkdir(directory, { recursive: true, mode: 0o700 });
      const record: CapabilityRecord = { schema: "tohseno.private-managed-capability/1", capability_hash: capabilityHash, expires_at: expiresAt, ...admitted };
      await writeFile(join(directory, `capability-${capabilityHash}.json`), canonical(record), { flag: "wx", mode: 0o600 });
      await append(this.config.root!, {
        installation_binding: binding, amount_microusd: 0, entry_type: "capability_issued", bucket: "none",
        related_execution_id: request.execution_id, related_reservation_id: reservationID,
        idempotency_key: `capability_${capabilityHash}`, description: "Short-lived managed inference capability issued",
      });
      return admitted;
    });
    return { reservation, capability, expires_at: expiresAt };
  }

  async complete(capability: string, requestBytes: Uint8Array): Promise<Response> {
    if (!this.config.root || !/^[A-Za-z0-9_-]{43}$/.test(capability)) throw new Error("managed capability is invalid");
    const hash = createHash("sha256").update(capability).digest("hex");
    const record = await findCapability(this.config.root, hash);
    const request = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(requestBytes)) as any;
    if (request?.model !== record.model || request?.privacy !== record.privacy || !Array.isArray(request?.messages)) throw new Error("managed request is outside its admitted job");
    integer(request.max_tokens, "maximum output tokens", 1_000_000);
    if (!request.max_tokens) throw new Error("maximum output tokens is required");
    const price: ModelPrice = {
      model: record.model,
      input_microusd_per_million: record.input_microusd_per_million,
      output_microusd_per_million: record.output_microusd_per_million,
      privacy_tiers: [record.privacy],
      snapshot_at: record.pricing_snapshot_at,
    };
    // Convert bounded UTF-8/JSON bytes to a deliberately conservative token
    // estimate. Charging still uses the provider's reported token usage.
    const worst = cost(Math.ceil(requestBytes.length / 3), request.max_tokens, price);
    if (worst > record.maximum_microusd) throw new Error("managed request exceeds its approved maximum");
    // Consume only after every locally decidable admission check. A malformed
    // or over-cap request must remain safely retryable without calling Bankr.
    await withAccountLock(this.config.root, record.binding, async () => {
      if (Date.parse(record.expires_at) <= Date.now()) throw new Error("managed capability expired");
      const usedPath = join(this.config.root!, `account-${accountName(record.binding)}`, "capabilities", `used-${hash}`);
      await writeFile(usedPath, record.reservation_id, { flag: "wx", mode: 0o600 }).catch(() => { throw new Error("managed capability was already used"); });
    });
    if (!this.admitRate(record.binding)) {
      await this.release(record, "tohseno_rate_limit");
      return json({ error: { code: "managed_rate_limited", message: "Managed compute is temporarily rate limited; admitted source is preserved." } }, 429);
    }
    let upstream: Response;
    try { upstream = await this.provider.completion(requestBytes); }
    catch {
      await this.reconciliation(record, "provider_timeout", "Provider outcome is ambiguous; reservation remains held");
      return json({ error: { code: "managed_provider_ambiguous", message: "Managed inference outcome needs reconciliation." } }, 503);
    }
    if (!upstream.ok) {
      const status = upstream.status;
      if ([401, 402, 429].includes(status)) await this.release(record, `provider_${status}`);
      else await this.reconciliation(record, `provider_${status}`, "Provider error may have consumed usage; reservation remains held");
      const code = status === 402 ? "managed_provider_balance_exhausted" : status === 429 ? "managed_provider_rate_limited" : status === 401 ? "managed_provider_authentication" : "managed_provider_unavailable";
      return json({ error: { code, message: "Managed compute is temporarily unavailable; admitted source is preserved." } }, status === 429 ? 429 : 503);
    }
    const bytes = await boundedBytes(upstream, MAX_PROVIDER_BYTES);
    const usage = usageFromResponse(bytes, request.stream === true);
    if (!usage) {
      await this.reconciliation(record, "malformed_usage", "Provider response lacked valid usage; reservation remains held");
      return json({ error: { code: "managed_usage_ambiguous", message: "Managed usage needs reconciliation." } }, 502);
    }
    const charge = cost(usage.input, usage.output, price);
    if (charge > record.maximum_microusd) {
      await this.reconciliation(record, "cap_exceeded", "Provider-reported usage exceeded the authorized reservation");
      return json({ error: { code: "managed_cap_reconciliation", message: "Managed usage exceeded its authorization and was not charged automatically." } }, 502);
    }
    const providerID = usage.providerID ?? upstream.headers.get("x-request-id") ?? undefined;
    await this.settle(record, charge, providerID, usage.providerCostMicrousd);
    const headers = new Headers({ "content-type": request.stream === true ? "text/event-stream; charset=utf-8" : "application/json; charset=utf-8", "cache-control": "no-store" });
    if (providerID) headers.set("x-tohseno-provider-request", providerID);
    return withSecurityHeaders(new Response(new Blob([bytes.slice().buffer as ArrayBuffer]), { status: 200, headers }));
  }

  private admitRate(binding: string, now = Date.now()): boolean {
    const cutoff = now - 60_000;
    const recent = (this.inferenceWindows.get(binding) ?? []).filter((value) => value > cutoff);
    if (recent.length >= this.config.rateLimitPerMinute) {
      this.inferenceWindows.set(binding, recent);
      return false;
    }
    recent.push(now);
    this.inferenceWindows.set(binding, recent);
    return true;
  }

  private async recoverInterruptedReservation(binding: string, reservationID: string, request: ReservationRequest): Promise<void> {
    const capabilities = await reservationCapabilities(this.config.root!, binding, reservationID);
    const related = (await entries(this.config.root!, binding)).filter((entry) => entry.related_reservation_id === reservationID);
    for (const capability of capabilities) {
      if (Date.parse(capability.record.expires_at) > Date.now()) continue;
      if (!capability.used) {
        await this.releaseUnlocked(capability.record, "expired_unused");
      } else if (reservationOutstandingForCall(related, capability.record.call_index) > 0
          && !hasPendingReconciliationForCall(related, capability.record.call_index)) {
        await this.reconciliationUnlocked(
          capability.record,
          "interrupted_after_provider_admission",
          "Managed execution stopped after provider admission; reservation remains held for reconciliation",
        );
      }
    }

    const capabilityCalls = new Set(capabilities.map(({ record }) => record.call_index));
    const orphanCalls = new Set<number>();
    for (const hold of related.filter((entry) => entry.entry_type === "reservation_hold")) {
      const match = hold.idempotency_key.match(/_call_(\d+)$/);
      const call = match ? Number(match[1]) : NaN;
      if (Number.isSafeInteger(call) && !capabilityCalls.has(call)
          && Date.parse(hold.created_at) <= Date.now() - 15 * 60_000) orphanCalls.add(call);
    }
    for (const call of orphanCalls) {
      const promotional = reservationOutstandingForCall(related, call, "promotional");
      const paid = reservationOutstandingForCall(related, call, "paid");
      if (promotional <= 0 && paid <= 0) continue;
      const orphan: CapabilityRecord = {
        schema: "tohseno.private-managed-capability/1",
        capability_hash: "0".repeat(64), expires_at: new Date(0).toISOString(),
        reservation_id: reservationID, binding, ...request,
        maximum_microusd: promotional + paid, promotional_hold: promotional,
        paid_hold: paid, call_index: call,
      };
      await this.releaseUnlocked(orphan, "interrupted_before_capability");
    }
  }

  private async release(record: CapabilityRecord, reason: string): Promise<void> {
    await withAccountLock(this.config.root!, record.binding, async () => {
      if (record.promotional_hold) await append(this.config.root!, releaseEntry(record, "promotional", record.promotional_hold, `release_${reason}_promo`));
      if (record.paid_hold) await append(this.config.root!, releaseEntry(record, "paid", record.paid_hold, `release_${reason}_paid`));
    });
  }

  private async settle(record: CapabilityRecord, charge: number, providerID?: string, providerCost?: number): Promise<void> {
    await withAccountLock(this.config.root!, record.binding, async () => {
      await this.releaseUnlocked(record, "settled");
      const promo = Math.min(record.promotional_hold, charge), paid = charge - promo;
      if (promo) await append(this.config.root!, chargeEntry(record, "promotional", promo, providerID, providerCost));
      if (paid) await append(this.config.root!, chargeEntry(record, "paid", paid, providerID, providerCost));
    });
  }

  private async releaseUnlocked(record: CapabilityRecord, reason: string): Promise<void> {
    if (record.promotional_hold) await append(this.config.root!, releaseEntry(record, "promotional", record.promotional_hold, `release_${reason}_promo`));
    if (record.paid_hold) await append(this.config.root!, releaseEntry(record, "paid", record.paid_hold, `release_${reason}_paid`));
  }

  private async reconciliation(record: CapabilityRecord, reason: string, description: string): Promise<void> {
    await this.reconciliationUnlocked(record, reason, description);
  }

  private async reconciliationUnlocked(record: CapabilityRecord, reason: string, description: string): Promise<void> {
    await append(this.config.root!, {
      installation_binding: record.binding, amount_microusd: 0, entry_type: "provider_reconciliation", bucket: "none",
      related_execution_id: record.execution_id, related_reservation_id: record.reservation_id,
      related_model: record.model, privacy_tier: record.privacy, reconciliation_status: "pending",
      idempotency_key: `reconcile_${record.reservation_id}_call_${record.call_index}_${reason}`, description,
      private_operator_metadata: { reason },
    });
  }
}

function reservationOutstandingForCall(
  related: BalanceLedgerEntry[],
  call: number,
  bucket?: Exclude<BalanceBucket, "none">,
): number {
  const marker = `_call_${call}`;
  return -related.filter((entry) =>
    ["reservation_hold", "reservation_release"].includes(entry.entry_type)
      && entry.idempotency_key.includes(marker)
      && (bucket === undefined || entry.bucket === bucket))
    .reduce((sum, entry) => sum + entry.amount_microusd, 0);
}

function hasPendingReconciliationForCall(related: BalanceLedgerEntry[], call: number): boolean {
  const settled = related.some((entry) =>
    entry.entry_type === "provider_reconciliation" && entry.reconciliation_status === "settled");
  const marker = `_call_${call}_`;
  const pending = related.some((entry) =>
    entry.entry_type === "provider_reconciliation"
      && entry.reconciliation_status === "pending"
      && (!entry.idempotency_key.includes("_call_") || entry.idempotency_key.includes(marker)));
  return pending && !settled;
}

function providerMicrosPerMillion(perToken: unknown, explicit: unknown): number {
  if (Number.isSafeInteger(explicit) && Number(explicit) > 0) return Number(explicit);
  const parsed = typeof perToken === "string" || typeof perToken === "number" ? Number(perToken) : NaN;
  const micros = parsed * 1_000_000 * 1_000_000;
  if (!Number.isSafeInteger(Math.ceil(micros)) || micros <= 0) throw new Error("Bankr model pricing is missing or invalid");
  return Math.ceil(micros);
}

function cost(input: number, output: number, price: ModelPrice): number {
  return Math.ceil((input * price.input_microusd_per_million + output * price.output_microusd_per_million) / 1_000_000);
}

function holdEntry(binding: string, request: ReservationRequest, reservation: string, bucket: Exclude<BalanceBucket, "none">, amount: number, call: number): Omit<BalanceLedgerEntry, "schema" | "entry_id" | "currency" | "created_at"> {
  return { installation_binding: binding, amount_microusd: -amount, entry_type: "reservation_hold", bucket,
    related_execution_id: request.execution_id, related_reservation_id: reservation,
    idempotency_key: `hold_${reservation}_${bucket}_call_${call}`, description: "Managed-compute maximum reserved" };
}

function releaseEntry(record: CapabilityRecord, bucket: Exclude<BalanceBucket, "none">, amount: number, suffix: string): Omit<BalanceLedgerEntry, "schema" | "entry_id" | "currency" | "created_at"> {
  return { installation_binding: record.binding, amount_microusd: amount, entry_type: "reservation_release", bucket,
    related_execution_id: record.execution_id, related_reservation_id: record.reservation_id,
    idempotency_key: `${record.reservation_id}_call_${record.call_index}_${suffix}`, description: "Unused managed-compute reservation released" };
}

function chargeEntry(record: CapabilityRecord, bucket: Exclude<BalanceBucket, "none">, amount: number, providerID?: string, providerCost?: number): Omit<BalanceLedgerEntry, "schema" | "entry_id" | "currency" | "created_at"> {
  return { installation_binding: record.binding, amount_microusd: -amount, entry_type: "inference_charge", bucket,
    related_execution_id: record.execution_id, related_reservation_id: record.reservation_id,
    related_model: record.model, privacy_tier: record.privacy, reconciliation_status: "settled",
    related_provider_id: providerID, idempotency_key: `charge_${record.reservation_id}_${bucket}_call_${record.call_index}`,
    description: "Actual managed inference usage", private_operator_metadata: {
      provider_cost_microusd: providerCost === undefined ? "unreported" : String(providerCost),
      retail_charge_microusd: String(amount), model: record.model, privacy_tier: record.privacy,
    } };
}

function matchesReconciliationAction(value: string): value is "release" | "charge" {
  return value === "release" || value === "charge";
}

function operatorReleaseEntry(record: CapabilityRecord, bucket: Exclude<BalanceBucket, "none">, amount: number, idempotency: string): Omit<BalanceLedgerEntry, "schema" | "entry_id" | "currency" | "created_at"> {
  return { installation_binding: record.binding, amount_microusd: amount, entry_type: "reservation_release", bucket,
    related_execution_id: record.execution_id, related_reservation_id: record.reservation_id,
    related_model: record.model, privacy_tier: record.privacy,
    idempotency_key: `operator_release_${idempotency}_${bucket}`, description: "Managed reservation released by operator reconciliation" };
}

function operatorChargeEntry(record: CapabilityRecord, bucket: Exclude<BalanceBucket, "none">, amount: number, providerID: string | undefined, idempotency: string): Omit<BalanceLedgerEntry, "schema" | "entry_id" | "currency" | "created_at"> {
  return { installation_binding: record.binding, amount_microusd: -amount, entry_type: "inference_charge", bucket,
    related_execution_id: record.execution_id, related_reservation_id: record.reservation_id,
    related_provider_id: providerID, related_model: record.model, privacy_tier: record.privacy,
    reconciliation_status: "settled", idempotency_key: `operator_charge_${idempotency}_${bucket}`,
    description: "Managed inference usage charged after operator reconciliation",
    private_operator_metadata: { retail_charge_microusd: String(amount), model: record.model, privacy_tier: record.privacy } };
}

async function reservationCapabilities(root: string, binding: string, reservationID: string): Promise<{ record: CapabilityRecord; used: boolean }[]> {
  const directory = join(root, `account-${accountName(binding)}`, "capabilities");
  let names: string[];
  try { names = await readdir(directory); }
  catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return [];
    throw error;
  }
  const output: { record: CapabilityRecord; used: boolean }[] = [];
  for (const name of names.filter((value) => /^capability-[a-f0-9]{64}\.json$/.test(value)).slice(0, 10_000)) {
    const path = join(directory, name); const metadata = await lstat(path);
    if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 64 * 1024) throw new Error("managed capability record is unsafe");
    const record = JSON.parse(await readFile(path, "utf8")) as CapabilityRecord;
    if (record.binding !== binding || record.reservation_id !== reservationID) continue;
    const hash = name.slice("capability-".length, -".json".length);
    let used = false;
    try {
      const usedMetadata = await lstat(join(directory, `used-${hash}`));
      if (!usedMetadata.isFile() || usedMetadata.isSymbolicLink()) throw new Error("managed capability use marker is unsafe");
      used = true;
    } catch (error) {
      if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) throw error;
    }
    output.push({ record, used });
  }
  return output;
}

async function findCapability(root: string, hash: string): Promise<CapabilityRecord> {
  const accounts = (await readdir(root)).filter((name) => /^account-[a-f0-9]{64}$/.test(name)).slice(0, 100_000);
  for (const account of accounts) {
    try {
      const path = join(root, account, "capabilities", `capability-${hash}.json`); const metadata = await lstat(path);
      if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 64 * 1024) throw new Error("managed capability record is unsafe");
      return JSON.parse(await readFile(path, "utf8")) as CapabilityRecord;
    } catch (error) { if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) throw error; }
  }
  throw new Error("managed capability does not exist");
}

function usageFromResponse(bytes: Uint8Array, streaming: boolean): { input: number; output: number; providerID?: string; providerCostMicrousd?: number } | null {
  let value: any;
  if (!streaming) {
    try { value = JSON.parse(new TextDecoder().decode(bytes)); } catch { return null; }
  } else {
    const events = new TextDecoder().decode(bytes).split("\n").filter((line) => line.startsWith("data: ") && line !== "data: [DONE]");
    for (const event of events) { try { const candidate = JSON.parse(event.slice(6)); if (candidate.usage) value = candidate; } catch { return null; } }
  }
  const input = Number(value?.usage?.prompt_tokens ?? value?.usage?.input_tokens);
  const output = Number(value?.usage?.completion_tokens ?? value?.usage?.output_tokens);
  if (!Number.isSafeInteger(input) || input < 0 || !Number.isSafeInteger(output) || output < 0) return null;
  const providerCost = Number(value?.usage?.cost);
  return { input, output, providerID: typeof value?.id === "string" ? value.id : undefined,
    providerCostMicrousd: Number.isFinite(providerCost) && providerCost >= 0 ? Math.round(providerCost * MICROS_PER_USD) : undefined };
}

export class BankrProvider implements ManagedProvider {
  constructor(readonly config: ManagedConfig, readonly fetchImpl: typeof fetch = fetch) {}
  private request(path: string, init?: RequestInit): Promise<Response> {
    if (!this.config.bankrApiKey) throw new Error("Bankr is not configured");
    const headers = new Headers(init?.headers); headers.set("x-api-key", this.config.bankrApiKey); headers.set("accept", "application/json");
    return this.fetchImpl(`${this.config.bankrBaseUrl}${path}`, { ...init, headers, redirect: "manual", signal: AbortSignal.timeout(15 * 60_000) });
  }
  async models(): Promise<unknown> { return this.readJson(await this.request("/v1/models")); }
  async credits(): Promise<unknown> { return this.readJson(await this.request("/v1/credits")); }
  async usage(): Promise<unknown> { return this.readJson(await this.request("/v1/usage?days=30")); }
  completion(body: Uint8Array): Promise<Response> { return this.request("/v1/chat/completions", { method: "POST", headers: { "content-type": "application/json" }, body: new Blob([body.slice().buffer as ArrayBuffer]) }); }
  private async readJson(response: Response): Promise<unknown> {
    if (!response.ok) throw new Error(`Bankr returned HTTP ${response.status}`);
    return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(await boundedBytes(response, 1024 * 1024)));
  }
}

function operatorAuthorized(request: Request, config: ManagedConfig): boolean {
  const token = request.headers.get("x-tohseno-operator-token") ?? "";
  const actual = createHash("sha256").update(token).digest();
  const expected = Buffer.from(config.operatorTokenSha256 ?? "", "hex");
  return expected.length === actual.length && timingSafeEqual(expected, actual);
}

async function stripeRequest(config: ManagedConfig, path: string, init: RequestInit, fetchImpl: typeof fetch): Promise<any> {
  if (!config.stripeSecretKey) throw new Error("Stripe is not configured");
  const headers = new Headers(init.headers); headers.set("authorization", `Bearer ${config.stripeSecretKey}`);
  const response = await fetchImpl(`https://api.stripe.com/v1/${path}`, { ...init, headers, redirect: "manual" });
  if (!response.ok) throw new Error(`Stripe returned HTTP ${response.status}`);
  return JSON.parse(new TextDecoder().decode(await boundedBytes(response, 256 * 1024)));
}

async function checkout(authority: ManagedAuthority, binding: string, request: any): Promise<Response> {
  if (!authority.config.root) throw new Error("managed balance is not configured");
  const pack = request?.pack_id as keyof typeof PACKS; if (!(pack in PACKS)) throw new Error("balance pack is invalid");
  const price = authority.config.priceIds[pack]; if (!price) throw new Error("balance pack is not configured");
  const params = new URLSearchParams({ mode: "payment", "line_items[0][price]": price, "line_items[0][quantity]": "1",
    success_url: authority.config.checkoutSuccessUrl ?? "", cancel_url: authority.config.checkoutCancelUrl ?? "",
    client_reference_id: request.claim_id ?? crypto.randomUUID(), "metadata[installation_binding]": binding, "metadata[pack_id]": pack });
  const session = await stripeRequest(authority.config, "checkout/sessions", {
    method: "POST", headers: { "content-type": "application/x-www-form-urlencoded", "idempotency-key": `checkout_${binding}_${request.claim_id ?? pack}` }, body: params,
  }, authority.fetchImpl);
  const url = new URL(session.url); if (url.protocol !== "https:" || url.hostname !== "checkout.stripe.com" || url.username || url.password) throw new Error("Stripe checkout URL is invalid");
  return json({ schema: "tohseno.managed-checkout-session/1", checkout_url: url.href }, 201);
}

async function stripeWebhook(authority: ManagedAuthority, request: Request): Promise<Response> {
  if (!authority.config.root) throw new Error("managed balance is not configured");
  const bytes = await boundedBytes(request, 256 * 1024);
  const text = verifyStripeWebhook(bytes, request.headers.get("stripe-signature") ?? "", authority.config.stripeWebhookSecret ?? "");
  const event = JSON.parse(text) as any; identifier(event.id, "Stripe event identifier");
  const type = String(event.type), object = event.data?.object;
  if (["checkout.session.completed", "checkout.session.async_payment_succeeded"].includes(type)) {
    const sessionID = String(object?.id); if (!/^cs_[A-Za-z0-9_]{1,160}$/.test(sessionID)) throw new Error("Stripe session identifier is invalid");
    const session = await stripeRequest(authority.config, `checkout/sessions/${encodeURIComponent(sessionID)}?expand[]=line_items`, { method: "GET" }, authority.fetchImpl);
    const pack = session?.metadata?.pack_id as keyof typeof PACKS, binding = session?.metadata?.installation_binding;
    identifier(binding, "installation binding"); if (!(pack in PACKS)) throw new Error("Stripe session pack is invalid");
    if (session.payment_status !== "paid" || session.currency !== "usd" || session.amount_total !== PACKS[pack] / 10_000) throw new Error("Stripe payment does not match its configured pack");
    const actualPrice = session?.line_items?.data?.[0]?.price?.id;
    if (actualPrice !== authority.config.priceIds[pack]) throw new Error("Stripe payment uses an unknown price");
    const payment = String(session.payment_intent); if (!/^pi_[A-Za-z0-9_]{1,160}$/.test(payment)) throw new Error("Stripe payment identifier is invalid");
    await withAccountLock(authority.config.root, binding, () => append(authority.config.root!, {
      installation_binding: binding, amount_microusd: PACKS[pack], entry_type: "purchase_credit", bucket: "paid",
      related_checkout_id: sessionID, related_payment_id: payment, idempotency_key: `stripe_purchase_${sessionID}`,
      description: `Stripe ${pack.replace("usd_", "$")} managed-compute balance purchase`,
    }));
  } else if (type === "checkout.session.async_payment_failed") {
    const binding = object?.metadata?.installation_binding; identifier(binding, "installation binding");
    await append(authority.config.root, { installation_binding: binding, amount_microusd: 0, entry_type: "checkout_failed", bucket: "none",
      related_checkout_id: String(object.id), idempotency_key: `stripe_failed_${event.id}`, description: "Stripe asynchronous payment failed" });
  } else if (type === "charge.refunded" || type === "charge.dispute.created" || type === "charge.dispute.closed") {
    const payment = String(object?.payment_intent); if (!/^pi_[A-Za-z0-9_]{1,160}$/.test(payment)) throw new Error("Stripe payment identifier is invalid");
    const purchase = await findPurchase(authority.config.root, payment);
    const amountCents = type === "charge.refunded" ? Number(object.amount_refunded) : Number(object.amount);
    integer(amountCents, "Stripe adjustment amount");
    await withAccountLock(authority.config.root, purchase.installation_binding, async () => {
      const accountEntries = await entries(authority.config.root!, purchase.installation_binding);
      if (type === "charge.refunded") {
        const target = amountCents * 10_000;
        if (target > purchase.amount_microusd) throw new Error("Stripe refund exceeds the credited purchase");
        const already = -accountEntries.filter((entry) => entry.entry_type === "refund_adjustment" && entry.related_payment_id === payment)
          .reduce((sum, entry) => sum + Math.min(0, entry.amount_microusd), 0);
        const delta = target - already;
        if (delta <= 0) return;
        await append(authority.config.root!, { installation_binding: purchase.installation_binding, amount_microusd: -delta,
          entry_type: "refund_adjustment", bucket: "paid", related_payment_id: payment,
          idempotency_key: `stripe_adjustment_${event.id}`, description: "Stripe refund adjustment" });
        return;
      }
      const disputeID = String(object?.id); identifier(disputeID, "Stripe dispute identifier");
      const related = accountEntries.filter((entry) => entry.entry_type === "dispute_adjustment" && entry.related_provider_id === disputeID);
      const won = related.some((entry) => entry.private_operator_metadata?.stripe_dispute_state === "won");
      let amount = 0, description = "Stripe dispute event recorded", state = "created";
      if (type === "charge.dispute.created") {
        if (!won && !related.some((entry) => entry.amount_microusd < 0)) amount = -amountCents * 10_000;
        description = amount < 0 ? "Stripe dispute hold" : "Duplicate or reordered Stripe dispute recorded";
      } else if (object?.status === "won") {
        amount = -related.reduce((sum, entry) => sum + Math.min(0, entry.amount_microusd), 0);
        description = amount > 0 ? "Stripe dispute won adjustment" : "Reordered Stripe dispute win recorded";
        state = "won";
      } else {
        description = "Stripe dispute closed without reversal";
        state = "lost";
      }
      await append(authority.config.root!, { installation_binding: purchase.installation_binding, amount_microusd: amount,
        entry_type: "dispute_adjustment", bucket: "paid", related_payment_id: payment, related_provider_id: disputeID,
        idempotency_key: `stripe_adjustment_${event.id}`, description,
        private_operator_metadata: { stripe_dispute_state: state } });
    });
  }
  return json({ received: true });
}

async function findPurchase(root: string, payment: string): Promise<BalanceLedgerEntry> {
  for (const account of (await readdir(root)).filter((name) => /^account-[a-f0-9]{64}$/.test(name))) {
    const bindingEntries = await readAccountEntries(root, account);
    const found = bindingEntries.find((entry) => entry.entry_type === "purchase_credit" && entry.related_payment_id === payment);
    if (found) return found;
  }
  throw new Error("Stripe payment has no credited purchase");
}

async function readAccountEntries(root: string, account: string): Promise<BalanceLedgerEntry[]> {
  try {
    const directory = join(root, account, "entries"); const names = await readdir(directory);
    return Promise.all(names.filter((name) => name.startsWith("entry_")).map(async (name) => JSON.parse(await readFile(join(directory, name), "utf8"))));
  } catch { return []; }
}

async function pendingReconciliationCount(root: string): Promise<number> {
  let count = 0;
  for (const account of (await readdir(root)).filter((name) => /^account-[a-f0-9]{64}$/.test(name)).slice(0, 100_000)) {
    const accountEntries = await readAccountEntries(root, account);
    const reservations = new Map<string, { pending: boolean; settled: boolean }>();
    for (const entry of accountEntries.filter((value) => value.entry_type === "provider_reconciliation" && value.related_reservation_id)) {
      const state = reservations.get(entry.related_reservation_id!) ?? { pending: false, settled: false };
      if (entry.reconciliation_status === "pending") state.pending = true;
      if (entry.reconciliation_status === "settled") state.settled = true;
      reservations.set(entry.related_reservation_id!, state);
    }
    count += [...reservations.values()].filter((state) => state.pending && !state.settled).length;
  }
  return count;
}

export interface ManagedRouter { handles(pathname: string): boolean; fetch(request: Request): Promise<Response> }

export async function createManagedRouter(config: AppConfig, provider?: ManagedProvider, fetchImpl: typeof fetch = fetch): Promise<ManagedRouter> {
  const prefix = "/api/managed/v1/";
  const managedProvider = provider ?? new BankrProvider(config.managed, fetchImpl);
  const authority = new ManagedAuthority(config.managed, managedProvider, fetchImpl);
  await authority.initialize();
  return {
    handles: (pathname) => pathname.startsWith(prefix),
    async fetch(request) {
      if (!config.managed.enabled || !config.managed.root) return json({ error: "Managed compute is not active" }, 503);
      const url = new URL(request.url);
      try {
        if (url.pathname === `${prefix}stripe/webhook` && request.method === "POST") return stripeWebhook(authority, request);
        if (url.pathname === `${prefix}operator/grants` && request.method === "POST") {
          if (!operatorAuthorized(request, config.managed)) return json({ error: "Operator authentication required" }, 401);
          const value = await jsonBody(request); const entry = await authority.grant(value.installation_binding, value.amount_microusd, value.reason, value.idempotency_key, value.operator ?? "operator");
          return json(entry, 201);
        }
        if (url.pathname === `${prefix}operator/revocations` && request.method === "POST") {
          if (!operatorAuthorized(request, config.managed)) return json({ error: "Operator authentication required" }, 401);
          const value = await jsonBody(request); const entry = await authority.revoke(value.installation_binding, value.amount_microusd, value.reason, value.idempotency_key, value.operator ?? "operator");
          return json(entry, 201);
        }
        if (url.pathname === `${prefix}operator/reconciliations` && request.method === "POST") {
          if (!operatorAuthorized(request, config.managed)) return json({ error: "Operator authentication required" }, 401);
          const value = await jsonBody(request); const entry = await authority.reconcile(value.installation_binding, value.reservation_id,
            value.action, value.retail_charge_microusd ?? 0, value.reason, value.idempotency_key, value.operator ?? "operator", value.provider_request_id);
          return json(entry, 201);
        }
        if (url.pathname === `${prefix}operator/health` && request.method === "GET") {
          if (!operatorAuthorized(request, config.managed)) return json({ error: "Operator authentication required" }, 401);
          return json({ schema: "tohseno.managed-operator-health/1", bankr_credits: await managedProvider.credits(), bankr_usage: await managedProvider.usage(),
            pending_reconciliations: await pendingReconciliationCount(config.managed.root),
            launch_fee_funding_active: config.managed.launchFeeFundingConfirmed,
            launch_fee_funding_reference: config.managed.launchFeeFundingConfirmed ? config.managed.launchFeeFundingReference : undefined });
        }
        if (url.pathname === `${prefix}chat/completions` && request.method === "POST") {
          const authorization = request.headers.get("authorization") ?? ""; const token = authorization.startsWith("Bearer ") ? authorization.slice(7) : "";
          return authority.complete(token, await boundedBytes(request, MAX_REQUEST_BYTES));
        }
        if (request.method !== "POST") return json({ error: "Method not allowed" }, 405);
        const value = await jsonBody(request, 128 * 1024); const action = url.pathname.slice(prefix.length).replaceAll("/", "_");
        const claim = verifyManagedClaim(value.claim, value.request, action);
        if (url.pathname === `${prefix}balance`) return json(await balanceProjection(config.managed.root, claim.installation_binding));
        if (url.pathname === `${prefix}catalog`) return json({ schema: "tohseno.managed-model-catalog/1", models: await authority.catalog() });
        if (url.pathname === `${prefix}checkout`) return checkout(authority, claim.installation_binding, { ...value.request, claim_id: claim.claim_id });
        if (url.pathname === `${prefix}reserve`) {
          const reserved = await authority.reserve(claim.installation_binding, value.request as ReservationRequest);
          return json({ schema: "tohseno.managed-reservation/1", ...reserved }, 201);
        }
        return json({ error: "Not found" }, 404);
      } catch (error) {
        const message = error instanceof Error ? error.message : "Managed request failed";
        const status = message.includes("insufficient managed") ? 402 : message.includes("already used") ? 409 : 400;
        return json({ error: message }, status);
      }
    },
  };
}
