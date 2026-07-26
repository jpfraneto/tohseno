import { createHash } from "node:crypto";
import type {
  PublicShotRecord,
  Sha256Hash,
  SignedPublicShotRecord,
} from "./types.ts";

const SIGNING_DOMAIN = new TextEncoder().encode(
  "TOHSENO-PUBLIC-SHOT-RECORD-V1\0",
);
const LONE_SURROGATE =
  /(?:[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?:^|[^\uD800-\uDBFF])[\uDC00-\uDFFF])/u;

export class CanonicalJsonError extends Error {
  override readonly name = "CanonicalJsonError";
}

function stringJson(value: string, path: string): string {
  if (LONE_SURROGATE.test(value)) {
    throw new CanonicalJsonError(`${path} contains a lone surrogate`);
  }
  return JSON.stringify(value);
}

function serialize(
  value: unknown,
  path: string,
  ancestors: Set<object>,
): string {
  if (value === null) return "null";
  if (typeof value === "string") return stringJson(value, path);
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || Object.is(value, -0)) {
      throw new CanonicalJsonError(
        `${path} must contain only safe, non-negative-zero JSON integers`,
      );
    }
    return String(value);
  }
  if (
    typeof value === "undefined" ||
    typeof value === "bigint" ||
    typeof value === "symbol" ||
    typeof value === "function"
  ) {
    throw new CanonicalJsonError(`${path} is outside the JSON data model`);
  }
  if (ancestors.has(value)) {
    throw new CanonicalJsonError(`${path} contains a cycle`);
  }
  ancestors.add(value);
  try {
    if (Array.isArray(value)) {
      const parts: string[] = [];
      for (let index = 0; index < value.length; index += 1) {
        const descriptor = Object.getOwnPropertyDescriptor(
          value,
          String(index),
        );
        if (descriptor === undefined) {
          throw new CanonicalJsonError(`${path}[${index}] is a sparse item`);
        }
        if (!descriptor.enumerable || !("value" in descriptor)) {
          throw new CanonicalJsonError(
            `${path}[${index}] must be an enumerable data property`,
          );
        }
        parts.push(
          serialize(descriptor.value, `${path}[${index}]`, ancestors),
        );
      }
      const ownNames = Object.getOwnPropertyNames(value);
      if (
        ownNames.some((name) => {
          if (name === "length") return false;
          const index = Number(name);
          return !Number.isSafeInteger(index) ||
            index < 0 ||
            index >= value.length ||
            String(index) !== name;
        }) ||
        Object.getOwnPropertySymbols(value).length > 0
      ) {
        throw new CanonicalJsonError(`${path} has non-JSON array properties`);
      }
      return `[${parts.join(",")}]`;
    }
    if (
      Object.getPrototypeOf(value) !== Object.prototype &&
      Object.getPrototypeOf(value) !== null
    ) {
      throw new CanonicalJsonError(`${path} must contain plain objects only`);
    }
    if (Object.getOwnPropertySymbols(value).length > 0) {
      throw new CanonicalJsonError(`${path} contains a symbol key`);
    }
    const candidate = value as Record<string, unknown>;
    const keys = Object.getOwnPropertyNames(candidate).sort();
    const parts: string[] = [];
    for (const key of keys) {
      stringJson(key, `${path} key`);
      const descriptor = Object.getOwnPropertyDescriptor(candidate, key);
      if (
        descriptor === undefined ||
        !descriptor.enumerable ||
        !("value" in descriptor)
      ) {
        throw new CanonicalJsonError(
          `${path}.${key} must be an enumerable data property`,
        );
      }
      parts.push(
        `${JSON.stringify(key)}:${serialize(
          descriptor.value,
          `${path}.${key}`,
          ancestors,
        )}`,
      );
    }
    return `{${parts.join(",")}}`;
  } finally {
    ancestors.delete(value);
  }
}

export function canonicalJson(value: unknown): string {
  return serialize(value, "$", new Set());
}

export function canonicalBytes(value: unknown): Uint8Array {
  return new TextEncoder().encode(canonicalJson(value));
}

export function publicShotRecordSigningBytes(
  record: PublicShotRecord,
): Uint8Array {
  return Buffer.concat([SIGNING_DOMAIN, canonicalBytes(record)]);
}

export function hashSignedPublicShotRecord(
  record: SignedPublicShotRecord,
): Sha256Hash {
  const digest = createHash("sha256")
    .update(canonicalBytes(record))
    .digest("hex");
  return `sha256:${digest}`;
}
