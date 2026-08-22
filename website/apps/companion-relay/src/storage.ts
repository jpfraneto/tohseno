import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import { constants } from "node:fs";
import {
  chmod,
  lstat,
  mkdir,
  open,
  readdir,
  rename,
  rm,
} from "node:fs/promises";
import { dirname, join } from "node:path";
import type { CompanionRelayConfig } from "../config.ts";
import { RelayError } from "./errors.ts";
import type {
  MailboxEvent,
  PushRegistration,
  RelayEnvelope,
  RelayMetrics,
} from "./types.ts";

type PairingState = "waiting" | "responded" | "cancelled";

interface PairingMetadata {
  schema: "tohseno.companion-relay-pairing/1";
  id: string;
  state: PairingState;
  createdAt: number;
  expiresAt: number;
  purgeAt?: number;
  readVerifier: string;
  cancelVerifier: string;
  responseBytes?: number;
  responseSha256?: string;
}

interface EnvelopeRecord {
  id: string;
  cursor: number;
  senderDeviceId: string;
  senderSequence: number;
  createdAt: number;
  expiresAt: number;
  bytes: number;
  sha256: string;
  discarded?: boolean;
}

interface MailboxMetadata {
  schema: "tohseno.companion-relay-mailbox/1";
  id: string;
  createdAt: number;
  writeVerifier: string;
  readVerifier: string;
  ackVerifier: string;
  revokeVerifier: string;
  pushVerifier: string;
  nextCursor: number;
  acknowledgedCursor: number;
  resetBeforeCursor: number;
  revocationEpoch: number;
  revokedAt?: number;
  purgeAt?: number;
  senderHighWater: Record<string, number>;
  envelopes: EnvelopeRecord[];
}

export interface CapabilityVerifiers {
  write: string;
  read: string;
  ack: string;
  revoke: string;
  push: string;
}

export interface PairingVerifiers {
  read: string;
  cancel: string;
}

export interface MailboxPage {
  resetRequired: boolean;
  resetBeforeCursor: number;
  nextCursor: number;
  headCursor: number;
  hasMore: boolean;
  envelopes: Array<{ cursor: number; envelope: RelayEnvelope }>;
}

type Listener = (event: MailboxEvent) => void;

export class CompanionRelayStorage {
  private readonly locks = new Map<string, Promise<void>>();
  private readonly listeners = new Map<string, Set<Listener>>();

  constructor(
    readonly root: string,
    private readonly limits: CompanionRelayConfig["limits"],
  ) {}

  async initialize(): Promise<void> {
    if (!this.root.startsWith("/")) throw new Error("companion relay root must be absolute");
    await ensurePrivateDirectory(this.root);
    await ensurePrivateDirectory(this.pairingsRoot());
    await ensurePrivateDirectory(this.mailboxesRoot());
    await ensurePrivateDirectory(this.pushRoot());
    const probePath = join(this.root, `.write-test-${randomId(8)}`);
    const probe = await open(probePath, "wx", 0o600);
    try {
      await probe.writeFile("ok");
      await probe.sync();
    } finally {
      await probe.close();
    }
    await rm(probePath);
    await this.cleanup(100);
  }

  async createPairing(
    verifiers: PairingVerifiers,
    expiresAt: number,
    now = Date.now(),
  ): Promise<{ id: string; expiresAt: number }> {
    validateVerifierSet(Object.values(verifiers));
    if (expiresAt <= now || expiresAt > now + this.limits.pairingLifetimeMs) {
      throw new RelayError(400, "Pairing expiry must be in the configured short-lived window", "expiry");
    }
    return this.withLock("storage", async () => {
      await this.cleanupUnlocked(20, now);
      await assertPrivateDirectory(this.root);
      await assertPrivateDirectory(this.pairingsRoot());
      const active = await this.countPairings(now);
      if (active >= this.limits.pairingSessions) {
        throw new RelayError(503, "Pairing capacity is temporarily exhausted", "capacity");
      }
      for (let attempt = 0; attempt < 5; attempt += 1) {
        const id = randomId(24);
        const directory = this.pairingDirectory(id);
        try {
          await mkdir(directory, { mode: 0o700 });
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code === "EEXIST") continue;
          throw error;
        }
        const metadata: PairingMetadata = {
          schema: "tohseno.companion-relay-pairing/1",
          id,
          state: "waiting",
          createdAt: now,
          expiresAt,
          readVerifier: verifiers.read,
          cancelVerifier: verifiers.cancel,
        };
        await writeJson(join(directory, "metadata.json"), metadata, true);
        return { id, expiresAt };
      }
      throw new RelayError(503, "Pairing session could not be allocated", "capacity");
    });
  }

  async submitPairingResponse(
    id: string,
    bytes: Uint8Array,
    now = Date.now(),
  ): Promise<{ duplicate: boolean }> {
    validateOpaqueId(id, "pairing session ID");
    if (bytes.byteLength === 0 || bytes.byteLength > this.limits.pairingResponseBytes) {
      throw new RelayError(413, "Pairing response exceeds the relay limit", "body_limit");
    }
    return this.withLock("storage", async () => {
      const metadata = await this.activePairing(id, now);
      if (metadata.state === "cancelled") throw new RelayError(410, "Pairing session is no longer active", "terminal");
      const digest = sha256Hex(bytes);
      if (metadata.state === "responded") {
        if (metadata.responseBytes !== bytes.byteLength || !safeHexEqual(metadata.responseSha256 ?? "", digest)) {
          throw new RelayError(409, "A pairing response was already submitted", "duplicate_conflict");
        }
        return { duplicate: true };
      }
      const path = join(this.pairingDirectory(id), "response.bin");
      await writeBytesExclusiveOrVerify(path, bytes, digest);
      metadata.state = "responded";
      metadata.responseBytes = bytes.byteLength;
      metadata.responseSha256 = digest;
      await this.writePairing(metadata);
      return { duplicate: false };
    });
  }

  async readPairingResponse(
    id: string,
    capability: string,
    now = Date.now(),
  ): Promise<Uint8Array | null> {
    return this.withLock("storage", () => this.readPairingResponseUnlocked(
      id,
      capability,
      now,
    ));
  }

  private async readPairingResponseUnlocked(
    id: string,
    capability: string,
    now = Date.now(),
  ): Promise<Uint8Array | null> {
    const metadata = await this.activePairing(id, now);
    verifyCapability(capability, metadata.readVerifier);
    if (metadata.state === "waiting") return null;
    if (metadata.state !== "responded") throw new RelayError(410, "Pairing session is no longer active", "terminal");
    const bytes = await readBoundedBytes(
      join(this.pairingDirectory(id), "response.bin"),
      this.limits.pairingResponseBytes,
    );
    if (
      bytes.byteLength !== metadata.responseBytes ||
      !safeHexEqual(sha256Hex(bytes), metadata.responseSha256 ?? "")
    ) {
      throw new RelayError(500, "Pairing response storage is corrupt", "corrupt");
    }
    return bytes;
  }

  async cancelPairing(
    id: string,
    capability: string,
    now = Date.now(),
  ): Promise<void> {
    validateOpaqueId(id, "pairing session ID");
    await this.withLock("storage", async () => {
      const metadata = await this.readPairing(id);
      verifyCapability(capability, metadata.cancelVerifier);
      if (metadata.state !== "cancelled") {
        metadata.state = "cancelled";
        metadata.purgeAt = now + this.limits.pairingLifetimeMs;
        delete metadata.responseBytes;
        delete metadata.responseSha256;
        // Publish the terminal state before deleting its payload. A crash may
        // leave an unreachable file, but never live metadata with no response.
        await this.writePairing(metadata);
      }
      await rm(join(this.pairingDirectory(id), "response.bin"), { force: true });
    });
  }

  async createMailbox(
    verifiers: CapabilityVerifiers,
    now = Date.now(),
  ): Promise<{ id: string; createdAt: number }> {
    validateVerifierSet(Object.values(verifiers));
    return this.withLock("storage", async () => {
      await this.cleanupUnlocked(20, now);
      await assertPrivateDirectory(this.root);
      await assertPrivateDirectory(this.mailboxesRoot());
      const count = (await this.mailboxIds()).length;
      if (count >= this.limits.mailboxes) {
        throw new RelayError(503, "Mailbox capacity is temporarily exhausted", "capacity");
      }
      for (let attempt = 0; attempt < 5; attempt += 1) {
        const id = randomId(24);
        const directory = this.mailboxDirectory(id);
        try {
          await mkdir(directory, { mode: 0o700 });
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code === "EEXIST") continue;
          throw error;
        }
        await mkdir(join(directory, "envelopes"), { mode: 0o700 });
        const metadata: MailboxMetadata = {
          schema: "tohseno.companion-relay-mailbox/1",
          id,
          createdAt: now,
          writeVerifier: verifiers.write,
          readVerifier: verifiers.read,
          ackVerifier: verifiers.ack,
          revokeVerifier: verifiers.revoke,
          pushVerifier: verifiers.push,
          nextCursor: 1,
          acknowledgedCursor: 0,
          resetBeforeCursor: 0,
          revocationEpoch: 0,
          senderHighWater: {},
          envelopes: [],
        };
        await writeJson(join(directory, "metadata.json"), metadata, true);
        return { id, createdAt: now };
      }
      throw new RelayError(503, "Mailbox could not be allocated", "capacity");
    });
  }

  async uploadEnvelope(
    mailboxId: string,
    capability: string,
    envelope: RelayEnvelope,
    canonicalBytes: Uint8Array,
    now = Date.now(),
  ): Promise<{ duplicate: boolean; cursor: number }> {
    validateOpaqueId(mailboxId, "mailbox ID");
    return this.withLock("storage", async () => {
      await this.cleanupUnlocked(20, now);
      const metadata = await this.activeMailbox(mailboxId);
      await this.assertEnvelopeDirectory(mailboxId);
      verifyCapability(capability, metadata.writeVerifier);
      if (envelope.mailbox_id !== mailboxId) {
        throw new RelayError(400, "Envelope mailbox does not match its route", "schema");
      }
      const existing = metadata.envelopes.find((item) => item.id === envelope.envelope_id);
      const digest = sha256Hex(canonicalBytes);
      if (existing) {
        if (existing.bytes !== canonicalBytes.byteLength || !safeHexEqual(existing.sha256, digest)) {
          throw new RelayError(409, "Envelope ID was already used with different bytes", "duplicate_conflict");
        }
        return { duplicate: true, cursor: existing.cursor };
      }
      const highWater = metadata.senderHighWater[envelope.sender_device_id] ?? 0;
      if (envelope.sender_sequence <= highWater) {
        throw new RelayError(409, "Sender sequence is not newer than the replay watermark", "replay");
      }
      if (!(envelope.sender_device_id in metadata.senderHighWater)
        && Object.keys(metadata.senderHighWater).length >= 64) {
        throw new RelayError(409, "Mailbox sender capacity is exhausted", "capacity");
      }
      if (metadata.envelopes.length >= this.limits.mailboxEnvelopes) {
        throw new RelayError(503, "Mailbox envelope capacity is temporarily exhausted", "capacity");
      }
      const metrics = await this.metricsUnlocked();
      if (
        metrics.envelopes >= this.limits.envelopes ||
        metrics.bytes + canonicalBytes.byteLength > this.limits.bytes
      ) {
        throw new RelayError(503, "Envelope capacity is temporarily exhausted", "capacity");
      }
      const cursor = metadata.nextCursor;
      const record: EnvelopeRecord = {
        id: envelope.envelope_id,
        cursor,
        senderDeviceId: envelope.sender_device_id,
        senderSequence: envelope.sender_sequence,
        createdAt: Date.parse(envelope.created_at),
        expiresAt: Date.parse(envelope.expires_at) + this.limits.clockSkewMs,
        bytes: canonicalBytes.byteLength,
        sha256: digest,
      };
      const path = this.envelopePath(mailboxId, envelope.envelope_id);
      await writeBytesExclusiveOrVerify(path, canonicalBytes, digest);
      metadata.envelopes.push(record);
      metadata.senderHighWater[envelope.sender_device_id] = envelope.sender_sequence;
      metadata.nextCursor += 1;
      await this.writeMailbox(metadata);
      this.emit(mailboxId, { kind: "envelope", cursor, envelope });
      return { duplicate: false, cursor };
    });
  }

  async listEnvelopes(
    mailboxId: string,
    capability: string,
    cursor: number,
    limit: number,
    now = Date.now(),
  ): Promise<MailboxPage> {
    return this.withLock("storage", () => this.listEnvelopesUnlocked(
      mailboxId,
      capability,
      cursor,
      limit,
      now,
    ));
  }

  private async listEnvelopesUnlocked(
    mailboxId: string,
    capability: string,
    cursor: number,
    limit: number,
    now = Date.now(),
  ): Promise<MailboxPage> {
    validateOpaqueId(mailboxId, "mailbox ID");
    const metadata = await this.activeMailbox(mailboxId);
    await this.assertEnvelopeDirectory(mailboxId);
    verifyCapability(capability, metadata.readVerifier);
    if (!Number.isSafeInteger(cursor) || cursor < 0) {
      throw new RelayError(400, "Cursor must be a non-negative whole number", "cursor");
    }
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > this.limits.catchUpLimit) {
      throw new RelayError(400, "Catch-up limit is out of range", "limit");
    }
    const expired = new Set<string>();
    for (const record of metadata.envelopes) {
      if (record.expiresAt > now) continue;
      if (!record.discarded) {
        metadata.resetBeforeCursor = Math.max(metadata.resetBeforeCursor, record.cursor);
        record.discarded = true;
      }
      expired.add(record.id);
    }
    if (expired.size > 0) {
      // Make the expired records unreachable before removing bytes. Keeping
      // the discarded records in metadata until deletion finishes makes the
      // cleanup restartable at every crash boundary.
      await this.writeMailbox(metadata);
      for (const id of expired) {
        await rm(this.envelopePath(mailboxId, id), { force: true });
      }
      metadata.envelopes = metadata.envelopes.filter((record) => !expired.has(record.id));
      await this.writeMailbox(metadata);
    }
    const headCursor = metadata.nextCursor - 1;
    if (cursor < metadata.resetBeforeCursor) {
      return {
        resetRequired: true,
        resetBeforeCursor: metadata.resetBeforeCursor,
        nextCursor: cursor,
        headCursor,
        hasMore: false,
        envelopes: [],
      };
    }
    const candidates = metadata.envelopes
      .filter((item) => !item.discarded && item.expiresAt > now && item.cursor > cursor)
      .sort((left, right) => left.cursor - right.cursor);
    const selected = candidates.slice(0, limit);
    const envelopes: MailboxPage["envelopes"] = [];
    for (const record of selected) {
      const bytes = await readBoundedBytes(
        this.envelopePath(mailboxId, record.id),
        this.limits.envelopeBodyBytes,
      );
      if (bytes.byteLength !== record.bytes || !safeHexEqual(sha256Hex(bytes), record.sha256)) {
        throw new RelayError(500, "Envelope storage is corrupt", "corrupt");
      }
      let envelope: RelayEnvelope;
      try {
        envelope = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)) as RelayEnvelope;
      } catch {
        throw new RelayError(500, "Envelope storage is corrupt", "corrupt");
      }
      envelopes.push({ cursor: record.cursor, envelope });
    }
    const nextCursor = envelopes.at(-1)?.cursor ?? cursor;
    return {
      resetRequired: false,
      resetBeforeCursor: metadata.resetBeforeCursor,
      nextCursor,
      headCursor,
      hasMore: candidates.length > selected.length,
      envelopes,
    };
  }

  async acknowledge(
    mailboxId: string,
    capability: string,
    cursor: number,
  ): Promise<{ acknowledgedCursor: number }> {
    validateOpaqueId(mailboxId, "mailbox ID");
    return this.withLock("storage", async () => {
      const metadata = await this.activeMailbox(mailboxId);
      await this.assertEnvelopeDirectory(mailboxId);
      verifyCapability(capability, metadata.ackVerifier);
      if (!Number.isSafeInteger(cursor) || cursor < 0 || cursor >= metadata.nextCursor) {
        throw new RelayError(400, "Acknowledgement cursor is out of range", "cursor");
      }
      if (cursor <= metadata.acknowledgedCursor) {
        for (const record of metadata.envelopes) {
          if (record.cursor <= cursor && record.discarded) {
            await rm(this.envelopePath(mailboxId, record.id), { force: true });
          }
        }
        return { acknowledgedCursor: metadata.acknowledgedCursor };
      }
      for (const record of metadata.envelopes) {
        if (record.cursor <= cursor && !record.discarded) {
          record.discarded = true;
        }
      }
      metadata.acknowledgedCursor = cursor;
      metadata.resetBeforeCursor = Math.max(metadata.resetBeforeCursor, cursor);
      // Commit the acknowledgement before deleting opaque bytes. Retrying the
      // same acknowledgement finishes any deletion interrupted by a crash.
      await this.writeMailbox(metadata);
      for (const record of metadata.envelopes) {
        if (record.cursor <= cursor && record.discarded) {
          await rm(this.envelopePath(mailboxId, record.id), { force: true });
        }
      }
      return { acknowledgedCursor: cursor };
    });
  }

  async revokeMailbox(
    mailboxId: string,
    capability: string,
    now = Date.now(),
  ): Promise<{ revocationEpoch: number }> {
    validateOpaqueId(mailboxId, "mailbox ID");
    return this.withLock("storage", async () => {
      const metadata = await this.readMailbox(mailboxId);
      await this.assertEnvelopeDirectory(mailboxId);
      verifyCapability(capability, metadata.revokeVerifier);
      if (!metadata.revokedAt) {
        for (const record of metadata.envelopes) record.discarded = true;
        metadata.resetBeforeCursor = metadata.nextCursor - 1;
        metadata.revocationEpoch += 1;
        metadata.revokedAt = now;
        metadata.purgeAt = now + this.limits.revocationRetentionMs;
        // Revocation becomes durable before payload cleanup. A retry can finish
        // cleanup, while no crash can make a partially deleted mailbox active.
        await this.writeMailbox(metadata);
        this.emit(mailboxId, {
          kind: "revoked",
          cursor: metadata.nextCursor - 1,
        });
      }
      for (const record of metadata.envelopes) {
        await rm(this.envelopePath(mailboxId, record.id), { force: true });
      }
      metadata.envelopes = [];
      await assertPrivateDirectory(this.pushRoot());
      await removePrivateDirectoryIfPresent(this.pushDirectory(mailboxId));
      await this.writeMailbox(metadata);
      return { revocationEpoch: metadata.revocationEpoch };
    });
  }

  async registerPush(
    mailboxId: string,
    capability: string,
    deviceId: string,
    token: string,
    now = Date.now(),
  ): Promise<void> {
    validateOpaqueId(mailboxId, "mailbox ID");
    validateDeviceId(deviceId);
    if (token.trim().length === 0 || new TextEncoder().encode(token).byteLength > 512) {
      throw new RelayError(400, "APNs device token is malformed", "schema");
    }
    await this.withLock("storage", async () => {
      const metadata = await this.activeMailbox(mailboxId);
      verifyCapability(capability, metadata.pushVerifier);
      const directory = this.pushDirectory(mailboxId);
      await assertPrivateDirectory(this.pushRoot());
      await ensurePrivateDirectory(directory);
      const registrations = (await readdir(directory)).filter((entry) => DEVICE_ID.test(entry));
      if (!registrations.includes(deviceId) && registrations.length >= 8) {
        throw new RelayError(409, "Push registration capacity is exhausted", "capacity");
      }
      const registration: PushRegistration = {
        schema: "tohseno.companion-push-registration/1",
        mailboxId,
        deviceId,
        token,
        registeredAt: now,
      };
      await writeJson(join(directory, deviceId), registration);
    });
  }

  async removePush(
    mailboxId: string,
    capability: string,
    deviceId: string,
  ): Promise<void> {
    validateOpaqueId(mailboxId, "mailbox ID");
    validateDeviceId(deviceId);
    await this.withLock("storage", async () => {
      const metadata = await this.readMailbox(mailboxId);
      verifyCapability(capability, metadata.pushVerifier);
      await assertPrivateDirectory(this.pushRoot());
      await safeReaddir(this.pushDirectory(mailboxId));
      await rm(join(this.pushDirectory(mailboxId), deviceId), { force: true });
    });
  }

  async pushRegistrations(mailboxId: string): Promise<PushRegistration[]> {
    validateOpaqueId(mailboxId, "mailbox ID");
    const metadata = await this.activeMailbox(mailboxId);
    if (metadata.revokedAt) return [];
    const directory = this.pushDirectory(mailboxId);
    await assertPrivateDirectory(this.pushRoot());
    const entries = await safeReaddir(directory);
    const registrations: PushRegistration[] = [];
    for (const entry of entries.filter((candidate) => DEVICE_ID.test(candidate)).slice(0, 8)) {
      registrations.push(await readJson<PushRegistration>(join(directory, entry), 4 * 1024));
    }
    return registrations;
  }

  subscribe(mailboxId: string, listener: Listener): () => void {
    validateOpaqueId(mailboxId, "mailbox ID");
    const listeners = this.listeners.get(mailboxId) ?? new Set<Listener>();
    listeners.add(listener);
    this.listeners.set(mailboxId, listeners);
    return () => {
      listeners.delete(listener);
      if (listeners.size === 0) this.listeners.delete(mailboxId);
    };
  }

  async authorizeLive(mailboxId: string, capability: string): Promise<void> {
    const metadata = await this.activeMailbox(mailboxId);
    verifyCapability(capability, metadata.readVerifier);
  }

  async cleanup(limit = 100, now = Date.now()): Promise<number> {
    return this.withLock("storage", () => this.cleanupUnlocked(limit, now));
  }

  async metrics(): Promise<RelayMetrics> {
    return this.metricsUnlocked();
  }

  private async cleanupUnlocked(limit: number, now: number): Promise<number> {
    let cleaned = 0;
    const pairings = await safeReaddir(this.pairingsRoot());
    for (const id of pairings.filter((entry) => OPAQUE_ID.test(entry)).slice(0, limit)) {
      try {
        const metadata = await this.readPairing(id);
        if (metadata.expiresAt <= now || (metadata.purgeAt ?? Number.MAX_SAFE_INTEGER) <= now) {
          await rm(this.pairingDirectory(id), { recursive: true, force: true });
          cleaned += 1;
        }
      } catch {
        // Corrupt/tampered records remain for operator inspection and never become active.
      }
    }

    const remaining = Math.max(0, limit - cleaned);
    const mailboxIds = (await this.mailboxIds()).slice(0, remaining);
    for (const id of mailboxIds) {
      try {
        const metadata = await this.readMailbox(id);
        if (metadata.revokedAt && (metadata.purgeAt ?? Number.MAX_SAFE_INTEGER) <= now) {
          await rm(this.mailboxDirectory(id), { recursive: true, force: true });
          await assertPrivateDirectory(this.pushRoot());
          await removePrivateDirectoryIfPresent(this.pushDirectory(id));
          cleaned += 1;
          continue;
        }
        const removeIds = new Set<string>();
        for (const record of metadata.envelopes) {
          if (cleaned >= limit) break;
          if (record.expiresAt > now) continue;
          if (!record.discarded) {
            metadata.resetBeforeCursor = Math.max(metadata.resetBeforeCursor, record.cursor);
            record.discarded = true;
          }
          removeIds.add(record.id);
          cleaned += 1;
        }
        if (removeIds.size > 0) {
          // Persist the logical discard first, then delete and finally compact
          // metadata. Each intermediate state is safe to resume after restart.
          await this.writeMailbox(metadata);
          for (const envelopeId of removeIds) {
            await rm(this.envelopePath(id, envelopeId), { force: true });
          }
          metadata.envelopes = metadata.envelopes.filter((record) => !removeIds.has(record.id));
          await this.writeMailbox(metadata);
        }
        if (cleaned >= limit) break;
      } catch {
        // Corrupt/tampered records stay unavailable and are not blindly deleted.
      }
    }
    return cleaned;
  }

  private async metricsUnlocked(): Promise<RelayMetrics> {
    let pairingSessions = 0;
    let mailboxes = 0;
    let revokedMailboxes = 0;
    let envelopes = 0;
    let bytes = 0;
    let pushRegistrations = 0;
    const now = Date.now();
    for (const id of (await safeReaddir(this.pairingsRoot())).filter((entry) => OPAQUE_ID.test(entry))) {
      try {
        const metadata = await this.readPairing(id);
        if (metadata.state !== "cancelled" && metadata.expiresAt > now) pairingSessions += 1;
      } catch { /* unavailable records are excluded */ }
    }
    for (const id of await this.mailboxIds()) {
      try {
        const metadata = await this.readMailbox(id);
        mailboxes += 1;
        if (metadata.revokedAt) revokedMailboxes += 1;
        for (const record of metadata.envelopes) {
          if (!record.discarded && record.expiresAt > now) {
            envelopes += 1;
            bytes += record.bytes;
          }
        }
        pushRegistrations += (await safeReaddir(this.pushDirectory(id)))
          .filter((entry) => DEVICE_ID.test(entry)).length;
      } catch { /* unavailable records are excluded */ }
    }
    return {
      pairingSessions,
      mailboxes,
      revokedMailboxes,
      envelopes,
      bytes,
      pushRegistrations,
      liveSubscribers: [...this.listeners.values()].reduce((sum, listeners) => sum + listeners.size, 0),
    };
  }

  private emit(mailboxId: string, event: MailboxEvent): void {
    for (const listener of this.listeners.get(mailboxId) ?? []) listener(event);
  }

  private async activePairing(id: string, now: number): Promise<PairingMetadata> {
    const metadata = await this.readPairing(id);
    if (metadata.state === "cancelled" || metadata.expiresAt <= now) {
      throw new RelayError(410, "Pairing session is no longer active", "expired");
    }
    return metadata;
  }

  private async activeMailbox(id: string): Promise<MailboxMetadata> {
    const metadata = await this.readMailbox(id);
    if (metadata.revokedAt) throw new RelayError(410, "Mailbox capability has been revoked", "revoked");
    return metadata;
  }

  private async countPairings(now: number): Promise<number> {
    let count = 0;
    for (const id of (await safeReaddir(this.pairingsRoot())).filter((entry) => OPAQUE_ID.test(entry))) {
      try {
        const metadata = await this.readPairing(id);
        if (metadata.state !== "cancelled" && metadata.expiresAt > now) count += 1;
      } catch { /* unavailable records are excluded */ }
    }
    return count;
  }

  private async mailboxIds(): Promise<string[]> {
    return (await safeReaddir(this.mailboxesRoot())).filter((entry) => OPAQUE_ID.test(entry));
  }

  private async readPairing(id: string): Promise<PairingMetadata> {
    validateOpaqueId(id, "pairing session ID");
    await assertPrivateDirectory(this.root);
    await assertPrivateDirectory(this.pairingsRoot());
    await assertPrivateDirectory(this.pairingDirectory(id));
    const value = await readJson<PairingMetadata>(join(this.pairingDirectory(id), "metadata.json"), 16 * 1024);
    if (value.schema !== "tohseno.companion-relay-pairing/1" || value.id !== id) {
      throw new RelayError(500, "Pairing metadata is corrupt", "corrupt");
    }
    return value;
  }

  private async readMailbox(id: string): Promise<MailboxMetadata> {
    validateOpaqueId(id, "mailbox ID");
    await assertPrivateDirectory(this.root);
    await assertPrivateDirectory(this.mailboxesRoot());
    await assertPrivateDirectory(this.mailboxDirectory(id));
    await this.assertEnvelopeDirectory(id);
    const value = await readJson<MailboxMetadata>(join(this.mailboxDirectory(id), "metadata.json"), 4 * 1024 * 1024);
    if (
      value.schema !== "tohseno.companion-relay-mailbox/1" ||
      value.id !== id ||
      !Array.isArray(value.envelopes) ||
      !value.senderHighWater
    ) {
      throw new RelayError(500, "Mailbox metadata is corrupt", "corrupt");
    }
    return value;
  }

  private writePairing(metadata: PairingMetadata): Promise<void> {
    return writeJson(join(this.pairingDirectory(metadata.id), "metadata.json"), metadata);
  }

  private writeMailbox(metadata: MailboxMetadata): Promise<void> {
    return writeJson(join(this.mailboxDirectory(metadata.id), "metadata.json"), metadata);
  }

  private pairingsRoot(): string { return join(this.root, "pairings"); }
  private mailboxesRoot(): string { return join(this.root, "mailboxes"); }
  private pushRoot(): string { return join(this.root, "push"); }
  private pairingDirectory(id: string): string { validateOpaqueId(id, "pairing session ID"); return join(this.pairingsRoot(), id); }
  private mailboxDirectory(id: string): string { validateOpaqueId(id, "mailbox ID"); return join(this.mailboxesRoot(), id); }
  private pushDirectory(id: string): string { validateOpaqueId(id, "mailbox ID"); return join(this.pushRoot(), id); }
  private envelopePath(mailboxId: string, envelopeId: string): string {
    validateEnvelopeId(envelopeId);
    return join(this.mailboxDirectory(mailboxId), "envelopes", `${envelopeId}.json`);
  }

  private assertEnvelopeDirectory(mailboxId: string): Promise<void> {
    return assertPrivateDirectory(join(this.mailboxDirectory(mailboxId), "envelopes"));
  }

  private async withLock<T>(key: string, task: () => Promise<T>): Promise<T> {
    const previous = this.locks.get(key) ?? Promise.resolve();
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const tail = previous.then(() => gate);
    this.locks.set(key, tail);
    await previous;
    try {
      return await task();
    } finally {
      release();
      if (this.locks.get(key) === tail) this.locks.delete(key);
    }
  }
}

const OPAQUE_ID = /^[A-Za-z0-9_-]{32}$/;
const DEVICE_ID = /^[A-Za-z0-9_-]{16,128}$/;
const ENVELOPE_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export function validateOpaqueId(value: string, name: string): void {
  if (!OPAQUE_ID.test(value)) throw new RelayError(400, `${name} is malformed`, "schema");
}

export function validateDeviceId(value: string): void {
  if (!DEVICE_ID.test(value)) throw new RelayError(400, "device ID is malformed", "schema");
}

export function validateEnvelopeId(value: string): void {
  if (!ENVELOPE_ID.test(value)) throw new RelayError(400, "envelope ID is malformed", "schema");
}

export function validateVerifier(value: string): void {
  if (!/^[a-f0-9]{64}$/.test(value)) {
    throw new RelayError(400, "Capability verifier must be a lowercase SHA-256 digest", "schema");
  }
}

function validateVerifierSet(values: string[]): void {
  for (const value of values) validateVerifier(value);
  if (new Set(values).size !== values.length) {
    throw new RelayError(400, "Relay capabilities must be independent", "schema");
  }
}

export function verifyCapability(capability: string, verifier: string): void {
  if (!/^[A-Za-z0-9_-]{43}$/.test(capability) || !/^[a-f0-9]{64}$/.test(verifier)) {
    throw new RelayError(401, "Relay capability is invalid", "authorization");
  }
  const capabilityBytes = Buffer.from(capability, "base64url");
  if (
    capabilityBytes.byteLength !== 32 ||
    capabilityBytes.toString("base64url") !== capability ||
    !safeHexEqual(sha256Hex(capabilityBytes), verifier)
  ) {
    throw new RelayError(401, "Relay capability is invalid", "authorization");
  }
}

export function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function safeHexEqual(left: string, right: string): boolean {
  if (!/^[a-f0-9]{64}$/.test(left) || !/^[a-f0-9]{64}$/.test(right)) return false;
  return timingSafeEqual(Buffer.from(left, "hex"), Buffer.from(right, "hex"));
}

function randomId(bytes: number): string {
  return randomBytes(bytes).toString("base64url");
}

async function ensurePrivateDirectory(path: string): Promise<void> {
  await mkdir(path, { recursive: true, mode: 0o700 });
  await assertPrivateDirectory(path);
  await chmod(path, 0o700);
}

async function assertPrivateDirectory(path: string): Promise<void> {
  let details;
  try {
    details = await lstat(path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      throw new RelayError(404, "Relay record was not found", "not_found");
    }
    throw error;
  }
  if (
    details.isSymbolicLink() ||
    !details.isDirectory() ||
    (typeof process.getuid === "function" && details.uid !== process.getuid())
  ) {
    throw new RelayError(500, "Relay storage path is unsafe", "unsafe_path");
  }
  if ((details.mode & 0o077) !== 0) {
    throw new RelayError(500, "Relay storage directory permissions are unsafe", "unsafe_path");
  }
}

async function readBoundedBytes(path: string, maximum: number): Promise<Uint8Array> {
  let handle;
  try {
    handle = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      throw new RelayError(404, "Relay record was not found", "not_found");
    }
    throw new RelayError(500, "Relay storage path is unsafe", "unsafe_path");
  }
  try {
    const details = await handle.stat();
    if (
      !details.isFile() ||
      details.size > maximum ||
      (details.mode & 0o077) !== 0 ||
      (typeof process.getuid === "function" && details.uid !== process.getuid())
    ) {
      throw new RelayError(500, "Relay storage record is invalid", "corrupt");
    }
    return new Uint8Array(await handle.readFile());
  } finally {
    await handle.close();
  }
}

async function readJson<T>(path: string, maximum: number): Promise<T> {
  const bytes = await readBoundedBytes(path, maximum);
  try {
    return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)) as T;
  } catch {
    throw new RelayError(500, "Relay metadata is corrupt", "corrupt");
  }
}

async function writeJson(path: string, value: unknown, create = false): Promise<void> {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  if (create) {
    const handle = await open(path, "wx", 0o600);
    try {
      await handle.writeFile(bytes);
      await handle.sync();
    } finally {
      await handle.close();
    }
    await syncDirectory(dirname(path));
    return;
  }
  try {
    const current = await lstat(path);
    if (
      current.isSymbolicLink() ||
      !current.isFile() ||
      (current.mode & 0o077) !== 0 ||
      (typeof process.getuid === "function" && current.uid !== process.getuid())
    ) {
      throw new RelayError(500, "Relay storage path is unsafe", "unsafe_path");
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
  const temporary = `${path}.tmp-${randomId(8)}`;
  const handle = await open(temporary, "wx", 0o600);
  try {
    await handle.writeFile(bytes);
    await handle.sync();
  } finally {
    await handle.close();
  }
  await rename(temporary, path);
  await syncDirectory(dirname(path));
}

async function writeBytesExclusiveOrVerify(
  path: string,
  bytes: Uint8Array,
  digest: string,
): Promise<void> {
  try {
    const handle = await open(path, "wx", 0o600);
    try {
      await handle.writeFile(bytes);
      await handle.sync();
    } finally {
      await handle.close();
    }
    await syncDirectory(dirname(path));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
    const current = await readBoundedBytes(path, bytes.byteLength);
    if (current.byteLength !== bytes.byteLength || !safeHexEqual(sha256Hex(current), digest)) {
      throw new RelayError(409, "Stored retry bytes conflict", "duplicate_conflict");
    }
  }
}

async function syncDirectory(path: string): Promise<void> {
  const handle = await open(path, "r");
  try { await handle.sync(); } finally { await handle.close(); }
}

async function safeReaddir(path: string): Promise<string[]> {
  try {
    await assertPrivateDirectory(path);
    return await readdir(path);
  } catch (error) {
    if (error instanceof RelayError && error.status === 404) return [];
    throw error;
  }
}

async function removePrivateDirectoryIfPresent(path: string): Promise<void> {
  try {
    await assertPrivateDirectory(path);
  } catch (error) {
    if (error instanceof RelayError && error.status === 404) return;
    throw error;
  }
  await rm(path, { recursive: true, force: true });
}
