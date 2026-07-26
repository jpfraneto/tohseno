import type { Database } from "bun:sqlite";
import type { SignatureVerifier } from "../../../packages/signer/src/index.ts";
import {
  canonicalJson,
  hashSignedPublicShotRecord,
  parseSignedPublicShotRecord,
  type SignedPublicShotRecord,
  type VerifiedSignedPublicShotRecord,
  verifySignedPublicShotRecord,
} from "../../../packages/protocol/src/index.ts";
import {
  projectPublicShotProjection,
  type PublicRecordRegistry,
  type PublicShotProjection,
  RegistryError,
  type RegistryAppendResult,
  validatePublicShotProjection,
} from "../../../packages/registry/src/index.ts";

interface StoredRecordRow {
  canonical_json: string;
  record_hash: string;
  sequence: number;
}

interface StoredProjectionRow {
  projection_json: string;
}

export const MAX_REFERENCE_NODE_RECORDS_PER_SHOT = 1_024;
export const MAX_REFERENCE_NODE_HISTORY_BYTES = 3 * 1024 * 1024;

export class ReferenceNodeStorageError extends Error {
  override readonly name = "ReferenceNodeStorageError";

  constructor(message: string) {
    super(message);
  }
}

export class ReferenceNodeCapacityError extends Error {
  override readonly name = "ReferenceNodeCapacityError";

  constructor(message: string) {
    super(message);
  }
}

export interface ReferenceNodeRegistryLimits {
  maxRecordsPerShot?: number;
  maxHistoryBytes?: number;
}

function parseStoredRecord(source: string): SignedPublicShotRecord {
  try {
    return parseSignedPublicShotRecord(JSON.parse(source) as unknown);
  } catch {
    throw new ReferenceNodeStorageError(
      "the stored public record is invalid",
    );
  }
}

function parseStoredProjection(source: string): PublicShotProjection {
  try {
    return validatePublicShotProjection(JSON.parse(source) as unknown);
  } catch {
    throw new ReferenceNodeStorageError(
      "the stored public projection is invalid",
    );
  }
}

function cloneProjection(projection: PublicShotProjection): PublicShotProjection {
  return parseStoredProjection(canonicalJson(projection));
}

function isUniquenessConstraint(error: unknown): boolean {
  if (typeof error !== "object" || error === null) return false;
  const code = (error as { code?: unknown }).code;
  return code === "SQLITE_CONSTRAINT_UNIQUE" ||
    code === "SQLITE_CONSTRAINT_PRIMARYKEY";
}

/**
 * SQLite is the replaceable reference adapter. Records are accepted as
 * Builder attestations only when their protocol signature verifies; this
 * class is not an ownership or consensus authority and has no signing seam.
 */
export class SqlitePublicRecordRegistry implements PublicRecordRegistry {
  #pendingAppend: Promise<void> = Promise.resolve();
  readonly #maxRecordsPerShot: number;
  readonly #maxHistoryBytes: number;

  constructor(
    private readonly database: Database,
    private readonly verifier: SignatureVerifier,
    private readonly now: () => Date = () => new Date(),
    limits: ReferenceNodeRegistryLimits = {},
  ) {
    this.#maxRecordsPerShot = limits.maxRecordsPerShot ??
      MAX_REFERENCE_NODE_RECORDS_PER_SHOT;
    this.#maxHistoryBytes = limits.maxHistoryBytes ??
      MAX_REFERENCE_NODE_HISTORY_BYTES;
    if (
      !Number.isSafeInteger(this.#maxRecordsPerShot) ||
      this.#maxRecordsPerShot < 1 ||
      !Number.isSafeInteger(this.#maxHistoryBytes) ||
      this.#maxHistoryBytes < 1
    ) {
      throw new Error("reference node registry limits are invalid");
    }
  }

  append(
    recordValue: SignedPublicShotRecord,
  ): Promise<RegistryAppendResult> {
    const operation = this.#pendingAppend.then(() =>
      this.#appendVerified(recordValue)
    );
    this.#pendingAppend = operation.then(
      () => undefined,
      () => undefined,
    );
    return operation;
  }

  async #appendVerified(
    recordValue: SignedPublicShotRecord,
  ): Promise<RegistryAppendResult> {
    const record = await verifySignedPublicShotRecord(
      recordValue,
      this.verifier,
    );
    const recordJson = canonicalJson(record);
    const recordHash = hashSignedPublicShotRecord(record);
    const rows = this.#recordRows(record.shotId);
    const atSequence = rows.find((row) => row.sequence === record.sequence);

    if (atSequence !== undefined) {
      if (
        atSequence.record_hash === recordHash &&
        atSequence.canonical_json === recordJson
      ) {
        const projection = this.getProjection(record.shotId);
        if (projection === undefined) {
          throw new ReferenceNodeStorageError(
            "the stored public record has no projection",
          );
        }
        return {
          status: "existing",
          recordHash,
          projection,
        };
      }
      throw new RegistryError(
        "sequence-conflict",
        `sequence ${record.sequence} is already occupied`,
      );
    }

    if (record.sequence !== rows.length) {
      throw new RegistryError(
        "sequence-gap",
        `expected sequence ${rows.length}`,
      );
    }
    if (rows.length >= this.#maxRecordsPerShot) {
      throw new ReferenceNodeCapacityError(
        "the reference node record limit for this Shot is exhausted",
      );
    }
    const historyBytes = rows.reduce(
      (total, row) => total + Buffer.byteLength(row.canonical_json) + 1,
      Buffer.byteLength(recordJson) + 1,
    );
    if (historyBytes > this.#maxHistoryBytes) {
      throw new ReferenceNodeCapacityError(
        "the reference node export limit for this Shot is exhausted",
      );
    }

    const verifiedHistory: VerifiedSignedPublicShotRecord[] = [];
    for (const row of rows) {
      const stored = parseStoredRecord(row.canonical_json);
      const verified = await verifySignedPublicShotRecord(
        stored,
        this.verifier,
      ).catch(() => {
        throw new ReferenceNodeStorageError(
          "a stored public record signature is invalid",
        );
      });
      if (
        verified.sequence !== row.sequence ||
        hashSignedPublicShotRecord(verified) !== row.record_hash
      ) {
        throw new ReferenceNodeStorageError(
          "a stored public record does not match its index",
        );
      }
      verifiedHistory.push(verified);
    }

    if (verifiedHistory.length > 0) {
      try {
        projectPublicShotProjection(verifiedHistory);
      } catch {
        throw new ReferenceNodeStorageError(
          "the stored public record history is inconsistent",
        );
      }
    }
    const projection = projectPublicShotProjection([
      ...verifiedHistory,
      record,
    ]);
    const projectionJson = canonicalJson(projection);
    const acceptedAt = this.now().toISOString();

    const commit = this.database.transaction(() => {
      const occupied = this.database
        .query<StoredRecordRow, [string, number]>(
          `SELECT canonical_json, record_hash, sequence
             FROM public_records
            WHERE shot_id = ? AND sequence = ?`,
        )
        .get(record.shotId, record.sequence);
      if (occupied !== null) {
        if (
          occupied.record_hash === recordHash &&
          occupied.canonical_json === recordJson
        ) {
          return "existing" as const;
        }
        throw new RegistryError(
          "sequence-conflict",
          `sequence ${record.sequence} is already occupied`,
        );
      }

      const currentCount = this.database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM public_records WHERE shot_id = ?",
        )
        .get(record.shotId)?.count ?? 0;
      if (currentCount !== record.sequence) {
        throw new RegistryError(
          "sequence-gap",
          `expected sequence ${currentCount}`,
        );
      }

      try {
        this.database
          .query(
            `INSERT INTO public_records
              (record_hash, shot_id, sequence, record_kind, canonical_json,
               accepted_at)
             VALUES (?, ?, ?, ?, ?, ?)`,
          )
          .run(
            recordHash,
            record.shotId,
            record.sequence,
            record.kind,
            recordJson,
            acceptedAt,
          );
        this.database
          .query(
            `INSERT INTO current_projections
              (shot_id, sequence, record_hash, projection_json)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (shot_id) DO UPDATE SET
               sequence = excluded.sequence,
               record_hash = excluded.record_hash,
               projection_json = excluded.projection_json`,
          )
          .run(
            record.shotId,
            record.sequence,
            recordHash,
            projectionJson,
          );
      } catch (error) {
        if (error instanceof RegistryError) throw error;
        if (isUniquenessConstraint(error)) {
          throw new RegistryError(
            "sequence-conflict",
            "the record conflicts with an existing append",
          );
        }
        throw new ReferenceNodeStorageError(
          "the public record transaction failed",
        );
      }
      return "appended" as const;
    });

    const status = commit();
    if (status === "existing") {
      const existingProjection = this.getProjection(record.shotId);
      if (existingProjection === undefined) {
        throw new ReferenceNodeStorageError(
          "the stored public record has no projection",
        );
      }
      return { status, recordHash, projection: existingProjection };
    }
    return {
      status,
      recordHash,
      projection: cloneProjection(projection),
    };
  }

  getProjection(shotId: string): PublicShotProjection | undefined {
    const row = this.database
      .query<StoredProjectionRow, [string]>(
        "SELECT projection_json FROM current_projections WHERE shot_id = ?",
      )
      .get(shotId);
    if (row === null) return undefined;

    const projection = parseStoredProjection(row.projection_json);
    const records = this.getRecords(shotId);
    let projected: PublicShotProjection;
    try {
      projected = projectPublicShotProjection(
        records as readonly VerifiedSignedPublicShotRecord[],
      );
    } catch {
      throw new ReferenceNodeStorageError(
        "the stored public record history is inconsistent",
      );
    }
    if (canonicalJson(projection) !== canonicalJson(projected)) {
      throw new ReferenceNodeStorageError(
        "the stored public projection does not match its record history",
      );
    }
    return cloneProjection(projection);
  }

  getRecords(shotId: string): readonly SignedPublicShotRecord[] {
    return this.#recordRows(shotId).map((row) => {
      const record = parseStoredRecord(row.canonical_json);
      if (
        record.sequence !== row.sequence ||
        hashSignedPublicShotRecord(record) !== row.record_hash
      ) {
        throw new ReferenceNodeStorageError(
          "a stored public record does not match its index",
        );
      }
      return record;
    });
  }

  listProjections(): readonly PublicShotProjection[] {
    const rows = this.database
      .query<{ shot_id: string }, []>(
        "SELECT shot_id FROM current_projections ORDER BY shot_id",
      )
      .all();
    return rows.map((row) => {
      const projection = this.getProjection(row.shot_id);
      if (projection === undefined) {
        throw new ReferenceNodeStorageError(
          "the stored public projection disappeared",
        );
      }
      return projection;
    });
  }

  #recordRows(shotId: string): StoredRecordRow[] {
    return this.database
      .query<StoredRecordRow, [string]>(
        `SELECT canonical_json, record_hash, sequence
           FROM public_records
          WHERE shot_id = ?
          ORDER BY sequence`,
      )
      .all(shotId);
  }
}
