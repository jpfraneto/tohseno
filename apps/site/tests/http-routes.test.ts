import { describe, expect, test } from "bun:test";
import { operatorRevokeCapability } from "../src/operator.ts";
import { transitionOrder } from "../src/state-machine.ts";
import type { SubmissionRow } from "../src/submissions.ts";
import {
  beginHttpCheckout,
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
}

function expectPrivateSecurity(response: Response): void {
  expectBaselineSecurity(response);
  expect(response.headers.get("cache-control")).toBe("no-store, private");
  expect(response.headers.get("x-robots-tag")).toBe("noindex, nofollow, noarchive");
}

describe("public HTTP surface", () => {
  test("serves the raw landing page with centralized prices, all modes, and security headers", async () => {
    const harness = await createSiteHarness();
    try {
      const response = await harness.request("/");
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
      expect(body).not.toMatch(/\{\{[A-Z0-9_]+\}\}/);
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
  test("a valid capability resolves while an unrecognized capability is indistinguishable from missing", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      const valid = await harness.request(`/status/${submission.token}`);
      expect(valid.status).toBe(200);
      expectPrivateSecurity(valid);
      expect(await valid.text()).toContain(submission.submissionId);

      const invalid = await harness.request(`/status/${"A".repeat(43)}`, {
        headers: { Accept: "application/json" },
      });
      expect(invalid.status).toBe(404);
      expectPrivateSecurity(invalid);
      expect(await invalid.json()).toEqual({ error: "Not found" });
    } finally {
      await harness.close();
    }
  });

  test("revoked capabilities return 404", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      operatorRevokeCapability(harness.application.database, submission.submissionId);

      const status = await harness.request(`/status/${submission.token}`);
      const capsule = await harness.request(`/c/${submission.token}`);
      expect(status.status).toBe(404);
      expect(capsule.status).toBe(404);
      expectPrivateSecurity(status);
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

      const response = await harness.request(`/status/${submission.token}`);
      expect(response.status).toBe(404);
      expectPrivateSecurity(response);
    } finally {
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
      const before = await harness.request(`/c/${submission.token}/MASTER_PROMPT.md`);
      expect(before.status).toBe(404);

      const checkoutSessionId = await beginHttpCheckout(harness, submission.token);
      const firstCompletion = await completeMockCheckout(harness, checkoutSessionId);
      expect(firstCompletion).toEqual(expect.objectContaining({ processed: true, finalStatus: "READY" }));
      await waitForEmailCount(harness.emailProvider, 3);

      const emailCountAfterFirst = harness.emailProvider.messages.length;
      const duplicateCompletion = await completeMockCheckout(harness, checkoutSessionId);
      expect(duplicateCompletion.processed).toBe(false);
      expect(harness.emailProvider.messages).toHaveLength(emailCountAfterFirst);

      const status = await harness.request(`/status/${submission.token}`);
      const statusBody = await status.text();
      expect(status.status).toBe(200);
      expectPrivateSecurity(status);
      expect(statusBody).toContain("private capsule and source contract are available");
      expect(statusBody).toContain("A native app has not already been generated");

      const capsule = await harness.request(`/c/${submission.token}/MASTER_PROMPT.md`);
      const capsuleBody = await capsule.text();
      expect(capsule.status).toBe(200);
      expectPrivateSecurity(capsule);
      expect(capsule.headers.get("content-type")).toContain("text/markdown");
      expect(capsuleBody).toContain("# Private TOHSENO agent capsule");
      expect(capsuleBody).toContain(markdown);
      expect(capsuleBody).toContain(submission.submissionId);
      expect(capsuleBody).toContain("skills/continuity-app/SKILL.md");
      expect(capsuleBody).toContain("does not mean that a native application has already been generated");
      expect(capsuleBody).toContain("Ask before creating paid resources");
      expect(capsuleBody).toContain("Return all repositories, credentials, production URLs");

      transitionOrder(harness.application.database, submission.submissionId, "FAILED", "operator");
      const capsuleAfterFailure = await harness.request(`/c/${submission.token}/MASTER_PROMPT.md`);
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
      const checkoutSessionId = await beginHttpCheckout(harness, submission.token);
      expect((await completeMockCheckout(harness, checkoutSessionId)).finalStatus).toBe("NEEDS_CREDENTIALS");
      await waitForEmailCount(harness.emailProvider, 3);

      const row = harness.application.database.query<SubmissionRow, [string]>(
        "SELECT * FROM submissions WHERE id = ?",
      ).get(submission.submissionId);
      expect(row?.status).toBe("NEEDS_CREDENTIALS");

      const capsule = await harness.request(`/c/${submission.token}/MASTER_PROMPT.md`);
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
        body: JSON.stringify({ token: submission.token }),
      });
      expect(checkout.status).toBe(409);
      expect(paymentProvider.checkouts).toHaveLength(0);
      const payments = harness.application.database.query<{ count: number }, []>(
        "SELECT count(*) AS count FROM payments",
      ).get();
      expect(payments?.count).toBe(0);

      const humanCapsuleRoute = await harness.request(`/c/${submission.token}`);
      const humanBody = await humanCapsuleRoute.text();
      expect(humanCapsuleRoute.status).toBe(200);
      expect(humanBody).toContain("selectively reviewing this application");
      expect(humanBody).not.toContain("Original MASTER_PROMPT.md");

      const rawCapsule = await harness.request(`/c/${submission.token}/MASTER_PROMPT.md`);
      expect(rawCapsule.status).toBe(404);
      transitionOrder(harness.application.database, submission.submissionId, "ANKY_ACCEPTED", "operator");
      expect((await harness.request(`/c/${submission.token}/MASTER_PROMPT.md`)).status).toBe(200);
      expect(harness.emailProvider.messages.map((message) => message.template)).toEqual([
        "anky-application-received",
      ]);
    } finally {
      await harness.close();
    }
  });
});
