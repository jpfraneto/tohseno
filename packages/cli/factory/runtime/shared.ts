import { randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";

export const MACHINE_PROTOCOL_VERSION = 1 as const;
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
  mkdirSync(paths.runtime, { recursive: true, mode: 0o700 });
  mkdirSync(paths.data, { recursive: true, mode: 0o700 });
  mkdirSync(paths.logs, { recursive: true, mode: 0o700 });
}

export function insideRoot(root: string, pathValue: string): boolean {
  const fromRoot = relative(resolve(root), resolve(pathValue));
  return fromRoot === "" || (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

export function atomicWrite(path: string, content: string, mode = 0o600): void {
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  const temporary = `${path}.writing-${process.pid}-${randomUUID()}`;
  try {
    writeFileSync(temporary, content, { mode });
    renameSync(temporary, path);
  } finally {
    rmSync(temporary, { force: true });
  }
}

export function atomicJson(path: string, value: unknown): void {
  atomicWrite(path, `${JSON.stringify(value, null, 2)}\n`);
}

export function readJson<T>(path: string): T {
  try {
    return JSON.parse(readFileSync(path, "utf8")) as T;
  } catch (error) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `cannot read ${path}: ${error instanceof Error ? error.message : String(error)}`,
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
  const state = readJson<Partial<DevelopmentState>>(paths.state);
  if (
    state.schemaVersion !== 1 ||
    typeof state.instanceId !== "string" ||
    (state.status !== "running" && state.status !== "unhealthy") ||
    typeof state.supervisor?.pid !== "number" ||
    typeof state.api?.pid !== "number" ||
    typeof state.api?.url !== "string" ||
    typeof state.endpoint?.url !== "string"
  ) {
    throw new MachineError("INVALID_CONFIGURATION", `runtime state is corrupt: ${paths.state}`);
  }
  return state as DevelopmentState;
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
    const [exitCode, stdout, stderr] = await Promise.all([
      child.exited,
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
    ]);
    return { exitCode, stdout, stderr };
  } catch (error) {
    throw new MachineError(
      "MISSING_DEPENDENCY",
      `cannot execute ${command[0]}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
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
  const result = await runCaptured(["ps", "-p", String(pid), "-o", "command="], {
    cwd: process.cwd(),
  });
  if (result.exitCode !== 0) return null;
  return result.stdout.trim() || null;
}

export async function isOwnedProcess(record: OwnedProcess): Promise<boolean> {
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
  const source = readFileSync(path, "utf8");
  const lines = source.split(/\r?\n/u);
  if (lines.at(-1) === "") lines.pop();
  return lines.slice(-count);
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
