import { PRODUCT } from "../config.ts";
import type { AppConfig } from "../config.ts";
import { decryptString, hashCapabilityToken, isCapabilityTokenShape, sha256Hex } from "./crypto.ts";
import type { TohsenoDatabase } from "./database.ts";
import type { SubmissionRow } from "./submissions.ts";

export async function resolveCapability(
  database: TohsenoDatabase,
  token: string,
  now = new Date(),
): Promise<SubmissionRow | null> {
  if (!isCapabilityTokenShape(token)) return null;
  const hash = await hashCapabilityToken(token);
  const row = database.query<SubmissionRow, [string]>("SELECT * FROM submissions WHERE capability_token_hash = ?").get(hash);
  if (!row || row.capability_revoked_at) return null;
  const expiresAt = Date.parse(row.capability_expires_at);
  if (!Number.isFinite(expiresAt) || expiresAt <= now.getTime()) return null;
  return row;
}

export function capsuleIsAvailable(
  submission: Pick<SubmissionRow, "capsule_released_at">,
): boolean {
  return submission.capsule_released_at !== null;
}

function selfHostedInstructions(url: string): string {
  return `Scaffold, validate, and deploy the continuity app described by this private TOHSENO capsule:

${url}

Follow the TOHSENO continuity-app skill and generated operator runbook.
Preserve the manifest. Ask before creating paid resources or submitting to
application stores. Return all repositories, credentials, production URLs,
ownership details, and ejection instructions to the owner.`;
}

function clientOwnedInstructions(url: string): string {
  return `${selfHostedInstructions(url)}

Before production work:

- Verify Apple Developer organization readiness.
- Verify Google Play Console readiness.
- Prepare scoped access rather than sharing passwords.
- Prepare domain and DNS ownership in the customer's account.
- Prepare the desired infrastructure account in the customer's name.
- Stop for human approval before spending money or submitting to stores.
- Preserve customer ownership of bundle IDs, package IDs, source, domains, application identities, and infrastructure.`;
}

function ankyInstructions(url: string): string {
  return `${selfHostedInstructions(url)}

This capsule was released only after Anky-operated review. Treat Anky, Inc. as the current product operator while preserving the repository's ejection contract.`;
}

export async function buildCapsuleMarkdown(
  submission: SubmissionRow,
  capabilityToken: string,
  config: AppConfig,
): Promise<string> {
  if (!capsuleIsAvailable(submission)) throw new Error("Capsule is not available");
  const masterPrompt = await decryptString(
    submission.encrypted_markdown,
    config.dataKeyBase64,
    `submission:${submission.id}:markdown`,
  );
  if (await sha256Hex(masterPrompt) !== submission.content_hash) {
    throw new Error("Submitted source integrity check failed");
  }
  const capsuleUrl = `${config.baseUrl}/c/${capabilityToken}`;
  const instructions = submission.operating_mode === "self-hosted"
    ? selfHostedInstructions(capsuleUrl)
    : submission.operating_mode === "client-owned"
      ? clientOwnedInstructions(capsuleUrl)
      : ankyInstructions(capsuleUrl);

  return `# Private TOHSENO agent capsule

This bearer document contains private customer source material. Do not publish it, commit it, place it in logs, or copy it into payment metadata.

## Source contract

- Submission ID: \`${submission.id}\`
- Content SHA-256: \`${submission.content_hash}\`
- Operating mode: \`${submission.operating_mode}\`
- Current order state: \`${submission.status}\`
- Manifest contract version: \`${submission.manifest_version}\`
- TOHSENO repository: ${PRODUCT.repositoryUrl}
- Continuity-app skill: \`${PRODUCT.skillPath}\`

**READY in this repository means this capsule and source contract are available. It does not mean that a native application has already been generated.**

## Exact operator instructions

${instructions}

> Never create paid infrastructure, spend money, alter DNS, submit to application stores, or rotate production credentials without explicit owner approval.

## Original MASTER_PROMPT.md

${masterPrompt}
`;
}
