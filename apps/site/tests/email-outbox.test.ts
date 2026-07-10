import { describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createApplication } from "../server.ts";
import type { EmailMessage } from "../src/email.ts";
import { deliverQueuedEmails } from "../src/email.ts";
import { openMigratedDatabase } from "../src/database.ts";
import { createSubmission } from "../src/submissions.ts";
import {
  beginHttpCheckout,
  completeMockCheckout,
  createSiteHarness,
  FakeEmailProvider,
  submitThroughHttp,
  syntheticMarkdown,
  testConfig,
  waitForEmailCount,
} from "./helpers.ts";

class ControlledEmailProvider extends FakeEmailProvider {
  readonly attempts: EmailMessage[] = [];
  behavior: "success" | "fail" | "gate" = "success";
  #release: (() => void) | undefined;

  release(): void {
    this.#release?.();
    this.#release = undefined;
  }

  override async send(message: EmailMessage): Promise<{ providerReference: string }> {
    this.attempts.push(structuredClone(message));
    if (this.behavior === "fail") throw new Error("synthetic provider failure");
    if (this.behavior === "gate") {
      await new Promise<void>((resolve) => { this.#release = resolve; });
    }
    return super.send(message);
  }
}

class SequencedEmailProvider extends FakeEmailProvider {
  readonly completedTemplates: string[] = [];

  override async send(message: EmailMessage): Promise<{ providerReference: string }> {
    if (message.template === "payment-confirmed") await Bun.sleep(5);
    const result = await super.send(message);
    this.completedTemplates.push(message.template);
    return result;
  }
}

describe("durable email outbox", () => {
  test("capability handoff does not wait for a blocked email provider", async () => {
    const provider = new ControlledEmailProvider();
    provider.behavior = "gate";
    const harness = await createSiteHarness({ emailProvider: provider });
    try {
      const submission = await Promise.race([
        submitThroughHttp(harness),
        Bun.sleep(100).then(() => null),
      ]);
      expect(submission).not.toBeNull();
      for (let attempt = 0; attempt < 1_000 && provider.attempts.length < 1; attempt += 1) await Bun.sleep(1);
      expect(provider.attempts).toHaveLength(1);
      provider.release();
      await waitForEmailCount(provider, 1);
    } finally {
      provider.release();
      await harness.close();
    }
  });

  test("provider failure persists one intent and an operator retry sends it once", async () => {
    const provider = new ControlledEmailProvider();
    provider.behavior = "fail";
    const harness = await createSiteHarness({ emailProvider: provider });
    try {
      const submission = await submitThroughHttp(harness);
      for (let attempt = 0; attempt < 1_000 && provider.attempts.length < 1; attempt += 1) await Bun.sleep(1);
      await harness.application.waitForBackgroundTasks();
      const failed = harness.application.database.query<{
        id: string;
        template: string;
        status: string;
        provider_reference: string | null;
      }, [string]>(`
        SELECT id, template, status, provider_reference FROM messages WHERE submission_id = ?
      `).get(submission.submissionId);
      expect(failed).toEqual(expect.objectContaining({
        template: "submission-received",
        status: "failed",
        provider_reference: null,
      }));
      expect(provider.attempts).toHaveLength(1);

      provider.behavior = "success";
      const authorization = { Authorization: `Bearer ${harness.config.operatorToken}` };
      const retry = await harness.request(
        `/api/operator/submissions/${submission.submissionId}/retry-email`,
        { method: "POST", headers: authorization },
      );
      expect(retry.status).toBe(200);
      expect(await retry.json()).toEqual(expect.objectContaining({
        submissionId: submission.submissionId,
        deliveries: [expect.objectContaining({ providerAccepted: true })],
      }));
      expect(provider.attempts).toHaveLength(2);
      expect(provider.attempts[0]?.idempotencyKey).toBe(provider.attempts[1]?.idempotencyKey);
      expect(harness.application.database.query<{ status: string; provider_reference: string }, [string]>(
        "SELECT status, provider_reference FROM messages WHERE id = ?",
      ).get(failed?.id ?? "missing")).toEqual(expect.objectContaining({ status: "sent" }));

      const duplicateRetry = await harness.request(
        `/api/operator/submissions/${submission.submissionId}/retry-email`,
        { method: "POST", headers: authorization },
      );
      expect(await duplicateRetry.json()).toEqual({ submissionId: submission.submissionId, deliveries: [] });
      expect(provider.attempts).toHaveLength(2);
    } finally {
      await harness.close();
    }
  });

  test("concurrent drains atomically claim a failed intent", async () => {
    const provider = new ControlledEmailProvider();
    provider.behavior = "fail";
    const harness = await createSiteHarness({ emailProvider: provider });
    try {
      const submission = await submitThroughHttp(harness);
      for (let attempt = 0; attempt < 1_000 && provider.attempts.length < 1; attempt += 1) await Bun.sleep(1);
      await harness.application.waitForBackgroundTasks();
      provider.behavior = "gate";
      const first = deliverQueuedEmails(
        harness.application.database,
        harness.config,
        provider,
        submission.submissionId,
      );
      const second = deliverQueuedEmails(
        harness.application.database,
        harness.config,
        provider,
        submission.submissionId,
      );
      for (let attempt = 0; attempt < 100 && provider.attempts.length < 2; attempt += 1) {
        await Bun.sleep(1);
      }
      expect(provider.attempts).toHaveLength(2);
      provider.release();
      expect(await first).toEqual([expect.objectContaining({ providerAccepted: true })]);
      expect(await second).toEqual([]);
      expect(provider.messages).toHaveLength(1);
      expect(harness.application.database.query<{ status: string }, [string]>(
        "SELECT status FROM messages WHERE submission_id = ?",
      ).get(submission.submissionId)?.status).toBe("sent");
    } finally {
      provider.release();
      await harness.close();
    }
  });

  test("application startup recovers and drains an interrupted intent", async () => {
    const directory = mkdtempSync(join(tmpdir(), "tohseno-outbox-restart-"));
    const databasePath = join(directory, "tohseno.sqlite");
    const config = testConfig({ databasePath });
    const firstDatabase = openMigratedDatabase(databasePath);
    try {
      const created = await createSubmission(firstDatabase, config, {
        markdown: syntheticMarkdown("restart notification"),
        email: "restart@example.test",
        operatingMode: "self-hosted",
      });
      expect(firstDatabase.query<{ status: string }, [string]>(
        "SELECT status FROM messages WHERE submission_id = ?",
      ).get(created.id)?.status).toBe("pending");
      firstDatabase.query("UPDATE messages SET status = 'sending' WHERE submission_id = ?")
        .run(created.id);
      firstDatabase.close();

      const reopened = openMigratedDatabase(databasePath);
      const provider = new ControlledEmailProvider();
      const application = await createApplication({ config, database: reopened, emailProvider: provider });
      try {
        await waitForEmailCount(provider, 1);
        expect(provider.messages.map((message) => message.template)).toEqual(["submission-received"]);
        expect(reopened.query<{ status: string }, [string]>(
          "SELECT status FROM messages WHERE submission_id = ?",
        ).get(created.id)?.status).toBe("sent");
      } finally {
        await application.close();
        reopened.close();
      }
    } finally {
      try { firstDatabase.close(); } catch { /* already closed */ }
      rmSync(directory, { recursive: true, force: true });
    }
  });

  test("payment confirmation is delivered before the release notice", async () => {
    const provider = new SequencedEmailProvider();
    const harness = await createSiteHarness({ emailProvider: provider });
    try {
      const submission = await submitThroughHttp(harness);
      await waitForEmailCount(provider, 1);
      provider.completedTemplates.length = 0;
      const checkoutSessionId = await beginHttpCheckout(harness, submission.token);
      await completeMockCheckout(harness, checkoutSessionId);
      await waitForEmailCount(provider, 3);
      expect(provider.completedTemplates).toEqual(["payment-confirmed", "self-hosted-ready"]);
    } finally {
      await harness.close();
    }
  });

  test("a blocked receipt drain serializes later payment notices", async () => {
    const provider = new ControlledEmailProvider();
    provider.behavior = "gate";
    const harness = await createSiteHarness({ emailProvider: provider });
    try {
      const submission = await submitThroughHttp(harness);
      for (let attempt = 0; attempt < 1_000 && provider.attempts.length < 1; attempt += 1) await Bun.sleep(1);
      const checkoutSessionId = await beginHttpCheckout(harness, submission.token);
      await completeMockCheckout(harness, checkoutSessionId);
      expect(provider.attempts.map((message) => message.template)).toEqual(["submission-received"]);

      provider.behavior = "success";
      provider.release();
      await waitForEmailCount(provider, 3);
      expect(provider.messages.map((message) => message.template)).toEqual([
        "submission-received",
        "payment-confirmed",
        "self-hosted-ready",
      ]);
    } finally {
      provider.release();
      await harness.close();
    }
  });
});
