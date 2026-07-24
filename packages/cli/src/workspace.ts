import { randomUUID } from "node:crypto";
import {
  closeSync,
  existsSync,
  fstatSync,
  lstatSync,
  mkdirSync,
  openSync,
  readdirSync,
  realpathSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { isAbsolute, join, relative, resolve, sep } from "node:path";
import type { ResolvedConfig } from "./config.ts";
import { CliError } from "./errors.ts";
import { readBoundedJson } from "./files.ts";
import { readShotMetadata, type ShotMetadata } from "./shot.ts";
import { validateShotSlug } from "./slug.ts";

const ALLOCATION_SCHEMA_VERSION = 1 as const;
const ALLOCATION_LOCK_SCHEMA_VERSION = 1 as const;
const LOCK_STALE_AFTER_MS = 30_000;
const LOCK_WAIT_MS = 5_000;

interface AllocationState {
  schemaVersion: typeof ALLOCATION_SCHEMA_VERSION;
  lastSequence: number;
}

interface AllocationLockRecord {
  schemaVersion: typeof ALLOCATION_LOCK_SCHEMA_VERSION;
  token: string;
  pid: number;
  acquiredAt: string;
}

interface AllocationLock {
  path: string;
  descriptor: number;
  token: string;
  device: number;
  inode: number;
}

export interface AllocationTestHooks {
  lockStaleAfterMs?: number;
  lockWaitMs?: number;
  isProcessAlive?: (pid: number) => boolean;
  afterSequenceClaimed?: (sequence: number) => void | Promise<void>;
}

export interface DiscoveredShot {
  path: string;
  metadata: ShotMetadata;
  name: string;
}

function isInside(root: string, candidate: string): boolean {
  const fromRoot = relative(root, candidate);
  return fromRoot === "" ||
    (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

export function canonicalShotsDirectory(path: string): string {
  const requested = resolve(path);
  mkdirSync(requested, { recursive: true });
  const canonical = realpathSync(requested);
  if (!lstatSync(canonical).isDirectory()) {
    throw new CliError(`shots workspace is not a directory: ${canonical}`);
  }
  return canonical;
}

function workspaceControlDirectory(shotsDirectory: string): string {
  const root = canonicalShotsDirectory(shotsDirectory);
  const control = join(root, ".tohseno");
  mkdirSync(control, { recursive: true, mode: 0o700 });
  const details = lstatSync(control);
  if (details.isSymbolicLink() || !details.isDirectory()) {
    throw new CliError(`workspace control path is not a private directory: ${control}`);
  }
  return control;
}

function readAllocationState(path: string): AllocationState | null {
  if (lstatSync(path, { throwIfNoEntry: false }) === undefined) return null;
  try {
    const value = readBoundedJson<Partial<AllocationState>>(
      path,
      4_096,
      "workspace allocation state",
    );
    if (
      value.schemaVersion === ALLOCATION_SCHEMA_VERSION &&
      Number.isSafeInteger(value.lastSequence) &&
      (value.lastSequence ?? 0) >= 0
    ) {
      return value as AllocationState;
    }
  } catch {
    // The caller fails closed below.
  }
  throw new CliError(`workspace allocation state is invalid: ${path}`);
}

function highestExistingSequence(shotsDirectory: string): number {
  const shots = discoverShotsInDirectory(shotsDirectory);
  let highest = 0;
  for (const shot of shots) {
    if (
      typeof shot.metadata.sequence === "number" &&
      Number.isSafeInteger(shot.metadata.sequence) &&
      shot.metadata.sequence > highest
    ) {
      highest = shot.metadata.sequence;
    }
  }
  return Math.max(highest, shots.length);
}

function processIsAlive(pid: number): boolean {
  if (!Number.isSafeInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

function readAllocationLockRecord(path: string): AllocationLockRecord | null {
  try {
    const value = readBoundedJson<Partial<AllocationLockRecord>>(
      path,
      4_096,
      "workspace allocation lock",
    );
    if (
      value.schemaVersion !== ALLOCATION_LOCK_SCHEMA_VERSION ||
      typeof value.token !== "string" ||
      !/^[a-f0-9-]{36}$/u.test(value.token) ||
      !Number.isSafeInteger(value.pid) ||
      (value.pid ?? 0) <= 0 ||
      typeof value.acquiredAt !== "string" ||
      !Number.isFinite(Date.parse(value.acquiredAt))
    ) {
      return null;
    }
    return value as AllocationLockRecord;
  } catch {
    return null;
  }
}

function lockPathStillMatches(
  path: string,
  expected: { device: number; inode: number },
): boolean {
  try {
    const details = lstatSync(path);
    return !details.isSymbolicLink() &&
      details.isFile() &&
      details.dev === expected.device &&
      details.ino === expected.inode;
  } catch {
    return false;
  }
}

function allocationLockIsOwned(lock: AllocationLock): boolean {
  try {
    const descriptor = fstatSync(lock.descriptor);
    if (
      descriptor.dev !== lock.device ||
      descriptor.ino !== lock.inode ||
      !lockPathStillMatches(lock.path, lock)
    ) {
      return false;
    }
    return readAllocationLockRecord(lock.path)?.token === lock.token;
  } catch {
    return false;
  }
}

class LostAllocationLockError extends Error {
  constructor() {
    super("workspace allocation lock ownership changed");
    this.name = "LostAllocationLockError";
  }
}

function requireAllocationLock(lock: AllocationLock): void {
  if (!allocationLockIsOwned(lock)) throw new LostAllocationLockError();
}

async function acquireAllocationLockWithHooks(
  control: string,
  hooks: AllocationTestHooks,
): Promise<AllocationLock> {
  const path = join(control, "allocation.lock");
  const started = Date.now();
  const staleAfter = hooks.lockStaleAfterMs ?? LOCK_STALE_AFTER_MS;
  const waitFor = hooks.lockWaitMs ?? LOCK_WAIT_MS;
  const isProcessAlive = hooks.isProcessAlive ?? processIsAlive;
  while (true) {
    try {
      const descriptor = openSync(path, "wx", 0o600);
      const token = randomUUID();
      const details = fstatSync(descriptor);
      try {
        const record: AllocationLockRecord = {
          schemaVersion: ALLOCATION_LOCK_SCHEMA_VERSION,
          token,
          pid: process.pid,
          acquiredAt: new Date().toISOString(),
        };
        writeFileSync(descriptor, `${JSON.stringify(record)}\n`);
        return {
          path,
          descriptor,
          token,
          device: details.dev,
          inode: details.ino,
        };
      } catch (error) {
        closeSync(descriptor);
        try {
          if (lockPathStillMatches(path, {
            device: details.dev,
            inode: details.ino,
          })) {
            unlinkSync(path);
          }
        } catch {
          // Never remove a replacement lock while cleaning a failed acquire.
        }
        throw error;
      }
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (code !== "EEXIST") throw error;
      let recovered = false;
      try {
        const details = lstatSync(path);
        const record = readAllocationLockRecord(path);
        const age = Date.now() - details.mtimeMs;
        const abandoned = record !== null && !isProcessAlive(record.pid);
        const invalidAndStale = record === null && age > staleAfter;
        if (
          (abandoned || invalidAndStale) &&
          lockPathStillMatches(path, {
            device: details.dev,
            inode: details.ino,
          })
        ) {
          unlinkSync(path);
          recovered = true;
        }
      } catch {
        recovered = !existsSync(path);
      }
      if (recovered) continue;
      if (Date.now() - started >= waitFor) {
        throw new CliError("another process is allocating a shot; retry in a moment");
      }
      await new Promise<void>((resolveDelay) => setTimeout(resolveDelay, 20));
    }
  }
}

function releaseAllocationLock(lock: AllocationLock): void {
  try {
    if (allocationLockIsOwned(lock)) unlinkSync(lock.path);
  } catch {
    // Losing a lock path must never remove a replacement owner's lock.
  } finally {
    closeSync(lock.descriptor);
  }
}

function allocationClaimsDirectory(control: string): string {
  const path = join(control, "allocations");
  mkdirSync(path, { recursive: true, mode: 0o700 });
  const details = lstatSync(path);
  if (details.isSymbolicLink() || !details.isDirectory()) {
    throw new CliError(`workspace allocation claims path is unsafe: ${path}`);
  }
  return path;
}

function claimShotSequence(
  directory: string,
  firstCandidate: number,
): number {
  let sequence = firstCandidate;
  while (Number.isSafeInteger(sequence) && sequence > 0) {
    const path = join(
      directory,
      `${String(sequence).padStart(16, "0")}.json`,
    );
    try {
      const descriptor = openSync(path, "wx", 0o600);
      try {
        writeFileSync(
          descriptor,
          `${JSON.stringify({
            schemaVersion: ALLOCATION_SCHEMA_VERSION,
            sequence,
            pid: process.pid,
            claimedAt: new Date().toISOString(),
          })}\n`,
        );
      } finally {
        closeSync(descriptor);
      }
      return sequence;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      sequence += 1;
    }
  }
  throw new CliError("shot sequence space is exhausted");
}

export async function allocateShotSequence(
  shotsDirectory: string,
  hooks: AllocationTestHooks = {},
): Promise<number> {
  const root = canonicalShotsDirectory(shotsDirectory);
  const control = workspaceControlDirectory(root);
  const claims = allocationClaimsDirectory(control);
  while (true) {
    const lock = await acquireAllocationLockWithHooks(control, hooks);
    try {
      requireAllocationLock(lock);
      const statePath = join(control, "allocation.json");
      const state = readAllocationState(statePath);
      const next = Math.max(
        state?.lastSequence ?? 0,
        highestExistingSequence(root),
      ) + 1;
      const sequence = claimShotSequence(claims, next);
      await hooks.afterSequenceClaimed?.(sequence);
      requireAllocationLock(lock);
      const latestState = readAllocationState(statePath);
      const temporary = `${statePath}.writing-${process.pid}-${randomUUID()}`;
      const value: AllocationState = {
        schemaVersion: ALLOCATION_SCHEMA_VERSION,
        lastSequence: Math.max(
          latestState?.lastSequence ?? 0,
          sequence,
        ),
      };
      try {
        writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, {
          flag: "wx",
          mode: 0o600,
        });
        requireAllocationLock(lock);
        renameSync(temporary, statePath);
      } finally {
        if (existsSync(temporary)) unlinkSync(temporary);
      }
      requireAllocationLock(lock);
      return sequence;
    } catch (error) {
      if (!(error instanceof LostAllocationLockError)) throw error;
      // The exclusive sequence claim remains consumed. Retrying from current
      // state cannot return it twice, even if a suspended allocator resumed.
    } finally {
      releaseAllocationLock(lock);
    }
  }
}

export function discoverShotsInDirectory(
  shotsDirectory: string,
): DiscoveredShot[] {
  if (!existsSync(shotsDirectory)) return [];
  const root = realpathSync(resolve(shotsDirectory));
  return readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith("."))
    .flatMap((entry) => {
      const path = join(root, entry.name);
      const metadata = readShotMetadata(path);
      if (metadata === undefined) return [];
      let name = metadata.slug;
      try {
        const manifest = readBoundedJson<{
          application?: { name?: unknown };
        }>(
          join(path, "continuity.manifest.json"),
          1_048_576,
          "continuity manifest",
        );
        if (
          typeof manifest.application?.name === "string" &&
          manifest.application.name.length <= 80 &&
          !/[\u0000-\u001f\u007f-\u009f]/u.test(
            manifest.application.name,
          )
        ) {
          name = manifest.application.name;
        }
      } catch {
        // Verification owns malformed-manifest diagnostics.
      }
      return [{ path, metadata, name }];
    });
}

export function discoverShotsNewestFirst(
  shotsDirectory: string,
): DiscoveredShot[] {
  return discoverShotsInDirectory(shotsDirectory).sort((left, right) => {
    const timeDifference = Date.parse(right.metadata.createdAt) -
      Date.parse(left.metadata.createdAt);
    if (Number.isFinite(timeDifference) && timeDifference !== 0) {
      return timeDifference;
    }
    const sequenceDifference = (right.metadata.sequence ?? 0) -
      (left.metadata.sequence ?? 0);
    return sequenceDifference || left.metadata.slug.localeCompare(right.metadata.slug);
  });
}

export function recognizedShotBySlug(
  shotsDirectory: string,
  slugValue: string,
): DiscoveredShot {
  const slug = validateShotSlug(slugValue);
  const root = canonicalShotsDirectory(shotsDirectory);
  const candidate = join(root, slug);
  if (!existsSync(candidate) || !lstatSync(candidate).isDirectory()) {
    throw new CliError(`shot does not exist: ${candidate}`, 2);
  }
  const canonical = realpathSync(candidate);
  if (!isInside(root, canonical) || canonical === root) {
    throw new CliError("shot path leaves the configured workspace", 2);
  }
  const metadata = readShotMetadata(canonical);
  if (metadata === undefined || metadata.slug !== slug) {
    throw new CliError(`not a recognized shot: ${candidate}`, 2);
  }
  return discoverShotsInDirectory(root).find(
    (shot) => shot.metadata.slug === slug,
  ) ?? { path: canonical, metadata, name: slug };
}

export function resolveRecognizedShot(
  value: string | undefined,
  context: { config: ResolvedConfig; cwd: string },
): DiscoveredShot {
  if (value === undefined) {
    let candidate = resolve(context.cwd);
    while (true) {
      const metadata = readShotMetadata(candidate);
      if (metadata !== undefined) {
        return {
          path: candidate,
          metadata,
          name: metadata.slug,
        };
      }
      const parent = resolve(candidate, "..");
      if (parent === candidate) {
        throw new CliError(
          "operation requires the current shot or an explicit shot slug/path",
          2,
        );
      }
      candidate = parent;
    }
  }
  const looksLikePath = isAbsolute(value) ||
    value.startsWith(".") ||
    value.includes(sep) ||
    value.includes("/");
  if (!looksLikePath) {
    return recognizedShotBySlug(context.config.shotsDirectory, value);
  }
  const candidate = resolve(context.cwd, value);
  if (!existsSync(candidate) || !lstatSync(candidate).isDirectory()) {
    throw new CliError(`shot does not exist: ${candidate}`, 2);
  }
  const canonical = realpathSync(candidate);
  const metadata = readShotMetadata(canonical);
  if (metadata === undefined) {
    throw new CliError(`not a recognized shot: ${canonical}`, 2);
  }
  return { path: canonical, metadata, name: metadata.slug };
}
