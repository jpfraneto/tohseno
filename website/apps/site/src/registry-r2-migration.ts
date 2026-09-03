import { lstat, mkdir, open, readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type { RegistryBlobDescriptor, RegistryBlobStore } from "./blob-store.ts";

const MAX_RECORD_BYTES = 1024 * 1024;
const HEX32 = /^0x[0-9a-f]{64}$/;

export interface RegistryR2MigrationAudit {
  schema: "tohseno.registry-r2-migration-audit/1";
  mode: "dry-run" | "apply";
  sourceCommit: string;
  observedAt: string;
  bucketFingerprint: `0x${string}`;
  catalogFingerprint: `0x${string}` | null;
  catalogRecordCount: number;
  catalogReferencedBlobCount: number;
  blobCount: number;
  unreferencedBlobCount: number;
  byteCount: number;
  digests: `0x${string}`[];
  anky: {
    releaseDigest: `0x${string}`;
    sourceDigest: `0x${string}`;
    catalogRecordSHA256: `0x${string}`;
    checkpointSequence: number;
  } | null;
  failures: Array<{ digest?: string; stage: string; message: string }>;
  passed: boolean;
  auditFilename?: string;
}

interface MigrationBlob extends RegistryBlobDescriptor {
  localPath: string;
}

interface MigrationPlan {
  records: number;
  blobs: MigrationBlob[];
  referencedBlobs: number;
  anky: RegistryR2MigrationAudit["anky"];
  catalogFingerprint: `0x${string}` | null;
}

export async function migrateRegistryBlobs(options: {
  root: string;
  blobStore: RegistryBlobStore;
  mode: "dry-run" | "apply";
  sourceCommit: string;
  observedAt: string;
  bucketFingerprint: `0x${string}`;
}): Promise<RegistryR2MigrationAudit> {
  const failures: RegistryR2MigrationAudit["failures"] = [];
  let plan: MigrationPlan = {
    records: 0, blobs: [], referencedBlobs: 0, anky: null, catalogFingerprint: null,
  };
  try {
    plan = await migrationPlan(options.root);
  } catch (error) {
    failures.push({ stage: "local_audit", message: publicFailure(error) });
  }
  const audit: RegistryR2MigrationAudit = {
    schema: "tohseno.registry-r2-migration-audit/1",
    mode: options.mode,
    sourceCommit: options.sourceCommit,
    observedAt: options.observedAt,
    bucketFingerprint: options.bucketFingerprint,
    catalogFingerprint: plan.catalogFingerprint,
    catalogRecordCount: plan.records,
    catalogReferencedBlobCount: plan.referencedBlobs,
    blobCount: plan.blobs.length,
    unreferencedBlobCount: plan.blobs.length - plan.referencedBlobs,
    byteCount: plan.blobs.reduce((sum, blob) => sum + blob.byteLength, 0),
    digests: plan.blobs.map((blob) => blob.digest),
    anky: plan.anky,
    failures,
    passed: failures.length === 0,
  };
  if (options.mode === "dry-run") return audit;

  if (failures.length === 0) {
    try {
      await options.blobStore.initialize();
      for (const blob of plan.blobs) {
        const stagingID = migrationStagingID(blob.digest, options.sourceCommit);
        try {
          await options.blobStore.stagePending(stagingID, "source", blob.localPath, blob);
          await options.blobStore.promotePending(stagingID, "source", blob);
          await options.blobStore.removePending(stagingID);
        } catch (error) {
          failures.push({ digest: blob.digest, stage: "r2_upload", message: publicFailure(error) });
        }
      }
    } catch (error) {
      failures.push({ stage: "r2_initialization", message: publicFailure(error) });
    }
  }
  if (plan.catalogFingerprint) {
    try {
      const after = await currentCatalogFingerprint(options.root);
      if (after !== plan.catalogFingerprint) {
        failures.push({
          stage: "catalog_consistency",
          message: "Registry catalog records changed during the migration",
        });
      }
    } catch (error) {
      failures.push({ stage: "catalog_consistency", message: publicFailure(error) });
    }
  }
  audit.passed = failures.length === 0;
  const auditDirectory = join(options.root, "r2-migration-audits");
  await mkdir(auditDirectory, { recursive: true, mode: 0o700 });
  const safeTimestamp = options.observedAt.replaceAll(":", "-");
  audit.auditFilename = `${safeTimestamp}.json`;
  const auditPath = join(auditDirectory, audit.auditFilename);
  await writeFile(auditPath, `${JSON.stringify(audit, null, 2)}\n`, { mode: 0o600, flag: "wx" });
  return audit;
}

export function r2BucketFingerprint(accountID: string, bucket: string): `0x${string}` {
  const bytes = new TextEncoder().encode(`TOHSENO-R2-BUCKET-V1\0${accountID}\0${bucket}`);
  return `0x${new Bun.CryptoHasher("sha256").update(bytes).digest("hex")}`;
}

async function migrationPlan(root: string): Promise<MigrationPlan> {
  const releasesRoot = join(root, "releases");
  const names = (await readdir(releasesRoot)).filter((name) => /^[0-9a-f]{64}\.json$/.test(name)).sort();
  if (names.length > 100_000) throw new Error("catalog contains too many release records");
  const catalogFingerprint = await catalogFingerprintForNames(releasesRoot, names);
  const blobs = await inventoryLocalBlobs(root);
  const referenced = new Set<string>();
  let anky: RegistryR2MigrationAudit["anky"] = null;
  let ankySequence = 0;
  for (const name of names) {
    const loaded = await safeJSON(join(releasesRoot, name));
    const record = loaded.value;
    const releaseDigest = hex32(record.releaseDigest, "release digest");
    if (`${releaseDigest.slice(2)}.json` !== name) throw new Error("release filename differs from its digest");
    const envelope = object(record.envelope, "catalog envelope");
    const release = object(envelope.release, "catalog release");
    const source = object(release.source, "catalog source");
    const sourceDigest = hex32(source.sha256, "source digest");
    const sourceLength = positiveInteger(source.byte_length, "source byte length");
    requireCatalogBlob(blobs, referenced, releaseDigest, sourceDigest, sourceLength);
    const display = object(release.display, "catalog display");
    if (display.icon_sha256 !== null && display.icon_sha256 !== undefined) {
      const iconDigest = hex32(display.icon_sha256, "icon digest");
      if (release.schema === "tohseno.catalog-release/2") {
        requireCatalogBlob(blobs, referenced, releaseDigest, iconDigest,
          positiveInteger(display.icon_byte_length, "icon byte length"));
      } else {
        const icon = blobs.get(iconDigest);
        if (!icon) {
          throw new Error(`catalog release ${releaseDigest} icon ${iconDigest} is absent from local storage`);
        }
        referenced.add(iconDigest);
      }
    }
    const screenshots = Array.isArray(display.screenshots) ? display.screenshots : [];
    for (const value of screenshots) {
      const screenshot = object(value, "catalog screenshot");
      requireCatalogBlob(blobs, referenced, releaseDigest,
        hex32(screenshot.sha256, "screenshot digest"),
        positiveInteger(screenshot.byte_length, "screenshot byte length"));
    }
    if (typeof display.name === "string" && display.name.toLocaleLowerCase("en-US") === "anky") {
      const sequence = positiveInteger(release.checkpoint_sequence, "checkpoint sequence");
      if (sequence >= ankySequence) {
        anky = {
          releaseDigest,
          sourceDigest,
          catalogRecordSHA256: loaded.sha256,
          checkpointSequence: sequence,
        };
        ankySequence = sequence;
      }
    }
  }
  const ordered = [...blobs.values()].sort((left, right) => left.digest.localeCompare(right.digest));
  for (const blob of ordered) await verifyLocalBlob(blob);
  return {
    records: names.length,
    blobs: ordered,
    referencedBlobs: referenced.size,
    anky,
    catalogFingerprint,
  };
}

function requireCatalogBlob(
  blobs: Map<string, MigrationBlob>,
  referenced: Set<string>,
  releaseDigest: `0x${string}`,
  digest: `0x${string}`,
  byteLength: number,
): void {
  const existing = blobs.get(digest);
  if (!existing) {
    throw new Error(`catalog release ${releaseDigest} source ${digest} is absent from local storage`);
  }
  if (existing.byteLength !== byteLength) {
    throw new Error(`catalog release ${releaseDigest} source ${digest} has the wrong local length`);
  }
  referenced.add(digest);
}

async function inventoryLocalBlobs(root: string): Promise<Map<string, MigrationBlob>> {
  const storageRoot = join(root, "blobs", "sha256");
  const prefixes = await readdir(storageRoot, { withFileTypes: true });
  const blobs = new Map<string, MigrationBlob>();
  for (const prefix of prefixes) {
    if (!prefix.isDirectory() || prefix.isSymbolicLink() || !/^[0-9a-f]{2}$/.test(prefix.name)) {
      throw new Error("local blob storage contains a noncanonical prefix entry");
    }
    const prefixRoot = join(storageRoot, prefix.name);
    const entries = await readdir(prefixRoot, { withFileTypes: true });
    for (const entry of entries) {
      if (!entry.isFile() || entry.isSymbolicLink() || !/^[0-9a-f]{62}$/.test(entry.name)) {
        throw new Error("local blob storage contains a noncanonical object entry");
      }
      const digest = `0x${prefix.name}${entry.name}` as `0x${string}`;
      const localPath = localBlobPath(root, digest);
      const details = await safeFile(localPath);
      blobs.set(digest, { digest, byteLength: details.size, localPath });
      if (blobs.size > 200_000) throw new Error("local blob storage contains too many objects");
    }
  }
  return blobs;
}

function localBlobPath(root: string, digest: `0x${string}`): string {
  const hex = digest.slice(2);
  return join(root, "blobs", "sha256", hex.slice(0, 2), hex.slice(2));
}

async function verifyLocalBlob(blob: MigrationBlob): Promise<void> {
  const initial = await safeFile(blob.localPath);
  if (initial.size !== blob.byteLength) throw new Error(`local blob ${blob.digest} has the wrong length`);
  const file = await open(blob.localPath, "r");
  const hasher = new Bun.CryptoHasher("sha256");
  const buffer = new Uint8Array(1024 * 1024);
  try {
    while (true) {
      const { bytesRead } = await file.read(buffer, 0, buffer.byteLength, null);
      if (bytesRead === 0) break;
      hasher.update(buffer.subarray(0, bytesRead));
    }
  } finally {
    await file.close();
  }
  const final = await safeFile(blob.localPath);
  if (initial.dev !== final.dev || initial.ino !== final.ino || initial.size !== final.size
      || initial.mtimeMs !== final.mtimeMs) {
    throw new Error(`local blob ${blob.digest} changed during migration audit`);
  }
  if (`0x${hasher.digest("hex")}` !== blob.digest) {
    throw new Error(`local blob ${blob.digest} failed SHA-256 verification`);
  }
}

async function safeFile(path: string) {
  const details = await lstat(path);
  if (!details.isFile() || details.isSymbolicLink()) throw new Error("catalog blob is not a regular file");
  return details;
}

async function safeJSON(path: string): Promise<{
  value: Record<string, unknown>;
  sha256: `0x${string}`;
}> {
  const details = await safeFile(path);
  if (details.size < 2 || details.size > MAX_RECORD_BYTES) throw new Error("catalog record is outside its size bound");
  const bytes = await readFile(path);
  const final = await safeFile(path);
  if (details.dev !== final.dev || details.ino !== final.ino || details.size !== final.size
      || details.mtimeMs !== final.mtimeMs) throw new Error("catalog record changed while it was read");
  return {
    value: object(JSON.parse(bytes.toString("utf8")), "catalog record"),
    sha256: `0x${new Bun.CryptoHasher("sha256").update(bytes).digest("hex")}`,
  };
}

async function currentCatalogFingerprint(root: string): Promise<`0x${string}`> {
  const releasesRoot = join(root, "releases");
  const names = (await readdir(releasesRoot))
    .filter((name) => /^[0-9a-f]{64}\.json$/.test(name))
    .sort();
  if (names.length > 100_000) throw new Error("catalog contains too many release records");
  return catalogFingerprintForNames(releasesRoot, names);
}

async function catalogFingerprintForNames(
  releasesRoot: string,
  names: string[],
): Promise<`0x${string}`> {
  const hasher = new Bun.CryptoHasher("sha256");
  hasher.update("TOHSENO-REGISTRY-CATALOG-MIGRATION-V1\0");
  for (const name of names) {
    const loaded = await safeJSON(join(releasesRoot, name));
    hasher.update(name);
    hasher.update("\0");
    hasher.update(loaded.sha256);
    hasher.update("\0");
  }
  return `0x${hasher.digest("hex")}`;
}

function object(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${label} is invalid`);
  return value as Record<string, unknown>;
}

function hex32(value: unknown, label: string): `0x${string}` {
  if (typeof value !== "string" || !HEX32.test(value)) throw new Error(`${label} is invalid`);
  return value as `0x${string}`;
}

function positiveInteger(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) {
    throw new Error(`${label} is invalid`);
  }
  return value;
}

function migrationStagingID(digest: string, commit: string): string {
  return new Bun.CryptoHasher("sha256")
    .update(`TOHSENO-R2-MIGRATION-PENDING-V1\0${digest}\0${commit}`)
    .digest("hex")
    .slice(0, 32);
}

function publicFailure(error: unknown): string {
  if (!(error instanceof Error)) return "migration failed safely";
  return error.message.replaceAll(/[/\\][^\s]+/g, "[local path]").slice(0, 300);
}
