import { Database } from "bun:sqlite";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  realpathSync,
  rmSync,
  statSync,
} from "node:fs";
import { basename, dirname, join, parse, resolve } from "node:path";
import { loadConfig } from "../apps/site/config.ts";

const BACKUP_DIRECTORY_NAME = "backups";

export interface DatabaseBackupOptions {
  databasePath: string;
  outputPath?: string | undefined;
  now?: Date | undefined;
}

export interface DatabaseBackupResult {
  backupPath: string;
  createdAt: string;
  sizeBytes: number;
  migrationVersions: string[];
}

interface IntegrityRow {
  integrity_check: string;
}

interface MigrationRow {
  version: string;
}

function timestampForFilename(now: Date): string {
  const iso = now.toISOString();
  return iso.replaceAll(":", "-");
}

export function defaultBackupPath(databasePath: string, now = new Date()): string {
  const activePath = resolve(databasePath);
  const databaseName = parse(activePath).name || "tohseno";
  return join(
    dirname(activePath),
    BACKUP_DIRECTORY_NAME,
    `${databaseName}-${timestampForFilename(now)}.sqlite`,
  );
}

function migrationVersions(database: Database): string[] {
  return database
    .query<MigrationRow, []>("SELECT version FROM schema_migrations ORDER BY version")
    .all()
    .map((row) => row.version);
}

function requireIntegrity(database: Database): void {
  const rows = database.query<IntegrityRow, []>("PRAGMA integrity_check").all();
  if (rows.length !== 1 || rows[0]?.integrity_check !== "ok") {
    throw new Error("Backup database failed PRAGMA integrity_check");
  }
}

function canonicalDestination(outputPath: string): string {
  const resolvedOutput = resolve(outputPath);
  mkdirSync(dirname(resolvedOutput), { recursive: true, mode: 0o700 });
  return join(realpathSync(dirname(resolvedOutput)), basename(resolvedOutput));
}

/**
 * Create a committed-state SQLite snapshot without decrypting application data.
 *
 * VACUUM INTO asks SQLite itself to read the logical database, including committed
 * pages that currently live in the WAL, and write a standalone database file.
 */
export function createConsistentBackup(options: DatabaseBackupOptions): DatabaseBackupResult {
  if (options.databasePath === ":memory:") {
    throw new Error("Cannot back up an in-memory database");
  }

  const activePath = resolve(options.databasePath);
  if (!existsSync(activePath) || !statSync(activePath).isFile()) {
    throw new Error(`Database file does not exist: ${activePath}`);
  }

  const createdAt = (options.now ?? new Date()).toISOString();
  const requestedOutput = options.outputPath ?? defaultBackupPath(activePath, new Date(createdAt));
  const outputPath = canonicalDestination(requestedOutput);
  const canonicalActivePath = realpathSync(activePath);

  if (
    outputPath === canonicalActivePath ||
    outputPath === `${canonicalActivePath}-wal` ||
    outputPath === `${canonicalActivePath}-shm`
  ) {
    throw new Error("Backup output must not replace the active SQLite database or its WAL files");
  }
  if (existsSync(outputPath)) {
    throw new Error(`Backup output already exists: ${outputPath}`);
  }

  let source: Database | undefined;
  let verifiedBackup: Database | undefined;
  let createdOutput = false;
  let completed = false;

  try {
    source = new Database(canonicalActivePath, { strict: true });
    source.exec("PRAGMA busy_timeout = 10000");
    const sourceMigrations = migrationVersions(source);

    // VACUUM INTO itself refuses an existing destination. The restrictive umask
    // ensures that the newly created file is owner-only before the explicit chmod.
    const previousUmask = process.umask(0o077);
    try {
      source.query("VACUUM INTO ?").run(outputPath);
      createdOutput = true;
    } finally {
      process.umask(previousUmask);
    }
    chmodSync(outputPath, 0o600);

    verifiedBackup = new Database(outputPath, { readonly: true, strict: true });
    requireIntegrity(verifiedBackup);
    const backupMigrations = migrationVersions(verifiedBackup);
    if (
      backupMigrations.length !== sourceMigrations.length ||
      backupMigrations.some((version, index) => version !== sourceMigrations[index])
    ) {
      throw new Error("Backup schema_migrations does not match the active database");
    }

    const sizeBytes = statSync(outputPath).size;
    completed = true;
    return {
      backupPath: outputPath,
      createdAt,
      sizeBytes,
      migrationVersions: backupMigrations,
    };
  } finally {
    verifiedBackup?.close();
    source?.close();
    if (createdOutput && !completed) rmSync(outputPath, { force: true });
  }
}

interface ParsedArguments {
  outputPath?: string | undefined;
  help: boolean;
}

export function parseBackupArguments(arguments_: string[]): ParsedArguments {
  let outputPath: string | undefined;
  let help = false;

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--help" || argument === "-h") {
      help = true;
      continue;
    }
    if (argument === "--output") {
      const value = arguments_[index + 1];
      if (!value || value.startsWith("--")) throw new Error("--output requires a file path");
      if (outputPath !== undefined) throw new Error("--output may be supplied only once");
      outputPath = value;
      index += 1;
      continue;
    }
    if (argument?.startsWith("--output=")) {
      const value = argument.slice("--output=".length);
      if (!value) throw new Error("--output requires a file path");
      if (outputPath !== undefined) throw new Error("--output may be supplied only once");
      outputPath = value;
      continue;
    }
    throw new Error(`Unknown backup argument: ${argument ?? ""}`);
  }

  return outputPath === undefined ? { help } : { outputPath, help };
}

function printUsage(): void {
  console.log("Usage: bun run backup -- [--output /safe/path/to/backup.sqlite]");
  console.log("TOHSENO_BACKUP_PATH may provide the output path; --output takes precedence.");
}

if (import.meta.main) {
  try {
    const arguments_ = parseBackupArguments(process.argv.slice(2));
    if (arguments_.help) {
      printUsage();
    } else {
      const config = loadConfig();
      const environmentOutput = process.env.TOHSENO_BACKUP_PATH?.trim() || undefined;
      const result = createConsistentBackup({
        databasePath: config.databasePath,
        outputPath: arguments_.outputPath ?? environmentOutput,
      });
      console.log(JSON.stringify({ event: "database.backup_created", ...result }));
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown backup failure";
    console.error(JSON.stringify({ event: "database.backup_failed", message }));
    process.exitCode = 1;
  }
}
