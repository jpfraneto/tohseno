import { describe, expect, test } from "bun:test";
import type { PaymentProvider, VerifiedPaymentEvent } from "../src/payments.ts";
import { processVerifiedPaymentEvent } from "../src/payments.ts";
import type { OrderState } from "../src/state-machine.ts";
import { transitionOrder } from "../src/state-machine.ts";
import {
  beginHttpCheckout,
  createSiteHarness,
  submitThroughHttp,
  waitForEmailCount,
  VerifiedFakePaymentProvider,
} from "./helpers.ts";

function verifiedEvent(
  eventId: string,
  checkoutSessionId: string,
  submissionId: string,
  overrides: Partial<VerifiedPaymentEvent> = {},
): VerifiedPaymentEvent {
  return {
    provider: "stripe",
    eventId,
    type: "checkout.session.completed",
    outcome: "paid",
    checkoutSessionId,
    submissionId,
    submissionReferenceValid: true,
    checkoutMode: "payment",
    amountTotal: 8_800,
    currency: "usd",
    ...overrides,
  };
}

function orderStatus(
  harness: Awaited<ReturnType<typeof createSiteHarness>>,
  submissionId: string,
): OrderState | undefined {
  return harness.application.database.query<{ status: OrderState }, [string]>(
    "SELECT status FROM submissions WHERE id = ?",
  ).get(submissionId)?.status;
}

describe("payment-integrity regressions", () => {
  test("a checkout provider amount mismatch fails the attempt without releasing its URL", async () => {
    const unsafeUrl = "https://checkout.example.test/mismatched-session";
    const mismatchedProvider: PaymentProvider = {
      name: "stripe",
      availability: () => ({ available: true }),
      createCheckout: async () => ({
        checkoutSessionId: "cs_mismatched_amount",
        url: unsafeUrl,
        amount: 1,
        currency: "usd",
      }),
      verifyWebhook: async () => {
        throw new Error("not used");
      },
    };
    const harness = await createSiteHarness({ paymentProvider: mismatchedProvider });
    try {
      const submission = await submitThroughHttp(harness);
      const response = await harness.request("/api/checkout", {
        method: "POST",
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        body: JSON.stringify({ token: submission.token }),
      });
      const body = await response.text();

      expect(response.status).toBe(503);
      expect(body).toContain("unexpected amount or currency");
      expect(body).not.toContain(unsafeUrl);
      expect(orderStatus(harness, submission.submissionId)).toBe("READY_FOR_PAYMENT");
      expect(harness.application.database.query<{
        status: string;
        checkout_url: string | null;
      }, [string]>(
        "SELECT status, checkout_url FROM payments WHERE submission_id = ?",
      ).get(submission.submissionId)).toEqual({ status: "failed", checkout_url: null });
    } finally {
      await harness.close();
    }
  });

  test("verified metadata mismatches fail closed, remain idempotent, and release nothing", async () => {
    const paymentProvider = new VerifiedFakePaymentProvider();
    const harness = await createSiteHarness({ paymentProvider });
    try {
      const submission = await submitThroughHttp(harness);
      await waitForEmailCount(harness.emailProvider, 1);
      const checkoutSessionId = await beginHttpCheckout(harness, submission.token);
      const emailsBeforeEvent = harness.emailProvider.messages.length;
      const event = verifiedEvent(
        "evt_amount_mismatch",
        checkoutSessionId,
        submission.submissionId,
        { amountTotal: 8_799 },
      );

      const first = processVerifiedPaymentEvent(harness.application.database, event);
      expect(first).toEqual(expect.objectContaining({
        processed: true,
        requiresReview: true,
        paymentAccepted: false,
        deliveryReleased: false,
        finalStatus: "FAILED",
      }));
      expect(orderStatus(harness, submission.submissionId)).toBe("FAILED");
      expect(harness.application.database.query<{ status: string }, [string]>(
        "SELECT status FROM payments WHERE checkout_session_id = ?",
      ).get(checkoutSessionId)?.status).toBe("requires_review");

      const eventCount = harness.application.database.query<{ count: number }, []>(
        "SELECT count(*) AS count FROM payment_events",
      ).get()?.count;
      const orderEventCount = harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ?",
      ).get(submission.submissionId)?.count;
      expect(processVerifiedPaymentEvent(harness.application.database, event)).toEqual(expect.objectContaining({
        processed: false,
        submissionId: submission.submissionId,
        finalStatus: "FAILED",
      }));
      expect(harness.application.database.query<{ count: number }, []>(
        "SELECT count(*) AS count FROM payment_events",
      ).get()?.count).toBe(eventCount);
      expect(harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ?",
      ).get(submission.submissionId)?.count).toBe(orderEventCount);
      expect(harness.emailProvider.messages).toHaveLength(emailsBeforeEvent);
    } finally {
      await harness.close();
    }
  });

  test("failed retries and late terminal events cannot move a newer or paid order backward", async () => {
    const paymentProvider = new VerifiedFakePaymentProvider();
    const harness = await createSiteHarness({ paymentProvider });
    try {
      const submission = await submitThroughHttp(harness);
      const firstSession = await beginHttpCheckout(harness, submission.token);
      const firstFailure = processVerifiedPaymentEvent(
        harness.application.database,
        verifiedEvent("evt_first_failed", firstSession, submission.submissionId, {
          type: "checkout.session.async_payment_failed",
          outcome: "failed",
        }),
      );
      expect(firstFailure.finalStatus).toBe("READY_FOR_PAYMENT");

      const secondSession = await beginHttpCheckout(harness, submission.token);
      expect(secondSession).not.toBe(firstSession);
      expect(orderStatus(harness, submission.submissionId)).toBe("PAYMENT_PENDING");

      const lateOldFailure = processVerifiedPaymentEvent(
        harness.application.database,
        verifiedEvent("evt_first_expired_late", firstSession, submission.submissionId, {
          type: "checkout.session.expired",
          outcome: "expired",
        }),
      );
      expect(lateOldFailure.finalStatus).toBe("PAYMENT_PENDING");
      expect(orderStatus(harness, submission.submissionId)).toBe("PAYMENT_PENDING");

      const paid = processVerifiedPaymentEvent(
        harness.application.database,
        verifiedEvent("evt_second_paid", secondSession, submission.submissionId),
      );
      expect(paid).toEqual(expect.objectContaining({
        paymentAccepted: true,
        deliveryReleased: true,
        finalStatus: "READY",
      }));

      const lateFailureAfterPaid = processVerifiedPaymentEvent(
        harness.application.database,
        verifiedEvent("evt_second_failed_late", secondSession, submission.submissionId, {
          type: "checkout.session.async_payment_failed",
          outcome: "failed",
        }),
      );
      expect(lateFailureAfterPaid).toEqual(expect.objectContaining({
        processed: true,
        paymentAccepted: true,
        deliveryReleased: false,
        finalStatus: "READY",
      }));
      expect(orderStatus(harness, submission.submissionId)).toBe("READY");
      expect(harness.application.database.query<{ status: string }, [string]>(
        "SELECT status FROM payments WHERE checkout_session_id = ?",
      ).get(secondSession)?.status).toBe("paid");
    } finally {
      await harness.close();
    }
  });

  test("a verified late payment after cancellation is accepted for review but cannot release delivery", async () => {
    const paymentProvider = new VerifiedFakePaymentProvider();
    const harness = await createSiteHarness({ paymentProvider });
    try {
      const submission = await submitThroughHttp(harness);
      await waitForEmailCount(harness.emailProvider, 1);
      const checkoutSessionId = await beginHttpCheckout(harness, submission.token);
      const emailsBeforeEvent = harness.emailProvider.messages.length;
      transitionOrder(
        harness.application.database,
        submission.submissionId,
        "CANCELLED",
        "operator",
      );

      const result = processVerifiedPaymentEvent(
        harness.application.database,
        verifiedEvent("evt_paid_after_cancel", checkoutSessionId, submission.submissionId),
      );
      expect(result).toEqual(expect.objectContaining({
        processed: true,
        requiresReview: true,
        paymentAccepted: true,
        deliveryReleased: false,
        finalStatus: "CANCELLED",
      }));
      expect(orderStatus(harness, submission.submissionId)).toBe("CANCELLED");
      expect(harness.emailProvider.messages).toHaveLength(emailsBeforeEvent);

      const audit = harness.application.database.query<{ metadata_json: string }, [string]>(
        `SELECT metadata_json FROM order_events
         WHERE submission_id = ? AND actor_type = 'payment-provider'
         ORDER BY sequence DESC LIMIT 1`,
      ).get(submission.submissionId);
      expect(audit?.metadata_json).toContain("late-payment-requires-review");

      expect(transitionOrder(
        harness.application.database,
        submission.submissionId,
        "REFUNDED",
        "operator",
        { resolution: "late-payment-refunded" },
      )).toBe("REFUNDED");
      expect(orderStatus(harness, submission.submissionId)).toBe("REFUNDED");
    } finally {
      await harness.close();
    }
  });

  test("an unpaid failed order cannot bypass payment into delivery states", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      const checkoutSessionId = await beginHttpCheckout(harness, submission.token);
      processVerifiedPaymentEvent(
        harness.application.database,
        verifiedEvent("evt_integrity_failure", checkoutSessionId, submission.submissionId, {
          amountTotal: 1,
        }),
      );
      expect(orderStatus(harness, submission.submissionId)).toBe("FAILED");
      expect(() => transitionOrder(
        harness.application.database,
        submission.submissionId,
        "GENERATING",
        "operator",
      )).toThrow("Illegal self-hosted transition");
      expect(harness.application.database.query<{ capsule_released_at: string | null }, [string]>(
        "SELECT capsule_released_at FROM submissions WHERE id = ?",
      ).get(submission.submissionId)?.capsule_released_at).toBeNull();
      expect((await harness.request(`/c/${submission.token}/MASTER_PROMPT.md`)).status).toBe(404);
    } finally {
      await harness.close();
    }
  });
});
