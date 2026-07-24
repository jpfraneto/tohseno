import { randomUUID } from "node:crypto";
import {
  closeSync,
  constants,
  existsSync,
  fchmodSync,
  fstatSync,
  fsyncSync,
  ftruncateSync,
  lstatSync,
  mkdirSync,
  openSync,
  readSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
  writeSync,
} from "node:fs";
import {
  basename,
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
  sep,
} from "node:path";

export const MACHINE_PROTOCOL_VERSION = 1 as const;
export const MAX_RUNTIME_LOG_BYTES = 5 * 1_048_576;
export const MAX_TAIL_READ_BYTES = 2 * 1_048_576;
export const MAX_CAPTURED_OUTPUT_BYTES = 8 * 1_048_576;
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

export interface RuntimePaths {
  root: string;
  local: string;
  runtime: string;
  data: string;
  logs: string;
  state: string;
  stopRequest: string;
  lock: string;
  lockMetadata: string;
  apiReady: string;
  endpoint: string;
  apiLog: string;
  tunnelLog: string;
  supervisorLog: string;
  iosLog: string;
  tokenLog: string;
}

export interface OwnedProcess {
  pid: number;
  role: "supervisor" | "api" | "tunnel";
  commandContains: string[];
}

export interface DevelopmentState {
  schemaVersion: 1;
  instanceId: string;
  status: "running" | "unhealthy";
  startedAt: string;
  updatedAt: string;
  shotRoot: string;
  supervisor: OwnedProcess;
  api: OwnedProcess & {
    hostname: "127.0.0.1";
    port: number;
    url: string;
    healthUrl: string;
    log: string;
  };
  tunnel: (OwnedProcess & {
    url: string;
    log: string;
    developmentOnly: true;
  }) | null;
  endpoint: {
    url: string;
    transport: "localhost" | "quick-tunnel";
    configuration: string;
  };
  issue?: string;
}

export interface CommandResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export function shotRoot(start = process.cwd()): string {
  let candidate = resolve(start);
  while (true) {
    if (existsSync(join(candidate, ".tohseno", "shot.json"))) {
      return realpathSync(candidate);
    }
    const parent = dirname(candidate);
    if (parent === candidate) break;
    candidate = parent;
  }
  throw new MachineError(
    "INVALID_CONFIGURATION",
    "run this operation inside a recognized shot or pass --shot to the global CLI",
  );
}

export function runtimePaths(rootValue: string): RuntimePaths {
  const root = realpathSync(resolve(rootValue));
  const local = join(root, ".tohseno");
  const runtime = join(local, "run");
  const logs = join(runtime, "logs");
  const paths: RuntimePaths = {
    root,
    local,
    runtime,
    data: join(local, "data"),
    logs,
    state: join(runtime, "state.json"),
    stopRequest: join(runtime, "stop-request.json"),
    lock: join(runtime, "start.lock"),
    lockMetadata: join(runtime, "start.lock", "owner.json"),
    apiReady: join(runtime, "api-ready.json"),
    endpoint: join(root, "Config", "DevelopmentEndpoint.xcconfig"),
    apiLog: join(logs, "api.log"),
    tunnelLog: join(logs, "tunnel.log"),
    supervisorLog: join(logs, "supervisor.log"),
    iosLog: join(logs, "ios.log"),
    tokenLog: join(logs, "token.log"),
  };
  validateRuntimeBoundaries(paths);
  return paths;
}

function validateRuntimeBoundaries(paths: RuntimePaths): void {
  const requiredDirectories = [paths.local, join(paths.root, "Config")];
  const optionalDirectories = [paths.runtime, paths.data, paths.logs, paths.lock];
  for (const path of requiredDirectories) {
    if (!existsSync(path)) {
      throw new MachineError("INVALID_CONFIGURATION", `shot runtime directory is missing: ${path}`);
    }
  }
  for (const path of [...requiredDirectories, ...optionalDirectories]) {
    if (!existsSync(path)) continue;
    const details = lstatSync(path);
    if (details.isSymbolicLink() || !details.isDirectory() || !insideRoot(paths.root, realpathSync(path))) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        `shot runtime path must be a real directory inside the shot: ${path}`,
      );
    }
  }
  for (const path of [
    join(paths.local, "shot.json"),
    paths.state,
    paths.stopRequest,
    paths.lockMetadata,
    paths.apiReady,
    paths.endpoint,
    paths.apiLog,
    paths.tunnelLog,
    paths.supervisorLog,
    paths.iosLog,
    paths.tokenLog,
    join(paths.data, "development.sqlite3"),
    join(paths.data, "development.sqlite3-wal"),
    join(paths.data, "development.sqlite3-shm"),
  ]) {
    if (!existsSync(path)) continue;
    const details = lstatSync(path);
    if (details.isSymbolicLink() || !details.isFile()) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        `shot runtime file must be a regular file inside the shot: ${path}`,
      );
    }
  }
}

export function ensureRuntimeDirectories(paths: RuntimePaths): void {
  for (const path of [paths.runtime, paths.data, paths.logs]) {
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
        !before.isDirectory() ||
        before.isSymbolicLink() ||
        !opened.isDirectory() ||
        opened.dev !== before.dev ||
        opened.ino !== before.ino
      ) {
        throw new MachineError(
          "INVALID_CONFIGURATION",
          `shot runtime directory is unsafe: ${path}`,
        );
      }
      fchmodSync(descriptor, 0o700);
    } finally {
      if (descriptor !== undefined) closeSync(descriptor);
    }
  }
}

export function insideRoot(root: string, pathValue: string): boolean {
  const fromRoot = relative(resolve(root), resolve(pathValue));
  return fromRoot === "" || (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

export function atomicWrite(path: string, content: string, mode = 0o600): void {
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  const temporary = `${path}.writing-${process.pid}-${randomUUID()}`;
  let descriptor: number | undefined;
  try {
    descriptor = openSync(
      temporary,
      constants.O_WRONLY |
        constants.O_CREAT |
        constants.O_EXCL |
        constants.O_NOFOLLOW,
      mode,
    );
    writeFileSync(descriptor, content, { encoding: "utf8" });
    fchmodSync(descriptor, mode);
    fsyncSync(descriptor);
    closeSync(descriptor);
    descriptor = undefined;
    renameSync(temporary, path);
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
    rmSync(temporary, { force: true });
  }
}

export function atomicJson(path: string, value: unknown): void {
  atomicWrite(path, `${JSON.stringify(value, null, 2)}\n`);
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
      if (total > maximumBytes) {
        throw new Error("file grew past its limit");
      }
      chunks.push(Buffer.from(buffer.subarray(0, length)));
    }
    return Buffer.concat(chunks, total);
  } catch {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `${label} must be a regular file with one link and no more than ${maximumBytes} bytes`,
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
      `cannot read ${path}: expected a private regular JSON file no larger than ${maximumBytes} bytes`,
    );
  }
}

export function requireRegularFile(path: string, label = path): void {
  if (!existsSync(path)) {
    throw new MachineError("INVALID_CONFIGURATION", `${label} is missing`);
  }
  const details = lstatSync(path);
  if (details.isSymbolicLink() || !details.isFile()) {
    throw new MachineError("INVALID_CONFIGURATION", `${label} must be a regular file`);
  }
}

export function readDevelopmentState(paths: RuntimePaths): DevelopmentState | null {
  if (!existsSync(paths.state)) return null;
  const state = readJson<Partial<DevelopmentState>>(paths.state, 65_536);
  const corrupt = (): never => {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `runtime state is corrupt: ${paths.state}`,
    );
  };
  const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
  const timestamp = (value: unknown): value is string =>
    typeof value === "string" && Number.isFinite(Date.parse(value));
  const processRecord = (
    value: unknown,
    role: OwnedProcess["role"],
  ): value is OwnedProcess => {
    if (
      typeof value !== "object" ||
      value === null ||
      Array.isArray(value)
    ) return false;
    const record = value as Partial<OwnedProcess>;
    return Number.isSafeInteger(record.pid) &&
      (record.pid ?? 0) > 0 &&
      record.role === role &&
      Array.isArray(record.commandContains) &&
      record.commandContains.length >= 3 &&
      record.commandContains.length <= 6 &&
      record.commandContains.every((fragment) =>
        typeof fragment === "string" &&
        fragment.length > 0 &&
        fragment.length <= 4_096 &&
        !/[\u0000\r\n]/u.test(fragment)
      );
  };
  if (!uuid.test(state.instanceId ?? "")) corrupt();
  const instanceId = state.instanceId!;
  const supervisor = state.supervisor;
  const api = state.api;
  const tunnel = state.tunnel;
  if (
    state.schemaVersion !== 1 ||
    (state.status !== "running" && state.status !== "unhealthy") ||
    !timestamp(state.startedAt) ||
    !timestamp(state.updatedAt) ||
    state.shotRoot !== paths.root ||
    !processRecord(supervisor, "supervisor") ||
    !processRecord(api, "api") ||
    (tunnel !== null && !processRecord(tunnel, "tunnel"))
  ) {
    corrupt();
  }
  const validated = state as DevelopmentState;
  const validatedSupervisor = validated.supervisor;
  const validatedApi = validated.api;
  const validatedTunnel = validated.tunnel;
  if (
    validatedSupervisor.commandContains.length !== 4 ||
    !isAbsolute(validatedSupervisor.commandContains[0]!) ||
    basename(validatedSupervisor.commandContains[0]!) !== "machine.ts" ||
    validatedSupervisor.commandContains[1] !== "__supervise" ||
    validatedSupervisor.commandContains[2] !== "--instance" ||
    validatedSupervisor.commandContains[3] !== instanceId
  ) corrupt();
  if (
    !Number.isInteger(validatedApi.port) ||
    validatedApi.port < 1 ||
    validatedApi.port > 65_535 ||
    validatedApi.hostname !== "127.0.0.1" ||
    validatedApi.url !== `http://127.0.0.1:${validatedApi.port}` ||
    validatedApi.healthUrl !== `${validatedApi.url}/health` ||
    validatedApi.log !== paths.apiLog ||
    validatedApi.commandContains.length !== 3 ||
    validatedApi.commandContains[0] !==
      join(paths.root, "Backend", "server.ts") ||
    validatedApi.commandContains[1] !== "--tohseno-instance" ||
    validatedApi.commandContains[2] !== instanceId
  ) corrupt();
  if (
    typeof validated.endpoint !== "object" ||
    validated.endpoint === null ||
    validated.endpoint.configuration !== paths.endpoint
  ) corrupt();
  if (validatedTunnel === null) {
    if (
      validated.endpoint.transport !== "localhost" ||
      validated.endpoint.url !== validatedApi.url
    ) corrupt();
  } else if (
    validatedTunnel.commandContains.length !== 4 ||
    !isAbsolute(validatedTunnel.commandContains[0]!) ||
    validatedTunnel.commandContains[1] !== "tunnel" ||
    validatedTunnel.commandContains[2] !== validatedApi.url ||
    validatedTunnel.commandContains[3] !== "--no-autoupdate" ||
    validatedTunnel.log !== paths.tunnelLog ||
    validatedTunnel.developmentOnly !== true ||
    parseQuickTunnelUrl(validatedTunnel.url) !== validatedTunnel.url ||
    validated.endpoint.transport !== "quick-tunnel" ||
    validated.endpoint.url !== validatedTunnel.url
  ) corrupt();
  if (
    validated.issue !== undefined &&
    !/^(?:api|tunnel|log-monitor) exited unexpectedly$/u.test(validated.issue)
  ) corrupt();
  return validated;
}

export function success(operation: string, root: string, result: unknown): MachineSuccess {
  return { schemaVersion: MACHINE_PROTOCOL_VERSION, ok: true, operation, shot: root, result };
}

export function failure(operation: string, root: string | null, error: unknown): MachineFailure {
  const machineError = error instanceof MachineError
    ? error
    : new MachineError(
      "INTERNAL_FAILURE",
      error instanceof Error ? error.message : String(error),
    );
  const failureValue: MachineFailure = {
    schemaVersion: MACHINE_PROTOCOL_VERSION,
    ok: false,
    operation,
    shot: root,
    error: { code: machineError.code, message: machineError.message },
  };
  if (machineError.details !== undefined) failureValue.error.details = machineError.details;
  return failureValue;
}

export function errorExitCode(error: unknown): MachineExitCode {
  return error instanceof MachineError ? error.exitCode : MACHINE_EXIT.internalFailure;
}

export function safeEnvironment(
  environment: Record<string, string | undefined> = process.env,
): Record<string, string> {
  const exact = new Set([
    "PATH", "HOME", "SHELL", "USER", "LOGNAME", "TMPDIR", "TMP", "TEMP", "LANG",
    "DEVELOPER_DIR",
  ]);
  const result: Record<string, string> = {};
  for (const [key, value] of Object.entries(environment)) {
    if (value !== undefined && (exact.has(key) || key.startsWith("LC_"))) result[key] = value;
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
        // It exited between the oversized chunk and the kill request.
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
      `cannot execute ${command[0]}: ${error instanceof Error ? error.message : String(error)}`,
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
      // The process closing its pipe first is expected.
    }
  }
  return Buffer.concat(chunks.map((chunk) => Buffer.from(chunk))).toString("utf8");
}

export function isProcessAlive(pid: number): boolean {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

export async function processCommand(pid: number): Promise<string | null> {
  if (!isProcessAlive(pid)) return null;
  const result = await runCaptured(["/bin/ps", "-p", String(pid), "-o", "command="], {
    cwd: process.cwd(),
  });
  if (result.exitCode !== 0) return null;
  return result.stdout.trim() || null;
}

export async function isOwnedProcess(record: OwnedProcess): Promise<boolean> {
  if (
    !Number.isSafeInteger(record.pid) ||
    record.pid <= 0 ||
    !Array.isArray(record.commandContains) ||
    record.commandContains.length < 3 ||
    record.commandContains.length > 6 ||
    !record.commandContains.every((fragment) =>
      typeof fragment === "string" &&
      fragment.length > 0 &&
      fragment.length <= 4_096 &&
      !/[\u0000\r\n]/u.test(fragment)
    )
  ) return false;
  const command = await processCommand(record.pid);
  return command !== null && record.commandContains.every((fragment) => command.includes(fragment));
}

export async function waitForProcessExit(pid: number, timeoutMs: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (!isProcessAlive(pid)) return true;
    await delay(50);
  }
  return !isProcessAlive(pid);
}

export async function terminateOwnedProcess(record: OwnedProcess): Promise<"absent" | "stopped" | "not-owned"> {
  if (!isProcessAlive(record.pid)) return "absent";
  if (!(await isOwnedProcess(record))) return "not-owned";
  try {
    process.kill(record.pid, "SIGTERM");
  } catch {
    return "absent";
  }
  const deadline = Date.now() + 5_000;
  while (Date.now() < deadline && await isOwnedProcess(record)) await delay(50);
  if (await isOwnedProcess(record)) {
    try {
      process.kill(record.pid, "SIGKILL");
    } catch {
      // It exited between the ownership check and signal.
    }
    const killDeadline = Date.now() + 2_000;
    while (Date.now() < killDeadline && await isOwnedProcess(record)) await delay(50);
  }
  return "stopped";
}

export function delay(milliseconds: number): Promise<void> {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}

export function requireAbsoluteInside(root: string, pathValue: string, label: string): string {
  if (!isAbsolute(pathValue)) {
    throw new MachineError("INVALID_CONFIGURATION", `${label} must be an absolute path`);
  }
  const path = resolve(pathValue);
  if (!insideRoot(root, path)) {
    throw new MachineError("INVALID_CONFIGURATION", `${label} must remain inside the shot`);
  }
  return path;
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
      opened.dev !== current.dev ||
      opened.ino !== current.ino
    ) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        `runtime log must be a private regular file: ${path}`,
      );
    }
    const size = opened.size;
    const length = Math.min(size, MAX_TAIL_READ_BYTES);
    const offset = size - length;
    const buffer = Buffer.alloc(length);
    let read = 0;
    while (read < length) {
      const chunk = readSync(descriptor, buffer, read, length - read, offset + read);
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

export function readLogSince(
  path: string,
  offset: number,
  maximumBytes = MAX_TAIL_READ_BYTES,
): string {
  if (!existsSync(path)) return "";
  const descriptor = openSync(
    path,
    constants.O_RDONLY | constants.O_NOFOLLOW,
  );
  try {
    const opened = fstatSync(descriptor);
    const current = lstatSync(path);
    if (
      !opened.isFile() ||
      opened.nlink !== 1 ||
      current.isSymbolicLink() ||
      !current.isFile() ||
      opened.dev !== current.dev ||
      opened.ino !== current.ino
    ) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        `runtime log must be a private regular file: ${path}`,
      );
    }
    const safeOffset = Number.isSafeInteger(offset) && offset >= 0 &&
        offset <= opened.size
      ? offset
      : 0;
    const available = opened.size - safeOffset;
    const length = Math.min(available, maximumBytes);
    const start = opened.size - length;
    const buffer = Buffer.alloc(length);
    let read = 0;
    while (read < length) {
      const chunk = readSync(
        descriptor,
        buffer,
        read,
        length - read,
        start + read,
      );
      if (chunk === 0) break;
      read += chunk;
    }
    return buffer.subarray(0, read).toString("utf8");
  } finally {
    closeSync(descriptor);
  }
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

export function appendStructuredLog(
  path: string,
  record: Record<string, unknown>,
): void {
  capRuntimeLog(path);
  const descriptor = openRuntimeLog(path);
  try {
    writeSync(
      descriptor,
      `${JSON.stringify({ at: new Date().toISOString(), ...record })}\n`,
    );
  } finally {
    closeSync(descriptor);
  }
  capRuntimeLog(path);
}

export function encodeXcconfigUrl(url: string): string {
  return url.replace("://", ":/$()/");
}

export function parseQuickTunnelUrl(source: string): string | null {
  const matches = source.match(/https:\/\/[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.trycloudflare\.com\b/giu);
  if (!matches || matches.length === 0) return null;
  const candidate = matches[matches.length - 1]!.toLowerCase();
  try {
    const url = new URL(candidate);
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      url.pathname !== "/" ||
      url.search ||
      url.hash ||
      !url.hostname.endsWith(".trycloudflare.com")
    ) return null;
    return url.origin;
  } catch {
    return null;
  }
}

export function publicErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
