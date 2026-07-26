import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  canonicalJson,
  type ShotCreatedRecord,
  signPublicShotRecord,
} from "../../../packages/protocol/src/index.ts";
import {
  localEd25519VerifierSet,
} from "../../../packages/signer/src/index.ts";
import { openReferenceNodeDatabase } from "../src/database.ts";
import {
  ReferenceNodeCapacityError,
  ReferenceNodeStorageError,
  SqlitePublicRecordRegistry,
} from "../src/registry.ts";
import {
  appStoreRecord,
  completeRecordChain,
  createRecord,
  TEST_SHOT_ID,
  testBuilder,
} from "./fixtures.ts";

const scratchDirectories: string[] = [];

afterEach(() => {
  for (const directory of scratchDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

function scratch(): string {
  const directory = mkdtempSync(join(tmpdir(), "tohseno-node-registry-"));
  scratchDirectories.push(directory);
  return directory;
}

describe("SQLite public record registry", () => {
  test("appends every public record kind and treats exact replay as idempotent", async () => {
    const opened = openReferenceNodeDatabase(":memory:");
    try {
      const registry = new SqlitePublicRecordRegistry(
        opened.database,
        localEd25519VerifierSet(),
        () => new Date("2026-07-25T12:00:00.000Z"),
      );
      const records = await completeRecordChain();
      for (const record of records) {
        expect((await registry.append(record)).status).toBe("appended");
      }
      const replay = await registry.append(records[2]!);
      expect(replay.status).toBe("existing");
      expect(registry.getRecords(TEST_SHOT_ID)).toEqual(records);
      expect(registry.getProjection(TEST_SHOT_ID)).toMatchObject({
        shotId: TEST_SHOT_ID,
        lifecycle: "APP_STORE",
        evolution: 1,
        recordCount: 5,
      });
      expect(registry.listProjections()).toHaveLength(1);
      expect(
        opened.database
          .query<{ count: number }, []>(
            "SELECT COUNT(*) AS count FROM public_records",
          )
          .get()?.count,
      ).toBe(5);
    } finally {
      opened.database.close();
    }
  });

  test("serializes concurrent duplicates and rejects forks without mutation", async () => {
    const opened = openReferenceNodeDatabase(":memory:");
    try {
      const signer = testBuilder("concurrent");
      const registry = new SqlitePublicRecordRegistry(
        opened.database,
        localEd25519VerifierSet(),
      );
      const record = await createRecord(signer);
      const results = await Promise.all([
        registry.append(record),
        registry.append(record),
      ]);
      expect(results.map((result) => result.status).toSorted()).toEqual([
        "appended",
        "existing",
      ]);

      const alternate = await createRecord(
        signer,
        TEST_SHOT_ID,
        "A conflicting public summary.",
      );
      await expect(registry.append(alternate)).rejects.toMatchObject({
        code: "sequence-conflict",
      });
      expect(registry.getRecords(TEST_SHOT_ID)).toEqual([record]);
    } finally {
      opened.database.close();
    }
  });

  test("keeps accepted per-Shot histories within export capacity", async () => {
    const opened = openReferenceNodeDatabase(":memory:");
    try {
      const registry = new SqlitePublicRecordRegistry(
        opened.database,
        localEd25519VerifierSet(),
        () => new Date("2026-07-25T12:00:00.000Z"),
        { maxRecordsPerShot: 1 },
      );
      const records = await completeRecordChain();
      const created = records[0]!;
      await registry.append(created);
      expect((await registry.append(created)).status).toBe("existing");
      await expect(registry.append(records[1]!)).rejects.toBeInstanceOf(
        ReferenceNodeCapacityError,
      );
      expect(registry.getRecords(TEST_SHOT_ID)).toEqual([created]);
    } finally {
      opened.database.close();
    }
  });

  test("rejects invalid signatures and lifecycle transitions transactionally", async () => {
    const opened = openReferenceNodeDatabase(":memory:");
    try {
      const signer = testBuilder("invalid");
      const registry = new SqlitePublicRecordRegistry(
        opened.database,
        localEd25519VerifierSet(),
      );
      const created = await createRecord(signer);
      const mutated = structuredClone(created);
      if (mutated.kind !== "SHOT_CREATED") throw new Error("unreachable");
      mutated.body.summary = "Changed after signing.";
      await expect(registry.append(mutated)).rejects.toMatchObject({
        code: "invalid-signature",
      });
      expect(registry.getRecords(TEST_SHOT_ID)).toEqual([]);

      await registry.append(created);
      const invalidTransition = await appStoreRecord(signer, created);
      await expect(registry.append(invalidTransition)).rejects.toMatchObject({
        code: "lifecycle-conflict",
      });
      expect(registry.getRecords(TEST_SHOT_ID)).toEqual([created]);

      if (created.kind !== "SHOT_CREATED") throw new Error("unreachable");
      const forkBody: ShotCreatedRecord = {
        protocolVersion: created.protocolVersion,
        kind: "SHOT_CREATED",
        shotId: created.shotId,
        sequence: created.sequence,
        previousRecordHash: created.previousRecordHash,
        recordedAt: created.recordedAt,
        authority: created.authority,
        body: {
          ...created.body,
          summary: "A signed fork.",
        },
      };
      const fork = await signPublicShotRecord(forkBody, signer);
      await expect(registry.append(fork)).rejects.toMatchObject({
        code: "sequence-conflict",
      });
    } finally {
      opened.database.close();
    }
  });

  test("recovers canonical records and projection after restart", async () => {
    const path = join(scratch(), "private", "reference.sqlite");
    const records = await completeRecordChain(
      testBuilder("persistent-chain"),
    );
    const first = openReferenceNodeDatabase(path);
    const firstRegistry = new SqlitePublicRecordRegistry(
      first.database,
      localEd25519VerifierSet(),
    );
    for (const record of records) await firstRegistry.append(record);
    const before = canonicalJson(firstRegistry.getProjection(TEST_SHOT_ID));
    first.database.close();

    const second = openReferenceNodeDatabase(path);
    try {
      const recovered = new SqlitePublicRecordRegistry(
        second.database,
        localEd25519VerifierSet(),
      );
      expect(canonicalJson(recovered.getRecords(TEST_SHOT_ID))).toBe(
        canonicalJson(records),
      );
      expect(canonicalJson(recovered.getProjection(TEST_SHOT_ID))).toBe(before);
    } finally {
      second.database.close();
    }
  });

  test("refuses a stored projection that differs from its signed history", async () => {
    const opened = openReferenceNodeDatabase(":memory:");
    try {
      const registry = new SqlitePublicRecordRegistry(
        opened.database,
        localEd25519VerifierSet(),
      );
      await registry.append(await createRecord(testBuilder("projection")));
      opened.database
        .query(
          "UPDATE current_projections SET projection_json = ? WHERE shot_id = ?",
        )
        .run("{}", TEST_SHOT_ID);
      expect(() => registry.getProjection(TEST_SHOT_ID)).toThrow(
        ReferenceNodeStorageError,
      );
    } finally {
      opened.database.close();
    }
  });
});
