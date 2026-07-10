import { join } from "node:path";
import type { AppConfig } from "./config.ts";
import { loadConfig, PRODUCT, safeStartupSummary } from "./config.ts";
import { buildCapsuleMarkdown, capsuleIsAvailable, resolveCapability } from "./src/capabilities.ts";
import type { TohsenoDatabase } from "./src/database.ts";
import { openMigratedDatabase } from "./src/database.ts";
import { createEmailProvider, deliverQueuedEmails, recoverInterruptedEmailDeliveries } from "./src/email.ts";
import type { EmailProvider } from "./src/email.ts";
import {
  operatorList,
  operatorInspectSource,
  operatorMessage,
  OperatorNotFoundError,
  operatorRevokeCapability,
  operatorSetSummary,
  operatorShow,
  operatorTransition,
  OperatorValidationError,
} from "./src/operator.ts";
import {
  beginCheckout,
  confirmCheckoutPayment,
  createPaymentProvider,
  PaymentConfigurationError,
  processVerifiedPaymentEvent,
} from "./src/payments.ts";
import type { PaymentProvider } from "./src/payments.ts";
import {
  constantTimeEqual,
  FixedWindowRateLimiter,
  HttpError,
  readLimitedUtf8,
  withSecurityHeaders,
} from "./src/security.ts";
import { IllegalTransitionError } from "./src/state-machine.ts";
import { createSubmission, SubmissionValidationError } from "./src/submissions.ts";
import type { SubmissionInput, SubmissionRow } from "./src/submissions.ts";

const PUBLIC_DIRECTORY = join(import.meta.dir, "public");
// URL-encoded fallback forms can expand UTF-8 bytes to roughly three times
// their source size. The inner Markdown limit remains the actual authority.
const MAX_API_BODY_BYTES = PRODUCT.maxMarkdownBytes * 4 + 16 * 1024;
// A 64 KiB operator-authored UTF-8 message can expand substantially when JSON
// escapes control characters. The decoded message is independently capped.
const MAX_OPERATOR_BODY_BYTES = 416 * 1024;
const MAX_WEBHOOK_BODY_BYTES = 1024 * 1024;

export interface ApplicationOptions {
  config?: AppConfig;
  database?: TohsenoDatabase;
  paymentProvider?: PaymentProvider;
  emailProvider?: EmailProvider;
}

export interface TohsenoApplication {
  config: AppConfig;
  database: TohsenoDatabase;
  paymentProvider: PaymentProvider;
  emailProvider: EmailProvider;
  fetch(request: Request): Promise<Response>;
  waitForBackgroundTasks(): Promise<void>;
  close(): Promise<void>;
}

interface RouteResult {
  response: Response;
  submissionId?: string | undefined;
}

function htmlEscape(value: string): string {
  return value.replace(/[&<>"']/g, (character) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    "\"": "&quot;",
    "'": "&#39;",
  })[character] ?? character);
}

function json(data: unknown, status = 200, privateResponse = false): Response {
  return withSecurityHeaders(new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8" },
  }), privateResponse);
}

function html(content: string, status = 200, privateResponse = false): Response {
  return withSecurityHeaders(new Response(content, {
    status,
    headers: { "Content-Type": "text/html; charset=utf-8" },
  }), privateResponse);
}

function documentPage(title: string, body: string): string {
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>${htmlEscape(title)} — TOHSENO</title><link rel="stylesheet" href="/styles.css"></head>
<body><header class="site-header"><a class="wordmark" href="/">TOHSENO</a><span class="header-note">PRIVATE OPERATING PATH</span></header>
<main class="document-main"><article class="document"><header class="document-header"><p class="eyebrow">TOHSENO / PRIVATE</p><h1>${htmlEscape(title)}</h1></header>${body}</article></main>
<footer class="site-footer"><span>ANKY, INC.</span><span>NO-STORE / NO-REFERRER</span></footer></body></html>`;
}

function renderLanding(template: string): string {
  const values: Record<string, string> = {
    ...PRODUCT.copy,
    MAX_MARKDOWN_BYTES: String(PRODUCT.maxMarkdownBytes),
    SELF_HOSTED_PRICE: PRODUCT.prices.selfHosted.display,
    CLIENT_PRICE: PRODUCT.prices.clientOwned.display,
    ANKY_PRICE: PRODUCT.prices.ankyOperated.display,
  };
  const rendered = template.replace(/\{\{([A-Z0-9_]+)\}\}/g, (_match, key: string) => {
    const value = values[key];
    if (value === undefined) throw new Error(`Unknown landing template placeholder: ${key}`);
    return htmlEscape(value);
  });
  if (/\{\{[A-Z0-9_]+\}\}/.test(rendered)) throw new Error("Landing template contains unresolved placeholders");
  return rendered;
}

function requestWantsJson(request: Request): boolean {
  return request.headers.get("accept")?.toLowerCase().includes("application/json") ?? false;
}

function clientKey(request: Request, config: AppConfig): string {
  if (!config.trustProxy) return "direct-client";
  return request.headers.get("x-forwarded-for")?.split(",")[0]?.trim() || request.headers.get("x-real-ip") || "proxied-client";
}

function contentType(request: Request): string {
  return request.headers.get("content-type")?.split(";", 1)[0]?.trim().toLowerCase() ?? "";
}

function objectFromUnknown(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new HttpError(400, "Request body must be a JSON object");
  return value as Record<string, unknown>;
}

async function parseObjectBody(request: Request, maximumBytes: number, allowForm = false): Promise<Record<string, unknown>> {
  const type = contentType(request);
  const raw = await readLimitedUtf8(request, maximumBytes);
  if (type === "application/json") {
    try {
      return objectFromUnknown(JSON.parse(raw));
    } catch (error) {
      if (error instanceof HttpError) throw error;
      throw new HttpError(400, "Request body must be valid JSON");
    }
  }
  if (allowForm && type === "application/x-www-form-urlencoded") {
    return Object.fromEntries(new URLSearchParams(raw));
  }
  throw new HttpError(415, "Content-Type must be application/json" + (allowForm ? " or application/x-www-form-urlencoded" : ""));
}

function stringField(body: Record<string, unknown>, key: string): string {
  const value = body[key];
  if (typeof value !== "string") throw new HttpError(400, `${key} must be a string`);
  return value;
}

function routeLabel(method: string, pathname: string): string {
  if (pathname === "/") return `${method} /`;
  if (pathname === "/privacy") return `${method} /privacy`;
  if (pathname === "/healthz") return `${method} /healthz`;
  if (pathname === "/api/submissions") return `${method} /api/submissions`;
  if (pathname === "/api/checkout") return `${method} /api/checkout`;
  if (pathname === "/api/webhooks/stripe") return `${method} /api/webhooks/stripe`;
  if (pathname === "/api/payments/mock/complete") return `${method} /api/payments/mock/complete`;
  if (pathname === "/checkout/success") return `${method} /checkout/success`;
  if (pathname === "/checkout/cancel") return `${method} /checkout/cancel`;
  if (/^\/mock-checkout\/[^/]+$/.test(pathname)) return `${method} /mock-checkout/:id`;
  if (/^\/status\/[^/]+$/.test(pathname)) return `${method} /status/:token`;
  if (/^\/c\/[^/]+\/MASTER_PROMPT\.md$/.test(pathname)) return `${method} /c/:token/MASTER_PROMPT.md`;
  if (/^\/c\/[^/]+$/.test(pathname)) return `${method} /c/:token`;
  if (/^\/api\/operator\/submissions\/[^/]+\/(transition|summary|message|revoke-capability|inspect-source|retry-email)$/.test(pathname)) {
    return `${method} /api/operator/submissions/:id/${pathname.split("/").at(-1)}`;
  }
  if (/^\/api\/operator\/submissions\/[^/]+$/.test(pathname)) return `${method} /api/operator/submissions/:id`;
  if (pathname === "/api/operator/submissions") return `${method} /api/operator/submissions`;
  if (["/styles.css", "/app.js", "/robots.txt"].includes(pathname)) return `${method} ${pathname}`;
  return `${method} unmatched`;
}

function statusExplanation(submission: SubmissionRow): string {
  if (submission.status === "READY") return "The private capsule and source contract are available. A native app has not already been generated.";
  if (submission.status === "NEEDS_CREDENTIALS") return "Payment is confirmed. Production preparation is waiting for owner-controlled, scoped account access.";
  if (submission.status === "ANKY_REVIEW") return "Anky, Inc. is selectively reviewing this application. No automatic publishing or production capsule has been promised.";
  if (submission.status === "PAYMENT_PENDING") return "Checkout was created. Only a verified provider event can confirm payment; a browser redirect is not proof.";
  if (submission.status === "READY_FOR_PAYMENT") return "Deterministic intake preflight passed. No semantic application compilation has occurred yet.";
  return "This is the current operator state. It does not imply that a native application exists.";
}

function renderStatus(
  submission: SubmissionRow,
  token: string,
  provider: PaymentProvider,
  activeCheckoutUrl?: string,
): string {
  const availability = provider.availability(submission.operating_mode);
  let action = "";
  if (submission.status === "READY_FOR_PAYMENT" && submission.operating_mode !== "anky-operated") {
    const label = submission.operating_mode === "self-hosted"
      ? `Continue to payment — ${PRODUCT.prices.selfHosted.display}`
      : `Continue to payment — ${PRODUCT.prices.clientOwned.display}`;
    action = availability.available
      ? `<form method="post" action="/api/checkout"><input type="hidden" name="token" value="${htmlEscape(token)}"><button class="primary-button" type="submit">${htmlEscape(label)}</button></form>`
      : `<section><h2>Payment unavailable</h2><p>${htmlEscape(availability.reason ?? "Payment configuration is incomplete.")}</p></section>`;
  }
  if (submission.status === "PAYMENT_PENDING") {
    action = availability.available && activeCheckoutUrl
      ? `<p><a class="primary-button" rel="noreferrer" href="${htmlEscape(activeCheckoutUrl)}">RESUME SECURE CHECKOUT</a></p>`
      : `<section><h2>Payment reconciliation paused</h2><p>${htmlEscape(availability.reason ?? "The configured payment provider is unavailable. Do not start or resume payment until the operator restores verified webhook handling.")}</p></section>`;
  }
  if (capsuleIsAvailable(submission)) {
    action += `<p><a class="primary-button" href="/c/${htmlEscape(token)}">OPEN PRIVATE AGENT CAPSULE</a></p>`;
  }
  return documentPage("Application status", `
    <dl><dt>Submission</dt><dd><code>${htmlEscape(submission.id)}</code></dd>
    <dt>Operating mode</dt><dd>${htmlEscape(submission.operating_mode)}</dd>
    <dt>Order state</dt><dd><strong>${htmlEscape(submission.status)}</strong></dd>
    <dt>Content SHA-256</dt><dd><code>${htmlEscape(submission.content_hash)}</code></dd></dl>
    <p>${htmlEscape(statusExplanation(submission))}</p>${action}
    <p><small>This private status capability expires ${htmlEscape(submission.capability_expires_at)}. Contact <a href="mailto:support@anky.app">support@anky.app</a> for revocation or deletion requests.</small></p>`);
}

function activeCheckoutUrl(
  database: TohsenoDatabase,
  submission: SubmissionRow,
  provider: PaymentProvider,
): string | undefined {
  if (!provider.availability(submission.operating_mode).available) return undefined;
  return database.query<{ checkout_url: string }, [string, string]>(`
    SELECT checkout_url FROM payments
    WHERE submission_id = ? AND provider = ? AND status = 'pending' AND checkout_url IS NOT NULL
    ORDER BY attempt DESC LIMIT 1
  `).get(submission.id, provider.name)?.checkout_url;
}

function renderCapsule(capsuleMarkdown: string, token: string): string {
  return documentPage("Private agent capsule", `<p><a href="/c/${htmlEscape(token)}/MASTER_PROMPT.md">Open as raw Markdown</a></p><pre class="capsule-source">${htmlEscape(capsuleMarkdown)}</pre>`);
}

function bearerToken(request: Request): string | null {
  const authorization = request.headers.get("authorization");
  if (!authorization?.startsWith("Bearer ")) return null;
  return authorization.slice(7);
}

export async function createApplication(options: ApplicationOptions = {}): Promise<TohsenoApplication> {
  const config = options.config ?? loadConfig();
  const database = options.database ?? openMigratedDatabase(config.databasePath);
  const ownsDatabase = options.database === undefined;
  const paymentProvider = options.paymentProvider ?? createPaymentProvider(config);
  const emailProvider = options.emailProvider ?? createEmailProvider(config);
  recoverInterruptedEmailDeliveries(database);
  const [landingTemplate, privacyPage] = await Promise.all([
    Bun.file(join(PUBLIC_DIRECTORY, "index.html")).text(),
    Bun.file(join(PUBLIC_DIRECTORY, "privacy.html")).text(),
  ]);
  const landingPage = renderLanding(landingTemplate);
  const backgroundEmailTasks = new Set<Promise<void>>();
  let closing = false;
  const scheduleEmailDrain = (
    submissionId?: string,
    limit = 20,
    includeSuppressed = false,
    includeFailed = false,
  ): void => {
    if (closing) return;
    const task = deliverQueuedEmails(
      database,
      config,
      emailProvider,
      submissionId,
      limit,
      includeSuppressed,
      includeFailed,
    )
        .then(() => undefined)
        .catch((error: unknown) => {
          console.error(JSON.stringify({
            event: "email_drain_failed",
            submissionId: submissionId ?? "backlog",
            provider: emailProvider.name,
            errorType: error instanceof Error ? error.constructor.name : "Unknown",
          }));
        });
    backgroundEmailTasks.add(task);
    void task.finally(() => backgroundEmailTasks.delete(task));
  };
  const waitForBackgroundTasks = async (): Promise<void> => {
    await Bun.sleep(0);
    while (backgroundEmailTasks.size > 0) {
      await Promise.allSettled([...backgroundEmailTasks]);
    }
  };
  scheduleEmailDrain(undefined, 100, false, true);
  const submissionLimiter = new FixedWindowRateLimiter(10, 60_000);
  const operatorFailureLimiter = new FixedWindowRateLimiter(10, 5 * 60_000);

  async function handle(request: Request): Promise<RouteResult> {
    const url = new URL(request.url);
    const { pathname } = url;
    const method = request.method.toUpperCase();

    if (method === "GET" && pathname === "/") return { response: html(landingPage) };
    if (method === "GET" && pathname === "/privacy") return { response: html(privacyPage) };
    if (method === "GET" && pathname === "/healthz") {
      database.query("SELECT 1").get();
      return { response: json({ status: "ok", service: "tohseno" }) };
    }

    const staticFiles: Record<string, { file: string; type: string }> = {
      "/styles.css": { file: "styles.css", type: "text/css; charset=utf-8" },
      "/app.js": { file: "app.js", type: "text/javascript; charset=utf-8" },
      "/robots.txt": { file: "robots.txt", type: "text/plain; charset=utf-8" },
    };
    const staticFile = staticFiles[pathname];
    if (method === "GET" && staticFile) {
      const response = new Response(Bun.file(join(PUBLIC_DIRECTORY, staticFile.file)), {
        headers: { "Content-Type": staticFile.type, "Cache-Control": "public, max-age=3600" },
      });
      return { response: withSecurityHeaders(response) };
    }

    if (method === "POST" && pathname === "/api/submissions") {
      if (!submissionLimiter.allow(clientKey(request, config))) throw new HttpError(429, "Too many submissions; wait before trying again");
      const body = await parseObjectBody(request, MAX_API_BODY_BYTES, true);
      const input: SubmissionInput = {
        markdown: stringField(body, "markdown"),
        email: stringField(body, "email"),
        operatingMode: stringField(body, "operatingMode"),
      };
      const created = await createSubmission(database, config, input);
      const statusUrl = `${config.baseUrl}/status/${created.capabilityToken}`;
      scheduleEmailDrain(created.id);
      if (!requestWantsJson(request)) {
        return { response: withSecurityHeaders(Response.redirect(statusUrl, 303), true), submissionId: created.id };
      }
      return {
        response: json({ submissionId: created.id, status: created.status, statusUrl, expiresAt: created.capabilityExpiresAt }, 201, true),
        submissionId: created.id,
      };
    }

    const statusMatch = /^\/status\/([A-Za-z0-9_-]+)$/.exec(pathname);
    if (method === "GET" && statusMatch?.[1]) {
      const token = statusMatch[1];
      const submission = await resolveCapability(database, token);
      if (!submission) throw new HttpError(404, "Not found");
      return {
        response: html(renderStatus(submission, token, paymentProvider, activeCheckoutUrl(database, submission, paymentProvider)), 200, true),
        submissionId: submission.id,
      };
    }

    if (method === "POST" && pathname === "/api/checkout") {
      const body = await parseObjectBody(request, 16 * 1024, true);
      const token = stringField(body, "token");
      const submission = await resolveCapability(database, token);
      if (!submission) throw new HttpError(404, "Not found");
      if (submission.operating_mode === "anky-operated") throw new HttpError(409, "Anky-operated applications do not use automatic Checkout");
      const checkout = await beginCheckout(database, paymentProvider, submission);
      if (!requestWantsJson(request)) {
        const page = documentPage(
          "Checkout prepared",
          `<p>The payment provider is ready. Continue only if the amount and ownership mode match your request.</p>
          <p><a class="primary-button" rel="noreferrer" href="${htmlEscape(checkout.url)}">CONTINUE TO SECURE CHECKOUT</a></p>`,
        );
        return { response: html(page, 200, true), submissionId: submission.id };
      }
      return { response: json({ checkoutUrl: checkout.url }, 201, true), submissionId: submission.id };
    }

    const mockCheckoutMatch = /^\/mock-checkout\/([A-Za-z0-9_-]+)$/.exec(pathname);
    if (method === "GET" && mockCheckoutMatch?.[1] && paymentProvider.name === "mock" && config.nodeEnv !== "production") {
      const session = mockCheckoutMatch[1];
      const page = documentPage("Mock Checkout", `<p>This development-only page simulates a verified payment event. It is unavailable in production.</p>
        <form method="post" action="/api/payments/mock/complete"><input type="hidden" name="checkoutSessionId" value="${htmlEscape(session)}"><button class="primary-button" type="submit">COMPLETE MOCK PAYMENT</button></form>`);
      return { response: html(page, 200, true) };
    }

    if (method === "POST" && pathname === "/api/payments/mock/complete" && paymentProvider.name === "mock" && config.nodeEnv !== "production") {
      const body = await parseObjectBody(request, 16 * 1024, true);
      const session = stringField(body, "checkoutSessionId");
      const result = confirmCheckoutPayment(database, "mock", `mock-event:${session}`, session);
      if (result.submissionId) scheduleEmailDrain(result.submissionId);
      if (!requestWantsJson(request)) return { response: html(documentPage("Payment recorded", "<p>The mock provider event was processed. Return to the private status URL already in your browser.</p>"), 200, true), submissionId: result.submissionId };
      return { response: json(result, 200, true), submissionId: result.submissionId };
    }

    if (method === "POST" && pathname === "/api/webhooks/stripe") {
      if (contentType(request) !== "application/json") throw new HttpError(415, "Content-Type must be application/json");
      const rawBody = await readLimitedUtf8(request, MAX_WEBHOOK_BODY_BYTES);
      let event;
      try {
        event = await paymentProvider.verifyWebhook(rawBody, request.headers.get("stripe-signature"));
      } catch {
        throw new HttpError(400, "Invalid Stripe webhook signature");
      }
      const result = processVerifiedPaymentEvent(database, event);
      if (result.submissionId) scheduleEmailDrain(result.submissionId);
      return { response: json({ received: true, processed: result.processed }), submissionId: result.submissionId };
    }

    if (method === "GET" && pathname === "/checkout/success") {
      return { response: html(documentPage("Checkout returned", "<p>The browser returned from Checkout. This is not proof of payment. TOHSENO will change the private status only after a verified provider webhook.</p>"), 200, true) };
    }
    if (method === "GET" && pathname === "/checkout/cancel") {
      return { response: html(documentPage("Checkout cancelled", "<p>No payment was confirmed. Return to your private status URL when you are ready.</p>"), 200, true) };
    }

    const rawCapsuleMatch = /^\/c\/([A-Za-z0-9_-]+)\/MASTER_PROMPT\.md$/.exec(pathname);
    if (method === "GET" && rawCapsuleMatch?.[1]) {
      const token = rawCapsuleMatch[1];
      const submission = await resolveCapability(database, token);
      if (!submission || !capsuleIsAvailable(submission)) throw new HttpError(404, "Not found");
      const capsule = await buildCapsuleMarkdown(submission, token, config);
      const response = new Response(capsule, {
        headers: {
          "Content-Type": "text/markdown; charset=utf-8",
          "Content-Disposition": "inline; filename=MASTER_PROMPT.md",
        },
      });
      return { response: withSecurityHeaders(response, true), submissionId: submission.id };
    }

    const capsuleMatch = /^\/c\/([A-Za-z0-9_-]+)$/.exec(pathname);
    if (method === "GET" && capsuleMatch?.[1]) {
      const token = capsuleMatch[1];
      const submission = await resolveCapability(database, token);
      if (!submission) throw new HttpError(404, "Not found");
      if (!capsuleIsAvailable(submission)) {
        if (submission.operating_mode === "anky-operated") {
          return {
            response: html(renderStatus(submission, token, paymentProvider, activeCheckoutUrl(database, submission, paymentProvider)), 200, true),
            submissionId: submission.id,
          };
        }
        throw new HttpError(404, "Not found");
      }
      const capsule = await buildCapsuleMarkdown(submission, token, config);
      return { response: html(renderCapsule(capsule, token), 200, true), submissionId: submission.id };
    }

    if (pathname.startsWith("/api/operator/")) {
      const key = clientKey(request, config);
      const supplied = bearerToken(request);
      if (!supplied || !constantTimeEqual(supplied, config.operatorToken)) {
        if (!operatorFailureLimiter.allow(key)) throw new HttpError(429, "Too many authentication failures");
        throw new HttpError(401, "Unauthorized");
      }
      if (method === "GET" && pathname === "/api/operator/submissions") {
        return { response: json({ submissions: operatorList(database) }, 200, true) };
      }
      const operatorMatch = /^\/api\/operator\/submissions\/(sub_[A-Za-z0-9_-]+)(?:\/(transition|summary|message|revoke-capability|inspect-source|retry-email))?$/.exec(pathname);
      if (operatorMatch?.[1]) {
        const submissionId = operatorMatch[1];
        const action = operatorMatch[2];
        if (method === "GET" && !action) return { response: json(operatorShow(database, submissionId), 200, true), submissionId };
        if (method === "POST" && action === "inspect-source") {
          const type = contentType(request);
          if (type && type !== "application/json") throw new HttpError(415, "Content-Type must be application/json");
          if (request.body) await readLimitedUtf8(request, 1024);
          return { response: json(await operatorInspectSource(database, config, submissionId), 200, true), submissionId };
        }
        if (method === "POST" && action === "transition") {
          const body = await parseObjectBody(request, MAX_OPERATOR_BODY_BYTES);
          if (body.message !== undefined && typeof body.message !== "string") {
            throw new OperatorValidationError("message must be a string when supplied");
          }
          const message = body.message as string | undefined;
          const result = await operatorTransition(database, config, emailProvider, submissionId, stringField(body, "nextStatus"), message);
          return { response: json({ submissionId, ...result }, 200, true), submissionId };
        }
        if (method === "POST" && action === "summary") {
          const body = await parseObjectBody(request, MAX_OPERATOR_BODY_BYTES);
          operatorSetSummary(database, submissionId, body.summary);
          return { response: json({ submissionId, updated: true }, 200, true), submissionId };
        }
        if (method === "POST" && action === "message") {
          const body = await parseObjectBody(request, MAX_OPERATOR_BODY_BYTES);
          const result = await operatorMessage(database, config, emailProvider, submissionId, stringField(body, "message"));
          return { response: json({ submissionId, ...result }, 200, true), submissionId };
        }
        if (method === "POST" && action === "revoke-capability") {
          const type = contentType(request);
          if (type && type !== "application/json") throw new HttpError(415, "Content-Type must be application/json");
          if (request.body) await readLimitedUtf8(request, 1024);
          operatorRevokeCapability(database, submissionId);
          return { response: json({ submissionId, revoked: true }, 200, true), submissionId };
        }
        if (method === "POST" && action === "retry-email") {
          const type = contentType(request);
          if (type && type !== "application/json") throw new HttpError(415, "Content-Type must be application/json");
          if (request.body) await readLimitedUtf8(request, 1024);
          if (!database.query("SELECT 1 FROM submissions WHERE id = ?").get(submissionId)) throw new OperatorNotFoundError();
          const deliveries = await deliverQueuedEmails(database, config, emailProvider, submissionId, 100, true, true);
          return { response: json({ submissionId, deliveries }, 200, true), submissionId };
        }
      }
    }

    throw new HttpError(404, "Not found");
  }

  return {
    config,
    database,
    paymentProvider,
    emailProvider,
    waitForBackgroundTasks,
    async fetch(request: Request): Promise<Response> {
      const requestId = crypto.randomUUID();
      const started = performance.now();
      const route = routeLabel(request.method.toUpperCase(), new URL(request.url).pathname);
      let status = 500;
      let submissionId: string | undefined;
      try {
        const result = await handle(request);
        status = result.response.status;
        submissionId = result.submissionId;
        return result.response;
      } catch (error) {
        let response: Response;
        if (error instanceof SubmissionValidationError) response = json({ error: error.message, field: error.field }, 422, true);
        else if (error instanceof HttpError) response = json({ error: error.message }, error.status, true);
        else if (error instanceof PaymentConfigurationError) response = json({ error: error.message }, 503, true);
        else if (error instanceof IllegalTransitionError) response = json({ error: error.message }, 409, true);
        else if (error instanceof OperatorNotFoundError) response = json({ error: "Not found" }, 404, true);
        else if (error instanceof OperatorValidationError) response = json({ error: error.message }, 422, true);
        else {
          console.error(JSON.stringify({ event: "request_failure", requestId, route, errorType: error instanceof Error ? error.constructor.name : "Unknown" }));
          response = json({ error: "The request could not be completed" }, 500, true);
        }
        status = response.status;
        return response;
      } finally {
        const log: Record<string, string | number> = {
          requestId,
          route,
          status,
          durationMs: Math.round((performance.now() - started) * 100) / 100,
        };
        if (submissionId) log.submissionId = submissionId;
        console.info(JSON.stringify(log));
      }
    },
    async close(): Promise<void> {
      closing = true;
      await waitForBackgroundTasks();
      if (ownsDatabase) database.close();
    },
  };
}

if (import.meta.main) {
  try {
    const application = await createApplication();
    console.info(JSON.stringify({
      event: "startup",
      ...safeStartupSummary(application.config),
      paymentAvailability: {
        selfHosted: application.paymentProvider.availability("self-hosted"),
        clientOwned: application.paymentProvider.availability("client-owned"),
        ankyOperated: application.paymentProvider.availability("anky-operated"),
      },
    }));
    Bun.serve({ port: application.config.port, fetch: application.fetch });
  } catch (error) {
    console.error(JSON.stringify({ event: "startup_failed", error: error instanceof Error ? error.message : "Unknown startup error" }));
    process.exit(1);
  }
}
