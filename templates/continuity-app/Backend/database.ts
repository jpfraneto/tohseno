import { chmodSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
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

export function initializeDatabase(pathValue: string): InitializedDatabase {
  const path = resolve(pathValue);
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  const database = new Database(path, { create: true, strict: true });
  try {
    database.exec("PRAGMA foreign_keys = ON;");
    database.exec("PRAGMA journal_mode = WAL;");
    database.exec("PRAGMA busy_timeout = 5000;");
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
    chmodSync(path, 0o600);
    return { database, path, schemaVersion: DATABASE_SCHEMA_VERSION };
  } catch (error) {
    database.close();
    throw error;
  }
}
