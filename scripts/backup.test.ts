import { Database } from "bun:sqlite";
import { afterEach, describe, expect, test } from "bun:test";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { openMigratedDatabase } from "../apps/site/src/database.ts";
import { createSubmission } from "../apps/site/src/submissions.ts";
import { testConfig } from "../apps/site/tests/helpers.ts";
import {
  createConsistentBackup,
  defaultBackupPath,
  parseBackupArguments,
} from "./backup.ts";

const temporaryDirectories: string[] = [];

function temporaryDirectory(): string {
  const directory = mkdtempSync(join(tmpdir(), "tohseno-backup-test-"));
  temporaryDirectories.push(directory);
  return directory;
}

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

describe("SQLite production backups", () => {
  test("VACUUM INTO captures committed WAL state and produces a private restorable snapshot", async () => {
    const directory = temporaryDirectory();
    const databasePath = join(directory, "tohseno.sqlite");
    const activeDatabase = openMigratedDatabase(databasePath);
    const config = testConfig({ databasePath });
    const privateMarker = "BACKUP_PRIVATE_MARKER_DO_NOT_LOG";
    const privateEmail = "backup-owner@example.test";

    try {
      const submission = await createSubmission(activeDatabase, config, {
        markdown: `# Internal backup practice\n\n${privateMarker} records one deliberate pause and returns tomorrow.`,
        email: privateEmail,
        operatingMode: "self-hosted",
      });
      const walPath = `${databasePath}-wal`;
      expect(existsSync(walPath)).toBe(true);
      expect(statSync(walPath).size).toBeGreaterThan(0);

      const now = new Date("2026-07-10T13:14:15.678Z");
      const result = createConsistentBackup({ databasePath, now });

      expect(result.backupPath).toBe(realpathSync(defaultBackupPath(databasePath, now)));
      expect(dirname(result.backupPath)).toBe(join(realpathSync(directory), "backups"));
      expect(result.createdAt).toBe(now.toISOString());
      expect(result.sizeBytes).toBeGreaterThan(0);
      expect(result.migrationVersions).toEqual([
        "001_initial.sql",
        "002_capsule_release.sql",
        "003_email_outbox.sql",
      ]);
      expect(statSync(result.backupPath).mode & 0o777).toBe(0o600);

      const backupBytes = readFileSync(result.backupPath);
      expect(backupBytes.includes(Buffer.from(privateMarker))).toBe(false);
      expect(backupBytes.includes(Buffer.from(privateEmail))).toBe(false);

      const backupDatabase = new Database(result.backupPath, { readonly: true, strict: true });
      try {
        expect(backupDatabase.query<{ id: string }, [string]>(
          "SELECT id FROM submissions WHERE id = ?",
        ).get(submission.id)?.id).toBe(submission.id);
        expect(backupDatabase.query<{ integrity_check: string }, []>(
          "PRAGMA integrity_check",
        ).get()?.integrity_check).toBe("ok");
      } finally {
        backupDatabase.close();
      }

      const restoredPath = join(directory, "restored", "tohseno.sqlite");
      mkdirSync(dirname(restoredPath), { recursive: true });
      copyFileSync(result.backupPath, restoredPath);
      const restoredDatabase = openMigratedDatabase(restoredPath);
      try {
        expect(restoredDatabase.query<{ id: string }, [string]>(
          "SELECT id FROM submissions WHERE id = ?",
        ).get(submission.id)?.id).toBe(submission.id);
        expect(restoredDatabase.query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM schema_migrations",
        ).get()?.count).toBe(3);
      } finally {
        restoredDatabase.close();
      }
    } finally {
      activeDatabase.close();
    }
  });

  test("explicit output never replaces the active database or an existing file", () => {
    const directory = temporaryDirectory();
    const databasePath = join(directory, "tohseno.sqlite");
    const database = openMigratedDatabase(databasePath);
    database.close();
    const outputPath = join(directory, "owner-backups", "known.sqlite");

    const result = createConsistentBackup({ databasePath, outputPath });
    const originalBackup = readFileSync(result.backupPath);
    expect(result.backupPath).toBe(realpathSync(outputPath));

    expect(() => createConsistentBackup({ databasePath, outputPath })).toThrow("already exists");
    expect(readFileSync(outputPath)).toEqual(originalBackup);
    expect(() => createConsistentBackup({ databasePath, outputPath: databasePath })).toThrow(
      "must not replace the active SQLite database",
    );
  });

  test("rejects a database without migration provenance and does not leave an output", () => {
    const directory = temporaryDirectory();
    const databasePath = join(directory, "unmanaged.sqlite");
    const database = new Database(databasePath, { create: true, strict: true });
    database.exec("CREATE TABLE example (id INTEGER PRIMARY KEY)");
    database.close();
    const outputPath = join(directory, "backups", "unmanaged.sqlite");

    expect(() => createConsistentBackup({ databasePath, outputPath })).toThrow("schema_migrations");
    expect(existsSync(outputPath)).toBe(false);
  });

  test("parses one explicit output flag and rejects ambiguous arguments", () => {
    expect(parseBackupArguments([])).toEqual({ help: false });
    expect(parseBackupArguments(["--output", "/safe/backup.sqlite"])).toEqual({
      outputPath: "/safe/backup.sqlite",
      help: false,
    });
    expect(parseBackupArguments(["--output=/safe/backup.sqlite"])).toEqual({
      outputPath: "/safe/backup.sqlite",
      help: false,
    });
    expect(parseBackupArguments(["--help"])).toEqual({ help: true });
    expect(() => parseBackupArguments(["--output"])).toThrow("requires a file path");
    expect(() => parseBackupArguments(["--output=a", "--output=b"])).toThrow(
      "may be supplied only once",
    );
    expect(() => parseBackupArguments(["backup.sqlite"])).toThrow("Unknown backup argument");
  });
});
