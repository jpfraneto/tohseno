import { afterEach, describe, expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FilesystemRegistryBlobStore, type RegistryBlobStore } from "../src/blob-store.ts";
import { migrateRegistryBlobs, r2BucketFingerprint } from "../src/registry-r2-migration.ts";

const roots: string[] = [];
afterEach(async () => Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))));

describe("Registry R2 migration", () => {
  test("dry-run audits every local byte without calling remote storage or writing an audit", async () => {
    const fixture = await catalogFixture();
    const remote = new CountingStore();
    const audit = await migrateRegistryBlobs({
      root: fixture.root,
      blobStore: remote,
      mode: "dry-run",
      sourceCommit: "1".repeat(40),
      observedAt: "2026-09-03T12:00:00.000Z",
      bucketFingerprint: r2BucketFingerprint("a".repeat(32), "registry-test"),
    });
    expect(audit.passed).toBeTrue();
    expect(audit.catalogRecordCount).toBe(1);
    expect(audit.blobCount).toBe(1);
    expect(audit.byteCount).toBe(fixture.source.byteLength);
    expect(audit.anky?.sourceDigest).toBe(fixture.sourceDigest);
    expect(remote.calls).toBe(0);
    expect(await stat(join(fixture.root, "r2-migration-audits")).catch(() => undefined)).toBeUndefined();
  });

  test("apply uses create-only promotion, verifies the destination, records evidence, and keeps local bytes", async () => {
    const fixture = await catalogFixture();
    const destination = await mkdtemp(join(tmpdir(), "tohseno-r2-migration-destination-"));
    roots.push(destination);
    const store = new FilesystemRegistryBlobStore(join(destination, "blobs"));
    const options = {
      root: fixture.root,
      blobStore: store,
      mode: "apply" as const,
      sourceCommit: "2".repeat(40),
      observedAt: "2026-09-03T12:01:00.000Z",
      bucketFingerprint: r2BucketFingerprint("a".repeat(32), "registry-test"),
    };
    const audit = await migrateRegistryBlobs(options);
    expect(audit.passed).toBeTrue();
    expect(audit.auditPath).toBeDefined();
    expect(JSON.parse(await readFile(audit.auditPath!, "utf8")).digests).toEqual([fixture.sourceDigest]);
    expect(new Uint8Array(await readFile(fixture.sourcePath))).toEqual(fixture.source);
    expect(await store.metadata({ digest: fixture.sourceDigest })).toMatchObject({
      digest: fixture.sourceDigest,
      byteLength: fixture.source.byteLength,
    });

    const repeated = await migrateRegistryBlobs({
      ...options,
      observedAt: "2026-09-03T12:01:01.000Z",
    });
    expect(repeated.passed).toBeTrue();
    expect(repeated.digests).toEqual([fixture.sourceDigest]);
    expect(new Uint8Array(await readFile(fixture.sourcePath))).toEqual(fixture.source);
    expect(await store.metadata({ digest: fixture.sourceDigest })).toMatchObject({
      digest: fixture.sourceDigest,
      byteLength: fixture.source.byteLength,
    });
  });

  test("a corrupt local blob fails before any remote write", async () => {
    const fixture = await catalogFixture();
    await writeFile(fixture.sourcePath, "substituted");
    const remote = new CountingStore();
    const audit = await migrateRegistryBlobs({
      root: fixture.root,
      blobStore: remote,
      mode: "apply",
      sourceCommit: "3".repeat(40),
      observedAt: "2026-09-03T12:02:00.000Z",
      bucketFingerprint: r2BucketFingerprint("a".repeat(32), "registry-test"),
    });
    expect(audit.passed).toBeFalse();
    expect(audit.failures[0]?.stage).toBe("local_audit");
    expect(remote.calls).toBe(0);
  });
});

async function catalogFixture() {
  const root = await mkdtemp(join(tmpdir(), "tohseno-r2-migration-source-"));
  roots.push(root);
  const source = new TextEncoder().encode("Anky exact deterministic source");
  const sourceDigest = `0x${new Bun.CryptoHasher("sha256").update(source).digest("hex")}` as const;
  const releaseDigest = `0x${"42".repeat(32)}` as const;
  const sourcePath = join(
    root, "blobs", "sha256", sourceDigest.slice(2, 4), sourceDigest.slice(4),
  );
  await mkdir(join(root, "releases"), { recursive: true });
  await mkdir(join(sourcePath, ".."), { recursive: true });
  await writeFile(sourcePath, source);
  await writeFile(join(root, "releases", `${releaseDigest.slice(2)}.json`), JSON.stringify({
    schema: "tohseno.catalog-record/1",
    releaseDigest,
    envelope: {
      release: {
        display: { name: "Anky", icon_sha256: null },
        source: { sha256: sourceDigest, byte_length: source.byteLength },
        checkpoint_sequence: 1,
      },
    },
  }));
  return { root, source, sourceDigest, sourcePath };
}

class CountingStore implements RegistryBlobStore {
  readonly kind = "r2" as const;
  calls = 0;
  async initialize() { this.calls += 1; }
  async stagePending(): Promise<void> { this.calls += 1; }
  async verifyPending(): Promise<void> { this.calls += 1; }
  async promotePending(): Promise<"created"> { this.calls += 1; return "created"; }
  async metadata(): Promise<never> { this.calls += 1; throw new Error("not used"); }
  async read(): Promise<never> { this.calls += 1; throw new Error("not used"); }
  async removePending(): Promise<void> { this.calls += 1; }
}
