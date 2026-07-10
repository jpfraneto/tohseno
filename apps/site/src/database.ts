import { Database } from "bun:sqlite";
import { mkdirSync, readdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

export type TohsenoDatabase = Database;

export interface MigrationResult {
  applied: string[];
}

function prepareDatabasePath(path: string): void {
  if (path === ":memory:") return;
  mkdirSync(dirname(resolve(path)), { recursive: true });
}

export function openDatabase(path: string): TohsenoDatabase {
  prepareDatabasePath(path);
  const database = new Database(path, { create: true, strict: true });
  database.exec("PRAGMA foreign_keys = ON");
  database.exec("PRAGMA busy_timeout = 5000");
  if (path !== ":memory:") database.exec("PRAGMA journal_mode = WAL");
  return database;
}

export function migrateDatabase(database: TohsenoDatabase, migrationsDirectory = new URL("../db/migrations/", import.meta.url).pathname): MigrationResult {
  database.exec(`
    CREATE TABLE IF NOT EXISTS schema_migrations (
      version TEXT PRIMARY KEY,
      applied_at TEXT NOT NULL
    )
  `);
  const appliedRows = database.query<{ version: string }, []>("SELECT version FROM schema_migrations").all();
  const appliedVersions = new Set(appliedRows.map((row) => row.version));
  const migrationFiles = readdirSync(migrationsDirectory)
    .filter((name) => /^\d+_[a-z0-9_-]+\.sql$/.test(name))
    .sort();
  const applied: string[] = [];

  for (const version of migrationFiles) {
    if (appliedVersions.has(version)) continue;
    const sql = readFileSync(resolve(migrationsDirectory, version), "utf8");
    const apply = database.transaction(() => {
      database.exec(sql);
      database.query("INSERT INTO schema_migrations (version, applied_at) VALUES (?, ?)").run(version, new Date().toISOString());
    });
    apply();
    applied.push(version);
  }
  return { applied };
}

export function openMigratedDatabase(path: string): TohsenoDatabase {
  const database = openDatabase(path);
  migrateDatabase(database);
  return database;
}
