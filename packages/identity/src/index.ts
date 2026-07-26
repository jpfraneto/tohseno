import { createHash } from "node:crypto";

export const IDENTITY_ROLES = ["BUILDER", "CONTINUITY"] as const;
export type IdentityRole = (typeof IDENTITY_ROLES)[number];

export interface IdentityReference {
  role: IdentityRole;
  method: string;
  id: string;
}

export interface VerificationMethod {
  identity: IdentityReference;
  suite: string;
  keyId: string;
  publicKey: string;
}

export class IdentityValidationError extends Error {
  override readonly name = "IdentityValidationError";

  constructor(
    readonly path: string,
    message: string,
  ) {
    super(`${path}: ${message}`);
  }
}

const METHOD_PATTERN = /^[a-z][a-z0-9.-]{0,63}$/u;
const IDENTIFIER_PATTERN = /^[A-Za-z0-9._:~-]{1,200}$/u;
const BASE64URL_PATTERN =
  /^(?:[A-Za-z0-9_-]{4})*(?:[A-Za-z0-9_-]{2,3})?$/u;

function objectAt(value: unknown, path: string): Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    (Object.getPrototypeOf(value) !== Object.prototype &&
      Object.getPrototypeOf(value) !== null)
  ) {
    throw new IdentityValidationError(path, "must be an object");
  }
  return value as Record<string, unknown>;
}

function closed(
  value: Record<string, unknown>,
  allowed: readonly string[],
  path: string,
): void {
  const allowedSet = new Set(allowed);
  for (const key of Object.keys(value)) {
    if (!allowedSet.has(key)) {
      throw new IdentityValidationError(`${path}.${key}`, "is not allowed");
    }
  }
  for (const key of allowed) {
    if (!Object.hasOwn(value, key)) {
      throw new IdentityValidationError(`${path}.${key}`, "is required");
    }
  }
}

function stringMatching(
  value: unknown,
  pattern: RegExp,
  path: string,
): string {
  if (typeof value !== "string" || !pattern.test(value)) {
    throw new IdentityValidationError(path, "has an invalid format");
  }
  return value;
}

function canonicalBase64url(value: unknown, path: string): string {
  const encoded = stringMatching(value, BASE64URL_PATTERN, path);
  if (
    encoded.length < 2 ||
    Buffer.from(encoded, "base64url").toString("base64url") !== encoded
  ) {
    throw new IdentityValidationError(
      path,
      "must be canonical unpadded base64url",
    );
  }
  return encoded;
}

export function validateIdentityReference(
  value: unknown,
  path = "identity",
): IdentityReference {
  const candidate = objectAt(value, path);
  closed(candidate, ["role", "method", "id"], path);
  if (
    typeof candidate.role !== "string" ||
    !IDENTITY_ROLES.includes(candidate.role as IdentityRole)
  ) {
    throw new IdentityValidationError(
      `${path}.role`,
      `must be one of ${IDENTITY_ROLES.join(", ")}`,
    );
  }
  return {
    role: candidate.role as IdentityRole,
    method: stringMatching(candidate.method, METHOD_PATTERN, `${path}.method`),
    id: stringMatching(candidate.id, IDENTIFIER_PATTERN, `${path}.id`),
  };
}

export function parseIdentityReference(value: unknown): IdentityReference {
  return validateIdentityReference(value);
}

export function isIdentityReference(
  value: unknown,
): value is IdentityReference {
  try {
    validateIdentityReference(value);
    return true;
  } catch {
    return false;
  }
}

export function validateVerificationMethod(
  value: unknown,
  path = "verificationMethod",
): VerificationMethod {
  const candidate = objectAt(value, path);
  closed(candidate, ["identity", "suite", "keyId", "publicKey"], path);
  return {
    identity: validateIdentityReference(candidate.identity, `${path}.identity`),
    suite: stringMatching(candidate.suite, METHOD_PATTERN, `${path}.suite`),
    keyId: stringMatching(candidate.keyId, IDENTIFIER_PATTERN, `${path}.keyId`),
    publicKey: canonicalBase64url(
      candidate.publicKey,
      `${path}.publicKey`,
    ),
  };
}

export function parseVerificationMethod(value: unknown): VerificationMethod {
  return validateVerificationMethod(value);
}

export function isVerificationMethod(
  value: unknown,
): value is VerificationMethod {
  try {
    validateVerificationMethod(value);
    return true;
  } catch {
    return false;
  }
}

export function identityReferencesEqual(
  left: IdentityReference,
  right: IdentityReference,
): boolean {
  return left.role === right.role &&
    left.method === right.method &&
    left.id === right.id;
}

export function verificationMethodsEqual(
  left: VerificationMethod,
  right: VerificationMethod,
): boolean {
  return identityReferencesEqual(left.identity, right.identity) &&
    left.suite === right.suite &&
    left.keyId === right.keyId &&
    left.publicKey === right.publicKey;
}

function lengthPrefix(bytes: Uint8Array): Uint8Array {
  const length = Buffer.allocUnsafe(4);
  length.writeUInt32BE(bytes.byteLength);
  return Buffer.concat([length, bytes]);
}

/**
 * Derives a stable, public test identity from non-secret fixture material.
 *
 * This is deliberately unsuitable for production identity, custody, or
 * recovery. It derives an identifier, never a signing key.
 */
export function deriveDeterministicTestIdentity(
  role: IdentityRole,
  seed: string | Uint8Array,
  namespace = "default",
): IdentityReference {
  if (!IDENTITY_ROLES.includes(role)) {
    throw new IdentityValidationError("role", "is invalid");
  }
  if (
    namespace.length < 1 ||
    namespace.length > 100 ||
    /[\u0000-\u001f\u007f]/u.test(namespace)
  ) {
    throw new IdentityValidationError("namespace", "has an invalid format");
  }
  const seedBytes = typeof seed === "string"
    ? new TextEncoder().encode(seed)
    : seed;
  const digest = createHash("sha256")
    .update("TOHSENO-LOCAL-TEST-IDENTITY-V1\0", "utf8")
    .update(lengthPrefix(new TextEncoder().encode(role)))
    .update(lengthPrefix(new TextEncoder().encode(namespace)))
    .update(lengthPrefix(seedBytes))
    .digest("base64url");
  return {
    role,
    method: "tohseno-local-test-v1",
    id: `test_${digest}`,
  };
}
