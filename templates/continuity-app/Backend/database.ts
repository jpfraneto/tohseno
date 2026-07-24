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
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { Database } from "bun:sqlite";

export const DATABASE_SCHEMA_VERSION = 1 as const;

export interface InitializedDatabase {
  database: Database;
  path: string;
  schemaVersion: typeof DATABASE_SCHEMA_VERSION;
}

interface Migration {
  version: number;
  apply(database: Database): void;
}

const MIGRATIONS: readonly Migration[] = [
  {
    version: 1,
    apply(database) {
      // This foundation stores operational metadata only. App content stays in
      // the iOS app's platform-private files unless a later manifest explicitly
      // declares and implements a different disclosure boundary.
      database.exec(`
        CREATE TABLE runtime_metadata (
          key TEXT PRIMARY KEY NOT NULL,
          value TEXT NOT NULL,
          updated_at TEXT NOT NULL
        ) STRICT;
      `);
    },
  },
];

function secureRegularFileWithoutFollowingLinks(path: string): void {
  let descriptor: number;
  try {
    descriptor = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  } catch {
    throw new Error("database storage must be a single-link regular file");
  }
  try {
    const opened = fstatSync(descriptor);
    const current = lstatSync(path);
    if (
      !opened.isFile() ||
      opened.nlink !== 1 ||
      current.isSymbolicLink() ||
      !current.isFile() ||
      opened.dev !== current.dev ||
      opened.ino !== current.ino
    ) {
      throw new Error("database storage must be a single-link regular file");
    }
    fchmodSync(descriptor, 0o600);
  } finally {
    closeSync(descriptor);
  }
}

function prepareDatabasePath(
  pathValue: string,
  boundaryValue?: string,
): string {
  const requested = resolve(pathValue);
  const parent = dirname(requested);
  mkdirSync(parent, { recursive: true, mode: 0o700 });
  const canonicalParent = realpathSync(parent);
  const path = join(canonicalParent, basename(requested));
  if (boundaryValue !== undefined) {
    const boundary = realpathSync(resolve(boundaryValue));
    const fromBoundary = relative(boundary, path);
    if (
      fromBoundary === ".." ||
      fromBoundary.startsWith(`..${sep}`) ||
      resolve(path) === boundary
    ) {
      throw new Error("development database storage must remain inside the shot");
    }
  }

  try {
    secureRegularFileWithoutFollowingLinks(path);
  } catch (error) {
    try {
      const descriptor = openSync(
        path,
        constants.O_CREAT | constants.O_EXCL | constants.O_RDWR | constants.O_NOFOLLOW,
        0o600,
      );
      closeSync(descriptor);
    } catch {
      throw error;
    }
    secureRegularFileWithoutFollowingLinks(path);
  }

  // SQLite may reuse these files after an unclean shutdown. Refuse a crafted
  // link before the library can follow it.
  for (const suffix of ["-wal", "-shm"]) {
    const auxiliaryPath = `${path}${suffix}`;
    try {
      lstatSync(auxiliaryPath);
    } catch {
      continue;
    }
    secureRegularFileWithoutFollowingLinks(auxiliaryPath);
  }
  return path;
}

export function initializeDatabase(
  pathValue: string,
  options: { boundary?: string } = {},
): InitializedDatabase {
  const path = prepareDatabasePath(pathValue, options.boundary);
  const database = new Database(path, { create: true, strict: true });
  try {
    database.exec("PRAGMA foreign_keys = ON;");
    database.exec("PRAGMA journal_mode = WAL;");
    database.exec("PRAGMA busy_timeout = 5000;");
    for (const suffix of ["-wal", "-shm"]) {
      const auxiliaryPath = `${path}${suffix}`;
      try {
        lstatSync(auxiliaryPath);
      } catch {
        continue;
      }
      secureRegularFileWithoutFollowingLinks(auxiliaryPath);
    }
    database.exec(`
      CREATE TABLE IF NOT EXISTS schema_migrations (
        version INTEGER PRIMARY KEY NOT NULL,
        applied_at TEXT NOT NULL
      ) STRICT;
    `);

    const applied = new Set(
      database.query<{ version: number }, []>("SELECT version FROM schema_migrations ORDER BY version")
        .all()
        .map((row) => row.version),
    );
    for (const migration of MIGRATIONS) {
      if (applied.has(migration.version)) continue;
      const apply = database.transaction(() => {
        migration.apply(database);
        database.query("INSERT INTO schema_migrations (version, applied_at) VALUES (?, ?)")
          .run(migration.version, new Date().toISOString());
      });
      apply();
    }
    secureRegularFileWithoutFollowingLinks(path);
    return { database, path, schemaVersion: DATABASE_SCHEMA_VERSION };
  } catch (error) {
    database.close();
    throw error;
  }
}
