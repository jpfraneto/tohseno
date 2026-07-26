import { afterEach, describe, expect, test } from "bun:test";
import {
  chmodSync,
  existsSync,
  linkSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Database } from "bun:sqlite";
import {
  openReferenceNodeDatabase,
  REFERENCE_NODE_DATABASE_FORMAT,
  REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
  UnsupportedReferenceNodeDatabaseError,
} from "../src/database.ts";

const scratchDirectories: string[] = [];

afterEach(() => {
  for (const directory of scratchDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

function scratch(label: string): string {
  const directory = mkdtempSync(join(tmpdir(), `tohseno-node-${label}-`));
  scratchDirectories.push(directory);
  return directory;
}

describe("reference node database", () => {
  test("initializes the exact 0.5 schema with append-only records", () => {
    const opened = openReferenceNodeDatabase(":memory:");
    try {
      expect(opened.schemaVersion).toBe(
        REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
      );
      expect(
        opened.database
          .query<{ format: string; schema_version: number }, []>(
            "SELECT format, schema_version FROM node_metadata",
          )
          .get(),
      ).toEqual({
        format: REFERENCE_NODE_DATABASE_FORMAT,
        schema_version: REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
      });
      expect(
        opened.database
          .query<{ name: string }, []>(
            "PRAGMA table_info(public_records)",
          )
          .all()
          .map((column) => column.name),
      ).toEqual([
        "record_hash",
        "shot_id",
        "sequence",
        "record_kind",
        "canonical_json",
        "accepted_at",
      ]);
      const recordHash = `sha256:${"a".repeat(64)}`;
      opened.database
        .query(
          `INSERT INTO public_records
            (record_hash, shot_id, sequence, record_kind, canonical_json,
             accepted_at)
           VALUES (?, ?, ?, ?, ?, ?)`,
        )
        .run(
          recordHash,
          `shot_${"A".repeat(32)}`,
          0,
          "SHOT_CREATED",
          "{}",
          "2026-07-25T00:00:00.000Z",
        );
      expect(() =>
        opened.database
          .query(
            "UPDATE public_records SET record_kind = ? WHERE record_hash = ?",
          )
          .run("EVOLUTION_RECORDED", recordHash)
      ).toThrow("append-only");
      expect(() =>
        opened.database
          .query("DELETE FROM public_records WHERE record_hash = ?")
          .run(recordHash)
      ).toThrow("append-only");
    } finally {
      opened.database.close();
    }
  });

  test("persists across restart with private file permissions", () => {
    const root = scratch("restart");
    const path = join(root, "private data", "node.sqlite");
    const first = openReferenceNodeDatabase(path);
    const recordHash = `sha256:${"b".repeat(64)}`;
    first.database
      .query(
        `INSERT INTO public_records
          (record_hash, shot_id, sequence, record_kind, canonical_json,
           accepted_at)
         VALUES (?, ?, ?, ?, ?, ?)`,
      )
      .run(
        recordHash,
        `shot_${"B".repeat(32)}`,
        0,
        "SHOT_CREATED",
        "{}",
        "2026-07-25T00:00:00.000Z",
      );
    first.database.close();

    const second = openReferenceNodeDatabase(path);
    try {
      expect(
        second.database
          .query<{ count: number }, []>(
            "SELECT COUNT(*) AS count FROM public_records",
          )
          .get()?.count,
      ).toBe(1);
      expect(statSync(path).mode & 0o077).toBe(0);
      expect(statSync(join(root, "private data")).mode & 0o077).toBe(0);
    } finally {
      second.database.close();
    }
  });

  test("rejects symbolic and hard-linked database files", () => {
    const root = scratch("links");
    const data = join(root, "data");
    mkdirSync(data, { mode: 0o700 });
    const victim = join(root, "owner-file");
    writeFileSync(victim, "owner data", { mode: 0o640 });
    const path = join(data, "node.sqlite");

    symlinkSync(victim, path);
    expect(() => openReferenceNodeDatabase(path)).toThrow("single-link");
    expect(readFileSync(victim, "utf8")).toBe("owner data");
    rmSync(path);

    linkSync(victim, path);
    expect(() => openReferenceNodeDatabase(path)).toThrow("single-link");
    expect(readFileSync(victim, "utf8")).toBe("owner data");
    expect(statSync(victim).mode & 0o777).toBe(0o640);
  });

  test("never changes permissions on an existing broad parent", () => {
    const root = scratch("broad-parent");
    const shared = join(root, "shared");
    mkdirSync(shared, { mode: 0o755 });

    expect(() =>
      openReferenceNodeDatabase(join(shared, "node.sqlite"))
    ).toThrow("must already be owner-private");
    expect(statSync(shared).mode & 0o777).toBe(0o755);
    expect(existsSync(join(shared, "node.sqlite"))).toBe(false);
  });

  test("rejects obsolete database state without mutating it", () => {
    const root = scratch("obsolete");
    const path = join(root, "node.sqlite");
    const obsolete = new Database(path, { create: true, strict: true });
    obsolete.exec(`
      CREATE TABLE schema_migrations (
        version INTEGER PRIMARY KEY NOT NULL,
        applied_at TEXT NOT NULL
      ) STRICT;
      INSERT INTO schema_migrations VALUES
        (1, '2026-07-25T00:00:00.000Z');
    `);
    obsolete.close();
    chmodSync(path, 0o600);
    const before = readFileSync(path);

    expect(() => openReferenceNodeDatabase(path)).toThrow(
      UnsupportedReferenceNodeDatabaseError,
    );
    expect(() => openReferenceNodeDatabase(path)).toThrow(
      "Pre-release compatibility is unsupported; start with fresh node state",
    );
    expect(readFileSync(path)).toEqual(before);
  });

  test("rejects extra schema objects without mutating them", () => {
    const root = scratch("unknown");
    const path = join(root, "node.sqlite");
    const canonical = openReferenceNodeDatabase(path);
    canonical.database.exec("CREATE TABLE unknown_state (value TEXT) STRICT;");
    canonical.database.close();
    const before = readFileSync(path);

    expect(() => openReferenceNodeDatabase(path)).toThrow(
      UnsupportedReferenceNodeDatabaseError,
    );
    expect(readFileSync(path)).toEqual(before);
  });

  test("does not adopt a pre-existing empty file", () => {
    const root = scratch("empty");
    const path = join(root, "node.sqlite");
    writeFileSync(path, "", { mode: 0o600 });
    expect(() => openReferenceNodeDatabase(path)).toThrow(
      UnsupportedReferenceNodeDatabaseError,
    );
    expect(readFileSync(path).byteLength).toBe(0);
  });
});
