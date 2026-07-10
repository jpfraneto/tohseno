import type { AppConfig } from "../config.ts";
import { PRODUCT } from "../config.ts";
import { encryptString, generateCapabilityToken, generateOpaqueId, hashCapabilityToken, sha256Hex } from "./crypto.ts";
import type { TohsenoDatabase } from "./database.ts";
import { queueSubmissionEmail } from "./email.ts";
import type { OperatingMode, OrderState } from "./state-machine.ts";
import { isOperatingMode } from "./state-machine.ts";

export interface SubmissionInput {
  markdown: string;
  email: string;
  operatingMode: string;
}

export interface CreatedSubmission {
  id: string;
  capabilityToken: string;
  contentHash: string;
  operatingMode: OperatingMode;
  status: OrderState;
  capabilityExpiresAt: string;
}

export interface SubmissionRow {
  id: string;
  content_hash: string;
  encrypted_markdown: string;
  encrypted_contact: string;
  capability_token_hash: string;
  capability_expires_at: string;
  capability_revoked_at: string | null;
  capsule_released_at: string | null;
  operating_mode: OperatingMode;
  status: OrderState;
  manifest_version: string;
  compiled_summary_json: string | null;
  created_at: string;
  updated_at: string;
}

export class SubmissionValidationError extends Error {
  constructor(readonly field: "markdown" | "email" | "operatingMode", message: string) {
    super(message);
  }
}

function containsUnpairedSurrogate(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return true;
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      return true;
    }
  }
  return false;
}

export function validateMarkdown(markdown: string): void {
  if (containsUnpairedSurrogate(markdown)) {
    throw new SubmissionValidationError("markdown", "Markdown must be valid Unicode text");
  }
  const bytes = new TextEncoder().encode(markdown);
  if (bytes.byteLength > PRODUCT.maxMarkdownBytes) {
    throw new SubmissionValidationError("markdown", `Markdown must be at most ${PRODUCT.maxMarkdownBytes} UTF-8 bytes`);
  }
  const meaningful = markdown.trim();
  if (meaningful.length < PRODUCT.minMarkdownCharacters || !/[\p{L}\p{N}]/u.test(meaningful)) {
    throw new SubmissionValidationError("markdown", `Markdown must contain at least ${PRODUCT.minMarkdownCharacters} useful characters`);
  }
  if (markdown.includes("\0") || markdown.includes("\uFFFD")) {
    throw new SubmissionValidationError("markdown", "Markdown appears to contain binary or undecodable content");
  }
  let suspiciousControls = 0;
  for (const character of markdown) {
    const code = character.codePointAt(0) ?? 0;
    if ((code < 32 && code !== 9 && code !== 10 && code !== 13) || (code >= 127 && code <= 159)) suspiciousControls += 1;
  }
  if (suspiciousControls > 2 && suspiciousControls / Math.max(markdown.length, 1) > 0.01) {
    throw new SubmissionValidationError("markdown", "Markdown appears to contain binary control data");
  }
}

export function validateEmail(email: string): string {
  const normalized = email.trim();
  if (normalized.length > 254 || !/^[A-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Z0-9](?:[A-Z0-9-]{0,61}[A-Z0-9])?(?:\.[A-Z0-9](?:[A-Z0-9-]{0,61}[A-Z0-9])?)+$/i.test(normalized)) {
    throw new SubmissionValidationError("email", "Enter a structurally valid email address");
  }
  const local = normalized.slice(0, normalized.indexOf("@"));
  if (local.length > 64 || local.startsWith(".") || local.endsWith(".") || local.includes("..")) {
    throw new SubmissionValidationError("email", "Enter a structurally valid email address");
  }
  return normalized;
}

export function validateSubmissionInput(input: SubmissionInput): { email: string; operatingMode: OperatingMode } {
  validateMarkdown(input.markdown);
  const email = validateEmail(input.email);
  if (!isOperatingMode(input.operatingMode)) {
    throw new SubmissionValidationError("operatingMode", "Choose a supported operating mode");
  }
  return { email, operatingMode: input.operatingMode };
}

export async function createSubmission(
  database: TohsenoDatabase,
  config: AppConfig,
  input: SubmissionInput,
): Promise<CreatedSubmission> {
  const validated = validateSubmissionInput(input);
  const capabilityToken = generateCapabilityToken();
  const id = generateOpaqueId("sub");
  const [contentHash, capabilityTokenHash, encryptedMarkdown, encryptedContact] = await Promise.all([
    sha256Hex(input.markdown),
    hashCapabilityToken(capabilityToken),
    encryptString(input.markdown, config.dataKeyBase64, `submission:${id}:markdown`),
    encryptString(JSON.stringify({ email: validated.email }), config.dataKeyBase64, `submission:${id}:contact`),
  ]);
  const now = new Date();
  const createdAt = now.toISOString();
  const capabilityExpiresAt = new Date(now.getTime() + PRODUCT.capabilityLifetimeDays * 86_400_000).toISOString();
  const target: OrderState = validated.operatingMode === "anky-operated" ? "ANKY_REVIEW" : "READY_FOR_PAYMENT";

  const persist = database.transaction(() => {
    database.query(`
      INSERT INTO submissions (
        id, content_hash, encrypted_markdown, encrypted_contact, capability_token_hash,
        capability_expires_at, operating_mode, status, manifest_version, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, 'DRAFT', ?, ?, ?)
    `).run(
      id,
      contentHash,
      encryptedMarkdown,
      encryptedContact,
      capabilityTokenHash,
      capabilityExpiresAt,
      validated.operatingMode,
      PRODUCT.manifestVersion,
      createdAt,
      createdAt,
    );
    database.query(`
      INSERT INTO order_events (id, submission_id, previous_status, next_status, actor_type, metadata_json, created_at)
      VALUES (?, ?, 'DRAFT', 'SUBMITTED', 'customer', '{}', ?)
    `).run(generateOpaqueId("evt"), id, createdAt);
    database.query(`
      INSERT INTO order_events (id, submission_id, previous_status, next_status, actor_type, metadata_json, created_at)
      VALUES (?, ?, 'SUBMITTED', ?, 'system', ?, ?)
    `).run(
      generateOpaqueId("evt"),
      id,
      target,
      JSON.stringify({ preflight: "deterministic-v1" }),
      createdAt,
    );
    database.query("UPDATE submissions SET status = ?, updated_at = ? WHERE id = ?").run(target, createdAt, id);
    const emailKind = validated.operatingMode === "anky-operated"
      ? "anky-application-received"
      : "submission-received";
    queueSubmissionEmail(database, id, emailKind, `submission:${id}:${emailKind}:v1`);
  });
  persist();
  return { id, capabilityToken, contentHash, operatingMode: validated.operatingMode, status: target, capabilityExpiresAt };
}

export function getSubmission(database: TohsenoDatabase, id: string): SubmissionRow | null {
  return database.query<SubmissionRow, [string]>("SELECT * FROM submissions WHERE id = ?").get(id);
}

export function listSubmissions(database: TohsenoDatabase, limit = 100): Array<Omit<SubmissionRow, "encrypted_markdown" | "encrypted_contact" | "capability_token_hash">> {
  return database.query<Omit<SubmissionRow, "encrypted_markdown" | "encrypted_contact" | "capability_token_hash">, [number]>(`
    SELECT id, content_hash, capability_expires_at, capability_revoked_at, capsule_released_at, operating_mode, status,
           manifest_version, compiled_summary_json, created_at, updated_at
    FROM submissions ORDER BY created_at DESC LIMIT ?
  `).all(Math.min(Math.max(limit, 1), 500));
}
