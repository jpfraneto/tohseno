import type { AppConfig } from "../config.ts";
import { decryptString, generateOpaqueId } from "./crypto.ts";
import type { TohsenoDatabase } from "./database.ts";
import type { SubmissionRow } from "./submissions.ts";

export const EMAIL_TEMPLATES = [
  "submission-received",
  "payment-confirmed",
  "self-hosted-ready",
  "client-credentials-required",
  "anky-application-received",
  "operator-status",
] as const;

export type EmailTemplate = (typeof EMAIL_TEMPLATES)[number];

export interface EmailMessage {
  to: string;
  subject: string;
  text: string;
  template: EmailTemplate;
  submissionId: string;
  idempotencyKey: string;
}

export interface EmailProvider {
  readonly name: "disabled" | "console" | "resend" | "fake";
  send(message: EmailMessage): Promise<{ providerReference?: string }>;
}

export interface EmailDeliveryResult {
  messageId: string;
  submissionId: string;
  template: string;
  deliveryMode: EmailProvider["name"];
  deliveryAttempted: boolean;
  providerAccepted: boolean;
  providerReference?: string;
}

interface QueuedEmailRow {
  id: string;
  submission_id: string;
  encrypted_body: string;
  template: string | null;
  status: string;
  idempotency_key: string | null;
  provider_reference: string | null;
}

interface DeliveryLock {
  running: boolean;
  queue: Array<() => void>;
}

const deliveryLocks = new WeakMap<TohsenoDatabase, DeliveryLock>();

function serializeDelivery<T>(database: TohsenoDatabase, operation: () => Promise<T>): Promise<T> {
  const lock = deliveryLocks.get(database) ?? { running: false, queue: [] };
  deliveryLocks.set(database, lock);
  return new Promise<T>((resolve, reject) => {
    const run = (): void => {
      lock.running = true;
      let result: Promise<T>;
      try {
        result = operation();
      } catch (error) {
        result = Promise.reject(error);
      }
      void result.then(resolve, reject).finally(() => {
        const next = lock.queue.shift();
        if (next) next();
        else lock.running = false;
      });
    };
    if (lock.running) lock.queue.push(run);
    else run();
  });
}

export class DisabledEmailProvider implements EmailProvider {
  readonly name = "disabled" as const;
  async send(_message: EmailMessage): Promise<Record<string, never>> { return {}; }
}

export class ConsoleEmailProvider implements EmailProvider {
  readonly name = "console" as const;
  async send(message: EmailMessage): Promise<{ providerReference: string }> {
    const reference = `console_${crypto.randomUUID()}`;
    console.info(JSON.stringify({
      event: "email_delivery",
      provider: this.name,
      template: message.template,
      submissionId: message.submissionId,
      providerReference: reference,
    }));
    return { providerReference: reference };
  }
}

export class ResendEmailProvider implements EmailProvider {
  readonly name = "resend" as const;
  #deliveryGate: Promise<void> = Promise.resolve();

  constructor(private readonly apiKey: string, private readonly from: string) {}

  async send(message: EmailMessage): Promise<{ providerReference?: string }> {
    const deliver = async (): Promise<{ providerReference?: string }> => {
      // Serialize and pace this optional side effect so a restart backlog does
      // not burst a provider shared-team rate boundary.
      await Bun.sleep(225);
      const response = await fetch("https://api.resend.com/emails", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${this.apiKey}`,
          "Content-Type": "application/json",
          "Idempotency-Key": message.idempotencyKey,
        },
        body: JSON.stringify({ from: this.from, to: [message.to], subject: message.subject, text: message.text }),
        signal: AbortSignal.timeout(10_000),
      });
      if (!response.ok) throw new Error(`Email provider rejected delivery with status ${response.status}`);
      const body = await response.json() as { id?: string };
      return body.id ? { providerReference: body.id } : {};
    };
    const result = this.#deliveryGate.then(deliver, deliver);
    this.#deliveryGate = result.then(() => undefined, () => undefined);
    return result;
  }
}

export function createEmailProvider(config: AppConfig): EmailProvider {
  if (config.emailMode === "console") return new ConsoleEmailProvider();
  if (config.emailMode === "resend" && config.resendApiKey && config.emailFrom) {
    return new ResendEmailProvider(config.resendApiKey, config.emailFrom);
  }
  return new DisabledEmailProvider();
}

export function recoverInterruptedEmailDeliveries(database: TohsenoDatabase): number {
  return Number(database.query(`
    UPDATE messages SET status = 'failed', updated_at = ? WHERE status = 'sending'
  `).run(new Date().toISOString()).changes);
}

function isEmailTemplate(value: string | null): value is EmailTemplate {
  return value !== null && (EMAIL_TEMPLATES as readonly string[]).includes(value);
}

function template(kind: EmailTemplate, customerMessage?: string): { subject: string; text: string } {
  switch (kind) {
    case "submission-received":
      return { subject: "TOHSENO submission received", text: "Your private Markdown intake was received and encrypted. Keep the private status URL returned when you submitted it." };
    case "payment-confirmed":
      return { subject: "TOHSENO payment confirmed", text: "Payment was confirmed by the payment provider. The success redirect alone was not treated as proof of payment." };
    case "self-hosted-ready":
      return { subject: "Your private TOHSENO capsule is ready", text: "Your private agent capsule and source contract are ready at your existing private status URL. READY does not mean a native application has already been generated." };
    case "client-credentials-required":
      return {
        subject: "TOHSENO production preparation",
        text: `Your continuity app is entering production preparation.

Your source contract, infrastructure plan, store materials, and production
candidate are expected within eight hours after all required account access
and credentials are ready.

Public App Store and Google Play availability follows platform review and
cannot be guaranteed within that window.`,
      };
    case "anky-application-received":
      return { subject: "Anky-operated application received", text: "Anky, Inc. received the application for selective review. This is not an automatic publishing or production commitment." };
    case "operator-status":
      return { subject: "Your TOHSENO status changed", text: customerMessage ?? "Your order status changed. Return to your private status URL for the current state." };
  }
}

async function contactEmail(submission: SubmissionRow, config: AppConfig): Promise<string> {
  const decrypted = await decryptString(
    submission.encrypted_contact,
    config.dataKeyBase64,
    `submission:${submission.id}:contact`,
  );
  const parsed: unknown = JSON.parse(decrypted);
  if (typeof parsed !== "object" || parsed === null || !("email" in parsed) || typeof parsed.email !== "string") {
    throw new Error("Stored contact envelope is invalid");
  }
  return parsed.email;
}

export function queueSubmissionEmail(
  database: TohsenoDatabase,
  submissionId: string,
  kind: EmailTemplate,
  idempotencyKey: string,
  encryptedBody = "",
  messageId = generateOpaqueId("msg"),
): string {
  const now = new Date().toISOString();
  database.query(`
    INSERT OR IGNORE INTO messages (
      id, submission_id, direction, channel, encrypted_body, provider_reference,
      created_at, template, status, idempotency_key, updated_at
    ) VALUES (?, ?, 'outbound', 'email', ?, NULL, ?, ?, 'pending', ?, ?)
  `).run(messageId, submissionId, encryptedBody, now, kind, idempotencyKey, now);
  const stored = database.query<{ id: string }, [string]>(
    "SELECT id FROM messages WHERE idempotency_key = ?",
  ).get(idempotencyKey);
  if (!stored) throw new Error("Email notification intent could not be persisted");
  return stored.id;
}

async function deliverRow(
  database: TohsenoDatabase,
  config: AppConfig,
  provider: EmailProvider,
  row: QueuedEmailRow,
): Promise<EmailDeliveryResult> {
  if (!isEmailTemplate(row.template)) {
    database.query("UPDATE messages SET status = 'failed', updated_at = ? WHERE id = ? AND status = 'sending'")
      .run(new Date().toISOString(), row.id);
    console.error(JSON.stringify({
      event: "email_delivery_failed",
      submissionId: row.submission_id,
      template: "invalid",
      provider: provider.name,
      errorType: "InvalidStoredTemplate",
    }));
    return {
      messageId: row.id,
      submissionId: row.submission_id,
      template: "invalid",
      deliveryMode: provider.name,
      deliveryAttempted: false,
      providerAccepted: false,
    };
  }
  if (provider.name === "disabled") {
    return {
      messageId: row.id,
      submissionId: row.submission_id,
      template: row.template,
      deliveryMode: provider.name,
      deliveryAttempted: false,
      providerAccepted: false,
    };
  }
  try {
    const submission = database.query<SubmissionRow, [string]>(
      "SELECT * FROM submissions WHERE id = ?",
    ).get(row.submission_id);
    if (!submission) throw new Error("Submission not found");
    const customerMessage = row.template === "operator-status"
      ? await decryptString(row.encrypted_body, config.dataKeyBase64, `message:${row.id}:body`)
      : undefined;
    const content = template(row.template, customerMessage);
    const delivery = await provider.send({
      to: await contactEmail(submission, config),
      subject: content.subject,
      text: content.text,
      template: row.template,
      submissionId: row.submission_id,
      idempotencyKey: row.idempotency_key ?? row.id,
    });
    database.query(`
      UPDATE messages SET status = 'sent', provider_reference = ?, updated_at = ?
      WHERE id = ? AND status = 'sending'
    `).run(delivery.providerReference ?? null, new Date().toISOString(), row.id);
    return delivery.providerReference
      ? {
          messageId: row.id,
          submissionId: row.submission_id,
          template: row.template,
          deliveryMode: provider.name,
          deliveryAttempted: true,
          providerAccepted: true,
          providerReference: delivery.providerReference,
        }
      : {
          messageId: row.id,
          submissionId: row.submission_id,
          template: row.template,
          deliveryMode: provider.name,
          deliveryAttempted: true,
          providerAccepted: true,
        };
  } catch (error) {
    database.query("UPDATE messages SET status = 'failed', updated_at = ? WHERE id = ? AND status = 'sending'")
      .run(new Date().toISOString(), row.id);
    console.error(JSON.stringify({
      event: "email_delivery_failed",
      submissionId: row.submission_id,
      template: row.template,
      provider: provider.name,
      errorType: error instanceof Error ? error.constructor.name : "Unknown",
    }));
    return {
      messageId: row.id,
      submissionId: row.submission_id,
      template: row.template,
      deliveryMode: provider.name,
      deliveryAttempted: true,
      providerAccepted: false,
    };
  }
}

async function deliverQueuedEmailsUnlocked(
  database: TohsenoDatabase,
  config: AppConfig,
  provider: EmailProvider,
  submissionId?: string,
  limit = 20,
  includeSuppressed = false,
  includeFailed = true,
): Promise<EmailDeliveryResult[]> {
  if (provider.name === "disabled") {
    const now = new Date().toISOString();
    if (submissionId) {
      database.query(`
        UPDATE messages SET status = 'suppressed', updated_at = ?
        WHERE submission_id = ? AND status IN ('pending', 'failed', 'sending')
      `).run(now, submissionId);
    } else {
      database.query("UPDATE messages SET status = 'suppressed', updated_at = ? WHERE status IN ('pending', 'failed', 'sending')")
        .run(now);
    }
    return [];
  }
  const boundedLimit = Math.min(Math.max(limit, 1), 100);
  const claim = database.transaction((): QueuedEmailRow[] => {
    const now = new Date();
    const nowText = now.toISOString();
    const staleBefore = new Date(now.getTime() - 5 * 60_000).toISOString();
    database.query(`
      UPDATE messages SET status = 'failed', updated_at = ?
      WHERE status = 'sending' AND (updated_at IS NULL OR updated_at < ?)
    `).run(nowText, staleBefore);
    const candidates = submissionId
      ? database.query<QueuedEmailRow, [string, number, number, number]>(`
          SELECT id, submission_id, encrypted_body, template, status, idempotency_key, provider_reference
          FROM messages
          WHERE submission_id = ?
            AND (
              status = 'pending'
              OR (? = 1 AND status = 'failed')
              OR (? = 1 AND status = 'suppressed')
            )
          ORDER BY rowid LIMIT ?
        `).all(submissionId, includeFailed ? 1 : 0, includeSuppressed ? 1 : 0, boundedLimit)
      : database.query<QueuedEmailRow, [number, number, number]>(`
          SELECT id, submission_id, encrypted_body, template, status, idempotency_key, provider_reference
          FROM messages
          WHERE status = 'pending'
            OR (? = 1 AND status = 'failed')
            OR (? = 1 AND status = 'suppressed')
          ORDER BY rowid LIMIT ?
        `).all(includeFailed ? 1 : 0, includeSuppressed ? 1 : 0, boundedLimit);
    const claimed: QueuedEmailRow[] = [];
    for (const candidate of candidates) {
      const updated = database.query(`
        UPDATE messages SET status = 'sending', updated_at = ?
        WHERE id = ? AND status = ?
      `).run(nowText, candidate.id, candidate.status);
      if (updated.changes === 1) claimed.push({ ...candidate, status: "sending" });
    }
    return claimed;
  });
  const rows = claim();
  const bySubmission = new Map<string, QueuedEmailRow[]>();
  for (const row of rows) {
    const group = bySubmission.get(row.submission_id) ?? [];
    group.push(row);
    bySubmission.set(row.submission_id, group);
  }
  const groupedResults = await Promise.all([...bySubmission.values()].map(async (group) => {
    const results: EmailDeliveryResult[] = [];
    for (const row of group) results.push(await deliverRow(database, config, provider, row));
    return results;
  }));
  return groupedResults.flat();
}

async function deliverQueuedEmailUnlocked(
  database: TohsenoDatabase,
  config: AppConfig,
  provider: EmailProvider,
  messageId: string,
): Promise<EmailDeliveryResult> {
  const select = (): QueuedEmailRow | null => database.query<QueuedEmailRow, [string]>(`
    SELECT id, submission_id, encrypted_body, template, status, idempotency_key, provider_reference
    FROM messages WHERE id = ?
  `).get(messageId) ?? null;
  if (provider.name === "disabled") {
    const pending = select();
    if (!pending || !isEmailTemplate(pending.template)) throw new Error("Queued email not found");
    if (["pending", "failed", "sending"].includes(pending.status)) {
      database.query("UPDATE messages SET status = 'suppressed', updated_at = ? WHERE id = ? AND status IN ('pending', 'failed', 'sending')")
        .run(new Date().toISOString(), pending.id);
    }
    return {
      messageId: pending.id,
      submissionId: pending.submission_id,
      template: pending.template,
      deliveryMode: provider.name,
      deliveryAttempted: false,
      providerAccepted: false,
    };
  }
  const claim = database.transaction((): QueuedEmailRow | null => {
    const updated = database.query(`
      UPDATE messages SET status = 'sending', updated_at = ?
      WHERE id = ? AND status IN ('pending', 'failed')
    `).run(new Date().toISOString(), messageId);
    return updated.changes === 1 ? select() : null;
  });
  const row = claim();
  if (!row) {
    const existing = select();
    if (!existing || !isEmailTemplate(existing.template)) throw new Error("Queued email not found");
    if (existing.status === "sent") {
      return existing.provider_reference
        ? {
            messageId: existing.id,
            submissionId: existing.submission_id,
            template: existing.template,
            deliveryMode: provider.name,
            deliveryAttempted: false,
            providerAccepted: true,
            providerReference: existing.provider_reference,
          }
        : {
            messageId: existing.id,
            submissionId: existing.submission_id,
            template: existing.template,
            deliveryMode: provider.name,
            deliveryAttempted: false,
            providerAccepted: true,
          };
    }
    return {
      messageId: existing.id,
      submissionId: existing.submission_id,
      template: existing.template,
      deliveryMode: provider.name,
      deliveryAttempted: false,
      providerAccepted: false,
    };
  }
  return deliverRow(database, config, provider, row);
}

export function deliverQueuedEmails(
  database: TohsenoDatabase,
  config: AppConfig,
  provider: EmailProvider,
  submissionId?: string,
  limit = 20,
  includeSuppressed = false,
  includeFailed = true,
): Promise<EmailDeliveryResult[]> {
  return serializeDelivery(database, () => deliverQueuedEmailsUnlocked(
    database,
    config,
    provider,
    submissionId,
    limit,
    includeSuppressed,
    includeFailed,
  ));
}

export function deliverQueuedEmail(
  database: TohsenoDatabase,
  config: AppConfig,
  provider: EmailProvider,
  messageId: string,
): Promise<EmailDeliveryResult> {
  return serializeDelivery(database, () => deliverQueuedEmailUnlocked(
    database,
    config,
    provider,
    messageId,
  ));
}
