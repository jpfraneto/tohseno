import { randomUUID } from "node:crypto";
import {
  closeSync,
  constants,
  existsSync,
  fstatSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import {
  PUBLIC_SHOT_PROTOCOL_VERSION,
  createShotId,
  isShotId,
  type ShotId,
} from "../../protocol/src/index.ts";
import { CliError } from "./errors.ts";
import { readBoundedJson } from "./files.ts";

export const SHOT_PROTOCOL_STATE_PATH =
  ".tohseno/protocol-state.json" as const;

export interface ShotProtocolPointer {
  version: typeof PUBLIC_SHOT_PROTOCOL_VERSION;
  shotId: ShotId;
  statePath: typeof SHOT_PROTOCOL_STATE_PATH;
}

export interface LocalShotProtocolState {
  protocolVersion: typeof PUBLIC_SHOT_PROTOCOL_VERSION;
  shotId: ShotId;
  lifecycle: "EVOLVING";
  evolution: number;
}

interface EvolutionLockRecord {
  version: 1;
  token: string;
  pid: number;
  acquiredAt: string;
}

export interface ShotEvolutionLock {
  readonly path: string;
  readonly descriptor: number;
  readonly token: string;
  readonly device: number;
  readonly inode: number;
  readonly stateBeforeEvolution: Readonly<LocalShotProtocolState>;
}

export function createShotProtocolPointer(): ShotProtocolPointer {
  return {
    version: PUBLIC_SHOT_PROTOCOL_VERSION,
    shotId: createShotId(),
    statePath: SHOT_PROTOCOL_STATE_PATH,
  };
}

export function validateShotProtocolPointer(
  value: unknown,
): ShotProtocolPointer {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value)
  ) {
    throw new CliError("shot protocol pointer must be an object", 2);
  }
  const candidate = value as Record<string, unknown>;
  if (
    Object.keys(candidate).length !== 3 ||
    candidate.version !== PUBLIC_SHOT_PROTOCOL_VERSION ||
    !isShotId(candidate.shotId) ||
    candidate.statePath !== SHOT_PROTOCOL_STATE_PATH
  ) {
    throw new CliError("shot protocol pointer is invalid", 2);
  }
  return {
    version: PUBLIC_SHOT_PROTOCOL_VERSION,
    shotId: candidate.shotId,
    statePath: SHOT_PROTOCOL_STATE_PATH,
  };
}

export function initialShotProtocolState(
  shotId: ShotId,
): LocalShotProtocolState {
  if (!isShotId(shotId)) {
    throw new CliError("cannot initialize an invalid Shot ID", 2);
  }
  return {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    shotId,
    lifecycle: "EVOLVING",
    evolution: 0,
  };
}

export function validateLocalShotProtocolState(
  value: unknown,
): LocalShotProtocolState {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value)
  ) {
    throw new CliError("local Shot protocol state must be an object", 2);
  }
  const candidate = value as Record<string, unknown>;
  if (
    Object.keys(candidate).length !== 4 ||
    candidate.protocolVersion !== PUBLIC_SHOT_PROTOCOL_VERSION ||
    !isShotId(candidate.shotId) ||
    candidate.lifecycle !== "EVOLVING" ||
    !Number.isSafeInteger(candidate.evolution) ||
    (candidate.evolution as number) < 0
  ) {
    throw new CliError(
      "local Shot protocol state is invalid; pre-release compatibility is unsupported; create a fresh Shot with `tohseno`",
      2,
    );
  }
  return {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    shotId: candidate.shotId,
    lifecycle: "EVOLVING",
    evolution: candidate.evolution as number,
  };
}

function stateFile(root: string): string {
  return join(resolve(root), SHOT_PROTOCOL_STATE_PATH);
}

function evolutionRuntimeDirectory(root: string): string {
  return join(resolve(root), ".tohseno", "run");
}

function evolutionLockFile(root: string): string {
  return join(evolutionRuntimeDirectory(root), "evolution.lock");
}

function processIsAlive(pid: number): boolean {
  if (!Number.isSafeInteger(pid) || pid < 1) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

function validEvolutionLockRecord(
  value: unknown,
): value is EvolutionLockRecord {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value)
  ) {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  return Object.keys(candidate).length === 4 &&
    candidate.version === 1 &&
    typeof candidate.token === "string" &&
    /^[0-9a-f-]{36}$/u.test(candidate.token) &&
    Number.isSafeInteger(candidate.pid) &&
    (candidate.pid as number) > 0 &&
    typeof candidate.acquiredAt === "string" &&
    Number.isFinite(Date.parse(candidate.acquiredAt)) &&
    new Date(candidate.acquiredAt).toISOString() === candidate.acquiredAt;
}

function lockPathStillMatches(
  path: string,
  expected: { device: number; inode: number },
): boolean {
  try {
    const details = lstatSync(path);
    return !details.isSymbolicLink() &&
      details.isFile() &&
      details.nlink === 1 &&
      details.dev === expected.device &&
      details.ino === expected.inode;
  } catch {
    return false;
  }
}

function evolutionLockIsOwned(lock: ShotEvolutionLock): boolean {
  try {
    const details = fstatSync(lock.descriptor);
    if (
      details.dev !== lock.device ||
      details.ino !== lock.inode ||
      !lockPathStillMatches(lock.path, lock)
    ) {
      return false;
    }
    const record = readBoundedJson<unknown>(
      lock.path,
      4_096,
      "Shot evolution lock",
    );
    return validEvolutionLockRecord(record) && record.token === lock.token;
  } catch {
    return false;
  }
}

function requireEvolutionLock(
  root: string,
  lock: ShotEvolutionLock,
): void {
  if (
    resolve(lock.path) !== resolve(evolutionLockFile(root)) ||
    !evolutionLockIsOwned(lock)
  ) {
    throw new CliError(
      "the Shot evolution lock changed while the coding agent was running",
      2,
    );
  }
}

export function acquireShotEvolutionLock(
  root: string,
): ShotEvolutionLock {
  const local = join(resolve(root), ".tohseno");
  const localDetails = lstatSync(local, { throwIfNoEntry: false });
  if (
    localDetails === undefined ||
    localDetails.isSymbolicLink() ||
    !localDetails.isDirectory()
  ) {
    throw new CliError("Shot evolution requires a real .tohseno directory");
  }
  const runtime = evolutionRuntimeDirectory(root);
  try {
    mkdirSync(runtime, { mode: 0o700 });
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
  }
  const runtimeDetails = lstatSync(runtime);
  if (runtimeDetails.isSymbolicLink() || !runtimeDetails.isDirectory()) {
    throw new CliError("Shot evolution runtime must be a real directory");
  }
  const path = evolutionLockFile(root);
  while (true) {
    try {
      const descriptor = openSync(
        path,
        constants.O_CREAT |
          constants.O_EXCL |
          constants.O_RDWR |
          constants.O_NOFOLLOW,
        0o600,
      );
      const token = randomUUID();
      const details = fstatSync(descriptor);
      try {
        const record: EvolutionLockRecord = {
          version: 1,
          token,
          pid: process.pid,
          acquiredAt: new Date().toISOString(),
        };
        writeFileSync(descriptor, `${JSON.stringify(record)}\n`);
        fsyncSync(descriptor);
        const stateBeforeEvolution = readLocalShotProtocolState(root);
        if (stateBeforeEvolution === null) {
          throw new CliError(
            "pre-release compatibility is unsupported; create a fresh Shot with `tohseno`",
            2,
          );
        }
        return {
          path,
          descriptor,
          token,
          device: details.dev,
          inode: details.ino,
          stateBeforeEvolution: Object.freeze({ ...stateBeforeEvolution }),
        };
      } catch (error) {
        closeSync(descriptor);
        if (lockPathStillMatches(path, {
          device: details.dev,
          inode: details.ino,
        })) {
          unlinkSync(path);
        }
        throw error;
      }
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      const details = lstatSync(path);
      if (
        details.isSymbolicLink() ||
        !details.isFile() ||
        details.nlink !== 1
      ) {
        throw new CliError("the Shot evolution lock is unsafe", 2);
      }
      let record: unknown;
      try {
        record = readBoundedJson<unknown>(
          path,
          4_096,
          "Shot evolution lock",
        );
      } catch {
        throw new CliError(
          "the Shot evolution lock is invalid; inspect .tohseno/run before retrying",
          2,
        );
      }
      if (!validEvolutionLockRecord(record)) {
        throw new CliError(
          "the Shot evolution lock is invalid; inspect .tohseno/run before retrying",
          2,
        );
      }
      if (processIsAlive(record.pid)) {
        throw new CliError(
          "another process is already evolving this Shot",
          2,
        );
      }
      if (lockPathStillMatches(path, {
        device: details.dev,
        inode: details.ino,
      })) {
        unlinkSync(path);
        continue;
      }
    }
  }
}

export function releaseShotEvolutionLock(
  lock: ShotEvolutionLock,
): void {
  try {
    if (evolutionLockIsOwned(lock)) unlinkSync(lock.path);
  } catch {
    // Never remove a replacement owner's lock.
  } finally {
    closeSync(lock.descriptor);
  }
}

export function readLocalShotProtocolState(
  root: string,
): LocalShotProtocolState | null {
  const path = stateFile(root);
  if (!existsSync(path)) return null;
  try {
    return validateLocalShotProtocolState(
      readBoundedJson<unknown>(
        path,
        16_384,
        "local Shot protocol state",
      ),
    );
  } catch (error) {
    if (error instanceof CliError) throw error;
    throw new CliError(
      "local Shot protocol state is invalid; pre-release compatibility is unsupported; create a fresh Shot with `tohseno`",
      2,
    );
  }
}

export function writeLocalShotProtocolState(
  root: string,
  value: LocalShotProtocolState,
): void {
  const state = validateLocalShotProtocolState(value);
  const path = stateFile(root);
  const directory = join(resolve(root), ".tohseno");
  const directoryDetails = lstatSync(directory, { throwIfNoEntry: false });
  if (
    directoryDetails === undefined ||
    directoryDetails.isSymbolicLink() ||
    !directoryDetails.isDirectory()
  ) {
    throw new CliError("shot protocol state requires a real .tohseno directory");
  }
  const current = lstatSync(path, { throwIfNoEntry: false });
  if (
    current !== undefined &&
    (current.isSymbolicLink() || !current.isFile() || current.nlink !== 1)
  ) {
    throw new CliError("local Shot protocol state must be a single-link regular file");
  }
  const temporary = join(
    directory,
    `.protocol-state.writing-${process.pid}-${randomUUID()}`,
  );
  try {
    writeFileSync(
      temporary,
      `${JSON.stringify(state, null, 2)}\n`,
      { encoding: "utf8", flag: "wx", mode: 0o644 },
    );
    renameSync(temporary, path);
  } finally {
    try {
      unlinkSync(temporary);
    } catch {
      // A successful rename consumes the temporary path.
    }
  }
}

export function advanceLocalShotEvolution(
  root: string,
  pointer: ShotProtocolPointer,
  lock: ShotEvolutionLock,
): LocalShotProtocolState {
  requireEvolutionLock(root, lock);
  const current = readLocalShotProtocolState(root);
  const expected = lock.stateBeforeEvolution;
  if (current === null) {
    throw new CliError(
      "pre-release compatibility is unsupported; create a fresh Shot with `tohseno`",
      2,
    );
  }
  if (
    current.protocolVersion !== expected.protocolVersion ||
    current.shotId !== expected.shotId ||
    current.lifecycle !== expected.lifecycle ||
    current.evolution !== expected.evolution
  ) {
    throw new CliError(
      "local Shot protocol state changed while the coding agent was running; refusing to record a forged Evolution",
      2,
    );
  }
  if (current.shotId !== pointer.shotId) {
    throw new CliError("local Shot protocol state does not match shot metadata", 2);
  }
  const next: LocalShotProtocolState = {
    ...current,
    evolution: current.evolution + 1,
  };
  if (!Number.isSafeInteger(next.evolution)) {
    throw new CliError("local Shot evolution counter is exhausted");
  }
  requireEvolutionLock(root, lock);
  writeLocalShotProtocolState(root, next);
  return next;
}
