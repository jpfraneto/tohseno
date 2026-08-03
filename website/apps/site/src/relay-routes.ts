import type { AppConfig } from "../config.ts";
import { ENVELOPE_ASSOCIATED_DATA, INTENT_LIMITS } from "./intent-limits.ts";
import { FilesystemRelayStorage, RelayStorageError } from "./relay-storage.ts";
import { withSecurityHeaders } from "./security.ts";

const JSON_TYPE = "application/json";
const BINARY_TYPE = "application/octet-stream";

export interface RelayRouter { handles(pathname: string): boolean; fetch(request: Request): Promise<Response> }

class RateLimiter {
  private windows = new Map<string, { started: number; count: number }>();
  take(key: string, maximum: number, now = Date.now()): boolean {
    if (this.windows.size > 10_000) {
      for (const [candidate, window] of this.windows) {
        if (now - window.started >= 60_000) this.windows.delete(candidate);
      }
      if (this.windows.size > 10_000 && !this.windows.has(key)) return false;
    }
    const current = this.windows.get(key);
    if (!current || now - current.started >= 60_000) { this.windows.set(key, { started: now, count: 1 }); return true; }
    if (current.count >= maximum) return false;
    current.count += 1; return true;
  }
}

export async function createRelayRouter(config: AppConfig): Promise<RelayRouter> {
  const enabled = config.relay.enabled && config.relay.claimInstallerReady;
  const storage = enabled
    ? new FilesystemRelayStorage(config.relay.root!, { maxRecords: config.relay.maxRecords, maxBytes: config.relay.maxBytes })
    : null;
  if (storage) await storage.initialize();
  const limiter = new RateLimiter();

  const json = (data: unknown, status = 200) => privateResponse(JSON.stringify(data), status, `${JSON_TYPE}; charset=utf-8`);
  return {
    handles: (pathname) => pathname === "/api/intent-relay" || pathname.startsWith("/api/intent-relay/"),
    async fetch(request) {
      try {
        const url = new URL(request.url);
        if (url.pathname === "/api/intent-relay" && request.method === "GET") {
          return json({
            schema: "tohseno.intent-relay-capabilities/1",
            available: enabled,
            installer_url: `${config.baseUrl}/oneshot.sh`,
            relay_lifetime_seconds: INTENT_LIMITS.relayLifetimeMs / 1000,
            limits: {
              prompt_bytes: INTENT_LIMITS.promptBytes, reference_bytes: INTENT_LIMITS.referenceBytes,
              total_reference_bytes: INTENT_LIMITS.totalReferenceBytes, package_bytes: INTENT_LIMITS.packageBytes,
              chunk_bytes: INTENT_LIMITS.chunkBytes, references: INTENT_LIMITS.references,
            },
          });
        }
        if (!enabled || !storage) return json({ error: "Encrypted handoff is not activated; download the private intent package instead." }, 503);
        const source = sourceKey(request, config);
        if (!limiter.take("global", config.relay.globalRequestsPerMinute) || !limiter.take(source, config.relay.sourceRequestsPerMinute)) {
          return json({ error: "Relay request rate exceeded; retry later." }, 429);
        }
        const record = url.pathname.match(/^\/api\/intent-relay\/records\/([A-Za-z0-9_-]{32})(?:\/(.*))?$/);
        if (url.pathname === "/api/intent-relay/records" && request.method === "POST") {
          requireBrowserMutation(request, config, JSON_TYPE);
          const body = await readJson(request, 16 * 1024) as Record<string, unknown>;
          exactKeys(body, ["schema", "ciphertext_bytes", "chunk_count", "ciphertext_sha256", "nonce", "associated_data", "upload_verifier", "status_verifier", "claim_verifier"]);
          if (body.schema !== "tohseno.intent-relay-create/1" || body.associated_data !== ENVELOPE_ASSOCIATED_DATA) throw new RelayStorageError(400, "Relay create schema is unsupported");
          const ciphertextBytes = integer(body.ciphertext_bytes, 17, INTENT_LIMITS.ciphertextBytes, "ciphertext_bytes");
          const chunkCount = integer(body.chunk_count, 1, INTENT_LIMITS.maxChunks, "chunk_count");
          if (chunkCount !== Math.ceil(ciphertextBytes / INTENT_LIMITS.chunkBytes)) throw new RelayStorageError(400, "chunk_count does not match ciphertext size");
          for (const field of ["ciphertext_sha256", "upload_verifier", "status_verifier", "claim_verifier"] as const) requireHex(body[field], field);
          if (new Set([body.upload_verifier, body.status_verifier, body.claim_verifier]).size !== 3) throw new RelayStorageError(400, "Relay capabilities must be independent");
          requireBase64Url(body.nonce, 16, "nonce");
          const created = await storage.create({ ciphertextBytes, chunkCount, ciphertextSha256: body.ciphertext_sha256 as string, nonce: body.nonce as string, associatedData: body.associated_data as string, uploadVerifier: body.upload_verifier as string, statusVerifier: body.status_verifier as string, claimVerifier: body.claim_verifier as string });
          return json({ schema: "tohseno.intent-relay-created/1", relay_id: created.id, expires_at: created.expiresAt }, 201);
        }
        if (!record) throw new RelayStorageError(404, "Not found");
        const [, id, action = ""] = record;
        if (request.method === "PUT" && /^chunks\/\d{6}$/.test(action)) {
          requireBrowserMutation(request, config, BINARY_TYPE);
          const index = Number(action.slice(7)); const capability = bearer(request);
          const digest = request.headers.get("x-chunk-sha256") ?? ""; requireHex(digest, "x-chunk-sha256");
          const contentLength = request.headers.get("content-length");
          if (contentLength && Number(contentLength) > INTENT_LIMITS.chunkBytes) throw new RelayStorageError(413, "Chunk body exceeds the relay limit", "body_limit");
          const bytes = new Uint8Array(await request.arrayBuffer());
          const result = await storage.uploadChunk(id, capability, index, bytes, digest);
          return json({ accepted: true, duplicate: result.duplicate });
        }
        if (request.method === "POST" && action === "finalize") {
          requireBrowserMutation(request, config, JSON_TYPE); await requireEmptyJson(request);
          const result = await storage.finalize(id, bearer(request)); return json({ state: "ready", expires_at: result.expiresAt });
        }
        if (request.method === "GET" && action === "status") {
          const result = await storage.status(id, bearer(request)); return json(result);
        }
        if (request.method === "POST" && action === "claim") {
          requireCliRequest(request); await requireEmptyJson(request);
          return json(await storage.lease(id, bearer(request)));
        }
        const claimChunk = action.match(/^claim\/chunks\/(\d{6})$/);
        if (request.method === "GET" && claimChunk) {
          requireCliRequest(request); const result = await storage.downloadChunk(id, bearer(request), Number(claimChunk[1]));
          const body = result.bytes.buffer.slice(
            result.bytes.byteOffset,
            result.bytes.byteOffset + result.bytes.byteLength,
          ) as ArrayBuffer;
          const response = privateResponse(body, 200, BINARY_TYPE); response.headers.set("X-Chunk-SHA256", result.sha256); return response;
        }
        if (request.method === "POST" && action === "complete") {
          requireCliRequest(request); await requireEmptyJson(request); await storage.complete(id, bearer(request)); return json({ state: "completed" });
        }
        if (request.method === "POST" && action === "release") {
          requireCliRequest(request); await requireEmptyJson(request); await storage.release(id, bearer(request)); return json({ state: "ready" });
        }
        if (request.method === "POST" && action === "cancel") {
          requireBrowserMutation(request, config, JSON_TYPE); await requireEmptyJson(request); await storage.cancel(id, bearer(request)); return json({ state: "cancelled" });
        }
        throw new RelayStorageError(404, "Not found");
      } catch (error) {
        if (error instanceof RelayStorageError) return json({ error: error.message }, error.status);
        return json({ error: "The relay request could not be completed" }, 500);
      }
    },
  };
}

function privateResponse(body: BodyInit | null, status: number, contentType: string): Response {
  const response = withSecurityHeaders(new Response(body, { status, headers: { "Content-Type": contentType, "Cache-Control": "no-store" } }));
  response.headers.set("Cache-Control", "no-store"); return response;
}
function requireBrowserMutation(request: Request, config: AppConfig, type: string): void {
  if (request.headers.get("origin") !== config.baseUrl) throw new RelayStorageError(403, "Exact same-origin request required", "origin");
  const fetchSite = request.headers.get("sec-fetch-site");
  if (fetchSite && fetchSite !== "same-origin") throw new RelayStorageError(403, "Cross-site request rejected", "origin");
  requireContentType(request, type);
}
function requireCliRequest(request: Request): void { requireContentType(request, JSON_TYPE, request.method === "GET"); }
function requireContentType(request: Request, expected: string, absentAllowed = false): void {
  const actual = request.headers.get("content-type")?.split(";", 1)[0]?.trim().toLowerCase();
  if ((!actual && !absentAllowed) || (actual && actual !== expected)) throw new RelayStorageError(415, `Content-Type must be ${expected}`, "content_type");
}
function bearer(request: Request): string {
  const match = request.headers.get("authorization")?.match(/^Bearer ([A-Za-z0-9_-]{43})$/);
  if (!match) throw new RelayStorageError(401, "A valid bearer capability is required", "authorization");
  return match[1];
}
async function readJson(request: Request, limit: number): Promise<unknown> {
  const contentLength = request.headers.get("content-length");
  if (contentLength && Number(contentLength) > limit) throw new RelayStorageError(413, "JSON body exceeds the relay limit", "body_limit");
  const bytes = new Uint8Array(await request.arrayBuffer());
  if (bytes.byteLength > limit) throw new RelayStorageError(413, "JSON body exceeds the relay limit", "body_limit");
  try { return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); } catch { throw new RelayStorageError(400, "JSON body is malformed"); }
}
async function requireEmptyJson(request: Request): Promise<void> { const body = await readJson(request, 128); if (!body || typeof body !== "object" || Array.isArray(body) || Object.keys(body as object).length !== 0) throw new RelayStorageError(400, "Request body must be an empty JSON object"); }
function exactKeys(value: Record<string, unknown>, keys: string[]): void { if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).sort().join("|") !== [...keys].sort().join("|")) throw new RelayStorageError(400, "Request schema fields are invalid"); }
function integer(value: unknown, minimum: number, maximum: number, name: string): number { if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new RelayStorageError(400, `${name} is out of range`); return value as number; }
function requireHex(value: unknown, name: string): void { if (typeof value !== "string" || !/^[a-f0-9]{64}$/.test(value)) throw new RelayStorageError(400, `${name} must be a lowercase SHA-256 digest`); }
function requireBase64Url(value: unknown, maximum: number, name: string): void { if (typeof value !== "string" || value.length > maximum || !/^[A-Za-z0-9_-]+$/.test(value)) throw new RelayStorageError(400, `${name} is malformed`); }
function sourceKey(request: Request, config: AppConfig): string { const forwarded = config.trustProxy ? request.headers.get("x-forwarded-for")?.split(",", 1)[0]?.trim() : undefined; return `source:${(forwarded || "direct").slice(0, 64)}`; }
