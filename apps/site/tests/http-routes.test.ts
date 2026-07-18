import { describe, expect, test } from "bun:test";
import { operatorRevokeCapability } from "../src/operator.ts";
import { transitionOrder } from "../src/state-machine.ts";
import type { SubmissionRow } from "../src/submissions.ts";
import {
  beginHttpCheckout,
  capabilityAuthorization,
  capabilityCookieHeader,
  completeMockCheckout,
  createSiteHarness,
  submitThroughHttp,
  syntheticMarkdown,
  waitForEmailCount,
  VerifiedFakePaymentProvider,
} from "./helpers.ts";

function expectBaselineSecurity(response: Response): void {
  expect(response.headers.get("content-security-policy")).toContain("frame-ancestors 'none'");
  expect(response.headers.get("content-security-policy")).toContain("script-src 'self'");
  expect(response.headers.get("x-content-type-options")).toBe("nosniff");
  expect(response.headers.get("referrer-policy")).toBe("no-referrer");
  expect(response.headers.get("permissions-policy")).toContain("camera=()");
  expect(response.headers.get("cross-origin-opener-policy")).toBe("same-origin");
  expect(response.headers.get("strict-transport-security")).toBe("max-age=31536000");
}

function expectPrivateSecurity(response: Response): void {
  expectBaselineSecurity(response);
  expect(response.headers.get("cache-control")).toBe("no-store, private");
  expect(response.headers.get("x-robots-tag")).toBe("noindex, nofollow, noarchive");
}

describe("public HTTP surface", () => {
  test("serves the one-command hero landing page without any intake form", async () => {
    const harness = await createSiteHarness();
    try {
      const response = await harness.request("/");
      const body = await response.text();

      expect(response.status).toBe(200);
      expect(response.headers.get("content-type")).toContain("text/html");
      expectBaselineSecurity(response);
      expect(body).toContain("curl -fsSL https://tohseno.com/oneshot.sh | bash");
      expect(body).toContain("data-copy-command");
      expect(body).toContain("https://github.com/jpfraneto/tohseno");
      expect(body).toContain('href="/intake"');
      expect(body).toContain('href="/privacy"');
      expect(body).not.toContain('action="/api/submissions"');
      expect(body).not.toContain("<form");
      expect(body).not.toMatch(/\{\{[A-Z0-9_]+\}\}/);
    } finally {
      await harness.close();
    }
  });

  test("serves the intake page with centralized prices, all modes, and security headers", async () => {
    const harness = await createSiteHarness();
    try {
      const response = await harness.request("/intake");
      const body = await response.text();

      expect(response.status).toBe(200);
      expect(response.headers.get("content-type")).toContain("text/html");
      expectBaselineSecurity(response);
      expect(body).toContain("Describe the action.");
      expect(body).toContain("Receive the app.");
      expect(body).toContain("CREATE CONTINUITY APP");
      expect(body).toContain("$88 once");
      expect(body).toContain("$888 setup + $88/month");
      expect(body).toContain('value="self-hosted"');
      expect(body).toContain('value="client-owned"');
      expect(body).toContain('value="anky-operated"');
      expect(body).toContain('action="/api/submissions"');
      expect(body).toContain("does not yet generate a complete native application");
      expect(body).toContain("Checkout is in test mode; no real payment will be taken");
      expect(body).not.toMatch(/\{\{[A-Z0-9_]+\}\}/);

      const browserScript = await harness.request("/app.js");
      const scriptBody = await browserScript.text();
      expect(browserScript.status).toBe(200);
      expect(scriptBody).toContain('hasAttribute("data-capability-bootstrap")');
      expect(scriptBody).toContain("data-private-content");
      expect(scriptBody).toContain("content.hidden = true");
      expect(scriptBody).toContain("window.location.reload()");
      expect(scriptBody).not.toContain("history.replaceState");
    } finally {
      await harness.close();
    }
  });

  test("serves the oneshot bootstrap as a pinned, revalidated, syntactically valid shell script", async () => {
    const harness = await createSiteHarness();
    try {
      const response = await harness.request("/oneshot.sh");
      const body = await response.text();

      expect(response.status).toBe(200);
      expect(response.headers.get("content-type")).toContain("text/x-shellscript");
      expect(response.headers.get("cache-control")).toBe("public, max-age=0, must-revalidate");

      // Trust and partial-download boundaries: an exact pinned commit, all
      // work inside main() invoked on the final line, and no secret intake.
      expect(body).toMatch(/^TOHSENO_PIN="[0-9a-f]{40}"$/m);
      expect(body.trimEnd().endsWith('main "$@"')).toBe(true);
      expect(body).toContain("set -euo pipefail");
      expect(body).toContain("refusing to overwrite");
      expect(body).toContain("secrets and capabilities are never accepted as arguments");
      expect(body.toLowerCase()).not.toContain("password");
      expect(body).not.toContain("sk_");

      const syntax = Bun.spawnSync(["bash", "-n", new URL("../public/oneshot.sh", import.meta.url).pathname]);
      expect(syntax.exitCode).toBe(0);

      const head = await harness.request("/oneshot.sh", { method: "HEAD" });
      expect(head.status).toBe(200);
      expect(await head.text()).toBe("");
    } finally {
      await harness.close();
    }
  });

  test("discloses disabled Checkout before a visitor submits private Markdown", async () => {
    const harness = await createSiteHarness({ config: { paymentsMode: "disabled" } });
    try {
      const response = await harness.request("/intake");
      const body = await response.text();
      expect(response.status).toBe(200);
      expect(body).toContain("Private intake is open");
      expect(body).toContain("Checkout is temporarily unavailable");
      expect(body).toContain("no payment will be taken");
    } finally {
      await harness.close();
    }
  });

  test("supports HEAD, returns 405 with Allow for known routes, and redirects www to the canonical host", async () => {
    const harness = await createSiteHarness({
      config: { trustProxy: true, nodeEnv: "production", paymentsMode: "disabled", emailMode: "disabled" },
    });
    try {
      const head = await harness.request("/", { method: "HEAD" });
      expect(head.status).toBe(200);
      expect(await head.text()).toBe("");
      expectBaselineSecurity(head);

      const wrongMethod = await harness.request("/api/submissions");
      expect(wrongMethod.status).toBe(405);
      expect(wrongMethod.headers.get("allow")).toBe("POST");

      const alias = await harness.request("/privacy?from=www", {
        headers: { "X-Forwarded-Host": "www.tohseno.test" },
        redirect: "manual",
      });
      expect(alias.status).toBe(308);
      expect(alias.headers.get("location")).toBe("https://tohseno.test/privacy?from=www");

      const insecure = await harness.request("/privacy?from=http", {
        headers: {
          "X-Forwarded-Host": "tohseno.test",
          "X-Forwarded-Proto": "http",
        },
        redirect: "manual",
      });
      expect(insecure.status).toBe(308);
      expect(insecure.headers.get("location")).toBe("https://tohseno.test/privacy?from=http");

      const aliasMutation = await harness.request("/api/submissions", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Forwarded-Host": "www.tohseno.test",
        },
        body: "{}",
      });
      expect(aliasMutation.status).toBe(405);
      expect(aliasMutation.headers.get("allow")).toBe("GET, HEAD");
    } finally {
      await harness.close();
    }
  });

  test("trusted-proxy rate limiting uses Railway X-Real-IP and ignores spoofed X-Forwarded-For", async () => {
    const harness = await createSiteHarness({ config: { trustProxy: true } });
    try {
      for (let index = 0; index < 10; index += 1) {
        const response = await harness.request("/api/submissions", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-Real-IP": "203.0.113.10",
            "X-Forwarded-For": `198.51.100.${index + 1}`,
          },
          body: JSON.stringify({ markdown: "too short", email: "owner@example.test", operatingMode: "self-hosted" }),
        });
        expect(response.status).toBe(422);
      }
      const limited = await harness.request("/api/submissions", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Real-IP": "203.0.113.10",
          "X-Forwarded-For": "192.0.2.250",
        },
        body: JSON.stringify({ markdown: "too short", email: "owner@example.test", operatingMode: "self-hosted" }),
      });
      expect(limited.status).toBe(429);
    } finally {
      await harness.close();
    }
  });

  test("serves the plain-language privacy notice", async () => {
    const harness = await createSiteHarness();
    try {
      const response = await harness.request("/privacy");
      const body = await response.text();

      expect(response.status).toBe(200);
      expectBaselineSecurity(response);
      expect(body).toContain("Submitted Markdown and contact details are encrypted at rest");
      expect(body).toContain("bearer capabilities");
      expect(body).toContain("Anky, Inc.");
      expect(body).toContain("support@anky.app");
      expect(body).toContain("not sent\n            in payment metadata");
    } finally {
      await harness.close();
    }
  });

  test("health checks the database and returns a small JSON response", async () => {
    const harness = await createSiteHarness();
    try {
      const response = await harness.request("/healthz");
      expect(response.status).toBe(200);
      expectBaselineSecurity(response);
      expect(await response.json()).toEqual({ status: "ok", service: "tohseno" });
    } finally {
      await harness.close();
    }
  });
});

describe("private status capabilities", () => {
  test("submission responses set the browser cookie and keep fallback redirects token-free", async () => {
    const harness = await createSiteHarness();
    try {
      const response = await harness.request("/api/submissions", {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams({
          markdown: syntheticMarkdown("fallback transport"),
          email: "fallback@example.test",
          operatingMode: "self-hosted",
        }).toString(),
      });
      expect(response.status).toBe(303);
      const location = new URL(response.headers.get("location") ?? "");
      const token = new URLSearchParams(location.hash.slice(1)).get("capability");
      const submissionId = location.pathname.split("/").at(-1);
      expect(token).toMatch(/^[A-Za-z0-9_-]{43}$/);
      expect(submissionId).toMatch(/^sub_[A-Za-z0-9_-]+$/);
      expect(location.pathname).toBe(`/status/${submissionId}`);
      expect(location.search).toBe("");
      expect(location.pathname).not.toContain(token ?? "missing");
      expect(response.headers.get("set-cookie")).toContain(`__Host-tohseno-capability-${submissionId}=${token}`);
    } finally {
      await harness.close();
    }
  });

  test("plain-HTTP development uses a host-only dev cookie without pretending it is Secure", async () => {
    const harness = await createSiteHarness({
      config: { nodeEnv: "development", baseUrl: "http://localhost:3000" },
    });
    try {
      const response = await harness.request("/api/submissions", {
        method: "POST",
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        body: JSON.stringify({
          markdown: syntheticMarkdown("development cookie"),
          email: "development-cookie@example.test",
          operatingMode: "self-hosted",
        }),
      });
      const setCookie = response.headers.get("set-cookie") ?? "";
      expect(setCookie).toContain("tohseno-capability-sub_");
      expect(setCookie).not.toContain("__Host-");
      expect(setCookie).not.toContain("; Secure");
      expect(setCookie).toContain("HttpOnly");
      expect(setCookie).toContain("SameSite=Strict");
    } finally {
      await harness.close();
    }
  });

  test("handoff keeps the bearer in the fragment and bootstrap establishes a strict HttpOnly cookie", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      const handoff = new URL(submission.statusUrl);
      expect(handoff.pathname).toBe(`/status/${submission.submissionId}`);
      expect(handoff.search).toBe("");
      expect(new URLSearchParams(handoff.hash.slice(1)).get("capability")).toBe(submission.token);
      expect(handoff.pathname).not.toContain(submission.token);

      const bootstrap = await harness.request("/api/capability/session", {
        method: "POST",
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        body: JSON.stringify({ submissionId: submission.submissionId, token: submission.token }),
      });
      expect(bootstrap.status).toBe(200);
      expectPrivateSecurity(bootstrap);
      expect(await bootstrap.json()).toEqual({ authenticated: true, changed: true });
      const setCookie = bootstrap.headers.get("set-cookie") ?? "";
      expect(setCookie).toContain(`__Host-tohseno-capability-${submission.submissionId}=${submission.token}`);
      expect(setCookie).toContain("Path=/");
      expect(setCookie).toContain("HttpOnly");
      expect(setCookie).toContain("SameSite=Strict");
      expect(setCookie).toContain("Secure");

      const unchanged = await harness.request("/api/capability/session", {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
          ...capabilityCookieHeader(submission.token, harness.config, submission.submissionId),
        },
        body: JSON.stringify({ submissionId: submission.submissionId, token: submission.token }),
      });
      expect(await unchanged.json()).toEqual({ authenticated: true, changed: false });

      const valid = await harness.request(`/status/${submission.submissionId}`, {
        headers: capabilityCookieHeader(submission.token, harness.config, submission.submissionId),
      });
      expect(valid.status).toBe(200);
      expectPrivateSecurity(valid);
      const validBody = await valid.text();
      expect(validBody).toContain(submission.submissionId);
      expect(validBody).toContain("data-private-content");
      expect(validBody).toContain("data-private-progress");
      expect(validBody).toContain("data-private-error");

      const invalid = await harness.request(`/status/${submission.submissionId}`, {
        headers: { Accept: "application/json", ...capabilityAuthorization("A".repeat(43)) },
      });
      expect(invalid.status).toBe(404);
      expectPrivateSecurity(invalid);
      expect(await invalid.json()).toEqual({ error: "Not found" });

      expect((await harness.request(`/status/${submission.token}`)).status).toBe(404);
      expect((await harness.request(`/c/${submission.token}`)).status).toBe(404);
      expect((await harness.request(`/c/${submission.token}/MASTER_PROMPT.md`)).status).toBe(404);

      const oversized = await harness.request("/api/capability/session", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ submissionId: submission.submissionId, token: "A".repeat(2_000) }),
      });
      expect(oversized.status).toBe(413);
      expect(await oversized.text()).not.toContain(submission.token);

      const formBootstrap = await harness.request("/api/capability/session", {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams({ submissionId: submission.submissionId, token: submission.token }).toString(),
      });
      expect(formBootstrap.status).toBe(415);
    } finally {
      await harness.close();
    }
  });

  test("revoked capabilities return 404", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      operatorRevokeCapability(harness.application.database, submission.submissionId);

      const status = await harness.request(`/status/${submission.submissionId}`, {
        headers: capabilityAuthorization(submission.token),
      });
      const capsule = await harness.request(`/c/${submission.submissionId}`, {
        headers: capabilityAuthorization(submission.token),
      });
      expect(status.status).toBe(404);
      expect(capsule.status).toBe(404);
      expectPrivateSecurity(status);
      const bootstrap = await harness.request("/api/capability/session", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ submissionId: submission.submissionId, token: submission.token }),
      });
      expect(bootstrap.status).toBe(404);
      expectPrivateSecurity(bootstrap);
    } finally {
      await harness.close();
    }
  });

  test("expired capabilities return 404", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      harness.application.database.query(
        "UPDATE submissions SET capability_expires_at = ? WHERE id = ?",
      ).run(new Date(0).toISOString(), submission.submissionId);

      const response = await harness.request(`/status/${submission.submissionId}`, {
        headers: capabilityAuthorization(submission.token),
      });
      expect(response.status).toBe(404);
      expectPrivateSecurity(response);
      const bootstrap = await harness.request("/api/capability/session", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ submissionId: submission.submissionId, token: submission.token }),
      });
      expect(bootstrap.status).toBe(404);
    } finally {
      await harness.close();
    }
  });

  test("Checkout accepts capability body, bearer header, or strict cookie without a capability URL", async () => {
    const harness = await createSiteHarness();
    try {
      const bodySubmission = await submitThroughHttp(harness, "self-hosted", {
        email: "body-checkout@example.test",
      });
      expect((await harness.request("/api/checkout", {
        method: "POST",
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        body: JSON.stringify({ submissionId: bodySubmission.submissionId, token: bodySubmission.token }),
      })).status).toBe(201);

      const headerSubmission = await submitThroughHttp(harness, "self-hosted", {
        email: "header-checkout@example.test",
      });
      expect((await harness.request("/api/checkout", {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
          ...capabilityAuthorization(headerSubmission.token),
        },
        body: JSON.stringify({ submissionId: headerSubmission.submissionId }),
      })).status).toBe(201);

      const cookieSubmission = await submitThroughHttp(harness, "self-hosted", {
        email: "cookie-checkout@example.test",
      });
      expect((await harness.request("/api/checkout", {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
          ...capabilityCookieHeader(cookieSubmission.token, harness.config, cookieSubmission.submissionId),
        },
        body: JSON.stringify({ submissionId: cookieSubmission.submissionId }),
      })).status).toBe(201);
    } finally {
      await harness.close();
    }
  });

  test("per-submission cookies isolate simultaneous handoffs and bind private reads and Checkout", async () => {
    const paymentProvider = new VerifiedFakePaymentProvider();
    const harness = await createSiteHarness({ paymentProvider });
    try {
      const first = await submitThroughHttp(harness, "self-hosted", {
        markdown: syntheticMarkdown("isolated first source"),
        email: "isolated-first@example.test",
      });
      const second = await submitThroughHttp(harness, "self-hosted", {
        markdown: syntheticMarkdown("isolated second source"),
        email: "isolated-second@example.test",
      });
      const firstCookie = capabilityCookieHeader(first.token, harness.config, first.submissionId).Cookie;
      const secondCookie = capabilityCookieHeader(second.token, harness.config, second.submissionId).Cookie;
      if (!firstCookie || !secondCookie) throw new Error("Capability cookie helper returned no cookie");

      const wrongCookieStatus = await harness.request(`/status/${second.submissionId}`, {
        headers: { Cookie: firstCookie },
      });
      const wrongCookieBody = await wrongCookieStatus.text();
      expect(wrongCookieStatus.status).toBe(404);
      expect(wrongCookieBody).toContain("data-capability-bootstrap");
      expect(wrongCookieBody).not.toContain(first.submissionId);
      expect(wrongCookieBody).not.toContain("Order state");

      const deliberatelyMisnamed = await harness.request(`/status/${second.submissionId}`, {
        headers: capabilityCookieHeader(first.token, harness.config, second.submissionId),
      });
      expect(deliberatelyMisnamed.status).toBe(404);
      expect(await deliberatelyMisnamed.text()).toContain("data-capability-bootstrap");

      const wrongCookieCapsule = await harness.request(`/c/${second.submissionId}`, {
        headers: capabilityCookieHeader(first.token, harness.config, second.submissionId),
      });
      expect(wrongCookieCapsule.status).toBe(404);
      expect((await wrongCookieCapsule.text())).not.toContain("Original MASTER_PROMPT.md");
      const wrongCookieRaw = await harness.request(`/c/${second.submissionId}/MASTER_PROMPT.md`, {
        headers: capabilityCookieHeader(first.token, harness.config, second.submissionId),
      });
      expect(wrongCookieRaw.status).toBe(404);

      const mismatch = await harness.request("/api/capability/session", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ submissionId: first.submissionId, token: second.token }),
      });
      expect(mismatch.status).toBe(404);
      expect(mismatch.headers.get("set-cookie")).toBeNull();

      const switchSecond = await harness.request("/api/capability/session", {
        method: "POST",
        headers: { "Content-Type": "application/json", Cookie: firstCookie },
        body: JSON.stringify({ submissionId: second.submissionId, token: second.token }),
      });
      expect(await switchSecond.json()).toEqual({ authenticated: true, changed: true });
      expect(switchSecond.headers.get("set-cookie")).toContain(
        `__Host-tohseno-capability-${second.submissionId}=${second.token}`,
      );

      const bothCookies = `${firstCookie}; ${secondCookie}`;
      const firstStatus = await harness.request(`/status/${first.submissionId}`, {
        headers: { Cookie: bothCookies },
      });
      const secondStatus = await harness.request(`/status/${second.submissionId}`, {
        headers: { Cookie: bothCookies },
      });
      const firstBody = await firstStatus.text();
      const secondBody = await secondStatus.text();
      expect(firstStatus.status).toBe(200);
      expect(secondStatus.status).toBe(200);
      expect(firstBody).toContain(first.submissionId);
      expect(firstBody).not.toContain(second.submissionId);
      expect(secondBody).toContain(second.submissionId);
      expect(secondBody).not.toContain(first.submissionId);

      const wrongCheckout = await harness.request("/api/checkout", {
        method: "POST",
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        body: JSON.stringify({ submissionId: first.submissionId, token: second.token }),
      });
      expect(wrongCheckout.status).toBe(404);
      expect(paymentProvider.checkouts).toHaveLength(0);
      expect(harness.application.database.query<{ status: string }, [string]>(
        "SELECT status FROM submissions WHERE id = ?",
      ).get(first.submissionId)?.status).toBe("READY_FOR_PAYMENT");
      expect(harness.application.database.query<{ status: string }, [string]>(
        "SELECT status FROM submissions WHERE id = ?",
      ).get(second.submissionId)?.status).toBe("READY_FOR_PAYMENT");
    } finally {
      await harness.close();
    }
  });

  test("an invalid or revoked old cookie can be replaced only by a valid scoped fragment", async () => {
    const harness = await createSiteHarness();
    try {
      const revoked = await submitThroughHttp(harness, "self-hosted", {
        email: "revoked-old-cookie@example.test",
      });
      const replacement = await submitThroughHttp(harness, "self-hosted", {
        email: "replacement-cookie@example.test",
      });
      operatorRevokeCapability(harness.application.database, revoked.submissionId);
      const revokedCookie = capabilityCookieHeader(revoked.token, harness.config, revoked.submissionId).Cookie;
      if (!revokedCookie) throw new Error("Capability cookie helper returned no cookie");

      const revokedStatus = await harness.request(`/status/${revoked.submissionId}`, {
        headers: { Cookie: revokedCookie },
      });
      expect(revokedStatus.status).toBe(404);
      expect(await revokedStatus.text()).toContain("data-capability-bootstrap");

      const invalidTargetCookie = capabilityCookieHeader(
        "A".repeat(43),
        harness.config,
        replacement.submissionId,
      ).Cookie;
      if (!invalidTargetCookie) throw new Error("Capability cookie helper returned no cookie");
      const replacementExchange = await harness.request("/api/capability/session", {
        method: "POST",
        headers: { "Content-Type": "application/json", Cookie: `${revokedCookie}; ${invalidTargetCookie}` },
        body: JSON.stringify({ submissionId: replacement.submissionId, token: replacement.token }),
      });
      expect(await replacementExchange.json()).toEqual({ authenticated: true, changed: true });
      const replacementStatus = await harness.request(`/status/${replacement.submissionId}`, {
        headers: capabilityCookieHeader(replacement.token, harness.config, replacement.submissionId),
      });
      expect(replacementStatus.status).toBe(200);
      const replacementBody = await replacementStatus.text();
      expect(replacementBody).toContain(replacement.submissionId);
      expect(replacementBody).not.toContain(revoked.submissionId);
    } finally {
      await harness.close();
    }
  });

  test("conflicting cookie and Authorization capabilities fail closed", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      const conflictingToken = "A".repeat(43);
      const conflictingHeaders = {
        Accept: "application/json",
        ...capabilityCookieHeader(submission.token, harness.config, submission.submissionId),
        ...capabilityAuthorization(conflictingToken),
      };
      const status = await harness.request(`/status/${submission.submissionId}`, { headers: conflictingHeaders });
      expect(status.status).toBe(404);
      expect(await status.json()).toEqual({ error: "Not found" });

      const checkout = await harness.request("/api/checkout", {
        method: "POST",
        headers: { ...conflictingHeaders, "Content-Type": "application/json" },
        body: JSON.stringify({ submissionId: submission.submissionId, token: submission.token }),
      });
      expect(checkout.status).toBe(404);
      expect(await checkout.json()).toEqual({ error: "Not found" });

      const matching = await harness.request(`/status/${submission.submissionId}`, {
        headers: {
          ...capabilityCookieHeader(submission.token, harness.config, submission.submissionId),
          ...capabilityAuthorization(submission.token),
        },
      });
      expect(matching.status).toBe(200);
    } finally {
      await harness.close();
    }
  });

  test("route-template logs never contain a capability, including rejected legacy paths", async () => {
    const harness = await createSiteHarness();
    const messages: string[] = [];
    const original = console.info;
    try {
      const submission = await submitThroughHttp(harness);
      console.info = (...values: unknown[]) => {
        messages.push(values.map(String).join(" "));
      };
      await harness.request(`/status/${submission.submissionId}`, { headers: capabilityAuthorization(submission.token) });
      await harness.request(`/status/${submission.token}`);
      expect(messages.join("\n")).not.toContain(submission.token);
      expect(messages.join("\n")).toContain("GET /status/:id");
      expect(messages.join("\n")).toContain("GET unmatched");
    } finally {
      console.info = original;
      await harness.close();
    }
  });
});

describe("capsule availability and mode ownership", () => {
  test("self-hosted capsules are gated until a verified mock payment and remain honest about READY", async () => {
    const harness = await createSiteHarness();
    try {
      const markdown = syntheticMarkdown("private self-hosted source");
      const submission = await submitThroughHttp(harness, "self-hosted", { markdown });
      const before = await harness.request(`/c/${submission.submissionId}/MASTER_PROMPT.md`, {
        headers: capabilityAuthorization(submission.token),
      });
      expect(before.status).toBe(404);

      const checkoutSessionId = await beginHttpCheckout(harness, submission.token, submission.submissionId);
      const firstCompletion = await completeMockCheckout(harness, checkoutSessionId);
      expect(firstCompletion).toEqual(expect.objectContaining({ processed: true, finalStatus: "READY" }));
      await waitForEmailCount(harness.emailProvider, 3);

      const emailCountAfterFirst = harness.emailProvider.messages.length;
      const duplicateCompletion = await completeMockCheckout(harness, checkoutSessionId);
      expect(duplicateCompletion.processed).toBe(false);
      expect(harness.emailProvider.messages).toHaveLength(emailCountAfterFirst);

      const status = await harness.request(`/status/${submission.submissionId}`, {
        headers: capabilityAuthorization(submission.token),
      });
      const statusBody = await status.text();
      expect(status.status).toBe(200);
      expectPrivateSecurity(status);
      expect(statusBody).toContain("private capsule and source contract are available");
      expect(statusBody).toContain("A native app has not already been generated");
      expect(statusBody).not.toContain(submission.token);
      expect(statusBody).toContain(`href="/c/${submission.submissionId}"`);

      const capsule = await harness.request(`/c/${submission.submissionId}/MASTER_PROMPT.md`, {
        headers: capabilityAuthorization(submission.token),
      });
      const capsuleBody = await capsule.text();
      expect(capsule.status).toBe(200);
      expectPrivateSecurity(capsule);
      expect(capsule.headers.get("content-type")).toContain("text/markdown");
      expect(capsuleBody).toContain("# Private TOHSENO agent capsule");
      expect(capsuleBody).toContain(markdown);
      expect(capsuleBody).toContain(submission.submissionId);
      expect(capsuleBody).toContain("skills/continuity-app/SKILL.md");
      expect(capsuleBody).toContain("does not mean that a native application has already been generated");
      expect(capsuleBody).toContain(`/c/${submission.submissionId}#capability=${submission.token}`);
      expect(capsuleBody).toContain(`/c/${submission.submissionId}/MASTER_PROMPT.md`);
      expect(capsuleBody).toContain("Authorization: Bearer <capability-token>");
      expect(capsuleBody).not.toContain(`/c/${submission.token}`);
      expect(capsuleBody).toContain("Ask before creating paid resources");
      expect(capsuleBody).toContain("Return all repositories, credentials, production URLs");

      const capsuleByCookie = await harness.request(`/c/${submission.submissionId}/MASTER_PROMPT.md`, {
        headers: capabilityCookieHeader(submission.token, harness.config, submission.submissionId),
      });
      expect(capsuleByCookie.status).toBe(200);
      expect(await capsuleByCookie.text()).toContain(markdown);

      transitionOrder(harness.application.database, submission.submissionId, "FAILED", "operator");
      const capsuleAfterFailure = await harness.request(`/c/${submission.submissionId}/MASTER_PROMPT.md`, {
        headers: capabilityAuthorization(submission.token),
      });
      expect(capsuleAfterFailure.status).toBe(200);
      expect(await capsuleAfterFailure.text()).toContain(markdown);

      expect(harness.emailProvider.messages.map((message) => message.template)).toEqual([
        "submission-received",
        "payment-confirmed",
        "self-hosted-ready",
      ]);
    } finally {
      await harness.close();
    }
  });

  test("client-owned payment stops at credentials and its runbook preserves customer ownership", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness, "client-owned");
      const checkoutSessionId = await beginHttpCheckout(harness, submission.token, submission.submissionId);
      expect((await completeMockCheckout(harness, checkoutSessionId)).finalStatus).toBe("NEEDS_CREDENTIALS");
      await waitForEmailCount(harness.emailProvider, 3);

      const row = harness.application.database.query<SubmissionRow, [string]>(
        "SELECT * FROM submissions WHERE id = ?",
      ).get(submission.submissionId);
      expect(row?.status).toBe("NEEDS_CREDENTIALS");

      const capsule = await harness.request(`/c/${submission.submissionId}/MASTER_PROMPT.md`, {
        headers: capabilityAuthorization(submission.token),
      });
      const body = await capsule.text();
      expect(capsule.status).toBe(200);
      expect(body).toContain("Verify Apple Developer organization readiness");
      expect(body).toContain("Verify Google Play Console readiness");
      expect(body).toContain("scoped access rather than sharing passwords");
      expect(body).toContain("domain and DNS ownership in the customer's account");
      expect(body).toContain("infrastructure account in the customer's name");
      expect(body).toContain("Preserve customer ownership of bundle IDs, package IDs, source, domains");
      expect(body).toContain("Stop for human approval before spending money or submitting to stores");

      const credentialsEmail = harness.emailProvider.messages.at(-1);
      expect(credentialsEmail?.template).toBe("client-credentials-required");
      expect(credentialsEmail?.text).toContain("within eight hours after all required account access");
      expect(credentialsEmail?.text).toContain("platform review");
      expect(credentialsEmail?.text).toContain("cannot be guaranteed within that window");
    } finally {
      await harness.close();
    }
  });

  test("Anky-operated intake creates no Checkout and exposes only review status before acceptance", async () => {
    const paymentProvider = new VerifiedFakePaymentProvider();
    const harness = await createSiteHarness({ paymentProvider });
    try {
      const submission = await submitThroughHttp(harness, "anky-operated");
      await waitForEmailCount(harness.emailProvider, 1);
      expect(submission.status).toBe("ANKY_REVIEW");

      const checkout = await harness.request("/api/checkout", {
        method: "POST",
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        body: JSON.stringify({ submissionId: submission.submissionId, token: submission.token }),
      });
      expect(checkout.status).toBe(409);
      expect(paymentProvider.checkouts).toHaveLength(0);
      const payments = harness.application.database.query<{ count: number }, []>(
        "SELECT count(*) AS count FROM payments",
      ).get();
      expect(payments?.count).toBe(0);

      const humanCapsuleRoute = await harness.request(`/c/${submission.submissionId}`, {
        headers: capabilityAuthorization(submission.token),
      });
      const humanBody = await humanCapsuleRoute.text();
      expect(humanCapsuleRoute.status).toBe(200);
      expect(humanBody).toContain("selectively reviewing this application");
      expect(humanBody).not.toContain("Original MASTER_PROMPT.md");

      const rawCapsule = await harness.request(`/c/${submission.submissionId}/MASTER_PROMPT.md`, {
        headers: capabilityAuthorization(submission.token),
      });
      expect(rawCapsule.status).toBe(404);
      transitionOrder(harness.application.database, submission.submissionId, "ANKY_ACCEPTED", "operator");
      expect((await harness.request(`/c/${submission.submissionId}/MASTER_PROMPT.md`, {
        headers: capabilityAuthorization(submission.token),
      })).status).toBe(200);
      expect(harness.emailProvider.messages.map((message) => message.template)).toEqual([
        "anky-application-received",
      ]);
    } finally {
      await harness.close();
    }
  });
});
