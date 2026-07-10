import type { TohsenoDatabase } from "./database.ts";
import { generateOpaqueId } from "./crypto.ts";

export const ORDER_STATES = [
  "DRAFT",
  "SUBMITTED",
  "PREFLIGHTING",
  "NEEDS_CLARIFICATION",
  "REFUSED",
  "READY_FOR_PAYMENT",
  "PAYMENT_PENDING",
  "PAID",
  "MANIFEST_LOCKED",
  "GENERATING",
  "NEEDS_CREDENTIALS",
  "QA",
  "FAILED",
  "READY",
  "SUBMITTED_TO_STORES",
  "LIVE",
  "EJECTED",
  "CANCELLED",
  "REFUNDED",
  "ANKY_REVIEW",
  "ANKY_ACCEPTED",
  "ANKY_DECLINED",
] as const;

export type OrderState = (typeof ORDER_STATES)[number];
export const OPERATING_MODES = ["self-hosted", "client-owned", "anky-operated"] as const;
export type OperatingMode = (typeof OPERATING_MODES)[number];
export type ActorType = "system" | "customer" | "operator" | "payment-provider" | "operator-access";

const commonIntake: Partial<Record<OrderState, readonly OrderState[]>> = {
  DRAFT: ["SUBMITTED"],
  SUBMITTED: ["PREFLIGHTING", "NEEDS_CLARIFICATION", "REFUSED", "CANCELLED"],
  PREFLIGHTING: ["NEEDS_CLARIFICATION", "REFUSED", "CANCELLED"],
  NEEDS_CLARIFICATION: ["PREFLIGHTING", "REFUSED", "CANCELLED"],
};

const delivery: Partial<Record<OrderState, readonly OrderState[]>> = {
  MANIFEST_LOCKED: ["GENERATING", "NEEDS_CREDENTIALS", "FAILED"],
  GENERATING: ["NEEDS_CREDENTIALS", "QA", "READY", "FAILED", "CANCELLED"],
  NEEDS_CREDENTIALS: ["GENERATING", "QA", "CANCELLED", "FAILED"],
  QA: ["GENERATING", "READY", "FAILED", "CANCELLED"],
  FAILED: ["GENERATING", "NEEDS_CREDENTIALS", "CANCELLED", "REFUNDED"],
  READY: ["SUBMITTED_TO_STORES", "LIVE", "EJECTED", "FAILED"],
  SUBMITTED_TO_STORES: ["LIVE", "READY", "FAILED"],
  LIVE: ["EJECTED", "FAILED"],
};

const selfHosted: Partial<Record<OrderState, readonly OrderState[]>> = {
  ...commonIntake,
  SUBMITTED: ["PREFLIGHTING", "NEEDS_CLARIFICATION", "REFUSED", "READY_FOR_PAYMENT", "CANCELLED"],
  PREFLIGHTING: ["NEEDS_CLARIFICATION", "REFUSED", "READY_FOR_PAYMENT", "CANCELLED"],
  READY_FOR_PAYMENT: ["PAYMENT_PENDING", "CANCELLED"],
  PAYMENT_PENDING: ["PAID", "READY_FOR_PAYMENT", "FAILED", "CANCELLED"],
  PAID: ["MANIFEST_LOCKED", "REFUNDED"],
  CANCELLED: ["REFUNDED"],
  ...delivery,
};

const clientOwned: Partial<Record<OrderState, readonly OrderState[]>> = {
  ...selfHosted,
  MANIFEST_LOCKED: ["NEEDS_CREDENTIALS", "FAILED"],
};

const ankyOperated: Partial<Record<OrderState, readonly OrderState[]>> = {
  ...commonIntake,
  SUBMITTED: ["PREFLIGHTING", "NEEDS_CLARIFICATION", "REFUSED", "ANKY_REVIEW", "CANCELLED"],
  PREFLIGHTING: ["NEEDS_CLARIFICATION", "REFUSED", "ANKY_REVIEW", "CANCELLED"],
  ANKY_REVIEW: ["ANKY_ACCEPTED", "ANKY_DECLINED", "NEEDS_CLARIFICATION", "CANCELLED"],
  NEEDS_CLARIFICATION: ["ANKY_REVIEW", "REFUSED", "CANCELLED"],
  ANKY_ACCEPTED: ["MANIFEST_LOCKED", "NEEDS_CREDENTIALS", "GENERATING", "CANCELLED"],
  ...delivery,
  FAILED: ["GENERATING", "NEEDS_CREDENTIALS", "CANCELLED"],
};

export const LEGAL_TRANSITIONS: Readonly<Record<OperatingMode, Partial<Record<OrderState, readonly OrderState[]>>>> = {
  "self-hosted": selfHosted,
  "client-owned": clientOwned,
  "anky-operated": ankyOperated,
};

export function isOrderState(value: string): value is OrderState {
  return (ORDER_STATES as readonly string[]).includes(value);
}

export function isOperatingMode(value: string): value is OperatingMode {
  return (OPERATING_MODES as readonly string[]).includes(value);
}

export function canTransition(mode: OperatingMode, previous: OrderState, next: OrderState): boolean {
  return LEGAL_TRANSITIONS[mode][previous]?.includes(next) ?? false;
}

const nextStateActors: Partial<Record<OrderState, readonly ActorType[]>> = {
  PAYMENT_PENDING: ["customer", "payment-provider"],
  PAID: ["payment-provider"],
  REFUNDED: ["operator", "payment-provider"],
};

function releasesCapsule(mode: OperatingMode, next: OrderState): boolean {
  if (mode === "self-hosted") return next === "READY";
  if (mode === "client-owned") return next === "NEEDS_CREDENTIALS";
  return next === "ANKY_ACCEPTED";
}

const paidModeStates = new Set<OrderState>([
  "PAID",
  "MANIFEST_LOCKED",
  "GENERATING",
  "NEEDS_CREDENTIALS",
  "QA",
  "READY",
  "SUBMITTED_TO_STORES",
  "LIVE",
  "EJECTED",
]);

export function canActorTransition(actor: ActorType, next: OrderState): boolean {
  return nextStateActors[next]?.includes(actor) ?? true;
}

const unsafeMetadataKey = /(markdown|email|contact|capability|token|secret|credential|body|message|content)/i;

export function safeMetadata(metadata: Record<string, unknown>): string {
  for (const key of Object.keys(metadata)) {
    if (unsafeMetadataKey.test(key)) throw new Error(`Unsafe transition metadata key: ${key}`);
  }
  const encoded = JSON.stringify(metadata);
  if (new TextEncoder().encode(encoded).byteLength > 4_096) throw new Error("Transition metadata is too large");
  return encoded;
}

interface SubmissionStateRow {
  id: string;
  operating_mode: OperatingMode;
  status: OrderState;
}

export function transitionOrder(
  database: TohsenoDatabase,
  submissionId: string,
  next: OrderState,
  actor: ActorType,
  metadata: Record<string, unknown> = {},
): OrderState {
  const perform = database.transaction(() => {
    const row = database.query<SubmissionStateRow, [string]>(
      "SELECT id, operating_mode, status FROM submissions WHERE id = ?",
    ).get(submissionId);
    if (!row) throw new Error("Submission not found");
    if (!canTransition(row.operating_mode, row.status, next)) {
      throw new IllegalTransitionError(row.operating_mode, row.status, next);
    }
    if (!canActorTransition(actor, next)) {
      throw new IllegalTransitionError(row.operating_mode, row.status, next, actor);
    }
    if (row.operating_mode !== "anky-operated" && (paidModeStates.has(next) || next === "REFUNDED")) {
      const recordedPayment = database.query<{ present: number }, [string]>(
        "SELECT 1 AS present FROM payments WHERE submission_id = ? AND status = 'paid' LIMIT 1",
      ).get(submissionId);
      if (!recordedPayment) throw new IllegalTransitionError(row.operating_mode, row.status, next, actor);
    }
    const now = new Date().toISOString();
    const releaseAt = releasesCapsule(row.operating_mode, next) ? now : null;
    const update = database.query(`
      UPDATE submissions
      SET status = ?, updated_at = ?, capsule_released_at = COALESCE(capsule_released_at, ?)
      WHERE id = ? AND status = ?
    `).run(next, now, releaseAt, submissionId, row.status);
    if (update.changes !== 1) throw new Error("Concurrent order transition rejected");
    database.query(`
      INSERT INTO order_events (id, submission_id, previous_status, next_status, actor_type, metadata_json, created_at)
      VALUES (?, ?, ?, ?, ?, ?, ?)
    `).run(generateOpaqueId("evt"), submissionId, row.status, next, actor, safeMetadata(metadata), now);
    return next;
  });
  return perform();
}

export function appendAuditEvent(
  database: TohsenoDatabase,
  submissionId: string,
  actor: ActorType,
  metadata: Record<string, unknown>,
): void {
  const row = database.query<{ status: OrderState }, [string]>("SELECT status FROM submissions WHERE id = ?").get(submissionId);
  if (!row) throw new Error("Submission not found");
  database.query(`
    INSERT INTO order_events (id, submission_id, previous_status, next_status, actor_type, metadata_json, created_at)
    VALUES (?, ?, ?, ?, ?, ?, ?)
  `).run(
    generateOpaqueId("evt"),
    submissionId,
    row.status,
    row.status,
    actor,
    safeMetadata(metadata),
    new Date().toISOString(),
  );
}

export class IllegalTransitionError extends Error {
  constructor(
    readonly mode: OperatingMode,
    readonly previous: OrderState,
    readonly next: OrderState,
    readonly actor?: ActorType,
  ) {
    super(actor
      ? `Illegal ${mode} transition actor ${actor}: ${previous} -> ${next}`
      : `Illegal ${mode} transition: ${previous} -> ${next}`);
  }
}
