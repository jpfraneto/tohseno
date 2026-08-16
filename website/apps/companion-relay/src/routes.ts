import type { CompanionRelayConfig } from "../config.ts";
import { RelayError } from "./errors.ts";
import type { PushProvider } from "./push-provider.ts";
import { secureResponse } from "./security.ts";
import {
  CompanionRelayStorage,
  validateDeviceId,
  validateEnvelopeId,
  validateOpaqueId,
  validateVerifier,
} from "./storage.ts";
import type { MailboxPage } from "./storage.ts";
import type { MailboxEvent, PushRegistration, RelayEnvelope } from "./types.ts";

const JSON_TYPE = "application/json";
const BINARY_TYPE = "application/octet-stream";
const CLOCK_SKEW_DESCRIPTION = "five-minute default, configurable to at most fifteen minutes";

export interface RuntimeCounters {
  pushWakeAccepted: number;
  pushWakeFailed: number;
}

export interface CompanionRouter {
  handles(pathname: string): boolean;
  fetch(request: Request): Promise<Response>;
}

export function createCompanionRouter(options: {
  config: CompanionRelayConfig;
  storage: CompanionRelayStorage | null;
  push: PushProvider;
  counters: RuntimeCounters;
  operationalLog: (record: Record<string, unknown>) => void;
  now?: () => number;
}): CompanionRouter {
  const { config, storage, push, counters, operationalLog } = options;
  const now = options.now ?? (() => Date.now());

  return {
    handles: (pathname) => pathname.startsWith("/v1/companion/") || pathname === "/v1/companion",
    async fetch(request) {
      if (!config.enabled || !storage) {
        return json({ error: "Companion relay is not activated" }, 503);
      }
      try {
        const url = new URL(request.url);
        const pathname = url.pathname;
        const method = request.method.toUpperCase();

        if (pathname === "/v1/companion" && method === "GET") {
          return json({
            schema: "tohseno.companion-relay-capabilities/1",
            available: true,
            service_version: "0.9.0",
            pairing_lifetime_seconds: config.limits.pairingLifetimeMs / 1000,
            retention_seconds: config.limits.retentionMs / 1000,
            clock_skew_seconds: config.limits.clockSkewMs / 1000,
            clock_skew_policy: CLOCK_SKEW_DESCRIPTION,
            max_envelope_bytes: config.limits.envelopeBytes,
            max_envelope_body_bytes: config.limits.envelopeBodyBytes,
            max_pairing_response_bytes: config.limits.pairingResponseBytes,
            max_catch_up_envelopes: config.limits.catchUpLimit,
            live_delivery: "sse",
            push_mode: push.mode,
          });
        }

        if (pathname === "/v1/companion/pairing-sessions" && method === "POST") {
          requireContentType(request, JSON_TYPE);
          const body = await readJsonObject(request, 8 * 1024);
          exactKeys(body, ["schema", "expires_at", "read_verifier", "cancel_verifier"]);
          if (body.schema !== "tohseno.companion-pairing-session-create/1") {
            throw new RelayError(400, "Pairing create schema is unsupported", "schema");
          }
          validateVerifier(string(body.read_verifier, "read_verifier"));
          validateVerifier(string(body.cancel_verifier, "cancel_verifier"));
          const expiresAt = timestamp(body.expires_at, "expires_at");
          const created = await storage.createPairing(
            {
              read: body.read_verifier as string,
              cancel: body.cancel_verifier as string,
            },
            expiresAt,
            now(),
          );
          return json({
            schema: "tohseno.companion-pairing-session-created/1",
            session_id: created.id,
            expires_at: formatTimestamp(created.expiresAt),
          }, 201);
        }

        const pairing = pathname.match(/^\/v1\/companion\/pairing-sessions\/([A-Za-z0-9_-]{32})(?:\/(respond))?$/);
        if (pairing) {
          const [, id, action] = pairing;
          if (action === "respond" && method === "POST") {
            requireContentType(request, BINARY_TYPE);
            const bytes = await readBytes(request, config.limits.pairingResponseBytes);
            const result = await storage.submitPairingResponse(id, bytes, now());
            return json({
              schema: "tohseno.companion-pairing-response-accepted/1",
              accepted: true,
              duplicate: result.duplicate,
            }, result.duplicate ? 200 : 201);
          }
          if (!action && method === "GET") {
            const bytes = await storage.readPairingResponse(id, bearer(request), now());
            if (!bytes) return privateResponse(null, 204, BINARY_TYPE);
            return privateResponse(
              bytes.buffer.slice(
                bytes.byteOffset,
                bytes.byteOffset + bytes.byteLength,
              ) as ArrayBuffer,
              200,
              BINARY_TYPE,
            );
          }
          if (!action && method === "DELETE") {
            await storage.cancelPairing(id, bearer(request), now());
            return privateResponse(null, 204, JSON_TYPE);
          }
        }

        if (pathname === "/v1/companion/mailboxes" && method === "POST") {
          requireContentType(request, JSON_TYPE);
          const body = await readJsonObject(request, 16 * 1024);
          exactKeys(body, [
            "schema",
            "write_verifier",
            "read_verifier",
            "ack_verifier",
            "revoke_verifier",
            "push_verifier",
          ]);
          if (body.schema !== "tohseno.companion-mailbox-create/1") {
            throw new RelayError(400, "Mailbox create schema is unsupported", "schema");
          }
          for (const field of ["write_verifier", "read_verifier", "ack_verifier", "revoke_verifier", "push_verifier"] as const) {
            validateVerifier(string(body[field], field));
          }
          const created = await storage.createMailbox({
            write: body.write_verifier as string,
            read: body.read_verifier as string,
            ack: body.ack_verifier as string,
            revoke: body.revoke_verifier as string,
            push: body.push_verifier as string,
          }, now());
          return json({
            schema: "tohseno.companion-mailbox-created/1",
            mailbox_id: created.id,
            created_at: formatTimestamp(created.createdAt),
          }, 201);
        }

        const mailbox = pathname.match(/^\/v1\/companion\/mailboxes\/([A-Za-z0-9_-]{32})(?:\/(envelopes|live|ack))?$/);
        if (mailbox) {
          const [, mailboxId, action] = mailbox;
          if (action === "envelopes" && method === "POST") {
            requireContentType(request, JSON_TYPE);
            const body = await readJsonObject(request, config.limits.envelopeBodyBytes);
            const envelope = validateEnvelope(body, mailboxId, config, now());
            const canonicalBytes = new TextEncoder().encode(JSON.stringify(envelope));
            if (canonicalBytes.byteLength > config.limits.envelopeBodyBytes) {
              throw new RelayError(413, "Envelope exceeds the relay limit", "body_limit");
            }
            const result = await storage.uploadEnvelope(
              mailboxId,
              bearer(request),
              envelope,
              canonicalBytes,
              now(),
            );
            if (!result.duplicate) {
              let registrations: PushRegistration[] = [];
              try {
                registrations = await storage.pushRegistrations(mailboxId);
              } catch {
                counters.pushWakeFailed += 1;
                operationalLog({
                  event: "push_registration_read_failed",
                  attempted: 0,
                  accepted: 0,
                  failed: 1,
                });
              }
              const outcomes = await Promise.allSettled(
                registrations.map((registration) => push.sendWake(registration)),
              );
              for (const outcome of outcomes) {
                if (outcome.status === "fulfilled") counters.pushWakeAccepted += 1;
                else counters.pushWakeFailed += 1;
              }
              if (outcomes.length > 0) {
                operationalLog({
                  event: "push_wake_batch",
                  attempted: outcomes.length,
                  accepted: outcomes.filter((item) => item.status === "fulfilled").length,
                  failed: outcomes.filter((item) => item.status === "rejected").length,
                });
              }
            }
            return json({
              schema: "tohseno.companion-envelope-accepted/1",
              accepted: true,
              duplicate: result.duplicate,
              cursor: result.cursor,
            }, result.duplicate ? 200 : 201);
          }
          if (action === "envelopes" && method === "GET") {
            const cursor = cursorFrom(url.searchParams.get("cursor"));
            const limit = optionalPositiveInteger(
              url.searchParams.get("limit"),
              config.limits.catchUpLimit,
            );
            const page = await storage.listEnvelopes(
              mailboxId,
              bearer(request),
              cursor,
              limit,
              now(),
            );
            if (page.resetRequired) {
              return json({
                schema: "tohseno.companion-mailbox-reset-required/1",
                reset_required: true,
                reset_before_cursor: page.resetBeforeCursor,
                head_cursor: page.headCursor,
              }, 409);
            }
            return json({
              schema: "tohseno.companion-mailbox-page/1",
              envelopes: page.envelopes,
              next_cursor: page.nextCursor,
              head_cursor: page.headCursor,
              has_more: page.hasMore,
            });
          }
          if (action === "live" && method === "GET") {
            const cursor = liveCursor(request, url);
            const capability = bearer(request);
            const page = await storage.listEnvelopes(
              mailboxId,
              capability,
              cursor,
              config.limits.catchUpLimit,
              now(),
            );
            if (page.resetRequired) {
              return json({
                schema: "tohseno.companion-mailbox-reset-required/1",
                reset_required: true,
                reset_before_cursor: page.resetBeforeCursor,
                head_cursor: page.headCursor,
              }, 409);
            }
            if (page.hasMore) {
              return json({
                schema: "tohseno.companion-mailbox-catch-up-required/1",
                catch_up_required: true,
                next_cursor: page.nextCursor,
                head_cursor: page.headCursor,
              }, 409);
            }
            return liveResponse(
              storage,
              mailboxId,
              capability,
              page,
              config.limits.catchUpLimit,
              now,
            );
          }
          if (action === "ack" && method === "POST") {
            requireContentType(request, JSON_TYPE);
            const body = await readJsonObject(request, 1024);
            exactKeys(body, ["schema", "cursor"]);
            if (body.schema !== "tohseno.companion-mailbox-ack/1") {
              throw new RelayError(400, "Mailbox acknowledgement schema is unsupported", "schema");
            }
            const cursor = wholeNumber(body.cursor, "cursor");
            const result = await storage.acknowledge(mailboxId, bearer(request), cursor);
            return json({
              schema: "tohseno.companion-mailbox-acknowledged/1",
              acknowledged_cursor: result.acknowledgedCursor,
            });
          }
          if (!action && method === "DELETE") {
            const result = await storage.revokeMailbox(mailboxId, bearer(request), now());
            return json({
              schema: "tohseno.companion-mailbox-revoked/1",
              revoked: true,
              revocation_epoch: result.revocationEpoch,
            });
          }
        }

        if (pathname === "/v1/companion/push/register" && method === "POST") {
          requireContentType(request, JSON_TYPE);
          const body = await readJsonObject(request, 8 * 1024);
          exactKeys(body, ["schema", "mailbox_id", "device_id", "apns_token"]);
          if (body.schema !== "tohseno.companion-push-register/1") {
            throw new RelayError(400, "Push registration schema is unsupported", "schema");
          }
          const mailboxId = string(body.mailbox_id, "mailbox_id");
          const deviceId = string(body.device_id, "device_id");
          validateOpaqueId(mailboxId, "mailbox ID");
          validateDeviceId(deviceId);
          await storage.registerPush(
            mailboxId,
            bearer(request),
            deviceId,
            string(body.apns_token, "apns_token"),
            now(),
          );
          return json({ schema: "tohseno.companion-push-registered/1", registered: true }, 201);
        }

        const pushRemoval = pathname.match(/^\/v1\/companion\/push\/register\/([A-Za-z0-9_-]{16,128})$/);
        if (pushRemoval && method === "DELETE") {
          requireContentType(request, JSON_TYPE);
          const body = await readJsonObject(request, 1024);
          exactKeys(body, ["schema", "mailbox_id"]);
          if (body.schema !== "tohseno.companion-push-unregister/1") {
            throw new RelayError(400, "Push removal schema is unsupported", "schema");
          }
          const mailboxId = string(body.mailbox_id, "mailbox_id");
          await storage.removePush(mailboxId, bearer(request), pushRemoval[1]);
          return privateResponse(null, 204, JSON_TYPE);
        }

        throw new RelayError(404, "Not found", "not_found");
      } catch (error) {
        if (error instanceof RelayError) {
          return json({ error: error.message, error_class: error.errorClass }, error.status);
        }
        throw error;
      }
    },
  };
}

function validateEnvelope(
  body: Record<string, unknown>,
  mailboxId: string,
  config: CompanionRelayConfig,
  now: number,
): RelayEnvelope {
  const keys = [
    "schema",
    "envelope_id",
    "mailbox_id",
    "sender_device_id",
    "recipient_device_id",
    "sender_sequence",
    "created_at",
    "expires_at",
    "ephemeral_public_key",
    "nonce",
    "ciphertext",
    "signature",
  ];
  exactKeys(body, keys);
  if (body.schema !== "tohseno.companion-envelope/1") {
    throw new RelayError(400, "Companion envelope schema is unsupported", "schema");
  }
  const envelopeId = string(body.envelope_id, "envelope_id");
  validateEnvelopeId(envelopeId);
  const routeMailbox = string(body.mailbox_id, "mailbox_id");
  validateOpaqueId(routeMailbox, "mailbox ID");
  if (routeMailbox !== mailboxId) throw new RelayError(400, "Envelope mailbox does not match its route", "schema");
  const senderDeviceId = string(body.sender_device_id, "sender_device_id");
  const recipientDeviceId = string(body.recipient_device_id, "recipient_device_id");
  validateDeviceId(senderDeviceId);
  validateDeviceId(recipientDeviceId);
  const senderSequence = positiveWholeNumber(body.sender_sequence, "sender_sequence");
  const createdAt = timestamp(body.created_at, "created_at");
  const expiresAt = timestamp(body.expires_at, "expires_at");
  if (createdAt > now + config.limits.clockSkewMs) {
    throw new RelayError(400, "Envelope creation time is too far in the future", "clock_skew");
  }
  if (expiresAt <= createdAt || expiresAt + config.limits.clockSkewMs <= now) {
    throw new RelayError(410, "Envelope is expired", "expired");
  }
  if (expiresAt - createdAt > config.limits.retentionMs) {
    throw new RelayError(400, "Envelope lifetime exceeds bounded retention", "expiry");
  }
  if (expiresAt > now + config.limits.retentionMs + config.limits.clockSkewMs) {
    throw new RelayError(400, "Envelope expiry exceeds bounded retention", "expiry");
  }
  const ephemeralPublicKey = base64Url(body.ephemeral_public_key, 43, 43, "ephemeral_public_key");
  const nonce = base64Url(body.nonce, 16, 16, "nonce");
  const ciphertext = base64Url(body.ciphertext, 22, Math.ceil(config.limits.envelopeBytes * 4 / 3), "ciphertext");
  const signature = base64Url(body.signature, 86, 86, "signature");
  return {
    schema: "tohseno.companion-envelope/1",
    envelope_id: envelopeId,
    mailbox_id: routeMailbox,
    sender_device_id: senderDeviceId,
    recipient_device_id: recipientDeviceId,
    sender_sequence: senderSequence,
    created_at: body.created_at as string,
    expires_at: body.expires_at as string,
    ephemeral_public_key: ephemeralPublicKey,
    nonce,
    ciphertext,
    signature,
  };
}

function liveResponse(
  storage: CompanionRelayStorage,
  mailboxId: string,
  capability: string,
  initial: MailboxPage,
  catchUpLimit: number,
  now: () => number,
): Response {
  const encoder = new TextEncoder();
  let unsubscribe = () => {};
  let heartbeat: ReturnType<typeof setInterval> | undefined;
  let closed = false;
  const stream = new ReadableStream<Uint8Array>({
    async start(controller) {
      const enqueue = (text: string) => {
        if (!closed) controller.enqueue(encoder.encode(text));
      };
      let lastSent = initial.envelopes.at(-1)?.cursor ?? initial.nextCursor;
      let reconciling = true;
      const pending: MailboxEvent[] = [];
      const send = (event: MailboxEvent) => {
        if (event.kind === "envelope" && event.envelope) {
          if (event.cursor <= lastSent) return;
          enqueue(sseEnvelope(event.cursor, event.envelope));
          lastSent = event.cursor;
          return;
        }
        if (event.kind === "revoked") {
          enqueue(`id: ${event.cursor}\nevent: revoked\ndata: {"revoked":true}\n\n`);
          closed = true;
          unsubscribe();
          if (heartbeat) clearInterval(heartbeat);
          controller.close();
        }
      };
      const listener = (event: MailboxEvent) => {
        if (reconciling) pending.push(event);
        else send(event);
      };
      unsubscribe = storage.subscribe(mailboxId, listener);
      enqueue(": tohseno companion relay\n\n");
      for (const item of initial.envelopes) {
        enqueue(sseEnvelope(item.cursor, item.envelope));
      }
      try {
        const gap = await storage.listEnvelopes(
          mailboxId,
          capability,
          initial.headCursor,
          catchUpLimit,
          now(),
        );
        if (gap.resetRequired || gap.hasMore) {
          enqueue(`event: reconcile\ndata: {"snapshot_required":true}\n\n`);
          closed = true;
          unsubscribe();
          controller.close();
          return;
        }
        for (const item of gap.envelopes) {
          send({ kind: "envelope", cursor: item.cursor, envelope: item.envelope });
        }
        reconciling = false;
        for (const event of pending.sort((left, right) => left.cursor - right.cursor)) send(event);
      } catch {
        const revoked = pending.find((event) => event.kind === "revoked");
        if (revoked) send(revoked);
        else {
          enqueue(`event: reconcile\ndata: {"snapshot_required":true}\n\n`);
          closed = true;
          unsubscribe();
          controller.close();
        }
        return;
      }
      if (closed) return;
      heartbeat = setInterval(() => enqueue(": keepalive\n\n"), 15_000);
    },
    cancel() {
      closed = true;
      unsubscribe();
      if (heartbeat) clearInterval(heartbeat);
    },
  });
  return privateResponse(stream, 200, "text/event-stream; charset=utf-8", {
    Connection: "keep-alive",
    "X-Accel-Buffering": "no",
  });
}

function sseEnvelope(cursor: number, envelope: RelayEnvelope): string {
  return `id: ${cursor}\nevent: envelope\ndata: ${JSON.stringify(envelope)}\n\n`;
}

function liveCursor(request: Request, url: URL): number {
  const query = url.searchParams.get("cursor");
  const header = request.headers.get("last-event-id");
  if (query !== null && header !== null && query !== header) {
    throw new RelayError(400, "Cursor and Last-Event-ID disagree", "cursor");
  }
  return cursorFrom(query ?? header);
}

function cursorFrom(value: string | null): number {
  if (value === null || value === "") return 0;
  if (!/^\d{1,16}$/.test(value)) throw new RelayError(400, "Cursor is malformed", "cursor");
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) throw new RelayError(400, "Cursor is malformed", "cursor");
  return parsed;
}

function optionalPositiveInteger(value: string | null, fallback: number): number {
  if (value === null) return fallback;
  if (!/^\d{1,4}$/.test(value)) throw new RelayError(400, "Limit is malformed", "limit");
  return Number(value);
}

function bearer(request: Request): string {
  const match = request.headers.get("authorization")?.match(/^Bearer ([A-Za-z0-9_-]{43})$/);
  if (!match) throw new RelayError(401, "A valid bearer capability is required", "authorization");
  return match[1];
}

function requireContentType(request: Request, expected: string): void {
  const actual = request.headers.get("content-type")?.split(";", 1)[0]?.trim().toLowerCase();
  if (actual !== expected) throw new RelayError(415, `Content-Type must be ${expected}`, "content_type");
}

async function readJsonObject(request: Request, maximum: number): Promise<Record<string, unknown>> {
  const bytes = await readBytes(request, maximum);
  let value: unknown;
  try {
    value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch {
    throw new RelayError(400, "JSON body is malformed", "schema");
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new RelayError(400, "JSON body must be an object", "schema");
  }
  return value as Record<string, unknown>;
}

async function readBytes(request: Request, maximum: number): Promise<Uint8Array> {
  const contentLength = request.headers.get("content-length");
  if (contentLength && (!/^\d+$/.test(contentLength) || Number(contentLength) > maximum)) {
    throw new RelayError(413, "Request body exceeds the relay limit", "body_limit");
  }
  const bytes = new Uint8Array(await request.arrayBuffer());
  if (bytes.byteLength > maximum) throw new RelayError(413, "Request body exceeds the relay limit", "body_limit");
  return bytes;
}

function exactKeys(value: Record<string, unknown>, keys: string[]): void {
  if (Object.keys(value).sort().join("|") !== [...keys].sort().join("|")) {
    throw new RelayError(400, "Request schema fields are invalid", "schema");
  }
}

function string(value: unknown, name: string): string {
  if (typeof value !== "string") throw new RelayError(400, `${name} must be a string`, "schema");
  return value;
}

function timestamp(value: unknown, name: string): number {
  const text = string(value, name);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(text)) {
    throw new RelayError(400, `${name} must use canonical UTC second format`, "schema");
  }
  const parsed = Date.parse(text);
  if (!Number.isFinite(parsed) || formatTimestamp(parsed) !== text) {
    throw new RelayError(400, `${name} is invalid`, "schema");
  }
  return parsed;
}

function formatTimestamp(value: number): string {
  return new Date(Math.floor(value / 1000) * 1000)
    .toISOString()
    .replace(".000Z", "Z");
}

function wholeNumber(value: unknown, name: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    throw new RelayError(400, `${name} must be a non-negative whole number`, "schema");
  }
  return value as number;
}

function positiveWholeNumber(value: unknown, name: string): number {
  const parsed = wholeNumber(value, name);
  if (parsed < 1) throw new RelayError(400, `${name} must be positive`, "schema");
  return parsed;
}

function base64Url(value: unknown, minimum: number, maximum: number, name: string): string {
  const text = string(value, name);
  if (
    text.length < minimum ||
    text.length > maximum ||
    !/^[A-Za-z0-9_-]+$/.test(text) ||
    text.length % 4 === 1
  ) {
    throw new RelayError(400, `${name} is malformed`, "schema");
  }
  let decoded: Buffer;
  try {
    decoded = Buffer.from(text, "base64url");
  } catch {
    throw new RelayError(400, `${name} is malformed`, "schema");
  }
  if (decoded.toString("base64url") !== text) {
    throw new RelayError(400, `${name} is not canonical base64url`, "schema");
  }
  return text;
}

function json(data: unknown, status = 200): Response {
  return privateResponse(JSON.stringify(data), status, `${JSON_TYPE}; charset=utf-8`);
}

function privateResponse(
  body: BodyInit | null,
  status: number,
  contentType: string,
  extraHeaders: Record<string, string> = {},
): Response {
  return secureResponse(new Response(body, {
    status,
    headers: { "Content-Type": contentType, ...extraHeaders },
  }));
}
