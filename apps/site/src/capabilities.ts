import { PRODUCT } from "../config.ts";
import type { AppConfig } from "../config.ts";
import { decryptString, hashCapabilityToken, isCapabilityTokenShape, sha256Hex } from "./crypto.ts";
import type { TohsenoDatabase } from "./database.ts";
import type { SubmissionRow } from "./submissions.ts";

const SECURE_CAPABILITY_COOKIE = "__Host-tohseno-capability";
const DEVELOPMENT_CAPABILITY_COOKIE = "tohseno-capability";

function assertSubmissionId(submissionId: string): void {
  if (!/^sub_[A-Za-z0-9_-]{24}$/.test(submissionId)) throw new Error("Invalid capability cookie scope");
}

export function capabilityCookieName(
  config: Pick<AppConfig, "baseUrl" | "nodeEnv">,
  submissionId: string,
): string {
  assertSubmissionId(submissionId);
  const prefix = config.nodeEnv === "production" || new URL(config.baseUrl).protocol === "https:"
    ? SECURE_CAPABILITY_COOKIE
    : DEVELOPMENT_CAPABILITY_COOKIE;
  return `${prefix}-${submissionId}`;
}

export function capabilityCookie(
  token: string,
  expiresAt: string,
  config: Pick<AppConfig, "baseUrl" | "nodeEnv">,
  submissionId: string,
): string {
  if (!isCapabilityTokenShape(token)) throw new Error("Cannot set a malformed capability cookie");
  const expires = new Date(expiresAt);
  if (!Number.isFinite(expires.getTime())) throw new Error("Cannot set a capability cookie with an invalid expiry");
  const secure = config.nodeEnv === "production" || new URL(config.baseUrl).protocol === "https:";
  return [
    `${capabilityCookieName(config, submissionId)}=${token}`,
    "Path=/",
    "HttpOnly",
    "SameSite=Strict",
    `Expires=${expires.toUTCString()}`,
    ...(secure ? ["Secure"] : []),
  ].join("; ");
}

function cookieValue(request: Request, name: string): string | null {
  const header = request.headers.get("cookie");
  if (!header) return null;
  for (const part of header.split(";")) {
    const separator = part.indexOf("=");
    if (separator < 0 || part.slice(0, separator).trim() !== name) continue;
    return part.slice(separator + 1).trim();
  }
  return null;
}

export interface RequestCapabilityCredentials {
  authorizationPresent: boolean;
  headerToken: string | null;
  cookieToken: string | null;
  conflict: boolean;
  token: string | null;
}

export function requestCapabilityCredentials(
  request: Request,
  config: Pick<AppConfig, "baseUrl" | "nodeEnv">,
  submissionId: string,
): RequestCapabilityCredentials {
  const authorization = request.headers.get("authorization");
  const headerToken = authorization?.startsWith("Bearer ") ? authorization.slice(7) : null;
  const cookieToken = cookieValue(request, capabilityCookieName(config, submissionId));
  const authorizationPresent = authorization !== null;
  const conflict = (authorizationPresent && headerToken === null) ||
    (headerToken !== null && cookieToken !== null && headerToken !== cookieToken);
  return {
    authorizationPresent,
    headerToken,
    cookieToken,
    conflict,
    // An empty string is deliberately an invalid credential. It prevents a
    // conflicting header/cookie pair from falling through to a body token.
    token: conflict ? "" : headerToken ?? cookieToken,
  };
}

export function requestCapabilityToken(
  request: Request,
  config: Pick<AppConfig, "baseUrl" | "nodeEnv">,
  submissionId: string,
): string | null {
  return requestCapabilityCredentials(request, config, submissionId).token;
}

export function capabilityHandoffUrl(
  config: Pick<AppConfig, "baseUrl">,
  pathname: "/status" | "/c",
  submissionId: string,
  token: string,
): string {
  assertSubmissionId(submissionId);
  if (!isCapabilityTokenShape(token)) throw new Error("Cannot create a handoff URL for a malformed capability");
  return `${config.baseUrl}${pathname}/${submissionId}#capability=${encodeURIComponent(token)}`;
}

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
  const capsuleUrl = capabilityHandoffUrl(config, "/c", submission.id, capabilityToken);
  const agentRoute = `${config.baseUrl}/c/${submission.id}/MASTER_PROMPT.md`;
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

Browser handoff: ${capsuleUrl}

For a non-browser coding agent, retrieve \`${agentRoute}\` with the same bearer
from the browser handoff fragment in the \`Authorization: Bearer <capability-token>\`
header. Never put the bearer in a request path or query string.

**READY in this repository means this capsule and source contract are available. It does not mean that a native application has already been generated.**

## Exact operator instructions

${instructions}

> Never create paid infrastructure, spend money, alter DNS, submit to application stores, or rotate production credentials without explicit owner approval.

## Original MASTER_PROMPT.md

${masterPrompt}
`;
}
