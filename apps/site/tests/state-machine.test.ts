import { describe, expect, test } from "bun:test";
import {
  canTransition,
  IllegalTransitionError,
  safeMetadata,
  transitionOrder,
} from "../src/state-machine.ts";
import type { OrderState } from "../src/state-machine.ts";
import { beginHttpCheckout, completeMockCheckout, createSiteHarness, submitThroughHttp } from "./helpers.ts";

function currentStatus(
  harness: Awaited<ReturnType<typeof createSiteHarness>>,
  submissionId: string,
): OrderState | undefined {
  return harness.application.database.query<{ status: OrderState }, [string]>(
    "SELECT status FROM submissions WHERE id = ?",
  ).get(submissionId)?.status;
}

describe("mode-specific order state machine", () => {
  test("permits the self-hosted payment and capsule-preparation path", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness, "self-hosted");
      const checkoutSessionId = await beginHttpCheckout(harness, submission.token, submission.submissionId);
      expect((await completeMockCheckout(harness, checkoutSessionId)).finalStatus).toBe("READY");

      expect(currentStatus(harness, submission.submissionId)).toBe("READY");
      const events = harness.application.database.query<{
        previous_status: OrderState;
        next_status: OrderState;
      }, [string]>(
        "SELECT previous_status, next_status FROM order_events WHERE submission_id = ? ORDER BY sequence",
      ).all(submission.submissionId);
      expect(events.map(({ previous_status, next_status }) => `${previous_status}->${next_status}`)).toEqual([
        "DRAFT->SUBMITTED",
        "SUBMITTED->READY_FOR_PAYMENT",
        "READY_FOR_PAYMENT->PAYMENT_PENDING",
        "PAYMENT_PENDING->PAID",
        "PAID->MANIFEST_LOCKED",
        "MANIFEST_LOCKED->GENERATING",
        "GENERATING->READY",
      ]);
    } finally {
      await harness.close();
    }
  });

  test("rejects illegal transitions without changing status or appending an event", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness, "self-hosted");
      const before = harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ?",
      ).get(submission.submissionId)?.count;

      expect(() => transitionOrder(
        harness.application.database,
        submission.submissionId,
        "READY",
        "operator",
      )).toThrow(IllegalTransitionError);
      expect(currentStatus(harness, submission.submissionId)).toBe("READY_FOR_PAYMENT");
      const after = harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ?",
      ).get(submission.submissionId)?.count;
      expect(after).toBe(before);
    } finally {
      await harness.close();
    }
  });

  test("client-owned moves from a locked manifest to credentials, never directly to generation", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness, "client-owned");
      const checkoutSessionId = await beginHttpCheckout(harness, submission.token, submission.submissionId);
      expect((await completeMockCheckout(harness, checkoutSessionId)).finalStatus).toBe("NEEDS_CREDENTIALS");

      expect(canTransition("client-owned", "MANIFEST_LOCKED", "GENERATING")).toBe(false);
      const events = harness.application.database.query<{ previous_status: string; next_status: string }, [string]>(
        "SELECT previous_status, next_status FROM order_events WHERE submission_id = ? ORDER BY sequence",
      ).all(submission.submissionId);
      expect(events.some((event) => event.previous_status === "MANIFEST_LOCKED" && event.next_status === "GENERATING")).toBe(false);
      expect(events.some((event) => event.previous_status === "MANIFEST_LOCKED" && event.next_status === "NEEDS_CREDENTIALS")).toBe(true);
    } finally {
      await harness.close();
    }
  });

  test("Anky-operated review has no payment transition and requires acceptance before production states", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness, "anky-operated");
      expect(currentStatus(harness, submission.submissionId)).toBe("ANKY_REVIEW");
      expect(canTransition("anky-operated", "ANKY_REVIEW", "PAYMENT_PENDING")).toBe(false);
      expect(canTransition("anky-operated", "FAILED", "REFUNDED")).toBe(false);
      expect(() => transitionOrder(
        harness.application.database,
        submission.submissionId,
        "PAYMENT_PENDING",
        "operator",
      )).toThrow(IllegalTransitionError);
      expect(transitionOrder(
        harness.application.database,
        submission.submissionId,
        "ANKY_ACCEPTED",
        "operator",
      )).toBe("ANKY_ACCEPTED");
      expect(transitionOrder(
        harness.application.database,
        submission.submissionId,
        "MANIFEST_LOCKED",
        "operator",
      )).toBe("MANIFEST_LOCKED");
    } finally {
      await harness.close();
    }
  });
});

describe("append-only transition audit", () => {
  test("database triggers prevent event mutation and deletion", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      const event = harness.application.database.query<{ id: string }, [string]>(
        "SELECT id FROM order_events WHERE submission_id = ? ORDER BY rowid LIMIT 1",
      ).get(submission.submissionId);
      expect(event).not.toBeNull();

      expect(() => harness.application.database.query(
        "UPDATE order_events SET actor_type = 'operator' WHERE id = ?",
      ).run(event?.id ?? "missing")).toThrow("append-only");
      expect(() => harness.application.database.query(
        "DELETE FROM order_events WHERE id = ?",
      ).run(event?.id ?? "missing")).toThrow("append-only");
      const count = harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ?",
      ).get(submission.submissionId)?.count;
      expect(count).toBe(2);
    } finally {
      await harness.close();
    }
  });

  test("transition metadata refuses private-content-shaped keys", () => {
    expect(safeMetadata({ provider: "mock", phase: "verified" })).toBe(
      '{"provider":"mock","phase":"verified"}',
    );
    expect(() => safeMetadata({ email: "private@example.test" })).toThrow("Unsafe");
    expect(() => safeMetadata({ capabilityToken: "secret" })).toThrow("Unsafe");
    expect(() => safeMetadata({ messageBody: "private" })).toThrow("Unsafe");
  });
});
