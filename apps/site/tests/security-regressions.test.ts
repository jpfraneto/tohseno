import { describe, expect, test } from "bun:test";
import { decryptString, encryptString } from "../src/crypto.ts";
import type { OrderState } from "../src/state-machine.ts";
import type { SubmissionRow } from "../src/submissions.ts";
import {
  beginHttpCheckout,
  createSiteHarness,
  submitThroughHttp,
  testConfig,
} from "./helpers.ts";

const authorization = (operatorToken: string): Record<string, string> => ({
  Authorization: `Bearer ${operatorToken}`,
});

describe("privacy-boundary regressions", () => {
  test("AES-GCM envelopes are bound to their declared record context", async () => {
    const key = testConfig().dataKeyBase64;
    const plaintext = "synthetic source that belongs only in the Markdown field";
    const markdownContext = "submission:sub_test_context:markdown";
    const contactContext = "submission:sub_test_context:contact";
    const serialized = await encryptString(plaintext, key, markdownContext);

    await expect(decryptString(serialized, key, contactContext)).rejects.toThrow(
      "Encrypted envelope context mismatch",
    );

    const tampered = JSON.parse(serialized) as Record<string, unknown>;
    tampered.context = contactContext;
    await expect(decryptString(JSON.stringify(tampered), key)).rejects.toThrow();
    expect(await decryptString(serialized, key, markdownContext)).toBe(plaintext);
  });

  test("malformed capability expiry fails closed on every private route", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      harness.application.database.query(
        "UPDATE submissions SET capability_expires_at = 'not-a-timestamp' WHERE id = ?",
      ).run(submission.submissionId);

      for (const path of [
        `/status/${submission.token}`,
        `/c/${submission.token}`,
        `/c/${submission.token}/MASTER_PROMPT.md`,
      ]) {
        const response = await harness.request(path, { headers: { Accept: "application/json" } });
        expect(response.status).toBe(404);
        expect(response.headers.get("cache-control")).toBe("no-store, private");
        expect(await response.json()).toEqual({ error: "Not found" });
      }
    } finally {
      await harness.close();
    }
  });

  test("a content hash identifies bytes but never authorizes a private route", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      const row = harness.application.database.query<SubmissionRow, [string]>(
        "SELECT * FROM submissions WHERE id = ?",
      ).get(submission.submissionId);
      expect(row).not.toBeNull();
      expect(row?.content_hash).toMatch(/^[a-f0-9]{64}$/);

      for (const path of [`/status/${row?.content_hash}`, `/c/${row?.content_hash}`]) {
        const response = await harness.request(path, { headers: { Accept: "application/json" } });
        expect(response.status).toBe(404);
        expect(await response.json()).toEqual({ error: "Not found" });
      }
    } finally {
      await harness.close();
    }
  });
});

describe("operator-boundary regressions", () => {
  test("an operator cannot assert PAID without a verified payment-provider event", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      await beginHttpCheckout(harness, submission.token);
      const before = harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ?",
      ).get(submission.submissionId)?.count;

      const response = await harness.request(
        `/api/operator/submissions/${submission.submissionId}/transition`,
        {
          method: "POST",
          headers: {
            ...authorization(harness.config.operatorToken),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({ nextStatus: "PAID" }),
        },
      );

      expect(response.status).toBe(409);
      expect(await response.json()).toEqual({
        error: "Illegal self-hosted transition actor operator: PAYMENT_PENDING -> PAID",
      });
      expect(harness.application.database.query<{ status: OrderState }, [string]>(
        "SELECT status FROM submissions WHERE id = ?",
      ).get(submission.submissionId)?.status).toBe("PAYMENT_PENDING");
      expect(harness.application.database.query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM order_events WHERE submission_id = ?",
      ).get(submission.submissionId)?.count).toBe(before);
    } finally {
      await harness.close();
    }
  });

  test("compiled summaries reject private-key fields and embedded key material", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      const endpoint = `/api/operator/submissions/${submission.submissionId}/summary`;
      const headers = {
        ...authorization(harness.config.operatorToken),
        "Content-Type": "application/json",
      };
      const summaries: unknown[] = [
        { privateKey: "synthetic-private-key-material" },
        {
          coreAction:
            "Record one action using -----BEGIN PRIVATE KEY----- synthetic material -----END PRIVATE KEY-----.",
        },
      ];

      for (const summary of summaries) {
        const response = await harness.request(endpoint, {
          method: "POST",
          headers,
          body: JSON.stringify({ summary }),
        });
        expect(response.status).toBe(422);
      }

      expect(harness.application.database.query<{ compiled_summary_json: string | null }, [string]>(
        "SELECT compiled_summary_json FROM submissions WHERE id = ?",
      ).get(submission.submissionId)?.compiled_summary_json).toBeNull();
      expect(harness.application.database.query<{ count: number }, [string]>(
        `SELECT count(*) AS count FROM order_events
         WHERE submission_id = ? AND metadata_json = '{"action":"compiled-summary-updated"}'`,
      ).get(submission.submissionId)?.count).toBe(0);
    } finally {
      await harness.close();
    }
  });

  test("a failed integrity check still records the explicit source inspection", async () => {
    const harness = await createSiteHarness();
    try {
      const submission = await submitThroughHttp(harness);
      harness.application.database.query("UPDATE submissions SET content_hash = ? WHERE id = ?")
        .run("0".repeat(64), submission.submissionId);
      const response = await harness.request(
        `/api/operator/submissions/${submission.submissionId}/inspect-source`,
        {
          method: "POST",
          headers: { Authorization: `Bearer ${harness.config.operatorToken}` },
        },
      );
      expect(response.status).toBe(500);
      const audit = harness.application.database.query<{ metadata_json: string }, [string]>(`
        SELECT metadata_json FROM order_events
        WHERE submission_id = ? AND actor_type = 'operator-access'
        ORDER BY sequence DESC LIMIT 1
      `).get(submission.submissionId);
      expect(audit?.metadata_json).toBe('{"action":"private-source-inspection"}');
    } finally {
      await harness.close();
    }
  });
});
