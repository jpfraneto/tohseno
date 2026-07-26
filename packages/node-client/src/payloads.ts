import {
  hashSignedPublicShotRecord,
  parseSignedPublicShotRecord,
  type Sha256Hash,
  type SignedPublicShotRecord,
  validateSha256Hash,
} from "../../protocol/src/index.ts";
import {
  type PublicShotProjection,
  type RegistryAppendResult,
  validatePublicShotProjection,
} from "../../registry/src/index.ts";

export const NODE_PAYLOAD_SCHEMA_VERSION = 1 as const;

export interface NodeSubmissionPayload {
  schemaVersion: typeof NODE_PAYLOAD_SCHEMA_VERSION;
  status: "appended" | "existing";
  recordHash: Sha256Hash;
  projection: PublicShotProjection;
}

export interface NodeRecordsPayload {
  schemaVersion: typeof NODE_PAYLOAD_SCHEMA_VERSION;
  records: SignedPublicShotRecord[];
}

export interface NodeErrorPayload {
  error: string;
}

export class NodePayloadValidationError extends Error {
  override readonly name = "NodePayloadValidationError";
}

function plainObject(
  value: unknown,
  path: string,
): Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    (Object.getPrototypeOf(value) !== Object.prototype &&
      Object.getPrototypeOf(value) !== null)
  ) {
    throw new NodePayloadValidationError(`${path} must be an object`);
  }
  return value as Record<string, unknown>;
}

function exactKeys(
  object: Record<string, unknown>,
  expected: readonly string[],
  path: string,
): void {
  const expectedSet = new Set(expected);
  for (const key of Object.keys(object)) {
    if (!expectedSet.has(key)) {
      throw new NodePayloadValidationError(`${path}.${key} is not allowed`);
    }
  }
  for (const key of expected) {
    if (!Object.hasOwn(object, key)) {
      throw new NodePayloadValidationError(`${path}.${key} is required`);
    }
  }
}

export function validateNodeProjectionPayload(
  value: unknown,
): PublicShotProjection {
  try {
    return validatePublicShotProjection(value);
  } catch {
    throw new NodePayloadValidationError(
      "node projection payload is invalid",
    );
  }
}

export function validateNodeSubmissionPayload(
  value: unknown,
): NodeSubmissionPayload {
  const object = plainObject(value, "submission");
  exactKeys(
    object,
    ["schemaVersion", "status", "recordHash", "projection"],
    "submission",
  );
  if (object.schemaVersion !== NODE_PAYLOAD_SCHEMA_VERSION) {
    throw new NodePayloadValidationError(
      "submission.schemaVersion is unsupported",
    );
  }
  if (object.status !== "appended" && object.status !== "existing") {
    throw new NodePayloadValidationError("submission.status is invalid");
  }
  try {
    return {
      schemaVersion: NODE_PAYLOAD_SCHEMA_VERSION,
      status: object.status,
      recordHash: validateSha256Hash(
        object.recordHash,
        "submission.recordHash",
      ),
      projection: validatePublicShotProjection(object.projection),
    };
  } catch (error) {
    if (error instanceof NodePayloadValidationError) throw error;
    throw new NodePayloadValidationError("submission payload is invalid");
  }
}

export function createNodeSubmissionPayload(
  result: RegistryAppendResult,
): NodeSubmissionPayload {
  return validateNodeSubmissionPayload({
    schemaVersion: NODE_PAYLOAD_SCHEMA_VERSION,
    status: result.status,
    recordHash: result.recordHash,
    projection: result.projection,
  });
}

export function validateNodeRecordsPayload(
  value: unknown,
): NodeRecordsPayload {
  const object = plainObject(value, "recordSet");
  exactKeys(object, ["schemaVersion", "records"], "recordSet");
  if (object.schemaVersion !== NODE_PAYLOAD_SCHEMA_VERSION) {
    throw new NodePayloadValidationError(
      "recordSet.schemaVersion is unsupported",
    );
  }
  if (!Array.isArray(object.records) || object.records.length === 0) {
    throw new NodePayloadValidationError(
      "recordSet.records must be a non-empty array",
    );
  }
  try {
    const records = object.records.map((record) =>
      parseSignedPublicShotRecord(record));
    const shotId = records[0]?.shotId;
    for (let index = 0; index < records.length; index += 1) {
      const record = records[index];
      if (
        record === undefined ||
        record.shotId !== shotId ||
        record.sequence !== index ||
        (index === 0 &&
          (record.kind !== "SHOT_CREATED" ||
            record.previousRecordHash !== null))
      ) {
        throw new NodePayloadValidationError(
          "recordSet.records is not one canonical Shot chain",
        );
      }
      if (index > 0) {
        const previous = records[index - 1];
        if (
          previous === undefined ||
          record.previousRecordHash !==
            hashSignedPublicShotRecord(previous)
        ) {
          throw new NodePayloadValidationError(
            "recordSet.records has an invalid hash link",
          );
        }
      }
    }
    return {
      schemaVersion: NODE_PAYLOAD_SCHEMA_VERSION,
      records,
    };
  } catch (error) {
    if (error instanceof NodePayloadValidationError) throw error;
    throw new NodePayloadValidationError("recordSet payload is invalid");
  }
}

export function createNodeRecordsPayload(
  records: readonly SignedPublicShotRecord[],
): NodeRecordsPayload {
  return validateNodeRecordsPayload({
    schemaVersion: NODE_PAYLOAD_SCHEMA_VERSION,
    records,
  });
}

export function validateNodeErrorPayload(
  value: unknown,
  expectedCode?: string,
): NodeErrorPayload {
  const object = plainObject(value, "error");
  exactKeys(object, ["error"], "error");
  if (
    typeof object.error !== "string" ||
    !/^[a-z][a-z0-9-]{0,63}$/u.test(object.error) ||
    (expectedCode !== undefined && object.error !== expectedCode)
  ) {
    throw new NodePayloadValidationError("error.error is invalid");
  }
  return { error: object.error };
}

export function createNodeErrorPayload(code: string): NodeErrorPayload {
  return validateNodeErrorPayload({ error: code });
}
