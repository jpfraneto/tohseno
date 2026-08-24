import type { CompanionRelayConfig } from "./config.ts";
import {
  loadCompanionRelayConfig,
  safeCompanionStartupSummary,
} from "./config.ts";
import { RelayError } from "./src/errors.ts";
import { createPushProvider } from "./src/push-provider.ts";
import type { PushProvider } from "./src/push-provider.ts";
import { WindowRateLimiter } from "./src/rate-limit.ts";
import type { Clock } from "./src/rate-limit.ts";
import { createCompanionRouter } from "./src/routes.ts";
import type { RuntimeCounters } from "./src/routes.ts";
import { secureResponse } from "./src/security.ts";
import { CompanionRelayStorage } from "./src/storage.ts";

export interface CompanionRelayApplicationOptions {
  config?: CompanionRelayConfig;
  storage?: CompanionRelayStorage | null;
  push?: PushProvider;
  clock?: Clock;
  log?: (record: Record<string, unknown>) => void;
  logError?: (record: Record<string, unknown>) => void;
}

export interface CompanionRelayApplication {
  config: CompanionRelayConfig;
  storage: CompanionRelayStorage | null;
  push: PushProvider;
  fetch(request: Request, directSourceAddress?: string): Promise<Response>;
}

export async function createCompanionRelayApplication(
  options: CompanionRelayApplicationOptions = {},
): Promise<CompanionRelayApplication> {
  const config = options.config ?? loadCompanionRelayConfig();
  const clock = options.clock ?? { now: () => Date.now() };
  const log = options.log ?? ((record) => console.info(JSON.stringify(record)));
  const logError = options.logError ?? ((record) => console.error(JSON.stringify(record)));
  const storage = options.storage === undefined
    ? config.enabled
      ? new CompanionRelayStorage(config.root!, config.limits)
      : null
    : options.storage;
  if (storage) await storage.initialize();
  const push = options.push ?? await createPushProvider(config, { now: () => clock.now() });
  const counters: RuntimeCounters & {
    requests: number;
    rateLimited: number;
  } = {
    requests: 0,
    rateLimited: 0,
    pushWakeAccepted: 0,
    pushWakeFailed: 0,
  };
  const router = createCompanionRouter({
    config,
    storage,
    push,
    counters,
    operationalLog: log,
    now: () => clock.now(),
  });
  const limiter = new WindowRateLimiter(clock);

  return {
    config,
    storage,
    push,
    async fetch(request: Request, directSourceAddress?: string): Promise<Response> {
      const requestId = crypto.randomUUID();
      const started = performance.now();
      const requestedMethod = request.method.toUpperCase();
      const method = ["GET", "POST", "DELETE"].includes(requestedMethod)
        ? requestedMethod
        : "OTHER";
      const pathname = new URL(request.url).pathname;
      const route = semanticRoute(pathname);
      let status = 500;
      counters.requests += 1;
      try {
        if (
          !hasExpectedOrigin(request, config) &&
          !isExpectedHealthcheck(request, config, pathname, requestedMethod)
        ) {
          throw new RelayError(421, "Request authority does not match the configured relay origin", "authority");
        }
        let response: Response;
        if (pathname === "/healthz" && requestedMethod === "GET") {
          response = json({
            schema: "tohseno.companion-relay-health/1",
            service_version: "1.0.0",
            ready: config.enabled,
            push_enabled: push.mode !== "noop",
            maximum_envelope_bytes: config.limits.envelopeBytes,
            retention_seconds: config.limits.retentionMs / 1000,
          });
        } else if (pathname === "/metrics" && requestedMethod === "GET") {
          const capacity = storage
            ? await storage.metrics()
            : {
                pairingSessions: 0,
                mailboxes: 0,
                revokedMailboxes: 0,
                envelopes: 0,
                bytes: 0,
                pushRegistrations: 0,
                liveSubscribers: 0,
              };
          response = json({
            schema: "tohseno.companion-relay-metrics/1",
            requests: counters.requests,
            rate_limited: counters.rateLimited,
            push_wake_accepted: counters.pushWakeAccepted,
            push_wake_failed: counters.pushWakeFailed,
            capacity,
          });
        } else if (router.handles(pathname)) {
          const source = sourceKey(request, config, directSourceAddress);
          if (
            !limiter.take("global", config.limits.globalRequestsPerMinute) ||
            !limiter.take(source, config.limits.sourceRequestsPerMinute)
          ) {
            counters.rateLimited += 1;
            response = json({ error: "Companion relay request rate exceeded" }, 429);
          } else {
            response = await router.fetch(request);
          }
        } else {
          response = json({ error: "Not found" }, 404);
        }
        status = response.status;
        return response;
      } catch (error) {
        let response: Response;
        if (error instanceof RelayError) {
          response = json({ error: error.message, error_class: error.errorClass }, error.status);
        } else {
          logError({
            event: "request_failure",
            requestId,
            method,
            route,
            errorType: error instanceof Error ? error.constructor.name : "Unknown",
          });
          response = json({ error: "The companion relay request could not be completed" }, 500);
        }
        status = response.status;
        return response;
      } finally {
        log({
          event: "request",
          requestId,
          method,
          route,
          status,
          durationMs: Math.round((performance.now() - started) * 100) / 100,
        });
      }
    },
  };
}

function semanticRoute(pathname: string): string {
  if (pathname === "/healthz") return "health";
  if (pathname === "/metrics") return "metrics";
  if (pathname === "/v1/companion") return "capabilities";
  if (pathname === "/v1/companion/pairing-sessions") return "pairing-create";
  if (/^\/v1\/companion\/pairing-sessions\/[A-Za-z0-9_-]{32}\/respond$/.test(pathname)) return "pairing-respond";
  if (/^\/v1\/companion\/pairing-sessions\/[A-Za-z0-9_-]{32}$/.test(pathname)) return "pairing-session";
  if (pathname === "/v1/companion/mailboxes") return "mailbox-create";
  if (/^\/v1\/companion\/mailboxes\/[A-Za-z0-9_-]{32}\/envelopes$/.test(pathname)) return "mailbox-envelopes";
  if (/^\/v1\/companion\/mailboxes\/[A-Za-z0-9_-]{32}\/live$/.test(pathname)) return "mailbox-live";
  if (/^\/v1\/companion\/mailboxes\/[A-Za-z0-9_-]{32}\/ack$/.test(pathname)) return "mailbox-ack";
  if (/^\/v1\/companion\/mailboxes\/[A-Za-z0-9_-]{32}$/.test(pathname)) return "mailbox-revoke";
  if (pathname === "/v1/companion/push/register") return "push-register";
  if (/^\/v1\/companion\/push\/register\/[A-Za-z0-9_-]{16,128}$/.test(pathname)) return "push-remove";
  if (pathname.startsWith("/v1/companion/")) return "companion-unmatched";
  return "unmatched";
}

function sourceKey(
  request: Request,
  config: CompanionRelayConfig,
  directSourceAddress?: string,
): string {
  const forwarded = config.trustProxy
    ? request.headers.get("x-forwarded-for")?.split(",", 1)[0]?.trim()
    : directSourceAddress;
  return `source:${(forwarded || "direct-unknown").slice(0, 64)}`;
}

function hasExpectedOrigin(request: Request, config: CompanionRelayConfig): boolean {
  const expected = new URL(config.baseUrl);
  if (!config.trustProxy) {
    const actual = new URL(request.url);
    if (actual.origin === expected.origin) return true;
    return isDevelopmentLocalNetworkAlias(actual, expected, config);
  }
  const forwardedHost = request.headers.get("x-forwarded-host")?.split(",", 1)[0]?.trim();
  const forwardedProtocol = request.headers.get("x-forwarded-proto")?.split(",", 1)[0]?.trim();
  if (!forwardedHost || !forwardedProtocol) return false;
  try {
    return new URL(`${forwardedProtocol}://${forwardedHost}`).origin === expected.origin;
  } catch {
    return false;
  }
}

function isExpectedHealthcheck(
  request: Request,
  config: CompanionRelayConfig,
  pathname: string,
  method: string,
): boolean {
  if (!config.healthcheckHost || pathname !== "/healthz" || method !== "GET") return false;
  try {
    return new URL(request.url).hostname === config.healthcheckHost;
  } catch {
    return false;
  }
}

function isDevelopmentLocalNetworkAlias(
  actual: URL,
  expected: URL,
  config: CompanionRelayConfig,
): boolean {
  if (config.nodeEnv !== "development") return false;
  if (actual.protocol !== "http:" || expected.protocol !== "http:") return false;
  if (!["127.0.0.1", "localhost", "[::1]"].includes(expected.hostname)) return false;
  if (actual.port !== expected.port) return false;
  return /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.local$/i.test(actual.hostname)
    || isPrivateIPv4Address(actual.hostname);
}

function isPrivateIPv4Address(host: string): boolean {
  const octets = host.split(".").map(Number);
  if (octets.length !== 4 || octets.some((value) => !Number.isInteger(value) || value < 0 || value > 255)) {
    return false;
  }
  return octets[0] === 10
    || (octets[0] === 172 && octets[1]! >= 16 && octets[1]! <= 31)
    || (octets[0] === 192 && octets[1] === 168)
    || (octets[0] === 169 && octets[1] === 254);
}

function json(data: unknown, status = 200): Response {
  return secureResponse(new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8" },
  }));
}

if (import.meta.main) {
  try {
    const application = await createCompanionRelayApplication();
    console.info(JSON.stringify({
      event: "startup",
      ...safeCompanionStartupSummary(application.config),
    }));
    Bun.serve({
      hostname: application.config.host,
      port: application.config.port,
      maxRequestBodySize:
        Math.max(
          application.config.limits.envelopeBytes,
          application.config.limits.envelopeBodyBytes,
          application.config.limits.pairingResponseBytes,
        ) + 32 * 1024,
      fetch(request, server) {
        return application.fetch(request, server.requestIP(request)?.address);
      },
    });
  } catch (error) {
    console.error(JSON.stringify({
      event: "startup_failed",
      errorType: error instanceof Error ? error.constructor.name : "Unknown",
    }));
    process.exit(1);
  }
}
