import { createHash, randomBytes } from "node:crypto";
import {
  identityReferencesEqual,
  type VerificationMethod,
  validateIdentityReference,
  validateVerificationMethod,
  verificationMethodsEqual,
} from "../../identity/src/index.ts";
import {
  type SignatureEnvelope,
  type SignatureVerifier,
  type Signer,
  validateSignatureEnvelope,
} from "../../signer/src/index.ts";
import { canonicalJson, publicShotRecordSigningBytes } from "./canonical.ts";
import {
  PUBLIC_SHOT_PROTOCOL_VERSION,
  PUBLIC_SHOT_RECORD_KINDS,
  type AppcoinLink,
  type AppcoinLinkedBody,
  type AppStoreEvidence,
  type CanonicalTimestamp,
  type TohsenoDownloadEvidence,
  type EvolutionRecordedBody,
  type LifecycleTransitionedBody,
  type PublicationEvidence,
  type PublicShotRecord,
  type PublicSourcePointer,
  type Sha256Hash,
  type ShotCreatedBody,
  type ShotId,
  type SignedPublicShotRecord,
  type VerifiedSignedPublicShotRecord,
} from "./types.ts";

export type ProtocolErrorCode =
  | "invalid-record"
  | "invalid-signature"
  | "authority-mismatch";

export class ProtocolValidationError extends Error {
  override readonly name = "ProtocolValidationError";

  constructor(
    readonly code: ProtocolErrorCode,
    readonly path: string,
    message: string,
  ) {
    super(`${path}: ${message}`);
  }
}

const SHOT_ID_PATTERN = /^shot_[A-Za-z0-9_-]{32}$/u;
const HASH_PATTERN = /^sha256:[a-f0-9]{64}$/u;
const TIMESTAMP_PATTERN =
  /^\d{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\d|3[01])T(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\d\.\d{3}Z$/u;
const PLATFORM_PATTERN = /^[A-Za-z][A-Za-z0-9._-]{0,31}$/u;
const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f]/u;

function objectAt(value: unknown, path: string): Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    (Object.getPrototypeOf(value) !== Object.prototype &&
      Object.getPrototypeOf(value) !== null)
  ) {
    throw new ProtocolValidationError(
      "invalid-record",
      path,
      "must be an object",
    );
  }
  return value as Record<string, unknown>;
}

function closed(
  candidate: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[],
  path: string,
): void {
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(candidate)) {
    if (!allowed.has(key)) {
      throw new ProtocolValidationError(
        "invalid-record",
        `${path}.${key}`,
        "is not allowed",
      );
    }
  }
  for (const key of required) {
    if (!Object.hasOwn(candidate, key)) {
      throw new ProtocolValidationError(
        "invalid-record",
        `${path}.${key}`,
        "is required",
      );
    }
  }
}

function safeString(
  value: unknown,
  path: string,
  maximum: number,
): string {
  if (
    typeof value !== "string" ||
    value.length < 1 ||
    value.length > maximum ||
    value !== value.trim() ||
    CONTROL_CHARACTERS.test(value)
  ) {
    throw new ProtocolValidationError(
      "invalid-record",
      path,
      `must be a trimmed public string from 1 to ${maximum} characters`,
    );
  }
  canonicalJson(value);
  return value;
}

function nonNegativeInteger(value: unknown, path: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    throw new ProtocolValidationError(
      "invalid-record",
      path,
      "must be a non-negative safe integer",
    );
  }
  return value as number;
}

function positiveInteger(value: unknown, path: string): number {
  const parsed = nonNegativeInteger(value, path);
  if (parsed === 0) {
    throw new ProtocolValidationError(
      "invalid-record",
      path,
      "must be at least 1",
    );
  }
  return parsed;
}

export function validateShotId(value: unknown, path = "shotId"): ShotId {
  if (typeof value !== "string" || !SHOT_ID_PATTERN.test(value)) {
    throw new ProtocolValidationError(
      "invalid-record",
      path,
      "must be a canonical Shot ID",
    );
  }
  return value as ShotId;
}

export function isShotId(value: unknown): value is ShotId {
  try {
    validateShotId(value);
    return true;
  } catch {
    return false;
  }
}

export function createShotId(): ShotId {
  return `shot_${randomBytes(24).toString("base64url")}`;
}

/**
 * Derives a stable Shot ID for public fixtures without weakening production
 * Shot ID randomness.
 */
export function deriveDeterministicTestShotId(
  seed: string | Uint8Array,
  namespace = "default",
): ShotId {
  if (
    namespace.length < 1 ||
    namespace.length > 100 ||
    CONTROL_CHARACTERS.test(namespace)
  ) {
    throw new ProtocolValidationError(
      "invalid-record",
      "namespace",
      "has an invalid format",
    );
  }
  const seedBytes = typeof seed === "string"
    ? new TextEncoder().encode(seed)
    : seed;
  const digest = createHash("sha256")
    .update("TOHSENO-DETERMINISTIC-TEST-SHOT-ID-V1\0", "utf8")
    .update(namespace, "utf8")
    .update("\0", "utf8")
    .update(seedBytes)
    .digest()
    .subarray(0, 24);
  return validateShotId(`shot_${digest.toString("base64url")}`);
}

export function validateSha256Hash(
  value: unknown,
  path = "hash",
): Sha256Hash {
  if (typeof value !== "string" || !HASH_PATTERN.test(value)) {
    throw new ProtocolValidationError(
      "invalid-record",
      path,
      "must be sha256 followed by 64 lowercase hexadecimal characters",
    );
  }
  return value as Sha256Hash;
}

export function validateCanonicalTimestamp(
  value: unknown,
  path = "recordedAt",
): CanonicalTimestamp {
  if (
    typeof value !== "string" ||
    !TIMESTAMP_PATTERN.test(value) ||
    !Number.isFinite(Date.parse(value)) ||
    new Date(value).toISOString() !== value
  ) {
    throw new ProtocolValidationError(
      "invalid-record",
      path,
      "must be an exact RFC 3339 UTC timestamp with millisecond precision",
    );
  }
  return value;
}

function httpsUrl(value: unknown, path: string): string {
  const source = safeString(value, path, 2_048);
  let parsed: URL;
  try {
    parsed = new URL(source);
  } catch {
    throw new ProtocolValidationError(
      "invalid-record",
      path,
      "must be an absolute HTTPS URL",
    );
  }
  if (
    parsed.protocol !== "https:" ||
    parsed.username !== "" ||
    parsed.password !== ""
  ) {
    throw new ProtocolValidationError(
      "invalid-record",
      path,
      "must be an absolute HTTPS URL without credentials",
    );
  }
  return source;
}

function publicSource(
  value: unknown,
  path: string,
): PublicSourcePointer {
  const candidate = objectAt(value, path);
  closed(candidate, ["url", "revision"], [], path);
  return {
    url: httpsUrl(candidate.url, `${path}.url`),
    revision: safeString(candidate.revision, `${path}.revision`, 200),
  };
}

function downloadEvidence(
  value: unknown,
  path: string,
): TohsenoDownloadEvidence {
  const candidate = objectAt(value, path);
  closed(
    candidate,
    ["url", "artifactDigest", "manifestDigest"],
    [],
    path,
  );
  return {
    url: httpsUrl(candidate.url, `${path}.url`),
    artifactDigest: validateSha256Hash(
      candidate.artifactDigest,
      `${path}.artifactDigest`,
    ),
    manifestDigest: validateSha256Hash(
      candidate.manifestDigest,
      `${path}.manifestDigest`,
    ),
  };
}

export function validatePublicationEvidence(
  value: unknown,
  path = "publication",
): PublicationEvidence {
  const candidate = objectAt(value, path);
  closed(candidate, ["source", "download"], [], path);
  return {
    source: publicSource(candidate.source, `${path}.source`),
    download: downloadEvidence(candidate.download, `${path}.download`),
  };
}

export function validateAppStoreEvidence(
  value: unknown,
  path = "appStore",
): AppStoreEvidence {
  const candidate = objectAt(value, path);
  closed(candidate, ["listingId", "listingUrl"], [], path);
  if (
    typeof candidate.listingId !== "string" ||
    !/^\d{5,20}$/u.test(candidate.listingId)
  ) {
    throw new ProtocolValidationError(
      "invalid-record",
      `${path}.listingId`,
      "must be an Apple numeric listing ID",
    );
  }
  const listingUrl = httpsUrl(candidate.listingUrl, `${path}.listingUrl`);
  const parsed = new URL(listingUrl);
  if (
    parsed.hostname !== "apps.apple.com" ||
    !parsed.pathname.split("/").includes(`id${candidate.listingId}`)
  ) {
    throw new ProtocolValidationError(
      "invalid-record",
      `${path}.listingUrl`,
      "must be the matching apps.apple.com listing URL",
    );
  }
  return { listingId: candidate.listingId, listingUrl };
}

function externalIdentifier(
  value: unknown,
  path: string,
  allowUrl: boolean,
): { namespace: string; id: string; url?: string } {
  const candidate = objectAt(value, path);
  closed(
    candidate,
    ["namespace", "id"],
    allowUrl ? ["url"] : [],
    path,
  );
  const result: { namespace: string; id: string; url?: string } = {
    namespace: safeString(candidate.namespace, `${path}.namespace`, 100),
    id: safeString(candidate.id, `${path}.id`, 300),
  };
  if (allowUrl && candidate.url !== undefined) {
    result.url = httpsUrl(candidate.url, `${path}.url`);
  }
  return result;
}

export function validateAppcoinLink(
  value: unknown,
  path = "link",
): AppcoinLink {
  const candidate = objectAt(value, path);
  closed(
    candidate,
    ["deployment", "network", "asset", "evidence"],
    [],
    path,
  );
  return {
    deployment: externalIdentifier(
      candidate.deployment,
      `${path}.deployment`,
      false,
    ),
    network: externalIdentifier(candidate.network, `${path}.network`, false),
    asset: externalIdentifier(candidate.asset, `${path}.asset`, false),
    evidence: externalIdentifier(
      candidate.evidence,
      `${path}.evidence`,
      true,
    ),
  };
}

function createdBody(
  value: unknown,
  authority: VerificationMethod,
  path: string,
): ShotCreatedBody {
  const candidate = objectAt(value, path);
  closed(
    candidate,
    ["name", "summary", "platform", "builder", "lifecycle", "evolution"],
    ["continuity"],
    path,
  );
  const builder = validateIdentityReference(candidate.builder, `${path}.builder`);
  if (
    builder.role !== "BUILDER" ||
    !identityReferencesEqual(builder, authority.identity)
  ) {
    throw new ProtocolValidationError(
      "authority-mismatch",
      `${path}.builder`,
      "must equal the Builder authority identity",
    );
  }
  if (candidate.lifecycle !== "EVOLVING" || candidate.evolution !== 0) {
    throw new ProtocolValidationError(
      "invalid-record",
      path,
      "a created Shot must start at EVOLVING evolution 0",
    );
  }
  const body: ShotCreatedBody = {
    name: safeString(candidate.name, `${path}.name`, 120),
    summary: safeString(candidate.summary, `${path}.summary`, 1_000),
    platform: safeString(candidate.platform, `${path}.platform`, 32),
    builder,
    lifecycle: "EVOLVING",
    evolution: 0,
  };
  if (!PLATFORM_PATTERN.test(body.platform)) {
    throw new ProtocolValidationError(
      "invalid-record",
      `${path}.platform`,
      "has an invalid platform identifier",
    );
  }
  if (candidate.continuity !== undefined) {
    const continuity = validateIdentityReference(
      candidate.continuity,
      `${path}.continuity`,
    );
    if (
      continuity.role !== "CONTINUITY" ||
      (continuity.method === builder.method && continuity.id === builder.id)
    ) {
      throw new ProtocolValidationError(
        "invalid-record",
        `${path}.continuity`,
        "must be a distinct Continuity identity",
      );
    }
    body.continuity = continuity;
  }
  return body;
}

function evolutionBody(value: unknown, path: string): EvolutionRecordedBody {
  const candidate = objectAt(value, path);
  closed(candidate, ["evolution", "title", "summary"], [], path);
  return {
    evolution: positiveInteger(candidate.evolution, `${path}.evolution`),
    title: safeString(candidate.title, `${path}.title`, 160),
    summary: safeString(candidate.summary, `${path}.summary`, 1_000),
  };
}

function transitionBody(
  value: unknown,
  path: string,
): LifecycleTransitionedBody {
  const candidate = objectAt(value, path);
  closed(candidate, ["from", "to", "evidence"], [], path);
  if (candidate.from === "EVOLVING" && candidate.to === "PUBLISHED") {
    return {
      from: "EVOLVING",
      to: "PUBLISHED",
      evidence: validatePublicationEvidence(
        candidate.evidence,
        `${path}.evidence`,
      ),
    };
  }
  if (candidate.from === "PUBLISHED" && candidate.to === "APP_STORE") {
    return {
      from: "PUBLISHED",
      to: "APP_STORE",
      evidence: validateAppStoreEvidence(
        candidate.evidence,
        `${path}.evidence`,
      ),
    };
  }
  throw new ProtocolValidationError(
    "invalid-record",
    path,
    "must be EVOLVING to PUBLISHED or PUBLISHED to APP_STORE",
  );
}

function appcoinBody(value: unknown, path: string): AppcoinLinkedBody {
  const candidate = objectAt(value, path);
  closed(candidate, ["link"], [], path);
  return { link: validateAppcoinLink(candidate.link, `${path}.link`) };
}

function commonRecord(
  candidate: Record<string, unknown>,
  path: string,
): {
  shotId: ShotId;
  sequence: number;
  previousRecordHash: Sha256Hash | null;
  recordedAt: CanonicalTimestamp;
  authority: VerificationMethod;
} {
  if (candidate.protocolVersion !== PUBLIC_SHOT_PROTOCOL_VERSION) {
    throw new ProtocolValidationError(
      "invalid-record",
      `${path}.protocolVersion`,
      `must be ${PUBLIC_SHOT_PROTOCOL_VERSION}`,
    );
  }
  if (
    typeof candidate.kind !== "string" ||
    !PUBLIC_SHOT_RECORD_KINDS.includes(
      candidate.kind as (typeof PUBLIC_SHOT_RECORD_KINDS)[number],
    )
  ) {
    throw new ProtocolValidationError(
      "invalid-record",
      `${path}.kind`,
      "is unsupported",
    );
  }
  const authority = validateVerificationMethod(
    candidate.authority,
    `${path}.authority`,
  );
  if (authority.identity.role !== "BUILDER") {
    throw new ProtocolValidationError(
      "invalid-record",
      `${path}.authority.identity.role`,
      "must be BUILDER",
    );
  }
  return {
    shotId: validateShotId(candidate.shotId, `${path}.shotId`),
    sequence: nonNegativeInteger(candidate.sequence, `${path}.sequence`),
    previousRecordHash: candidate.previousRecordHash === null
      ? null
      : validateSha256Hash(
        candidate.previousRecordHash,
        `${path}.previousRecordHash`,
      ),
    recordedAt: validateCanonicalTimestamp(
      candidate.recordedAt,
      `${path}.recordedAt`,
    ),
    authority,
  };
}

export function validatePublicShotRecord(
  value: unknown,
  path = "record",
): PublicShotRecord {
  const candidate = objectAt(value, path);
  closed(
    candidate,
    [
      "protocolVersion",
      "kind",
      "shotId",
      "sequence",
      "previousRecordHash",
      "recordedAt",
      "authority",
      "body",
    ],
    [],
    path,
  );
  const common = commonRecord(candidate, path);
  switch (candidate.kind) {
    case "SHOT_CREATED": {
      if (common.sequence !== 0 || common.previousRecordHash !== null) {
        throw new ProtocolValidationError(
          "invalid-record",
          path,
          "SHOT_CREATED must be sequence 0 with a null previous hash",
        );
      }
      return {
        protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
        kind: "SHOT_CREATED",
        ...common,
        body: createdBody(candidate.body, common.authority, `${path}.body`),
      };
    }
    case "EVOLUTION_RECORDED":
      return {
        protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
        kind: "EVOLUTION_RECORDED",
        ...common,
        body: evolutionBody(candidate.body, `${path}.body`),
      };
    case "LIFECYCLE_TRANSITIONED":
      return {
        protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
        kind: "LIFECYCLE_TRANSITIONED",
        ...common,
        body: transitionBody(candidate.body, `${path}.body`),
      };
    case "APPCOIN_LINKED":
      return {
        protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
        kind: "APPCOIN_LINKED",
        ...common,
        body: appcoinBody(candidate.body, `${path}.body`),
      };
    default:
      throw new ProtocolValidationError(
        "invalid-record",
        `${path}.kind`,
        "is unsupported",
      );
  }
}

export function validateSignedPublicShotRecord(
  value: unknown,
  path = "record",
): SignedPublicShotRecord {
  const candidate = objectAt(value, path);
  closed(
    candidate,
    [
      "protocolVersion",
      "kind",
      "shotId",
      "sequence",
      "previousRecordHash",
      "recordedAt",
      "authority",
      "body",
      "signature",
    ],
    [],
    path,
  );
  const {
    signature: signatureValue,
    ...unsignedValue
  } = candidate;
  const record = validatePublicShotRecord(unsignedValue, path);
  let signature: SignatureEnvelope;
  try {
    signature = validateSignatureEnvelope(
      signatureValue,
      `${path}.signature`,
    );
  } catch (error) {
    throw new ProtocolValidationError(
      "invalid-record",
      `${path}.signature`,
      error instanceof Error ? error.message : "is invalid",
    );
  }
  if (
    !verificationMethodsEqual(record.authority, {
      identity: signature.identity,
      suite: signature.suite,
      keyId: signature.keyId,
      publicKey: signature.publicKey,
    })
  ) {
    throw new ProtocolValidationError(
      "authority-mismatch",
      `${path}.signature`,
      "must use the record authority verification method",
    );
  }
  return { ...record, signature };
}

export function parseSignedPublicShotRecord(
  value: unknown,
): SignedPublicShotRecord {
  return validateSignedPublicShotRecord(value);
}

export async function signPublicShotRecord(
  value: PublicShotRecord,
  signer: Signer,
): Promise<SignedPublicShotRecord> {
  const record = validatePublicShotRecord(value);
  if (!verificationMethodsEqual(record.authority, signer.verificationMethod)) {
    throw new ProtocolValidationError(
      "authority-mismatch",
      "record.authority",
      "must equal the signer's verification method",
    );
  }
  const signature = await signer.sign(publicShotRecordSigningBytes(record));
  return validateSignedPublicShotRecord({ ...record, signature });
}

export async function verifySignedPublicShotRecord(
  value: unknown,
  verifier: SignatureVerifier,
): Promise<VerifiedSignedPublicShotRecord> {
  const record = validateSignedPublicShotRecord(value);
  const { signature, ...unsigned } = record;
  if (
    !(await verifier.verify(
      publicShotRecordSigningBytes(unsigned as PublicShotRecord),
      signature,
    ))
  ) {
    throw new ProtocolValidationError(
      "invalid-signature",
      "record.signature",
      "could not be verified",
    );
  }
  return record as VerifiedSignedPublicShotRecord;
}
