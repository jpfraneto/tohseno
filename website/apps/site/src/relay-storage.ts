import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import { chmod, lstat, mkdir, open, readFile, readdir, rename, rm, stat, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { INTENT_LIMITS } from "./intent-limits.ts";

export type RelayState = "uploading" | "ready" | "leased" | "completing";
export type TerminalRelayState = "completed" | "cancelled" | "expired" | "corrupt";

interface ChunkMetadata { index: number; byteLength: number; sha256: string }

interface RelayMetadata {
  schema: "tohseno.intent-relay-record/1";
  id: string;
  state: RelayState;
  createdAt: number;
  expiresAt: number;
  uploadExpiresAt: number;
  ciphertextBytes: number;
  chunkCount: number;
  ciphertextSha256: string;
  nonce: string;
  associatedData: string;
  uploadVerifier: string;
  statusVerifier: string;
  claimVerifier: string;
  leaseVerifier?: string;
  leaseExpiresAt?: number;
  chunks: ChunkMetadata[];
}

interface Tombstone {
  schema: "tohseno.intent-relay-tombstone/1";
  state: TerminalRelayState;
  statusVerifier: string;
  completedAt: number;
  expiresAt: number;
  leaseVerifier?: string;
}

export interface CreateRelayRecord {
  ciphertextBytes: number;
  chunkCount: number;
  ciphertextSha256: string;
  nonce: string;
  associatedData: string;
  uploadVerifier: string;
  statusVerifier: string;
  claimVerifier: string;
}

export interface RelayCapacity { maxRecords: number; maxBytes: number }

export class RelayStorageError extends Error {
  constructor(readonly status: number, message: string, readonly errorClass = "relay_invalid") {
    super(message);
  }
}

export class FilesystemRelayStorage {
  constructor(readonly root: string, private readonly capacity: RelayCapacity) {}

  async initialize(): Promise<void> {
    if (!this.root.startsWith("/")) throw new Error("relay root must be absolute");
    await mkdir(this.root, { recursive: true, mode: 0o700 });
    const linkDetails = await lstat(this.root);
    if (linkDetails.isSymbolicLink()) throw new Error("relay root must not be a symbolic link");
    const details = await stat(this.root);
    if (!details.isDirectory()) throw new Error("relay root is not a directory");
    await chmod(this.root, 0o700);
    const writeProbe = join(this.root, `.write-test-${randomBase64Url(8)}`);
    const probe = await open(writeProbe, "wx", 0o600);
    try { await probe.writeFile("ok"); await probe.sync(); } finally { await probe.close(); }
    await rm(writeProbe);
    await this.cleanup(100);
  }

  async create(input: CreateRelayRecord, now = Date.now()): Promise<{ id: string; expiresAt: number }> {
    await this.cleanup(20, now);
    await this.assertCapacity(input.ciphertextBytes);
    for (let attempts = 0; attempts < 5; attempts += 1) {
      const id = randomBase64Url(24);
      const directory = this.directory(id);
      try {
        await mkdir(directory, { mode: 0o700 });
        await mkdir(join(directory, "chunks"), { mode: 0o700 });
        const metadata: RelayMetadata = {
          schema: "tohseno.intent-relay-record/1",
          id,
          state: "uploading",
          createdAt: now,
          expiresAt: now + INTENT_LIMITS.relayLifetimeMs,
          uploadExpiresAt: now + INTENT_LIMITS.uploadLifetimeMs,
          ciphertextBytes: input.ciphertextBytes,
          chunkCount: input.chunkCount,
          ciphertextSha256: input.ciphertextSha256,
          nonce: input.nonce,
          associatedData: input.associatedData,
          uploadVerifier: input.uploadVerifier,
          statusVerifier: input.statusVerifier,
          claimVerifier: input.claimVerifier,
          chunks: [],
        };
        await this.writeJson(join(directory, "metadata.json"), metadata, true);
        return { id, expiresAt: metadata.expiresAt };
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code === "EEXIST") continue;
        throw error;
      }
    }
    throw new RelayStorageError(503, "Relay could not allocate a record", "capacity");
  }

  async uploadChunk(id: string, capability: string, index: number, bytes: Uint8Array, expectedDigest: string, now = Date.now()): Promise<{ duplicate: boolean }> {
    const metadata = await this.active(id, now);
    await verifyCapability(capability, metadata.uploadVerifier);
    if (metadata.state !== "uploading") throw new RelayStorageError(409, "Relay record is not uploading", "state");
    if (index < 0 || index >= metadata.chunkCount) throw new RelayStorageError(400, "Chunk index is out of range");
    if (bytes.byteLength === 0 || bytes.byteLength > INTENT_LIMITS.chunkBytes) throw new RelayStorageError(413, "Chunk body exceeds the relay limit", "body_limit");
    const actualDigest = sha256Hex(bytes);
    if (!safeHexEqual(actualDigest, expectedDigest)) throw new RelayStorageError(422, "Chunk digest does not match", "digest");
    const existing = metadata.chunks[index];
    if (existing) {
      if (existing.byteLength !== bytes.byteLength || !safeHexEqual(existing.sha256, actualDigest)) {
        throw new RelayStorageError(409, "A conflicting chunk already exists", "chunk_conflict");
      }
      return { duplicate: true };
    }
    if (index !== metadata.chunks.length) throw new RelayStorageError(409, "Encrypted chunks must be uploaded in order", "chunk_order");
    const path = this.chunkPath(id, index);
    let recoveredRetry = false;
    try {
      const handle = await open(path, "wx", 0o600);
      try { await handle.writeFile(bytes); await handle.sync(); } finally { await handle.close(); }
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      const details = await lstat(path);
      if (details.isSymbolicLink() || !details.isFile() || details.size !== bytes.byteLength) {
        throw new RelayStorageError(409, "A conflicting chunk already exists", "chunk_conflict");
      }
      const recovered = await readFile(path);
      if (!safeHexEqual(sha256Hex(recovered), actualDigest)) {
        throw new RelayStorageError(409, "A conflicting chunk already exists", "chunk_conflict");
      }
      recoveredRetry = true;
    }
    metadata.chunks.push({ index, byteLength: bytes.byteLength, sha256: actualDigest });
    await this.writeMetadata(metadata);
    return { duplicate: recoveredRetry };
  }

  async finalize(id: string, capability: string, now = Date.now()): Promise<{ expiresAt: number }> {
    const metadata = await this.active(id, now);
    await verifyCapability(capability, metadata.uploadVerifier);
    if (metadata.state === "ready") return { expiresAt: metadata.expiresAt };
    if (metadata.state !== "uploading") throw new RelayStorageError(409, "Relay record cannot be finalized", "state");
    if (metadata.chunks.length !== metadata.chunkCount) throw new RelayStorageError(409, "Encrypted upload is incomplete", "missing_chunks");
    const total = metadata.chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0);
    if (total !== metadata.ciphertextBytes) throw new RelayStorageError(422, "Ciphertext byte count does not match", "digest");
    const digest = createHash("sha256");
    for (let index = 0; index < metadata.chunkCount; index += 1) digest.update(await readFile(this.chunkPath(id, index)));
    if (!safeHexEqual(digest.digest("hex"), metadata.ciphertextSha256)) throw new RelayStorageError(422, "Ciphertext digest does not match", "digest");
    metadata.state = "ready";
    await this.writeMetadata(metadata);
    return { expiresAt: metadata.expiresAt };
  }

  async status(id: string, capability: string, now = Date.now()): Promise<{ state: RelayState | TerminalRelayState; expiresAt: number }> {
    validateRelayId(id);
    const tombstone = await this.readTombstone(id);
    if (tombstone) {
      await verifyCapability(capability, tombstone.statusVerifier);
      return { state: tombstone.state, expiresAt: tombstone.expiresAt };
    }
    const metadata = await this.active(id, now);
    await verifyCapability(capability, metadata.statusVerifier);
    const state = metadata.state === "completing" ? "leased" : metadata.state;
    return { state, expiresAt: metadata.expiresAt };
  }

  async lease(id: string, capability: string, now = Date.now()): Promise<Record<string, unknown>> {
    const metadata = await this.active(id, now);
    await verifyCapability(capability, metadata.claimVerifier);
    if (metadata.state !== "ready") throw new RelayStorageError(409, "Encrypted intention is not available to claim", "state");
    const leaseCapability = randomBase64Url(32);
    metadata.state = "leased";
    metadata.leaseVerifier = sha256Hex(Buffer.from(leaseCapability));
    metadata.leaseExpiresAt = now + INTENT_LIMITS.leaseLifetimeMs;
    await this.writeMetadata(metadata);
    return {
      schema: "tohseno.intent-relay-lease/1",
      lease_capability: leaseCapability,
      lease_expires_at: metadata.leaseExpiresAt,
      ciphertext_bytes: metadata.ciphertextBytes,
      chunk_count: metadata.chunkCount,
      ciphertext_sha256: metadata.ciphertextSha256,
      nonce: metadata.nonce,
      associated_data: metadata.associatedData,
      chunks: metadata.chunks.map(({ index, byteLength, sha256 }) => ({ index, byte_length: byteLength, sha256 })),
    };
  }

  async downloadChunk(id: string, leaseCapability: string, index: number, now = Date.now()): Promise<{ bytes: Uint8Array; sha256: string }> {
    const metadata = await this.active(id, now);
    await this.verifyLease(metadata, leaseCapability, now);
    const chunk = metadata.chunks[index];
    if (!chunk || index >= metadata.chunkCount) throw new RelayStorageError(404, "Encrypted chunk not found");
    const bytes = await readFile(this.chunkPath(id, index));
    if (bytes.byteLength !== chunk.byteLength || !safeHexEqual(sha256Hex(bytes), chunk.sha256)) {
      metadata.state = "completing";
      await this.writeMetadata(metadata);
      throw new RelayStorageError(422, "Stored ciphertext chunk is corrupt", "corrupt");
    }
    return { bytes, sha256: chunk.sha256 };
  }

  async release(id: string, leaseCapability: string, now = Date.now()): Promise<void> {
    const metadata = await this.active(id, now);
    await this.verifyLease(metadata, leaseCapability, now);
    if (metadata.state !== "leased") throw new RelayStorageError(409, "Claim lease cannot be released", "state");
    metadata.state = "ready";
    delete metadata.leaseVerifier;
    delete metadata.leaseExpiresAt;
    await this.writeMetadata(metadata);
  }

  async complete(id: string, leaseCapability: string, now = Date.now()): Promise<void> {
    validateRelayId(id);
    const completedTombstone = await this.readTombstone(id);
    if (completedTombstone?.state === "completed" && completedTombstone.leaseVerifier) {
      await verifyCapability(leaseCapability, completedTombstone.leaseVerifier);
      return;
    }
    const metadata = await this.active(id, now);
    await this.verifyLease(metadata, leaseCapability, now, true);
    if (metadata.state === "leased") {
      metadata.state = "completing";
      await this.writeMetadata(metadata);
    }
    if (metadata.state !== "completing") throw new RelayStorageError(409, "Claim cannot be completed", "state");
    await rm(join(this.directory(id), "chunks"), { recursive: true, force: true });
    const tombstone: Tombstone = {
      schema: "tohseno.intent-relay-tombstone/1",
      state: "completed",
      statusVerifier: metadata.statusVerifier,
      completedAt: now,
      expiresAt: now + INTENT_LIMITS.tombstoneLifetimeMs,
      leaseVerifier: metadata.leaseVerifier,
    };
    await this.writeJson(join(this.directory(id), "tombstone.json"), tombstone, true);
    await rm(join(this.directory(id), "metadata.json"), { force: true });
    await syncDirectory(this.directory(id));
  }

  async cancel(id: string, capability: string, now = Date.now()): Promise<void> {
    const metadata = await this.active(id, now);
    await verifyCapability(capability, metadata.uploadVerifier);
    if (metadata.state !== "uploading" && metadata.state !== "ready") throw new RelayStorageError(409, "Claimed transfer cannot be cancelled", "state");
    await this.terminalize(metadata, "cancelled", now);
  }

  async cleanup(limit = 20, now = Date.now()): Promise<number> {
    let cleaned = 0;
    let entries: string[];
    try { entries = await readdir(this.root); } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return 0;
      throw error;
    }
    for (const entry of entries.slice(0, limit)) {
      if (!/^[A-Za-z0-9_-]{32}$/.test(entry)) continue;
      const tombstone = await this.readTombstone(entry);
      if (tombstone) {
        if (tombstone.expiresAt <= now) { await rm(this.directory(entry), { recursive: true, force: true }); cleaned += 1; }
        continue;
      }
      let metadata: RelayMetadata;
      try { metadata = await this.readMetadata(entry); } catch {
        try {
          const details = await lstat(this.directory(entry));
          if (details.isDirectory() && !details.isSymbolicLink() && details.mtimeMs + INTENT_LIMITS.uploadLifetimeMs <= now) {
            await rm(this.directory(entry), { recursive: true, force: true });
            cleaned += 1;
          }
        } catch { /* a concurrent create or removal will be checked later */ }
        continue;
      }
      if (metadata.expiresAt <= now || (metadata.state === "uploading" && metadata.uploadExpiresAt <= now)) {
        await this.terminalize(metadata, "expired", now); cleaned += 1;
      } else if (metadata.state === "leased" && (metadata.leaseExpiresAt ?? 0) <= now) {
        metadata.state = "ready"; delete metadata.leaseVerifier; delete metadata.leaseExpiresAt;
        await this.writeMetadata(metadata); cleaned += 1;
      }
    }
    return cleaned;
  }

  private async active(id: string, now: number): Promise<RelayMetadata> {
    validateRelayId(id);
    if (await this.readTombstone(id)) throw new RelayStorageError(410, "Relay record is terminal", "terminal");
    const metadata = await this.readMetadata(id);
    if (metadata.expiresAt <= now || (metadata.state === "uploading" && metadata.uploadExpiresAt <= now)) {
      await this.terminalize(metadata, "expired", now);
      throw new RelayStorageError(410, "Relay record expired", "expired");
    }
    if (metadata.state === "leased" && (metadata.leaseExpiresAt ?? 0) <= now) {
      metadata.state = "ready"; delete metadata.leaseVerifier; delete metadata.leaseExpiresAt;
      await this.writeMetadata(metadata);
    }
    return metadata;
  }

  private async verifyLease(metadata: RelayMetadata, capability: string, now: number, allowCompleting = false): Promise<void> {
    if ((metadata.state !== "leased" && !(allowCompleting && metadata.state === "completing")) || !metadata.leaseVerifier || (metadata.leaseExpiresAt ?? 0) <= now) {
      throw new RelayStorageError(409, "Claim lease is not active", "lease");
    }
    await verifyCapability(capability, metadata.leaseVerifier);
  }

  private async terminalize(metadata: RelayMetadata, state: TerminalRelayState, now: number): Promise<void> {
    await rm(join(this.directory(metadata.id), "chunks"), { recursive: true, force: true });
    await this.writeJson(join(this.directory(metadata.id), "tombstone.json"), {
      schema: "tohseno.intent-relay-tombstone/1", state, statusVerifier: metadata.statusVerifier,
      completedAt: now, expiresAt: now + INTENT_LIMITS.tombstoneLifetimeMs,
    } satisfies Tombstone, true);
    await rm(join(this.directory(metadata.id), "metadata.json"), { force: true });
    await syncDirectory(this.directory(metadata.id));
  }

  private async assertCapacity(additionalBytes: number): Promise<void> {
    let records = 0; let bytes = 0;
    for (const entry of await readdir(this.root)) {
      if (!/^[A-Za-z0-9_-]{32}$/.test(entry)) continue;
      try { const metadata = await this.readMetadata(entry); records += 1; bytes += metadata.ciphertextBytes; } catch { /* tombstone or incomplete record */ }
    }
    if (records >= this.capacity.maxRecords || bytes + additionalBytes > this.capacity.maxBytes) {
      throw new RelayStorageError(503, "Encrypted relay capacity is temporarily exhausted", "capacity");
    }
  }

  private directory(id: string): string { validateRelayId(id); return join(this.root, id); }
  private chunkPath(id: string, index: number): string { return join(this.directory(id), "chunks", String(index).padStart(6, "0")); }
  private async readMetadata(id: string): Promise<RelayMetadata> {
    const metadata = JSON.parse(await readFile(join(this.directory(id), "metadata.json"), "utf8")) as RelayMetadata;
    if (metadata.schema !== "tohseno.intent-relay-record/1" || metadata.id !== id || !Array.isArray(metadata.chunks)) throw new RelayStorageError(500, "Relay metadata is corrupt", "corrupt");
    return metadata;
  }
  private async readTombstone(id: string): Promise<Tombstone | null> {
    try {
      const value = JSON.parse(await readFile(join(this.directory(id), "tombstone.json"), "utf8")) as Tombstone;
      return value.schema === "tohseno.intent-relay-tombstone/1" ? value : null;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
      return null;
    }
  }
  private writeMetadata(metadata: RelayMetadata): Promise<void> { return this.writeJson(join(this.directory(metadata.id), "metadata.json"), metadata); }
  private async writeJson(path: string, value: unknown, create = false): Promise<void> {
    const bytes = JSON.stringify(value);
    if (create) {
      const handle = await open(path, "wx", 0o600);
      try { await handle.writeFile(bytes); await handle.sync(); } finally { await handle.close(); }
      await syncDirectory(dirname(path)); return;
    }
    const temporary = `${path}.tmp-${randomBase64Url(8)}`;
    const handle = await open(temporary, "wx", 0o600);
    try { await handle.writeFile(bytes); await handle.sync(); } finally { await handle.close(); }
    await rename(temporary, path); await syncDirectory(dirname(path));
  }
}

export function validateRelayId(id: string): void {
  if (!/^[A-Za-z0-9_-]{32}$/.test(id)) throw new RelayStorageError(400, "Relay ID is malformed");
}
export function sha256Hex(bytes: Uint8Array): string { return createHash("sha256").update(bytes).digest("hex"); }
export async function verifyCapability(capability: string, verifier: string): Promise<void> {
  if (!/^[A-Za-z0-9_-]{43}$/.test(capability) || !/^[a-f0-9]{64}$/.test(verifier)) throw new RelayStorageError(401, "Relay capability is invalid", "authorization");
  if (!safeHexEqual(sha256Hex(Buffer.from(capability)), verifier)) throw new RelayStorageError(401, "Relay capability is invalid", "authorization");
}
function safeHexEqual(left: string, right: string): boolean {
  if (!/^[a-f0-9]{64}$/.test(left) || !/^[a-f0-9]{64}$/.test(right)) return false;
  return timingSafeEqual(Buffer.from(left, "hex"), Buffer.from(right, "hex"));
}
function randomBase64Url(bytes: number): string { return randomBytes(bytes).toString("base64url"); }
async function syncDirectory(path: string): Promise<void> { const handle = await open(path, "r"); try { await handle.sync(); } finally { await handle.close(); } }
