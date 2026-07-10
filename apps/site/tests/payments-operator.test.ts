import { describe, expect, test } from "bun:test";
import { loadConfig } from "../config.ts";
import { buildStripeCheckoutParams } from "../src/payments.ts";
import type { SubmissionRow } from "../src/submissions.ts";
import {
  beginHttpCheckout,
  capabilityAuthorization,
  createSiteHarness,
  submitThroughHttp,
  syntheticMarkdown,
  testConfig,
  waitForEmailCount,
  VerifiedFakePaymentProvider,
} from "./helpers.ts";

describe("payment configuration and metadata boundary", () => {
  test("mock payments are forbidden in production", () => {
    expect(() => loadConfig({
      NODE_ENV: "production",
      PAYMENTS_MODE: "mock",
    })).toThrow("PAYMENTS_MODE=mock is forbidden when NODE_ENV=production");
  });

  test("console email is forbidden in production", () => {
    expect(() => loadConfig({
      NODE_ENV: "production",
      PAYMENTS_MODE: "disabled",
      EMAIL_MODE: "console",
      BASE_URL: "https://tohseno.example",
    })).toThrow("EMAIL_MODE=console is forbidden when NODE_ENV=production");
  });

  test("Stripe Checkout parameters include only a safe submission ID", () => {
    const config = testConfig({
      paymentsMode: "stripe",
      stripeSelfHostedPriceId: "price_self_hosted",
      stripeClientSetupPriceId: "price_client_setup",
      stripeClientMonthlyPriceId: "price_client_monthly",
    });
    const privateMarker = "must-never-enter-payment-metadata";
    const selfHosted = buildStripeCheckoutParams(config, {
      id: "sub_safe_self",
      operating_mode: "self-hosted",
    });
    const clientOwned = buildStripeCheckoutParams(config, {
      id: "sub_safe_client",
      operating_mode: "client-owned",
    });

    expect(selfHosted.mode).toBe("payment");
    expect(selfHosted.metadata).toEqual({ submission_id: "sub_safe_self" });
    expect(selfHosted.payment_intent_data?.metadata).toEqual({ submission_id: "sub_safe_self" });
    expect(selfHosted.line_items).toEqual([{ price: "price_self_hosted", quantity: 1 }]);

    expect(clientOwned.mode).toBe("subscription");
    expect(clientOwned.metadata).toEqual({ submission_id: "sub_safe_client" });
    expect(clientOwned.subscription_data?.metadata).toEqual({ submission_id: "sub_safe_client" });
    expect(clientOwned.line_items).toEqual([
      { price: "price_client_setup", quantity: 1 },
      { price: "price_client_monthly", quantity: 1 },
    ]);
    const serialized = JSON.stringify({ selfHosted, clientOwned });
    expect(serialized).not.toContain(privateMarker);
    expect(serialized).not.toMatch(/markdown|email|capability|contact|token/i);
  });

  test("an actual Stripe provider rejects a bad webhook signature without network access", async () => {
    const harness = await createSiteHarness({
      config: {
        paymentsMode: "stripe",
        stripeSecretKey: "sk_test_not_used_for_network",
        stripeWebhookSecret: "whsec_test_secret",
        stripeSelfHostedPriceId: "price_test_self",
        stripeClientSetupPriceId: "price_test_setup",
        stripeClientMonthlyPriceId: "price_test_monthly",
      },
    });
    try {
      const response = await harness.request("/api/webhooks/stripe", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Stripe-Signature": "t=1700000000,v1=invalid",
        },
        body: JSON.stringify({ id: "evt_synthetic" }),
      });
      expect(response.status).toBe(400);
      expect(await response.json()).toEqual({ error: "Invalid Stripe webhook signature" });
      const eventCount = harness.application.database.query<{ count: number }, []>(
        "SELECT count(*) AS count FROM payment_events",
      ).get();
      expect(eventCount?.count).toBe(0);
    } finally {
      await harness.close();
    }
  });
});

describe("verified payment event processing", () => {
  test("uses the raw verified body and handles a duplicate provider event exactly once", async () => {
    const paymentProvider = new VerifiedFakePaymentProvider();
    const harness = await createSiteHarness({ paymentProvider });
    try {
      const submission = await submitThroughHttp(harness, "self-hosted");
      const checkoutSessionId = await beginHttpCheckout(harness, submission.token, submission.submissionId);
      const rawBody = JSON.stringify({
        eventId: "evt_verified_once",
        checkoutSessionId,
      });
      const webhook = (): Promise<Response> => harness.request("/api/webhooks/stripe", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Stripe-Signature": paymentProvider.expectedSignature,
        },
        body: rawBody,
      });

      const first = await webhook();
      expect(first.status).toBe(200);
      expect(await first.json()).toEqual({ received: true, processed: true });
      await waitForEmailCount(harness.emailProvider, 3);
      expect(paymentProvider.webhookBodies[0]).toBe(rawBody);

      const eventCountAfterFirst = harness.application.database.query<{ count: number }, []>(
        "SELECT count(*) AS count FROM payment_events",
      ).get()?.count;
      const orderEventCountAfterFirst = harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ?",
      ).get(submission.submissionId)?.count;
      const emailsAfterFirst = harness.emailProvider.messages.length;

      const duplicate = await webhook();
      expect(duplicate.status).toBe(200);
      expect(await duplicate.json()).toEqual({ received: true, processed: false });
      expect(harness.application.database.query<{ count: number }, []>(
        "SELECT count(*) AS count FROM payment_events",
      ).get()?.count).toBe(eventCountAfterFirst);
      expect(harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ?",
      ).get(submission.submissionId)?.count).toBe(orderEventCountAfterFirst);
      expect(harness.emailProvider.messages).toHaveLength(emailsAfterFirst);

      const row = harness.application.database.query<SubmissionRow, [string]>(
        "SELECT * FROM submissions WHERE id = ?",
      ).get(submission.submissionId);
      expect(row?.status).toBe("READY");
      const payment = harness.application.database.query<{
        status: string;
        amount: number;
        currency: string;
        provider_reference: string | null;
      }, [string]>(
        "SELECT status, amount, currency, provider_reference FROM payments WHERE submission_id = ?",
      ).get(submission.submissionId);
      expect(payment).toEqual({
        status: "paid",
        amount: 8_800,
        currency: "usd",
        provider_reference: "evt_verified_once",
      });
    } finally {
      await harness.close();
    }
  });
});

describe("authenticated operator boundary", () => {
  test("requires Bearer authentication, audits explicit inspection, messages safely, and revokes capabilities", async () => {
    const harness = await createSiteHarness();
    try {
      const markdown = syntheticMarkdown("operator-inspection-source");
      const email = "operator-inspection@example.test";
      const customerMessage = "This synthetic request was cancelled during the test.";
      const submission = await submitThroughHttp(harness, "self-hosted", { markdown, email });

      const unauthenticated = await harness.request("/api/operator/submissions");
      expect(unauthenticated.status).toBe(401);
      expect(await unauthenticated.json()).toEqual({ error: "Unauthorized" });

      const wrongToken = await harness.request("/api/operator/submissions", {
        headers: { Authorization: "Bearer definitely-not-the-operator-token" },
      });
      expect(wrongToken.status).toBe(401);

      const authorization = { Authorization: `Bearer ${harness.config.operatorToken}` };
      const list = await harness.request("/api/operator/submissions", { headers: authorization });
      const listText = await list.text();
      expect(list.status).toBe(200);
      expect(listText).toContain(submission.submissionId);
      expect(listText).not.toContain(markdown);
      expect(listText).not.toContain(email);
      expect(listText).not.toContain(submission.token);
      expect(listText).not.toContain("encrypted_markdown");

      const show = await harness.request(`/api/operator/submissions/${submission.submissionId}`, {
        headers: authorization,
      });
      const showBody = await show.json() as {
        submission: { id: string };
        masterPrompt?: string;
        contact?: { email: string };
      };
      expect(show.status).toBe(200);
      expect(show.headers.get("cache-control")).toBe("no-store, private");
      expect(showBody.submission.id).toBe(submission.submissionId);
      expect(showBody.masterPrompt).toBeUndefined();
      expect(showBody.contact).toBeUndefined();
      expect(harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ? AND actor_type = 'operator-access'",
      ).get(submission.submissionId)?.count).toBe(0);

      const inspection = await harness.request(
        `/api/operator/submissions/${submission.submissionId}/inspect-source`,
        { method: "POST", headers: authorization },
      );
      const inspectionBody = await inspection.json() as {
        masterPrompt: string;
        contact: { email: string };
      };
      expect(inspection.status).toBe(200);
      expect(inspectionBody.masterPrompt).toBe(markdown);
      expect(inspectionBody.contact.email).toBe(email);

      const accessAudit = harness.application.database.query<{
        actor_type: string;
        metadata_json: string;
      }, [string]>(
        "SELECT actor_type, metadata_json FROM order_events WHERE submission_id = ? AND actor_type = 'operator-access'",
      ).get(submission.submissionId);
      expect(accessAudit).toEqual({
        actor_type: "operator-access",
        metadata_json: '{"action":"private-source-inspection"}',
      });

      const transition = await harness.request(
        `/api/operator/submissions/${submission.submissionId}/transition`,
        {
          method: "POST",
          headers: { ...authorization, "Content-Type": "application/json" },
          body: JSON.stringify({ nextStatus: "CANCELLED", message: customerMessage }),
        },
      );
      expect(transition.status).toBe(200);
      expect(await transition.json()).toEqual(expect.objectContaining({
        submissionId: submission.submissionId,
        status: "CANCELLED",
        messageDelivery: expect.objectContaining({
          recorded: true,
          deliveryAttempted: true,
          providerAccepted: true,
        }),
      }));
      await waitForEmailCount(harness.emailProvider, 2);
      expect(harness.emailProvider.messages.at(-1)?.template).toBe("operator-status");
      expect(harness.emailProvider.messages.at(-1)?.text).toBe(customerMessage);
      const storedMessage = harness.application.database.query<{ encrypted_body: string }, [string]>(
        "SELECT encrypted_body FROM messages WHERE submission_id = ?",
      ).get(submission.submissionId);
      expect(storedMessage?.encrypted_body).not.toContain(customerMessage);

      const revoke = await harness.request(
        `/api/operator/submissions/${submission.submissionId}/revoke-capability`,
        { method: "POST", headers: authorization },
      );
      expect(revoke.status).toBe(200);
      expect(await revoke.json()).toEqual({ submissionId: submission.submissionId, revoked: true });
      expect((await harness.request(`/status/${submission.submissionId}`, {
        headers: capabilityAuthorization(submission.token),
      })).status).toBe(404);

      const persisted = harness.persistedBytes();
      expect(persisted.includes(Buffer.from(customerMessage))).toBe(false);
      expect(persisted.includes(Buffer.from(submission.token))).toBe(false);
    } finally {
      await harness.close();
    }
  });
});
