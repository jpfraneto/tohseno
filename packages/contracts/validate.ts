import { decodeBase64, verifySha256Digest } from "./canonical";
import type {
  ContractKind,
  ContractValidationIssue,
  ContractValidationResult,
  ContinuityArtifact,
  SignedRequestEnvelopeV1,
} from "./types";

type UnknownRecord = Record<string, unknown>;

const EVENT_ID = /^evt_[A-Za-z0-9_-]{16,}$/;
const ARTIFACT_ID = /^art_[A-Za-z0-9_-]{16,}$/;
const REFLECTION_ID = /^ref_[A-Za-z0-9_-]{16,}$/;
const PROOF_ID = /^prf_[A-Za-z0-9_-]{16,}$/;
const PRACTICE_ID = /^practice_[A-Za-z0-9_-]{8,}$/;
const CONTRACT_ID = /^[a-z0-9][a-z0-9._-]*$/;
const MEDIA_TYPE = /^[a-z0-9.+-]+\/[a-z0-9.+-]+$/;
const SHA256 = /^[a-f0-9]{64}$/;
const BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
const BASE64URL = /^[A-Za-z0-9_-]+$/;
const HTTP_METHOD = /^[A-Z]{3,16}$/;
const ORIGIN_PATH = /^\/(?:[A-Za-z0-9._~!$&'()*+,;=:@%/-]*)$/;
const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$/;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOwn(value: UnknownRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function issue(
  issues: ContractValidationIssue[],
  path: string,
  code: string,
  message: string,
): void {
  issues.push({ path, code, message });
}

function child(parent: string, key: string | number): string {
  return typeof key === "number" ? `${parent}[${key}]` : `${parent}.${key}`;
}

function record(
  value: unknown,
  path: string,
  issues: ContractValidationIssue[],
): UnknownRecord | undefined {
  if (!isRecord(value)) {
    issue(issues, path, "type.object", "must be an object");
    return undefined;
  }
  return value;
}

function shape(
  value: UnknownRecord,
  path: string,
  required: readonly string[],
  allowed: readonly string[],
  issues: ContractValidationIssue[],
): void {
  const allowedSet = new Set(allowed);
  for (const key of required) {
    if (!hasOwn(value, key)) {
      issue(issues, child(path, key), "required", "is required");
    }
  }
  for (const key of Object.keys(value)) {
    if (!allowedSet.has(key)) {
      issue(issues, child(path, key), "additional-property", "is not allowed");
    }
  }
}

function text(
  value: unknown,
  path: string,
  issues: ContractValidationIssue[],
  pattern?: RegExp,
  minimum = 1,
  maximum = 4096,
): string | undefined {
  if (typeof value !== "string") {
    issue(issues, path, "type.string", "must be a string");
    return undefined;
  }
  if (value.length < minimum || (minimum > 0 && !/\S/u.test(value))) {
    issue(issues, path, "string.too-short", `must contain at least ${minimum} useful characters`);
  }
  if (value.length > maximum) {
    issue(issues, path, "string.too-long", `must contain at most ${maximum} characters`);
  }
  if (pattern !== undefined && !pattern.test(value)) {
    issue(issues, path, "string.pattern", "has an invalid format");
  }
  return value;
}

function timestamp(
  value: unknown,
  path: string,
  issues: ContractValidationIssue[],
): number | undefined {
  const string = text(value, path, issues, TIMESTAMP, 20, 40);
  if (string === undefined) return undefined;
  const milliseconds = Date.parse(string);
  if (!Number.isFinite(milliseconds)) {
    issue(issues, path, "timestamp.invalid", "must be a real RFC 3339 UTC timestamp");
    return undefined;
  }
  return milliseconds;
}

function literal(
  value: unknown,
  path: string,
  expected: string | boolean,
  issues: ContractValidationIssue[],
): void {
  if (value !== expected) {
    issue(issues, path, "const", `must equal ${String(expected)}`);
  }
}

function oneOf(
  value: unknown,
  path: string,
  values: readonly string[],
  issues: ContractValidationIssue[],
): string | undefined {
  if (typeof value !== "string" || !values.includes(value)) {
    issue(issues, path, "enum", `must be one of: ${values.join(", ")}`);
    return undefined;
  }
  return value;
}

function nonNegativeInteger(
  value: unknown,
  path: string,
  issues: ContractValidationIssue[],
): number | undefined {
  if (!Number.isInteger(value) || (value as number) < 0) {
    issue(issues, path, "type.non-negative-integer", "must be a non-negative integer");
    return undefined;
  }
  return value as number;
}

function validateVersionedRoot(
  value: UnknownRecord,
  issues: ContractValidationIssue[],
): void {
  literal(value.schemaVersion, "$.schemaVersion", "0.1.0", issues);
}

function validateContractVersion(
  value: unknown,
  path: string,
  issues: ContractValidationIssue[],
): void {
  const object = record(value, path, issues);
  if (object === undefined) return;
  shape(object, path, ["id", "version"], ["id", "version"], issues);
  text(object.id, `${path}.id`, issues, CONTRACT_ID, 3, 120);
  text(object.version, `${path}.version`, issues, /^[A-Za-z0-9][A-Za-z0-9._-]*$/, 1, 40);
}

function validateDigest(
  value: unknown,
  path: string,
  issues: ContractValidationIssue[],
): value is { algorithm: "sha-256"; value: string } {
  const object = record(value, path, issues);
  if (object === undefined) return false;
  shape(object, path, ["algorithm", "value"], ["algorithm", "value"], issues);
  literal(object.algorithm, `${path}.algorithm`, "sha-256", issues);
  const digest = text(object.value, `${path}.value`, issues, SHA256, 64, 64);
  return object.algorithm === "sha-256" && digest !== undefined && SHA256.test(digest);
}

function validateSigner(
  value: unknown,
  path: string,
  issues: ContractValidationIssue[],
): void {
  const object = record(value, path, issues);
  if (object === undefined) return;
  shape(
    object,
    path,
    ["suite", "keyId", "publicKey"],
    ["suite", "keyId", "publicKey"],
    issues,
  );
  text(object.suite, `${path}.suite`, issues, /^[A-Za-z0-9._+-]+$/, 2, 120);
  text(object.keyId, `${path}.keyId`, issues, /^[^\r\n]+$/, 1, 4096);
  text(object.publicKey, `${path}.publicKey`, issues, /^[^\r\n]+$/, 1, 4096);
}

export function validateContinuityEvent(
  input: unknown,
): ContractValidationResult {
  const issues: ContractValidationIssue[] = [];
  const object = record(input, "$", issues);
  if (object === undefined) return { valid: false, issues };
  const keys = [
    "schemaVersion",
    "eventId",
    "applicationId",
    "practiceContextId",
    "actionPolicy",
    "lifecycle",
    "completion",
    "artifactRefs",
    "createdAt",
  ] as const;
  shape(object, "$", keys, keys, issues);
  validateVersionedRoot(object, issues);
  text(object.eventId, "$.eventId", issues, EVENT_ID, 20, 200);
  text(object.applicationId, "$.applicationId", issues, CONTRACT_ID, 3, 120);
  text(object.practiceContextId, "$.practiceContextId", issues, PRACTICE_ID, 17, 200);
  validateContractVersion(object.actionPolicy, "$.actionPolicy", issues);

  const lifecycle = record(object.lifecycle, "$.lifecycle", issues);
  let startedAt: number | undefined;
  let endedAt: number | undefined;
  let sealedAt: number | undefined;
  if (lifecycle !== undefined) {
    shape(
      lifecycle,
      "$.lifecycle",
      ["startedAt", "endedAt", "sealedAt"],
      ["startedAt", "endedAt", "sealedAt"],
      issues,
    );
    startedAt = timestamp(lifecycle.startedAt, "$.lifecycle.startedAt", issues);
    endedAt = timestamp(lifecycle.endedAt, "$.lifecycle.endedAt", issues);
    sealedAt = timestamp(lifecycle.sealedAt, "$.lifecycle.sealedAt", issues);
  }
  const createdAt = timestamp(object.createdAt, "$.createdAt", issues);
  if (startedAt !== undefined && endedAt !== undefined && endedAt < startedAt) {
    issue(issues, "$.lifecycle.endedAt", "timing.order", "must not precede startedAt");
  }
  if (endedAt !== undefined && sealedAt !== undefined && sealedAt < endedAt) {
    issue(issues, "$.lifecycle.sealedAt", "timing.order", "must not precede endedAt");
  }
  if (sealedAt !== undefined && createdAt !== undefined && createdAt < sealedAt) {
    issue(issues, "$.createdAt", "timing.order", "must not precede sealedAt");
  }

  const completion = record(object.completion, "$.completion", issues);
  if (completion !== undefined) {
    shape(
      completion,
      "$.completion",
      ["state", "conditionId", "reason"],
      ["state", "conditionId", "reason"],
      issues,
    );
    oneOf(completion.state, "$.completion.state", ["completed", "interrupted"], issues);
    text(completion.conditionId, "$.completion.conditionId", issues, CONTRACT_ID, 3, 120);
    text(completion.reason, "$.completion.reason", issues, undefined, 1, 500);
  }

  if (!Array.isArray(object.artifactRefs)) {
    issue(issues, "$.artifactRefs", "type.array", "must be an array");
  } else {
    if (object.artifactRefs.length < 1 || object.artifactRefs.length > 32) {
      issue(issues, "$.artifactRefs", "array.range", "must contain between 1 and 32 references");
    }
    const ids = new Set<string>();
    let primaryCount = 0;
    object.artifactRefs.forEach((entry, index) => {
      const path = child("$.artifactRefs", index);
      const artifactRef = record(entry, path, issues);
      if (artifactRef === undefined) return;
      shape(artifactRef, path, ["artifactId", "relation"], ["artifactId", "relation"], issues);
      const id = text(artifactRef.artifactId, `${path}.artifactId`, issues, ARTIFACT_ID, 20, 200);
      if (id !== undefined) {
        if (ids.has(id)) issue(issues, `${path}.artifactId`, "artifact-ref.duplicate", "must be unique");
        ids.add(id);
      }
      const relation = oneOf(
        artifactRef.relation,
        `${path}.relation`,
        ["primary", "attachment", "derived"],
        issues,
      );
      if (relation === "primary") primaryCount += 1;
    });
    if (primaryCount < 1) {
      issue(issues, "$.artifactRefs", "artifact-ref.primary", "must include a primary artifact");
    }
  }

  return { valid: issues.length === 0, issues };
}

export async function validateContinuityArtifact(
  input: unknown,
): Promise<ContractValidationResult> {
  const issues: ContractValidationIssue[] = [];
  const object = record(input, "$", issues);
  if (object === undefined) return { valid: false, issues };
  const keys = [
    "schemaVersion",
    "artifactId",
    "eventId",
    "mediaType",
    "codec",
    "content",
    "digest",
    "createdAt",
    "sealedAt",
    "seal",
  ] as const;
  shape(object, "$", keys, keys, issues);
  validateVersionedRoot(object, issues);
  text(object.artifactId, "$.artifactId", issues, ARTIFACT_ID, 20, 200);
  text(object.eventId, "$.eventId", issues, EVENT_ID, 20, 200);
  text(object.mediaType, "$.mediaType", issues, MEDIA_TYPE, 3, 200);
  text(object.codec, "$.codec", issues, CONTRACT_ID, 3, 120);

  const digestValid = validateDigest(object.digest, "$.digest", issues);
  const content = record(object.content, "$.content", issues);
  if (content !== undefined) {
    const kind = oneOf(content.kind, "$.content.kind", ["embedded", "reference"], issues);
    if (kind === "embedded") {
      shape(
        content,
        "$.content",
        ["kind", "encoding", "bytes", "byteLength"],
        ["kind", "encoding", "bytes", "byteLength"],
        issues,
      );
      literal(content.encoding, "$.content.encoding", "base64", issues);
      const encoded = text(content.bytes, "$.content.bytes", issues, BASE64, 0, 22_369_624);
      const declaredLength = nonNegativeInteger(
        content.byteLength,
        "$.content.byteLength",
        issues,
      );
      if (encoded !== undefined && BASE64.test(encoded)) {
        try {
          const bytes = decodeBase64(encoded);
          if (declaredLength !== undefined && bytes.byteLength !== declaredLength) {
            issue(
              issues,
              "$.content.byteLength",
              "artifact.byte-length",
              `declares ${declaredLength} bytes but embedded content has ${bytes.byteLength}`,
            );
          }
          if (digestValid) {
            const digest = object.digest as ContinuityArtifact["digest"];
            if (!(await verifySha256Digest(bytes, digest))) {
              issue(
                issues,
                "$.digest.value",
                "artifact.digest-mismatch",
                "does not match the exact embedded bytes",
              );
            }
          }
        } catch {
          issue(issues, "$.content.bytes", "artifact.base64", "must be canonical base64");
        }
      }
    } else if (kind === "reference") {
      shape(
        content,
        "$.content",
        ["kind", "uri", "byteLength"],
        ["kind", "uri", "byteLength"],
        issues,
      );
      text(content.uri, "$.content.uri", issues, /^[a-z][a-z0-9+.-]*:/, 3, 2048);
      nonNegativeInteger(content.byteLength, "$.content.byteLength", issues);
    }
  }

  const createdAt = timestamp(object.createdAt, "$.createdAt", issues);
  const sealedAt = timestamp(object.sealedAt, "$.sealedAt", issues);
  if (createdAt !== undefined && sealedAt !== undefined && sealedAt < createdAt) {
    issue(issues, "$.sealedAt", "timing.order", "must not precede createdAt");
  }
  const seal = record(object.seal, "$.seal", issues);
  if (seal !== undefined) {
    shape(seal, "$.seal", ["sealedBy", "immutable"], ["sealedBy", "immutable"], issues);
    text(seal.sealedBy, "$.seal.sealedBy", issues, CONTRACT_ID, 3, 120);
    literal(seal.immutable, "$.seal.immutable", true, issues);
  }

  return { valid: issues.length === 0, issues };
}

export function validateContinuityReflection(
  input: unknown,
): ContractValidationResult {
  const issues: ContractValidationIssue[] = [];
  const object = record(input, "$", issues);
  if (object === undefined) return { valid: false, issues };
  const required = [
    "schemaVersion",
    "reflectionId",
    "eventId",
    "provider",
    "consent",
    "generatedAt",
    "output",
    "deletion",
  ] as const;
  shape(object, "$", required, [...required, "artifactId"], issues);
  validateVersionedRoot(object, issues);
  text(object.reflectionId, "$.reflectionId", issues, REFLECTION_ID, 20, 200);
  text(object.eventId, "$.eventId", issues, EVENT_ID, 20, 200);
  if (hasOwn(object, "artifactId")) {
    text(object.artifactId, "$.artifactId", issues, ARTIFACT_ID, 20, 200);
  }
  const provider = record(object.provider, "$.provider", issues);
  let providerKind: string | undefined;
  if (provider !== undefined) {
    shape(
      provider,
      "$.provider",
      ["kind", "id", "policyVersion"],
      ["kind", "id", "policyVersion", "model"],
      issues,
    );
    providerKind = oneOf(provider.kind, "$.provider.kind", ["local", "remote"], issues);
    text(provider.id, "$.provider.id", issues, CONTRACT_ID, 3, 120);
    text(provider.policyVersion, "$.provider.policyVersion", issues, /^[A-Za-z0-9][A-Za-z0-9._-]*$/, 1, 40);
    if (hasOwn(provider, "model")) {
      text(provider.model, "$.provider.model", issues, undefined, 1, 200);
    }
  }
  const consent = record(object.consent, "$.consent", issues);
  let consentBasis: string | undefined;
  let consentAt: number | undefined;
  if (consent !== undefined) {
    shape(
      consent,
      "$.consent",
      ["basis", "recordedAt", "disclosure"],
      ["basis", "recordedAt", "disclosure"],
      issues,
    );
    consentBasis = oneOf(
      consent.basis,
      "$.consent.basis",
      ["not-required-local", "per-event-opt-in", "standing-explicit"],
      issues,
    );
    consentAt = timestamp(consent.recordedAt, "$.consent.recordedAt", issues);
    if (!Array.isArray(consent.disclosure)) {
      issue(issues, "$.consent.disclosure", "type.array", "must be an array");
    } else {
      const disclosures: string[] = [];
      consent.disclosure.forEach((entry, index) => {
        const result = text(entry, child("$.consent.disclosure", index), issues, undefined, 1, 500);
        if (result !== undefined) disclosures.push(result);
      });
      if (new Set(disclosures).size !== disclosures.length) {
        issue(issues, "$.consent.disclosure", "array.unique", "must not contain duplicates");
      }
      if (providerKind === "remote" && disclosures.length === 0) {
        issue(
          issues,
          "$.consent.disclosure",
          "reflection.remote-disclosure",
          "must identify what a remote provider received",
        );
      }
    }
  }
  if (providerKind === "remote" && consentBasis === "not-required-local") {
    issue(
      issues,
      "$.consent.basis",
      "reflection.remote-consent",
      "a remote provider cannot use the local-only consent basis",
    );
  }
  const generatedAt = timestamp(object.generatedAt, "$.generatedAt", issues);
  if (consentAt !== undefined && generatedAt !== undefined && generatedAt < consentAt) {
    issue(issues, "$.generatedAt", "timing.order", "must not precede recorded consent");
  }
  const deletion = record(object.deletion, "$.deletion", issues);
  if (deletion !== undefined) {
    shape(
      deletion,
      "$.deletion",
      ["independentlyDeletable"],
      ["independentlyDeletable"],
      issues,
    );
    literal(
      deletion.independentlyDeletable,
      "$.deletion.independentlyDeletable",
      true,
      issues,
    );
  }
  return { valid: issues.length === 0, issues };
}

export function validateContinuityProof(
  input: unknown,
): ContractValidationResult {
  const issues: ContractValidationIssue[] = [];
  const object = record(input, "$", issues);
  if (object === undefined) return { valid: false, issues };
  const keys = [
    "schemaVersion",
    "proofId",
    "eventId",
    "proofVersion",
    "statement",
    "disclosure",
    "signer",
    "verification",
    "generatedAt",
  ] as const;
  shape(object, "$", keys, keys, issues);
  validateVersionedRoot(object, issues);
  text(object.proofId, "$.proofId", issues, PROOF_ID, 20, 200);
  text(object.eventId, "$.eventId", issues, EVENT_ID, 20, 200);
  literal(object.proofVersion, "$.proofVersion", "1", issues);

  const statement = record(object.statement, "$.statement", issues);
  if (statement !== undefined) {
    shape(
      statement,
      "$.statement",
      ["type", "text", "claims"],
      ["type", "text", "claims"],
      issues,
    );
    oneOf(
      statement.type,
      "$.statement.type",
      ["practice-key-attestation", "server-witness"],
      issues,
    );
    text(statement.text, "$.statement.text", issues, undefined, 12, 1000);
    const claims = record(statement.claims, "$.statement.claims", issues);
    if (claims !== undefined) {
      const entries = Object.entries(claims);
      if (entries.length < 1 || entries.length > 12) {
        issue(issues, "$.statement.claims", "object.size", "must contain between 1 and 12 claims");
      }
      for (const [key, value] of entries) {
        if (!["string", "number", "boolean"].includes(typeof value)) {
          issue(
            issues,
            `$.statement.claims.${key}`,
            "claim.primitive",
            "must be a string, number, or boolean",
          );
        }
      }
    }
  }

  const disclosure = record(object.disclosure, "$.disclosure", issues);
  if (disclosure !== undefined) {
    shape(
      disclosure,
      "$.disclosure",
      ["fields", "artifactContentIncluded"],
      ["fields", "artifactContentIncluded"],
      issues,
    );
    if (!Array.isArray(disclosure.fields)) {
      issue(issues, "$.disclosure.fields", "type.array", "must be an array");
    } else {
      if (disclosure.fields.length < 1 || disclosure.fields.length > 12) {
        issue(issues, "$.disclosure.fields", "array.range", "must contain between 1 and 12 fields");
      }
      disclosure.fields.forEach((field, index) => {
        text(field, child("$.disclosure.fields", index), issues, /^[A-Za-z0-9._-]+$/, 1, 120);
      });
      const stringFields = disclosure.fields.filter((field): field is string => typeof field === "string");
      if (new Set(stringFields).size !== stringFields.length) {
        issue(issues, "$.disclosure.fields", "array.unique", "must not contain duplicate fields");
      }
    }
    literal(
      disclosure.artifactContentIncluded,
      "$.disclosure.artifactContentIncluded",
      false,
      issues,
    );
  }
  validateSigner(object.signer, "$.signer", issues);
  const verification = record(object.verification, "$.verification", issues);
  if (verification !== undefined) {
    shape(
      verification,
      "$.verification",
      ["algorithm", "signature", "material"],
      ["algorithm", "signature", "material"],
      issues,
    );
    text(verification.algorithm, "$.verification.algorithm", issues, /^[A-Za-z0-9._+-]+$/, 2, 120);
    text(verification.signature, "$.verification.signature", issues, undefined, 1, 4096);
    text(verification.material, "$.verification.material", issues, undefined, 1, 4096);
  }
  timestamp(object.generatedAt, "$.generatedAt", issues);
  return { valid: issues.length === 0, issues };
}

export function validateSignedRequestEnvelopeV1(
  input: unknown,
): ContractValidationResult {
  const issues: ContractValidationIssue[] = [];
  const object = record(input, "$", issues);
  if (object === undefined) return { valid: false, issues };
  const keys = [
    "protocolVersion",
    "method",
    "path",
    "bodyHash",
    "timestamp",
    "nonce",
    "signer",
    "signature",
  ] as const;
  shape(object, "$", keys, keys, issues);
  literal(object.protocolVersion, "$.protocolVersion", "1", issues);
  text(object.method, "$.method", issues, HTTP_METHOD, 3, 16);
  text(object.path, "$.path", issues, ORIGIN_PATH, 1, 2048);
  validateDigest(object.bodyHash, "$.bodyHash", issues);
  timestamp(object.timestamp, "$.timestamp", issues);
  text(object.nonce, "$.nonce", issues, BASE64URL, 16, 128);
  validateSigner(object.signer, "$.signer", issues);
  const signature = record(object.signature, "$.signature", issues);
  if (signature !== undefined) {
    shape(
      signature,
      "$.signature",
      ["encoding", "value"],
      ["encoding", "value"],
      issues,
    );
    literal(signature.encoding, "$.signature.encoding", "base64url", issues);
    text(signature.value, "$.signature.value", issues, BASE64URL, 16, 2048);
  }
  return { valid: issues.length === 0, issues };
}

export async function validateContract(
  kind: ContractKind,
  input: unknown,
): Promise<ContractValidationResult> {
  switch (kind) {
    case "ContinuityEvent":
      return validateContinuityEvent(input);
    case "ContinuityArtifact":
      return validateContinuityArtifact(input);
    case "ContinuityReflection":
      return validateContinuityReflection(input);
    case "ContinuityProof":
      return validateContinuityProof(input);
    case "SignedRequestEnvelopeV1":
      return validateSignedRequestEnvelopeV1(input);
  }
}

export function asSignedRequestEnvelopeV1(
  input: unknown,
): SignedRequestEnvelopeV1 {
  const result = validateSignedRequestEnvelopeV1(input);
  if (!result.valid) {
    const detail = result.issues
      .map((entry) => `${entry.path} [${entry.code}]: ${entry.message}`)
      .join("\n");
    throw new TypeError(`Invalid SignedRequestEnvelopeV1:\n${detail}`);
  }
  return input as SignedRequestEnvelopeV1;
}
