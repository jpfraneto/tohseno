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
    expect(audit.anky?.releaseDigest).toBe(fixture.releaseDigest);
    expect(audit.anky?.checkpointSequence).toBe(1);
    expect(audit.anky?.catalogRecordSHA256).toMatch(/^0x[0-9a-f]{64}$/);
    expect(audit.catalogFingerprint).toMatch(/^0x[0-9a-f]{64}$/);
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
    expect(audit.auditFilename).toBeDefined();
    const auditPath = join(fixture.root, "r2-migration-audits", audit.auditFilename!);
    expect(JSON.parse(await readFile(auditPath, "utf8")).digests).toEqual([fixture.sourceDigest]);
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

  test("deduplicates shared source and icon references while retaining orphaned permanent blobs", async () => {
    const fixture = await catalogFixture();
    const icon = new TextEncoder().encode("one exact icon");
    const orphan = new TextEncoder().encode("one exact orphaned permanent blob");
    const iconDigest = contentDigest(icon);
    const orphanDigest = contentDigest(orphan);
    await writeCanonicalBlob(fixture.root, iconDigest, icon);
    await writeCanonicalBlob(fixture.root, orphanDigest, orphan);
    const secondReleaseDigest = `0x${"43".repeat(32)}` as const;
    await writeCatalogRecord(fixture.root, {
      releaseDigest: secondReleaseDigest,
      sourceDigest: fixture.sourceDigest,
      sourceLength: fixture.source.byteLength,
      iconDigest,
      name: "Second app",
      sequence: 2,
    });
    await rewriteCatalogRecord(fixture.root, fixture.releaseDigest, {
      releaseDigest: fixture.releaseDigest,
      sourceDigest: fixture.sourceDigest,
      sourceLength: fixture.source.byteLength,
      iconDigest,
      name: "Anky",
      sequence: 1,
    });
    const remote = new CountingStore();
    const audit = await migrateRegistryBlobs({
      root: fixture.root,
      blobStore: remote,
      mode: "dry-run",
      sourceCommit: "4".repeat(40),
      observedAt: "2026-09-03T12:03:00.000Z",
      bucketFingerprint: r2BucketFingerprint("a".repeat(32), "registry-test"),
    });
    expect(audit.passed).toBeTrue();
    expect(audit.catalogRecordCount).toBe(2);
    expect(audit.catalogReferencedBlobCount).toBe(2);
    expect(audit.blobCount).toBe(3);
    expect(audit.unreferencedBlobCount).toBe(1);
    expect(audit.digests).toEqual([fixture.sourceDigest, iconDigest, orphanDigest].sort());
    expect(remote.calls).toBe(0);
  });

  test("audits every signed version-2 icon and screenshot byte", async () => {
    const fixture = await catalogFixture();
    const icon = new TextEncoder().encode("one version-2 app icon");
    const screenshot = new TextEncoder().encode("one version-2 screenshot");
    const iconDigest = contentDigest(icon);
    const screenshotDigest = contentDigest(screenshot);
    await writeCanonicalBlob(fixture.root, iconDigest, icon);
    await writeCanonicalBlob(fixture.root, screenshotDigest, screenshot);
    await writeFile(
      join(fixture.root, "releases", `${fixture.releaseDigest.slice(2)}.json`),
      JSON.stringify({
        schema: "tohseno.catalog-record/1",
        releaseDigest: fixture.releaseDigest,
        envelope: { release: {
          schema: "tohseno.catalog-release/2",
          display: { name: "Anky", icon_sha256: iconDigest,
            icon_byte_length: icon.byteLength, screenshots: [{ sha256: screenshotDigest,
              byte_length: screenshot.byteLength, media_type: "image/png" }] },
          source: { sha256: fixture.sourceDigest, byte_length: fixture.source.byteLength },
          checkpoint_sequence: 1,
        } },
      }),
    );

    const audit = await migrateRegistryBlobs({
      root: fixture.root,
      blobStore: new CountingStore(),
      mode: "dry-run",
      sourceCommit: "5".repeat(40),
      observedAt: "2026-09-03T12:03:30.000Z",
      bucketFingerprint: r2BucketFingerprint("a".repeat(32), "registry-test"),
    });
    expect(audit.passed).toBeTrue();
    expect(audit.catalogReferencedBlobCount).toBe(3);
    expect(audit.digests).toEqual([fixture.sourceDigest, iconDigest, screenshotDigest].sort());
  });

  test("fails apply instead of overwriting different destination bytes at an expected digest", async () => {
    const fixture = await catalogFixture();
    const destination = await mkdtemp(join(tmpdir(), "tohseno-r2-migration-conflict-"));
    roots.push(destination);
    const conflictingPath = canonicalBlobPath(join(destination, "blobs"), fixture.sourceDigest);
    await mkdir(join(conflictingPath, ".."), { recursive: true });
    await writeFile(conflictingPath, new Uint8Array(fixture.source.byteLength).fill(0x78));
    const audit = await migrateRegistryBlobs({
      root: fixture.root,
      blobStore: new FilesystemRegistryBlobStore(join(destination, "blobs")),
      mode: "apply",
      sourceCommit: "5".repeat(40),
      observedAt: "2026-09-03T12:04:00.000Z",
      bucketFingerprint: r2BucketFingerprint("a".repeat(32), "registry-test"),
    });
    expect(audit.passed).toBeFalse();
    expect(audit.failures).toHaveLength(1);
    expect(audit.failures[0]).toMatchObject({
      digest: fixture.sourceDigest,
      stage: "r2_upload",
    });
    expect(new Uint8Array(await readFile(conflictingPath)))
      .toEqual(new Uint8Array(fixture.source.byteLength).fill(0x78));
    expect(new Uint8Array(await readFile(fixture.sourcePath))).toEqual(fixture.source);
  });
});

async function catalogFixture() {
  const root = await mkdtemp(join(tmpdir(), "tohseno-r2-migration-source-"));
  roots.push(root);
  const source = new TextEncoder().encode("Anky exact deterministic source");
  const sourceDigest = `0x${new Bun.CryptoHasher("sha256").update(source).digest("hex")}` as const;
  const releaseDigest = `0x${"42".repeat(32)}` as const;
  const sourcePath = canonicalBlobPath(join(root, "blobs"), sourceDigest);
  await mkdir(join(root, "releases"), { recursive: true });
  await mkdir(join(sourcePath, ".."), { recursive: true });
  await writeFile(sourcePath, source);
  await writeCatalogRecord(root, {
    releaseDigest, sourceDigest, sourceLength: source.byteLength,
    iconDigest: null, name: "Anky", sequence: 1,
  });
  return { root, source, sourceDigest, sourcePath, releaseDigest };
}

function contentDigest(bytes: Uint8Array): `0x${string}` {
  return `0x${new Bun.CryptoHasher("sha256").update(bytes).digest("hex")}`;
}

function canonicalBlobPath(root: string, digest: `0x${string}`): string {
  return join(root, "sha256", digest.slice(2, 4), digest.slice(4));
}

async function writeCanonicalBlob(root: string, digest: `0x${string}`, bytes: Uint8Array) {
  const path = canonicalBlobPath(join(root, "blobs"), digest);
  await mkdir(join(path, ".."), { recursive: true });
  await writeFile(path, bytes);
}

async function writeCatalogRecord(root: string, record: {
  releaseDigest: `0x${string}`;
  sourceDigest: `0x${string}`;
  sourceLength: number;
  iconDigest: `0x${string}` | null;
  name: string;
  sequence: number;
}) {
  const path = join(root, "releases", `${record.releaseDigest.slice(2)}.json`);
  await writeFile(path, JSON.stringify({
    schema: "tohseno.catalog-record/1",
    releaseDigest: record.releaseDigest,
    envelope: {
      release: {
        display: { name: record.name, icon_sha256: record.iconDigest },
        source: { sha256: record.sourceDigest, byte_length: record.sourceLength },
        checkpoint_sequence: record.sequence,
      },
    },
  }));
}

async function rewriteCatalogRecord(
  root: string,
  releaseDigest: `0x${string}`,
  record: Parameters<typeof writeCatalogRecord>[1],
) {
  await writeFile(
    join(root, "releases", `${releaseDigest.slice(2)}.json`),
    JSON.stringify({
      schema: "tohseno.catalog-record/1",
      releaseDigest: record.releaseDigest,
      envelope: {
        release: {
          display: { name: record.name, icon_sha256: record.iconDigest },
          source: { sha256: record.sourceDigest, byte_length: record.sourceLength },
          checkpoint_sequence: record.sequence,
        },
      },
    }),
  );
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
