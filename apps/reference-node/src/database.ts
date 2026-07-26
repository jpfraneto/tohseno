import {
  closeSync,
  constants,
  fchmodSync,
  fstatSync,
  lstatSync,
  mkdirSync,
  openSync,
  realpathSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { Database } from "bun:sqlite";

export const REFERENCE_NODE_DATABASE_SCHEMA_VERSION = 1 as const;
export const REFERENCE_NODE_DATABASE_FORMAT =
  "tohseno-reference-node/0.5" as const;

const UNSUPPORTED_DATABASE_MESSAGE =
  "reference node database is not canonical TOHSENO 0.5 state. " +
  "Pre-release compatibility is unsupported; start with fresh node state at a new empty path.";

const CANONICAL_SCHEMA_SQL = `
  CREATE TABLE node_metadata (
    singleton INTEGER PRIMARY KEY NOT NULL
      CHECK (singleton = 1),
    format TEXT NOT NULL
      CHECK (format = 'tohseno-reference-node/0.5'),
    schema_version INTEGER NOT NULL
      CHECK (schema_version = 1)
  ) STRICT;

  INSERT INTO node_metadata (singleton, format, schema_version)
  VALUES (1, 'tohseno-reference-node/0.5', 1);

  CREATE TABLE public_records (
    record_hash TEXT PRIMARY KEY NOT NULL
      CHECK (
        length(record_hash) = 71 AND
        substr(record_hash, 1, 7) = 'sha256:' AND
        substr(record_hash, 8) NOT GLOB '*[^0-9a-f]*'
      ),
    shot_id TEXT NOT NULL
      CHECK (
        length(shot_id) = 37 AND
        substr(shot_id, 1, 5) = 'shot_' AND
        substr(shot_id, 6) NOT GLOB '*[^A-Za-z0-9_-]*'
      ),
    sequence INTEGER NOT NULL
      CHECK (sequence >= 0 AND sequence <= 9007199254740991),
    record_kind TEXT NOT NULL
      CHECK (
        record_kind IN (
          'SHOT_CREATED',
          'EVOLUTION_RECORDED',
          'LIFECYCLE_TRANSITIONED',
          'APPCOIN_LINKED'
        )
      ),
    canonical_json TEXT NOT NULL
      CHECK (
        length(CAST(canonical_json AS BLOB)) <= 262144 AND
        json_valid(canonical_json)
      ),
    accepted_at TEXT NOT NULL
      CHECK (length(accepted_at) BETWEEN 20 AND 40),
    UNIQUE (shot_id, sequence)
  ) STRICT;

  CREATE TABLE current_projections (
    shot_id TEXT PRIMARY KEY NOT NULL
      CHECK (
        length(shot_id) = 37 AND
        substr(shot_id, 1, 5) = 'shot_' AND
        substr(shot_id, 6) NOT GLOB '*[^A-Za-z0-9_-]*'
      ),
    sequence INTEGER NOT NULL
      CHECK (sequence >= 0 AND sequence <= 9007199254740991),
    record_hash TEXT UNIQUE NOT NULL
      CHECK (
        length(record_hash) = 71 AND
        substr(record_hash, 1, 7) = 'sha256:' AND
        substr(record_hash, 8) NOT GLOB '*[^0-9a-f]*'
      ),
    projection_json TEXT NOT NULL
      CHECK (
        length(CAST(projection_json AS BLOB)) <= 4194304 AND
        json_valid(projection_json)
      ),
    FOREIGN KEY (record_hash)
      REFERENCES public_records(record_hash)
      ON UPDATE RESTRICT ON DELETE RESTRICT
  ) STRICT;

  CREATE TRIGGER node_metadata_is_immutable_update
  BEFORE UPDATE ON node_metadata
  BEGIN
    SELECT RAISE(ABORT, 'node metadata is immutable');
  END;

  CREATE TRIGGER node_metadata_is_immutable_delete
  BEFORE DELETE ON node_metadata
  BEGIN
    SELECT RAISE(ABORT, 'node metadata is immutable');
  END;

  CREATE TRIGGER public_records_are_append_only_update
  BEFORE UPDATE ON public_records
  BEGIN
    SELECT RAISE(ABORT, 'public records are append-only');
  END;

  CREATE TRIGGER public_records_are_append_only_delete
  BEFORE DELETE ON public_records
  BEGIN
    SELECT RAISE(ABORT, 'public records are append-only');
  END;
`;

export interface ReferenceNodeDatabase {
  database: Database;
  path: string;
  schemaVersion: typeof REFERENCE_NODE_DATABASE_SCHEMA_VERSION;
}

export class UnsupportedReferenceNodeDatabaseError extends Error {
  override readonly name = "UnsupportedReferenceNodeDatabaseError";

  constructor() {
    super(UNSUPPORTED_DATABASE_MESSAGE);
  }
}

interface PreparedDatabasePath {
  path: string;
  created: boolean;
}

interface SchemaRow {
  type: string;
  name: string;
  tbl_name: string;
  sql: string;
}

function assertOwnerPrivateRegularFile(path: string): void {
  let descriptor: number;
  try {
    descriptor = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  } catch {
    throw new Error(
      "reference node storage must be an owner-private single-link regular file",
    );
  }
  try {
    const opened = fstatSync(descriptor);
    const current = lstatSync(path);
    const currentUser = process.getuid?.();
    if (
      !opened.isFile() ||
      opened.nlink !== 1 ||
      current.isSymbolicLink() ||
      !current.isFile() ||
      opened.dev !== current.dev ||
      opened.ino !== current.ino ||
      (currentUser !== undefined && opened.uid !== currentUser) ||
      (opened.mode & 0o077) !== 0
    ) {
      throw new Error(
        "reference node storage must be an owner-private single-link regular file",
      );
    }
  } finally {
    closeSync(descriptor);
  }
}

function assertAuxiliaryFiles(path: string): void {
  for (const suffix of ["-wal", "-shm"]) {
    const auxiliary = `${path}${suffix}`;
    try {
      lstatSync(auxiliary);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") continue;
      throw error;
    }
    assertOwnerPrivateRegularFile(auxiliary);
  }
}

function prepareDatabasePath(pathValue: string): PreparedDatabasePath {
  const requested = resolve(pathValue);
  const parent = dirname(requested);
  let createdParent = false;
  try {
    mkdirSync(parent, { mode: 0o700 });
    createdParent = true;
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code;
    if (code === "ENOENT") {
      throw new Error(
        "reference node storage parent must have an existing parent directory",
      );
    }
    if (code !== "EEXIST") throw error;
  }

  let parentDescriptor: number;
  try {
    parentDescriptor = openSync(
      parent,
      constants.O_RDONLY | constants.O_DIRECTORY | constants.O_NOFOLLOW,
    );
  } catch {
    throw new Error("reference node storage parent must be a real directory");
  }
  try {
    const openedParent = fstatSync(parentDescriptor);
    const parentDetails = lstatSync(parent);
    const currentUser = process.getuid?.();
    if (
      !openedParent.isDirectory() ||
      parentDetails.isSymbolicLink() ||
      !parentDetails.isDirectory() ||
      openedParent.dev !== parentDetails.dev ||
      openedParent.ino !== parentDetails.ino
    ) {
      throw new Error(
        "reference node storage parent must be a real directory",
      );
    }
    if (
      (currentUser !== undefined && openedParent.uid !== currentUser) ||
      (!createdParent && (openedParent.mode & 0o077) !== 0)
    ) {
      throw new Error(
        "reference node storage parent must already be owner-private",
      );
    }
    if (createdParent) fchmodSync(parentDescriptor, 0o700);
  } finally {
    closeSync(parentDescriptor);
  }

  const canonicalParent = realpathSync(parent);
  const path = join(canonicalParent, basename(requested));
  let created = false;
  try {
    lstatSync(path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    const descriptor = openSync(
      path,
      constants.O_CREAT |
        constants.O_EXCL |
        constants.O_RDWR |
        constants.O_NOFOLLOW,
      0o600,
    );
    closeSync(descriptor);
    created = true;
  }
  assertOwnerPrivateRegularFile(path);
  assertAuxiliaryFiles(path);
  return { path, created };
}

function schemaRows(database: Database): SchemaRow[] {
  return database
    .query<SchemaRow, []>(
      `SELECT type, name, tbl_name, sql
         FROM sqlite_schema
        WHERE name NOT LIKE 'sqlite_%'
        ORDER BY type, name`,
    )
    .all();
}

function createCanonicalSchema(database: Database): void {
  const create = database.transaction(() => {
    database.exec(CANONICAL_SCHEMA_SQL);
  });
  create();
}

let expectedSchema: string | undefined;

function expectedSchemaJson(): string {
  if (expectedSchema !== undefined) return expectedSchema;
  const database = new Database(":memory:", { strict: true });
  try {
    database.exec("PRAGMA foreign_keys = ON;");
    createCanonicalSchema(database);
    expectedSchema = JSON.stringify(schemaRows(database));
    return expectedSchema;
  } finally {
    database.close();
  }
}

function assertCanonicalSchema(database: Database): void {
  try {
    if (JSON.stringify(schemaRows(database)) !== expectedSchemaJson()) {
      throw new UnsupportedReferenceNodeDatabaseError();
    }
    const metadata = database
      .query<
        { singleton: number; format: string; schema_version: number },
        []
      >(
        `SELECT singleton, format, schema_version
           FROM node_metadata`,
      )
      .all();
    if (
      metadata.length !== 1 ||
      metadata[0]?.singleton !== 1 ||
      metadata[0]?.format !== REFERENCE_NODE_DATABASE_FORMAT ||
      metadata[0]?.schema_version !==
        REFERENCE_NODE_DATABASE_SCHEMA_VERSION
    ) {
      throw new UnsupportedReferenceNodeDatabaseError();
    }
    const integrity = database
      .query<{ integrity_check: string }, []>("PRAGMA integrity_check")
      .all();
    if (
      integrity.length !== 1 ||
      integrity[0]?.integrity_check !== "ok" ||
      database.query<Record<string, unknown>, []>(
          "PRAGMA foreign_key_check",
        ).all().length !== 0
    ) {
      throw new UnsupportedReferenceNodeDatabaseError();
    }
  } catch (error) {
    if (error instanceof UnsupportedReferenceNodeDatabaseError) throw error;
    throw new UnsupportedReferenceNodeDatabaseError();
  }
}

function assertExistingDatabaseReadOnly(path: string): void {
  let database: Database | undefined;
  try {
    database = new Database(path, {
      readonly: true,
      strict: true,
    });
    assertCanonicalSchema(database);
  } catch (error) {
    if (error instanceof UnsupportedReferenceNodeDatabaseError) throw error;
    throw new UnsupportedReferenceNodeDatabaseError();
  } finally {
    database?.close();
  }
}

function configureOpenDatabase(database: Database, inMemory: boolean): void {
  database.exec("PRAGMA foreign_keys = ON;");
  if (!inMemory) database.exec("PRAGMA journal_mode = WAL;");
  database.exec("PRAGMA busy_timeout = 5000;");
}

export function openReferenceNodeDatabase(
  pathValue: string,
): ReferenceNodeDatabase {
  const inMemory = pathValue === ":memory:";
  if (inMemory) {
    const database = new Database(":memory:", { strict: true });
    try {
      configureOpenDatabase(database, true);
      createCanonicalSchema(database);
      assertCanonicalSchema(database);
      return {
        database,
        path: ":memory:",
        schemaVersion: REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
      };
    } catch (error) {
      database.close();
      throw error;
    }
  }

  const prepared = prepareDatabasePath(pathValue);
  if (!prepared.created) {
    assertExistingDatabaseReadOnly(prepared.path);
  }
  const database = new Database(prepared.path, {
    create: false,
    strict: true,
  });
  try {
    configureOpenDatabase(database, false);
    if (prepared.created) createCanonicalSchema(database);
    assertCanonicalSchema(database);
    assertOwnerPrivateRegularFile(prepared.path);
    assertAuxiliaryFiles(prepared.path);
    return {
      database,
      path: prepared.path,
      schemaVersion: REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
    };
  } catch (error) {
    database.close();
    throw error;
  }
}
