import type { AppConfig } from "../config.ts";
import { decryptString, encryptString, generateOpaqueId, sha256Hex } from "./crypto.ts";
import type { TohsenoDatabase } from "./database.ts";
import type { EmailProvider } from "./email.ts";
import { deliverQueuedEmail, queueSubmissionEmail } from "./email.ts";
import { appendAuditEvent, isOrderState, transitionOrder } from "./state-machine.ts";
import type { OrderState } from "./state-machine.ts";
import { getSubmission, listSubmissions } from "./submissions.ts";

export function operatorList(database: TohsenoDatabase): ReturnType<typeof listSubmissions> {
  return listSubmissions(database);
}

const SUMMARY_KEYS = [
  "applicationId",
  "applicationName",
  "coreAction",
  "manifestVersion",
  "manifestStatus",
  "deploymentTargets",
  "riskFlags",
  "refusalCodes",
  "requiresHumanDecision",
] as const;

function summaryText(value: unknown, key: string, minimum: number, maximum: number, pattern?: RegExp): string {
  if (typeof value !== "string" || value.length < minimum || value.length > maximum || /[\r\n]/.test(value) || (pattern && !pattern.test(value))) {
    throw new OperatorValidationError(`Summary field ${key} is invalid`);
  }
  const looksPrivate = /[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/i.test(value) ||
    /\/(?:c|status)\/[A-Za-z0-9_-]{43}/.test(value) ||
    /-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----/i.test(value) ||
    /(?:api[_ -]?key|password|secret|bearer|credential|access[_ -]?token)\s*[:=]/i.test(value) ||
    /\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9_-]{12,}\b|\bwhsec_[A-Za-z0-9_-]{12,}\b/.test(value);
  if (looksPrivate) {
    throw new OperatorValidationError(`Summary field ${key} appears to contain private access or contact data`);
  }
  return value;
}

function summaryStringArray(
  value: unknown,
  key: string,
  allowed?: ReadonlySet<string>,
): string[] {
  if (!Array.isArray(value) || value.length > 20) throw new OperatorValidationError(`Summary field ${key} must be an array of at most 20 values`);
  const result = value.map((entry) => summaryText(entry, key, 1, 80, /^[a-z0-9][a-z0-9._-]*$/));
  if (new Set(result).size !== result.length) throw new OperatorValidationError(`Summary field ${key} must not contain duplicates`);
  if (allowed && result.some((entry) => !allowed.has(entry))) throw new OperatorValidationError(`Summary field ${key} contains an unsupported value`);
  return result;
}

function validateCompiledSummary(summary: unknown): Record<string, unknown> {
  if (typeof summary !== "object" || summary === null || Array.isArray(summary)) {
    throw new OperatorValidationError("Summary must be a JSON object");
  }
  const input = summary as Record<string, unknown>;
  const keys = Object.keys(input);
  if (keys.length < 1 || keys.some((key) => !(SUMMARY_KEYS as readonly string[]).includes(key))) {
    throw new OperatorValidationError(`Summary supports only these fields: ${SUMMARY_KEYS.join(", ")}`);
  }
  const output: Record<string, unknown> = {};
  if (input.applicationId !== undefined) output.applicationId = summaryText(input.applicationId, "applicationId", 3, 120, /^[a-z0-9][a-z0-9._-]*$/);
  if (input.applicationName !== undefined) output.applicationName = summaryText(input.applicationName, "applicationName", 1, 80);
  if (input.coreAction !== undefined) output.coreAction = summaryText(input.coreAction, "coreAction", 12, 240);
  if (input.manifestVersion !== undefined) output.manifestVersion = summaryText(input.manifestVersion, "manifestVersion", 1, 40, /^[A-Za-z0-9][A-Za-z0-9._-]*$/);
  if (input.manifestStatus !== undefined) {
    const value = summaryText(input.manifestStatus, "manifestStatus", 1, 40, /^[a-z-]+$/);
    if (!["not-started", "draft", "validated", "locked"].includes(value)) throw new OperatorValidationError("Summary field manifestStatus is unsupported");
    output.manifestStatus = value;
  }
  if (input.deploymentTargets !== undefined) {
    output.deploymentTargets = summaryStringArray(
      input.deploymentTargets,
      "deploymentTargets",
      new Set(["native-ios", "native-android", "web", "server"]),
    );
  }
  if (input.riskFlags !== undefined) output.riskFlags = summaryStringArray(input.riskFlags, "riskFlags");
  if (input.refusalCodes !== undefined) output.refusalCodes = summaryStringArray(input.refusalCodes, "refusalCodes");
  if (input.requiresHumanDecision !== undefined) {
    if (typeof input.requiresHumanDecision !== "boolean") throw new OperatorValidationError("Summary field requiresHumanDecision must be boolean");
    output.requiresHumanDecision = input.requiresHumanDecision;
  }
  return output;
}

export function operatorShow(database: TohsenoDatabase, submissionId: string): Record<string, unknown> {
  const submission = getSubmission(database, submissionId);
  if (!submission) throw new OperatorNotFoundError();
  const events = database.query<Record<string, string | number>, [string]>(`
    SELECT sequence, id, previous_status, next_status, actor_type, metadata_json, created_at
    FROM order_events WHERE submission_id = ? ORDER BY sequence
  `).all(submissionId);
  const payments = database.query<Record<string, string | number | null>, [string]>(`
    SELECT id, provider, provider_reference, checkout_session_id, attempt, amount, currency, status, created_at, updated_at
    FROM payments WHERE submission_id = ? ORDER BY created_at
  `).all(submissionId);
  const messages = database.query<Record<string, string | null>, [string]>(`
    SELECT id, template, status, provider_reference, created_at, updated_at
    FROM messages WHERE submission_id = ? ORDER BY rowid
  `).all(submissionId);
  return {
    submission: {
      id: submission.id,
      contentHash: submission.content_hash,
      operatingMode: submission.operating_mode,
      status: submission.status,
      manifestVersion: submission.manifest_version,
      compiledSummary: submission.compiled_summary_json ? JSON.parse(submission.compiled_summary_json) : null,
      capabilityExpiresAt: submission.capability_expires_at,
      capabilityRevokedAt: submission.capability_revoked_at,
      capsuleReleasedAt: submission.capsule_released_at,
      createdAt: submission.created_at,
      updatedAt: submission.updated_at,
    },
    events,
    payments,
    messages,
  };
}

export async function operatorInspectSource(
  database: TohsenoDatabase,
  config: AppConfig,
  submissionId: string,
): Promise<Record<string, unknown>> {
  const submission = getSubmission(database, submissionId);
  if (!submission) throw new OperatorNotFoundError();
  appendAuditEvent(database, submissionId, "operator-access", { action: "private-source-inspection" });
  const [masterPrompt, contactEnvelope] = await Promise.all([
    decryptString(
      submission.encrypted_markdown,
      config.dataKeyBase64,
      `submission:${submission.id}:markdown`,
    ),
    decryptString(
      submission.encrypted_contact,
      config.dataKeyBase64,
      `submission:${submission.id}:contact`,
    ),
  ]);
  if (await sha256Hex(masterPrompt) !== submission.content_hash) {
    throw new Error("Submitted source integrity check failed");
  }
  const contact: unknown = JSON.parse(contactEnvelope);
  return { submissionId, contact, masterPrompt };
}

export function operatorSetSummary(database: TohsenoDatabase, submissionId: string, summary: unknown): void {
  const serialized = JSON.stringify(validateCompiledSummary(summary));
  if (new TextEncoder().encode(serialized).byteLength > 64 * 1024) throw new Error("Summary is too large");
  const now = new Date().toISOString();
  const persist = database.transaction(() => {
    const result = database.query("UPDATE submissions SET compiled_summary_json = ?, updated_at = ? WHERE id = ?")
      .run(serialized, now, submissionId);
    if (result.changes !== 1) throw new OperatorNotFoundError();
    appendAuditEvent(database, submissionId, "operator", { action: "compiled-summary-updated" });
  });
  persist();
}

interface PreparedOperatorMessage {
  id: string;
  encryptedBody: string;
}

export interface OperatorMessageResult {
  recorded: true;
  deliveryMode: EmailProvider["name"];
  deliveryAttempted: boolean;
  providerAccepted: boolean;
  providerReference?: string;
}

export interface OperatorTransitionResult {
  status: OrderState;
  messageDelivery?: OperatorMessageResult;
}

async function prepareOperatorMessage(
  database: TohsenoDatabase,
  config: AppConfig,
  submissionId: string,
  message: string,
): Promise<PreparedOperatorMessage> {
  const trimmed = message.trim();
  if (!trimmed || new TextEncoder().encode(trimmed).byteLength > 64 * 1024) {
    throw new OperatorValidationError("Message must contain 1 to 65536 UTF-8 bytes");
  }
  if (!getSubmission(database, submissionId)) throw new OperatorNotFoundError();
  const id = generateOpaqueId("msg");
  return {
    id,
    encryptedBody: await encryptString(trimmed, config.dataKeyBase64, `message:${id}:body`),
  };
}

function persistOperatorMessage(
  database: TohsenoDatabase,
  submissionId: string,
  prepared: PreparedOperatorMessage,
  emailProvider: EmailProvider,
): void {
  queueSubmissionEmail(
    database,
    submissionId,
    "operator-status",
    `operator-message:${prepared.id}:v1`,
    prepared.encryptedBody,
    prepared.id,
  );
  appendAuditEvent(database, submissionId, "operator", {
    action: "customer-update-recorded",
    channel: "email",
    deliveryMode: emailProvider.name,
  });
}

async function deliverOperatorMessage(
  database: TohsenoDatabase,
  config: AppConfig,
  emailProvider: EmailProvider,
  submissionId: string,
  prepared: PreparedOperatorMessage,
): Promise<OperatorMessageResult> {
  const delivery = await deliverQueuedEmail(database, config, emailProvider, prepared.id);
  appendAuditEvent(database, submissionId, "operator", {
    action: "customer-update-delivery",
    channel: "email",
    deliveryMode: delivery.deliveryMode,
    providerAccepted: delivery.providerAccepted,
  });
  return delivery.providerReference
    ? {
        recorded: true,
        deliveryMode: delivery.deliveryMode,
        deliveryAttempted: delivery.deliveryAttempted,
        providerAccepted: delivery.providerAccepted,
        providerReference: delivery.providerReference,
      }
    : {
        recorded: true,
        deliveryMode: delivery.deliveryMode,
        deliveryAttempted: delivery.deliveryAttempted,
        providerAccepted: delivery.providerAccepted,
      };
}

export async function operatorMessage(
  database: TohsenoDatabase,
  config: AppConfig,
  emailProvider: EmailProvider,
  submissionId: string,
  message: string,
): Promise<OperatorMessageResult> {
  const prepared = await prepareOperatorMessage(database, config, submissionId, message);
  const persist = database.transaction(() => persistOperatorMessage(database, submissionId, prepared, emailProvider));
  persist();
  return deliverOperatorMessage(database, config, emailProvider, submissionId, prepared);
}

export async function operatorTransition(
  database: TohsenoDatabase,
  config: AppConfig,
  emailProvider: EmailProvider,
  submissionId: string,
  nextStatus: string,
  customerMessage?: string,
): Promise<OperatorTransitionResult> {
  if (!isOrderState(nextStatus)) throw new OperatorValidationError("Unknown order state");
  if (!getSubmission(database, submissionId)) throw new OperatorNotFoundError();
  const prepared = customerMessage === undefined
    ? undefined
    : await prepareOperatorMessage(database, config, submissionId, customerMessage);
  let next: OrderState = nextStatus;
  const persist = database.transaction(() => {
    next = transitionOrder(database, submissionId, nextStatus, "operator", { customerUpdateSupplied: prepared !== undefined });
    if (prepared) persistOperatorMessage(database, submissionId, prepared, emailProvider);
  });
  persist();
  if (!prepared) return { status: next };
  const messageDelivery = await deliverOperatorMessage(database, config, emailProvider, submissionId, prepared);
  return { status: next, messageDelivery };
}

export function operatorRevokeCapability(database: TohsenoDatabase, submissionId: string): void {
  const persist = database.transaction(() => {
    const now = new Date().toISOString();
    const result = database.query(`
      UPDATE submissions SET capability_revoked_at = COALESCE(capability_revoked_at, ?), updated_at = ? WHERE id = ?
    `).run(now, now, submissionId);
    if (result.changes !== 1) throw new OperatorNotFoundError();
    appendAuditEvent(database, submissionId, "operator", { action: "capability-revoked" });
  });
  persist();
}

export class OperatorNotFoundError extends Error {
  constructor() { super("Submission not found"); }
}

export class OperatorValidationError extends Error {}
