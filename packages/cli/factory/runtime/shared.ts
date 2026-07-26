import {
  closeSync,
  constants,
  existsSync,
  fchmodSync,
  fstatSync,
  ftruncateSync,
  lstatSync,
  mkdirSync,
  openSync,
  readSync,
  realpathSync,
  writeSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import {
  APP_MANIFEST_SCHEMA_VERSION,
  type AppManifest,
  validateAppManifest,
} from "../manifest/app.ts";

export const MACHINE_PROTOCOL_VERSION = 1 as const;
export const MAX_RUNTIME_LOG_BYTES = 5 * 1_048_576;
export const MAX_TAIL_READ_BYTES = 2 * 1_048_576;
export const MAX_CAPTURED_OUTPUT_BYTES = 8 * 1_048_576;
export const PRE_RELEASE_UNSUPPORTED =
  "pre-release compatibility is unsupported; create a fresh Shot" as const;
const MAX_RUNTIME_JSON_BYTES = 1_048_576;

export const MACHINE_EXIT = Object.freeze({
  success: 0,
  invalidConfiguration: 2,
  missingDependency: 3,
  unhealthyServices: 4,
  internalFailure: 5,
});

export type MachineExitCode = typeof MACHINE_EXIT[keyof typeof MACHINE_EXIT];
export type MachineErrorCode =
  | "INVALID_CONFIGURATION"
  | "MISSING_DEPENDENCY"
  | "UNHEALTHY_SERVICES"
  | "INTERNAL_FAILURE";

export class MachineError extends Error {
  readonly code: MachineErrorCode;
  readonly exitCode: MachineExitCode;
  readonly details: Record<string, unknown> | undefined;

  constructor(
    code: MachineErrorCode,
    message: string,
    details?: Record<string, unknown>,
  ) {
    super(message);
    this.name = "MachineError";
    this.code = code;
    this.exitCode = code === "INVALID_CONFIGURATION"
      ? MACHINE_EXIT.invalidConfiguration
      : code === "MISSING_DEPENDENCY"
        ? MACHINE_EXIT.missingDependency
        : code === "UNHEALTHY_SERVICES"
          ? MACHINE_EXIT.unhealthyServices
          : MACHINE_EXIT.internalFailure;
    this.details = details;
  }
}

export interface MachineSuccess {
  schemaVersion: typeof MACHINE_PROTOCOL_VERSION;
  ok: true;
  operation: string;
  shot: string;
  result: unknown;
}

export interface MachineFailure {
  schemaVersion: typeof MACHINE_PROTOCOL_VERSION;
  ok: false;
  operation: string;
  shot: string | null;
  error: {
    code: MachineErrorCode;
    message: string;
    details?: Record<string, unknown>;
  };
}

export interface CanonicalShotMetadata {
  schemaVersion: 1;
  slug: string;
  platform: "ios";
  createdAt: string;
  sequence: number;
  selectedAgent: "codex" | "claude" | null;
  creation: {
    door: "cli" | "studio";
    inputDigest: string;
    hasIntention: boolean;
    referenceCount: number;
    provenancePath: ".tohseno/provenance/provenance.json";
    options: {
      selectedAgent: "codex" | "claude" | null;
      agentMode: "interactive" | "automated" | "none";
      verifyAfterAgent: boolean;
      runAfterCreate: boolean;
    };
  };
  factory: {
    releaseId: string;
    cliVersion: "0.5.0";
    templateVersion: "ios-kernel-v1";
    manifestSchemaVersion: "1.0.0";
    sourceCommit: string | null;
    sourceDirty: boolean;
    bundleDigest: string;
  };
  app: {
    name: string;
    bundleId: string;
  };
  composition: {
    kernel: CompositionItem;
    template: CompositionItem;
    skills: CompositionItem[];
  };
  sanitizedPlanDigest: string;
  protocol: {
    version: 1;
    shotId: string;
    statePath: ".tohseno/protocol-state.json";
  };
}

export interface CanonicalLocalShotState {
  protocolVersion: 1;
  shotId: string;
  lifecycle: "EVOLVING";
  evolution: number;
}

interface CompositionItem {
  id: string;
  version: string;
  digest: string;
}

export interface CommandResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export interface RuntimePaths {
  root: string;
  local: string;
  runtime: string;
  logs: string;
  iosLog: string;
}

const HEX_64 = /^[a-f0-9]{64}$/u;
const SHOT_ID = /^shot_[A-Za-z0-9_-]{32}$/u;
const SLUG = /^[a-z0-9]+(?:-[a-z0-9]+)*$/u;
const BUNDLE_ID = /^[A-Za-z0-9]+(?:\.[A-Za-z0-9-]+)+$/u;
const CATALOG_ID = /^[a-z0-9]+(?:-[a-z0-9]+)*$/u;
const SEMVER = /^[0-9]+\.[0-9]+\.[0-9]+$/u;
const RELEASE_ID =
  /^(?:git-[0-9a-f]{40}(?:-dirty)?-[0-9a-f]{16}|content-[0-9a-f]{32})$/u;
const GIT_COMMIT = /^[0-9a-f]{40}$/u;

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function exactKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
): boolean {
  const actual = Object.keys(value).sort();
  const canonical = [...expected].sort();
  return actual.length === canonical.length &&
    actual.every((key, index) => key === canonical[index]);
}

function isAgent(value: unknown): value is "codex" | "claude" | null {
  return value === null || value === "codex" || value === "claude";
}

function isCanonicalTimestamp(value: unknown): value is string {
  if (typeof value !== "string") return false;
  const milliseconds = Date.parse(value);
  return Number.isFinite(milliseconds) &&
    new Date(milliseconds).toISOString() === value;
}

function compositionItem(value: unknown): value is CompositionItem {
  const item = record(value);
  return item !== null &&
    exactKeys(item, ["id", "version", "digest"]) &&
    typeof item.id === "string" &&
    CATALOG_ID.test(item.id) &&
    typeof item.version === "string" &&
    SEMVER.test(item.version) &&
    typeof item.digest === "string" &&
    HEX_64.test(item.digest);
}

function obsoleteState(detail: string): MachineError {
  return new MachineError(
    "INVALID_CONFIGURATION",
    `${PRE_RELEASE_UNSUPPORTED}: ${detail}`,
  );
}

export function validateCanonicalShotMetadata(
  value: unknown,
): CanonicalShotMetadata {
  const metadata = record(value);
  if (
    metadata === null ||
    !exactKeys(metadata, [
      "schemaVersion",
      "slug",
      "platform",
      "createdAt",
      "sequence",
      "selectedAgent",
      "creation",
      "factory",
      "app",
      "composition",
      "sanitizedPlanDigest",
      "protocol",
    ])
  ) {
    throw obsoleteState("Shot metadata does not have the canonical 0.5 shape");
  }

  const creation = record(metadata.creation);
  const options = record(creation?.options);
  const factory = record(metadata.factory);
  const app = record(metadata.app);
  const composition = record(metadata.composition);
  const protocol = record(metadata.protocol);
  if (
    metadata.schemaVersion !== 1 ||
    typeof metadata.slug !== "string" ||
    metadata.slug.length > 63 ||
    !SLUG.test(metadata.slug) ||
    metadata.platform !== "ios" ||
    !isCanonicalTimestamp(metadata.createdAt) ||
    !Number.isSafeInteger(metadata.sequence) ||
    (metadata.sequence as number) < 1 ||
    !isAgent(metadata.selectedAgent) ||
    typeof metadata.sanitizedPlanDigest !== "string" ||
    !HEX_64.test(metadata.sanitizedPlanDigest) ||
    creation === null ||
    options === null ||
    factory === null ||
    app === null ||
    composition === null ||
    protocol === null
  ) {
    throw obsoleteState("Shot metadata contains invalid canonical values");
  }

  if (
    !exactKeys(creation, [
      "door",
      "inputDigest",
      "hasIntention",
      "referenceCount",
      "provenancePath",
      "options",
    ]) ||
    (creation.door !== "cli" && creation.door !== "studio") ||
    typeof creation.inputDigest !== "string" ||
    !HEX_64.test(creation.inputDigest) ||
    typeof creation.hasIntention !== "boolean" ||
    !Number.isSafeInteger(creation.referenceCount) ||
    (creation.referenceCount as number) < 0 ||
    (creation.referenceCount as number) > 8 ||
    creation.provenancePath !== ".tohseno/provenance/provenance.json" ||
    !exactKeys(options, [
      "selectedAgent",
      "agentMode",
      "verifyAfterAgent",
      "runAfterCreate",
    ]) ||
    !isAgent(options.selectedAgent) ||
    options.selectedAgent !== metadata.selectedAgent ||
    (
      options.agentMode !== "interactive" &&
      options.agentMode !== "automated" &&
      options.agentMode !== "none"
    ) ||
    typeof options.verifyAfterAgent !== "boolean" ||
    typeof options.runAfterCreate !== "boolean"
  ) {
    throw obsoleteState("Shot creation provenance is not canonical");
  }

  if (
    !exactKeys(factory, [
      "releaseId",
      "cliVersion",
      "templateVersion",
      "manifestSchemaVersion",
      "sourceCommit",
      "sourceDirty",
      "bundleDigest",
    ]) ||
    typeof factory.releaseId !== "string" ||
    !RELEASE_ID.test(factory.releaseId) ||
    factory.cliVersion !== "0.5.0" ||
    factory.templateVersion !== "ios-kernel-v1" ||
    factory.manifestSchemaVersion !== "1.0.0" ||
    (
      factory.sourceCommit !== null &&
      (
        typeof factory.sourceCommit !== "string" ||
        !GIT_COMMIT.test(factory.sourceCommit)
      )
    ) ||
    typeof factory.sourceDirty !== "boolean" ||
    typeof factory.bundleDigest !== "string" ||
    !HEX_64.test(factory.bundleDigest)
  ) {
    throw obsoleteState("Shot factory provenance is not canonical 0.5");
  }
  if (
    (
      factory.releaseId.startsWith("content-") &&
      (factory.sourceCommit !== null || factory.sourceDirty)
    ) ||
    (
      factory.releaseId.startsWith("git-") &&
      (
        factory.sourceCommit === null ||
        !factory.releaseId.startsWith(`git-${factory.sourceCommit}`) ||
        factory.releaseId.includes("-dirty-") !== factory.sourceDirty
      )
    )
  ) {
    throw obsoleteState("Shot factory source provenance is inconsistent");
  }

  if (
    !exactKeys(app, ["name", "bundleId"]) ||
    typeof app.name !== "string" ||
    app.name.trim() !== app.name ||
    app.name.length < 1 ||
    app.name.length > 80 ||
    /[\u0000-\u001f\u007f-\u009f]/u.test(app.name) ||
    typeof app.bundleId !== "string" ||
    !BUNDLE_ID.test(app.bundleId) ||
    app.bundleId !== `com.tohseno.${metadata.slug}`
  ) {
    throw obsoleteState("Shot app identity is not canonical");
  }

  if (
    !exactKeys(composition, ["kernel", "template", "skills"]) ||
    !compositionItem(composition.kernel) ||
    composition.kernel.id !== "ios-kernel" ||
    composition.kernel.version !== "1.0.0" ||
    !compositionItem(composition.template) ||
    !Array.isArray(composition.skills) ||
    !composition.skills.every(compositionItem) ||
    new Set(
      (composition.skills as CompositionItem[]).map((skill) => skill.id),
    ).size !== composition.skills.length
  ) {
    throw obsoleteState("Shot composition is not canonical");
  }

  if (
    !exactKeys(protocol, ["version", "shotId", "statePath"]) ||
    protocol.version !== 1 ||
    typeof protocol.shotId !== "string" ||
    !SHOT_ID.test(protocol.shotId) ||
    protocol.statePath !== ".tohseno/protocol-state.json"
  ) {
    throw obsoleteState("Shot protocol identity is not canonical");
  }

  return metadata as unknown as CanonicalShotMetadata;
}

export function validateCanonicalLocalShotState(
  value: unknown,
  shotId: string,
): CanonicalLocalShotState {
  const state = record(value);
  if (
    !SHOT_ID.test(shotId) ||
    state === null ||
    !exactKeys(state, [
      "protocolVersion",
      "shotId",
      "lifecycle",
      "evolution",
    ]) ||
    state.protocolVersion !== 1 ||
    typeof state.shotId !== "string" ||
    !SHOT_ID.test(state.shotId) ||
    state.shotId !== shotId ||
    state.lifecycle !== "EVOLVING" ||
    !Number.isSafeInteger(state.evolution) ||
    (state.evolution as number) < 0
  ) {
    throw obsoleteState(
      "local Shot protocol state is not canonical or does not match its identity",
    );
  }
  return state as unknown as CanonicalLocalShotState;
}

export function readCanonicalShotMetadata(
  rootValue: string,
): CanonicalShotMetadata {
  const root = realpathSync(resolve(rootValue));
  if (existsSync(join(root, "continuity.manifest.json"))) {
    throw obsoleteState("obsolete continuity state was found");
  }
  const metadataPath = join(root, ".tohseno", "shot.json");
  const manifestPath = join(root, "app.manifest.json");
  if (!existsSync(metadataPath) || !existsSync(manifestPath)) {
    throw obsoleteState("the canonical Shot metadata or app manifest is missing");
  }
  try {
    requireRegularFile(metadataPath, "canonical Shot metadata");
    requireRegularFile(manifestPath, "canonical app manifest");
    const metadata = validateCanonicalShotMetadata(
      readJson<unknown>(metadataPath, 65_536),
    );
    const manifest = readJson<unknown>(manifestPath);
    if (!validateAppManifest(manifest).valid) {
      throw obsoleteState(
        `app.manifest ${APP_MANIFEST_SCHEMA_VERSION} is not canonical`,
      );
    }
    const appManifest = manifest as AppManifest;
    if (
      appManifest.application.id !== metadata.app.bundleId ||
      appManifest.application.name !== metadata.app.name ||
      appManifest.composition.kernel !== metadata.composition.kernel.id ||
      appManifest.composition.template !== metadata.composition.template.id ||
      JSON.stringify(appManifest.composition.skills) !==
        JSON.stringify(metadata.composition.skills.map((skill) => skill.id))
    ) {
      throw obsoleteState(
        "canonical Shot metadata and app.manifest do not agree",
      );
    }
    validateCanonicalLocalShotState(
      readJson<unknown>(
        join(root, metadata.protocol.statePath),
        65_536,
      ),
      metadata.protocol.shotId,
    );
    return metadata;
  } catch (error) {
    if (
      error instanceof MachineError &&
      error.message.includes(PRE_RELEASE_UNSUPPORTED)
    ) {
      throw error;
    }
    throw obsoleteState(
      error instanceof Error ? error.message : "Shot metadata is unreadable",
    );
  }
}

export function shotRoot(start = process.cwd()): string {
  let candidate = resolve(start);
  while (true) {
    if (existsSync(join(candidate, ".tohseno", "shot.json"))) {
      return realpathSync(candidate);
    }
    if (existsSync(join(candidate, "continuity.manifest.json"))) {
      throw obsoleteState("obsolete continuity state was found");
    }
    const parent = dirname(candidate);
    if (parent === candidate) break;
    candidate = parent;
  }
  throw new MachineError(
    "INVALID_CONFIGURATION",
    "run this operation inside a canonical Shot or pass --shot to the global CLI",
  );
}

export function insideRoot(root: string, pathValue: string): boolean {
  const fromRoot = relative(resolve(root), resolve(pathValue));
  return fromRoot === "" ||
    (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

function ensurePrivateDirectory(root: string, path: string): void {
  mkdirSync(path, { recursive: true, mode: 0o700 });
  const before = lstatSync(path);
  let descriptor: number | undefined;
  try {
    descriptor = openSync(
      path,
      constants.O_RDONLY | constants.O_DIRECTORY | constants.O_NOFOLLOW,
    );
    const opened = fstatSync(descriptor);
    if (
      before.isSymbolicLink() ||
      !before.isDirectory() ||
      !opened.isDirectory() ||
      opened.dev !== before.dev ||
      opened.ino !== before.ino ||
      !insideRoot(root, realpathSync(path))
    ) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        `Shot runtime directory is unsafe: ${path}`,
      );
    }
    fchmodSync(descriptor, 0o700);
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
}

export function runtimePaths(rootValue: string): RuntimePaths {
  const root = realpathSync(resolve(rootValue));
  const local = join(root, ".tohseno");
  if (
    !existsSync(local) ||
    lstatSync(local).isSymbolicLink() ||
    !lstatSync(local).isDirectory() ||
    !insideRoot(root, realpathSync(local))
  ) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "canonical Shot runtime directory is missing or unsafe",
    );
  }
  const runtime = join(local, "run");
  const logs = join(runtime, "logs");
  ensurePrivateDirectory(root, runtime);
  ensurePrivateDirectory(root, logs);
  const iosLog = join(logs, "ios.log");
  if (existsSync(iosLog)) requireRegularFile(iosLog, "iOS runtime log");
  return { root, local, runtime, logs, iosLog };
}

export function readBoundedRegularFile(
  path: string,
  maximumBytes: number,
  label = path,
): Buffer {
  let descriptor: number | undefined;
  try {
    descriptor = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW);
    const opened = fstatSync(descriptor);
    const current = lstatSync(path);
    if (
      !opened.isFile() ||
      opened.nlink !== 1 ||
      current.isSymbolicLink() ||
      !current.isFile() ||
      current.nlink !== 1 ||
      opened.dev !== current.dev ||
      opened.ino !== current.ino ||
      opened.size > maximumBytes
    ) {
      throw new Error("unsafe or oversized file");
    }
    const chunks: Buffer[] = [];
    const buffer = Buffer.allocUnsafe(65_536);
    let total = 0;
    while (true) {
      const length = readSync(descriptor, buffer, 0, buffer.length, null);
      if (length === 0) break;
      total += length;
      if (total > maximumBytes) throw new Error("file grew past its limit");
      chunks.push(Buffer.from(buffer.subarray(0, length)));
    }
    return Buffer.concat(chunks, total);
  } catch {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `${label} must be a single-link regular file no larger than ${maximumBytes} bytes`,
    );
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
}

export function readBoundedUtf8(
  path: string,
  maximumBytes: number,
  label = path,
): string {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(
      readBoundedRegularFile(path, maximumBytes, label),
    );
  } catch (error) {
    if (error instanceof MachineError) throw error;
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `${label} must contain valid UTF-8`,
    );
  }
}

export function readJson<T>(
  path: string,
  maximumBytes = MAX_RUNTIME_JSON_BYTES,
): T {
  try {
    return JSON.parse(
      readBoundedUtf8(path, maximumBytes, path),
    ) as T;
  } catch {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `cannot read ${path}: expected canonical JSON no larger than ${maximumBytes} bytes`,
    );
  }
}

export function requireRegularFile(path: string, label = path): void {
  if (!existsSync(path)) {
    throw new MachineError("INVALID_CONFIGURATION", `${label} is missing`);
  }
  const details = lstatSync(path);
  if (
    details.isSymbolicLink() ||
    !details.isFile() ||
    details.nlink !== 1
  ) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `${label} must be a single-link regular file`,
    );
  }
}

export function success(
  operation: string,
  root: string,
  result: unknown,
): MachineSuccess {
  return {
    schemaVersion: MACHINE_PROTOCOL_VERSION,
    ok: true,
    operation,
    shot: root,
    result,
  };
}

export function failure(
  operation: string,
  root: string | null,
  error: unknown,
): MachineFailure {
  const machineError = error instanceof MachineError
    ? error
    : new MachineError(
      "INTERNAL_FAILURE",
      error instanceof Error ? error.message : String(error),
    );
  const value: MachineFailure = {
    schemaVersion: MACHINE_PROTOCOL_VERSION,
    ok: false,
    operation,
    shot: root,
    error: { code: machineError.code, message: machineError.message },
  };
  if (machineError.details !== undefined) {
    value.error.details = machineError.details;
  }
  return value;
}

export function errorExitCode(error: unknown): MachineExitCode {
  return error instanceof MachineError
    ? error.exitCode
    : MACHINE_EXIT.internalFailure;
}

export function safeEnvironment(
  environment: Record<string, string | undefined> = process.env,
): Record<string, string> {
  const exact = new Set([
    "PATH",
    "HOME",
    "SHELL",
    "USER",
    "LOGNAME",
    "TMPDIR",
    "TMP",
    "TEMP",
    "LANG",
    "DEVELOPER_DIR",
  ]);
  const result: Record<string, string> = {};
  for (const [key, value] of Object.entries(environment)) {
    if (
      value !== undefined &&
      (exact.has(key) || key.startsWith("LC_"))
    ) {
      result[key] = value;
    }
  }
  return result;
}

export async function runCaptured(
  command: readonly string[],
  options: { cwd: string; environment?: Record<string, string> },
): Promise<CommandResult> {
  try {
    const child = Bun.spawn([...command], {
      cwd: options.cwd,
      env: options.environment ?? safeEnvironment(),
      stdin: "ignore",
      stdout: "pipe",
      stderr: "pipe",
    });
    let outputExceeded = false;
    const stopForOutputLimit = (): void => {
      if (outputExceeded) return;
      outputExceeded = true;
      try {
        child.kill("SIGKILL");
      } catch {
        // The child exited between the oversized chunk and the kill request.
      }
    };
    const [exitCode, stdout, stderr] = await Promise.all([
      child.exited,
      boundedStreamText(
        child.stdout,
        MAX_CAPTURED_OUTPUT_BYTES,
        stopForOutputLimit,
      ),
      boundedStreamText(
        child.stderr,
        MAX_CAPTURED_OUTPUT_BYTES,
        stopForOutputLimit,
      ),
    ]);
    if (outputExceeded) {
      throw new MachineError(
        "INTERNAL_FAILURE",
        `subprocess output exceeded the ${MAX_CAPTURED_OUTPUT_BYTES}-byte safety limit`,
      );
    }
    return { exitCode, stdout, stderr };
  } catch (error) {
    if (error instanceof MachineError) throw error;
    throw new MachineError(
      "MISSING_DEPENDENCY",
      `cannot execute ${command[0]}: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
}

async function boundedStreamText(
  stream: ReadableStream<Uint8Array>,
  maximumBytes: number,
  onLimit: () => void,
): Promise<string> {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    while (true) {
      const next = await reader.read();
      if (next.done) break;
      const remaining = maximumBytes - length;
      if (next.value.byteLength > remaining) {
        if (remaining > 0) chunks.push(next.value.subarray(0, remaining));
        onLimit();
        break;
      }
      chunks.push(next.value);
      length += next.value.byteLength;
    }
  } finally {
    try {
      await reader.cancel();
    } catch {
      // A process closing its pipe first is expected.
    }
  }
  return Buffer.concat(chunks.map((chunk) => Buffer.from(chunk))).toString(
    "utf8",
  );
}

export function tailLines(path: string, count: number): string[] {
  if (!existsSync(path)) return [];
  const descriptor = openSync(
    path,
    constants.O_RDONLY | constants.O_NOFOLLOW,
  );
  let source: string;
  try {
    const opened = fstatSync(descriptor);
    const current = lstatSync(path);
    if (
      !opened.isFile() ||
      opened.nlink !== 1 ||
      current.isSymbolicLink() ||
      !current.isFile() ||
      current.nlink !== 1 ||
      opened.dev !== current.dev ||
      opened.ino !== current.ino
    ) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        `runtime log must be a private regular file: ${path}`,
      );
    }
    const length = Math.min(opened.size, MAX_TAIL_READ_BYTES);
    const offset = opened.size - length;
    const buffer = Buffer.alloc(length);
    let read = 0;
    while (read < length) {
      const chunk = readSync(
        descriptor,
        buffer,
        read,
        length - read,
        offset + read,
      );
      if (chunk === 0) break;
      read += chunk;
    }
    source = buffer.subarray(0, read).toString("utf8");
    if (offset > 0) {
      const firstNewline = source.indexOf("\n");
      source = firstNewline === -1 ? "" : source.slice(firstNewline + 1);
    }
  } finally {
    closeSync(descriptor);
  }
  const lines = source.split(/\r?\n/u);
  if (lines.at(-1) === "") lines.pop();
  return lines.slice(-count);
}

export function openRuntimeLog(path: string): number {
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  let descriptor: number;
  try {
    descriptor = openSync(
      path,
      constants.O_WRONLY |
        constants.O_APPEND |
        constants.O_CREAT |
        constants.O_EXCL |
        constants.O_NOFOLLOW,
      0o600,
    );
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
    descriptor = openSync(
      path,
      constants.O_WRONLY |
        constants.O_APPEND |
        constants.O_NOFOLLOW,
    );
  }
  const opened = fstatSync(descriptor);
  const current = lstatSync(path);
  if (
    !opened.isFile() ||
    opened.nlink !== 1 ||
    current.isSymbolicLink() ||
    !current.isFile() ||
    current.nlink !== 1 ||
    opened.dev !== current.dev ||
    opened.ino !== current.ino
  ) {
    closeSync(descriptor);
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `runtime log must be a private regular file: ${path}`,
    );
  }
  fchmodSync(descriptor, 0o600);
  return descriptor;
}

export function capRuntimeLog(
  path: string,
  maximumBytes = MAX_RUNTIME_LOG_BYTES,
): boolean {
  if (!existsSync(path)) return false;
  const descriptor = openRuntimeLog(path);
  try {
    if (fstatSync(descriptor).size <= maximumBytes) return false;
    ftruncateSync(descriptor, 0);
    writeSync(
      descriptor,
      `${JSON.stringify({
        at: new Date().toISOString(),
        event: "log_rotated",
        maximumBytes,
      })}\n`,
    );
    return true;
  } finally {
    closeSync(descriptor);
  }
}

export function appendStructuredLog(
  path: string,
  value: Record<string, unknown>,
): void {
  capRuntimeLog(path);
  const descriptor = openRuntimeLog(path);
  try {
    writeSync(
      descriptor,
      `${JSON.stringify({ at: new Date().toISOString(), ...value })}\n`,
    );
  } finally {
    closeSync(descriptor);
  }
  capRuntimeLog(path);
}

export function publicErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
