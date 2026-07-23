import { spawn as spawnChild } from "node:child_process";
import { randomBytes, randomUUID } from "node:crypto";
import {
  accessSync,
  chmodSync,
  constants as fsConstants,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import {
  basename,
  delimiter,
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
  sep,
} from "node:path";
import { fileURLToPath } from "node:url";
import type { Readable } from "node:stream";
import type { CreationRunner, CreationRunnerResult } from "./creation.ts";
import { bunExecutable, sanitizedRuntimeEnvironment } from "./process.ts";
import { readShotMetadata } from "./shot.ts";
import { trustedShotToolFromCache } from "./trusted-tools.ts";

export const SERVE_SIM_VERSION = "0.1.45" as const;
export const LIVE_PREVIEW_HOST = "127.0.0.1" as const;

const MINIMUM_NODE_MAJOR = 20;
const COMMAND_OUTPUT_LIMIT = 2 * 1024 * 1024;
const DEFAULT_SIDECAR_START_TIMEOUT_MS = 200_000;
const SIDECAR_ENV_UDID = "TOHSENO_SERVE_SIM_UDID";
const SIDECAR_ENV_CAPABILITY = "TOHSENO_SERVE_SIM_CAPABILITY";
const SIDECAR_DIRECTORY_PREFIX = "tohseno-studio-sim-";
const CANONICAL_UDID =
  /^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$/u;
const BUNDLE_IDENTIFIER = /^[A-Za-z0-9]+(?:\.[A-Za-z0-9-]+)+$/u;
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

export type SimulatorErrorCode =
  | "ABORTED"
  | "INVALID_SHOT"
  | "INVALID_DEVICE"
  | "MISSING_DEPENDENCY"
  | "MACHINE_FAILED"
  | "INVALID_MACHINE_RESPONSE"
  | "SCREENSHOT_FAILED"
  | "UNSUPPORTED_PLATFORM"
  | "UNSUPPORTED_ARCHITECTURE"
  | "UNSUPPORTED_NODE"
  | "SERVE_SIM_UNAVAILABLE"
  | "LIVE_PREVIEW_BUSY"
  | "LIVE_PREVIEW_FAILED";

/**
 * Errors intentionally use stable, content-free messages. In particular, shot
 * machine stderr and build output are never copied into Studio responses.
 */
export class SimulatorError extends Error {
  override readonly name = "SimulatorError";
  readonly code: SimulatorErrorCode;
  readonly details: Readonly<Record<string, string | number | boolean | null>>;

  constructor(
    code: SimulatorErrorCode,
    message: string,
    details: Readonly<Record<string, string | number | boolean | null>> = {},
  ) {
    super(message);
    this.code = code;
    this.details = details;
  }
}

export interface CommandExecutionResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export interface CommandExecutionOptions {
  cwd: string;
  environment: Record<string, string | undefined>;
  signal?: AbortSignal;
  timeoutMs?: number;
  maxOutputBytes?: number;
}

export type CommandExecutor = (
  argv: readonly string[],
  options: CommandExecutionOptions,
) => Promise<CommandExecutionResult>;

export interface SimulatorDevice {
  name: string;
  udid: string;
  state: string;
  runtime: string;
  available: boolean;
}

export interface SimulatorReadinessBlocker {
  code:
    | "macos-required"
    | "apple-silicon-required"
    | "node-required"
    | "node-20-required"
    | "node-arm64-required"
    | "xcode-tools-required"
    | "simctl-unhealthy"
    | "simulator-required"
    | "serve-sim-required"
    | "serve-sim-version";
  message: string;
}

export interface ServeSimInstallation {
  packageJsonPath: string;
  middlewarePath: string;
  version: string;
  middlewareExport: boolean;
}

export interface SimulatorDiagnostics {
  platform: {
    current: NodeJS.Platform;
    macos: boolean;
  };
  cpu: {
    architecture: string;
    appleSilicon: boolean;
  };
  node: {
    available: boolean;
    executable: string | null;
    version: string | null;
    architecture: string | null;
    supported: boolean;
    compatible: boolean;
    minimumMajor: typeof MINIMUM_NODE_MAJOR;
  };
  xcode: {
    available: boolean;
    xcodebuild: string | null;
    xcrun: string | null;
    version: string | null;
  };
  simctl: {
    healthy: boolean;
    availableDevice: boolean;
    devices: SimulatorDevice[];
  };
  serveSim: {
    available: boolean;
    version: string | null;
    expectedVersion: typeof SERVE_SIM_VERSION;
    exactVersion: boolean;
    middlewareExport: boolean;
    compatible: boolean;
  };
  previewReady: boolean;
  blockers: SimulatorReadinessBlocker[];
}

export interface SimulatorDoctorRecord {
  id:
    | "studio-platform"
    | "studio-cpu"
    | "studio-node"
    | "studio-xcode"
    | "studio-simctl"
    | "studio-serve-sim"
    | "studio-preview";
  status: "ok" | "warning";
  message: string;
}

export interface SimulatorDiagnosticsDependencies {
  executor?: CommandExecutor;
  environment?: Record<string, string | undefined>;
  platform?: NodeJS.Platform;
  architecture?: string;
  cwd?: string;
  findExecutable?: (
    name: string,
    environment: Record<string, string | undefined>,
    cwd: string,
  ) => string | null;
  resolveServeSim?: () => ServeSimInstallation | null;
}

export type SimulatorProgressEvent =
  | { type: "development-starting" }
  | { type: "development-ready" }
  | { type: "building" }
  | { type: "simulator-launching" }
  | { type: "simulator-launched"; device: SimulatorDevice; bundleId: string }
  | { type: "screenshot-capturing" }
  | { type: "screenshot-captured"; path: string }
  | {
      type: "screenshot-unavailable";
      code: SimulatorErrorCode;
      message: string;
    }
  | { type: "completed" }
  | { type: "interrupted" }
  | { type: "failed"; code: SimulatorErrorCode; message: string };

export interface ShotRunResult {
  shotRoot: string;
  device: SimulatorDevice;
  bundleId: string;
  appPath: string;
  screenshotPath: string | null;
}

export interface RunShotOptions {
  shotRoot: string;
  releasesDirectory?: string;
  deviceUdid?: string;
  environment?: Record<string, string | undefined>;
  signal?: AbortSignal;
  onProgress?: (event: SimulatorProgressEvent) => void | Promise<void>;
}

export interface RunShotDependencies {
  executor?: CommandExecutor;
  findExecutable?: (
    name: string,
    environment: Record<string, string | undefined>,
    cwd: string,
  ) => string | null;
  randomId?: () => string;
  resolveMachine?: (
    shotRoot: string,
    releasesDirectory: string | undefined,
  ) => { root: string; machine: string };
}

interface MachineEnvelope {
  schemaVersion: 1;
  ok: boolean;
  operation: string;
  shot: string | null;
  result?: unknown;
  error?: {
    code?: unknown;
  };
}

interface IosLaunchResult {
  launched: true;
  device: SimulatorDevice;
  bundleId: string;
  appPath: string;
}

export interface SpawnedProcessExit {
  exitCode: number | null;
  signal: NodeJS.Signals | null;
}

export interface SpawnedProcess {
  pid: number;
  stdout: AsyncIterable<Uint8Array | string>;
  stderr: AsyncIterable<Uint8Array | string>;
  exited: Promise<SpawnedProcessExit>;
  kill(signal: NodeJS.Signals): boolean;
}

export interface SpawnProcessOptions {
  cwd: string;
  environment: Record<string, string | undefined>;
  signal?: AbortSignal;
}

export type ProcessSpawner = (
  argv: readonly string[],
  options: SpawnProcessOptions,
) => Promise<SpawnedProcess>;

export interface LivePreviewStatus {
  active: boolean;
  deviceUdid: string | null;
  host: typeof LIVE_PREVIEW_HOST | null;
  port: number | null;
  pid: number | null;
}

/**
 * The capability-bearing URL is deliberately a method, not enumerable status.
 * Callers should put it directly into the local iframe and must not serialize
 * it into diagnostics, logs, or shot metadata.
 */
export interface LivePreviewHandle {
  readonly deviceUdid: string;
  readonly host: typeof LIVE_PREVIEW_HOST;
  readonly port: number;
  iframeUrl(): string;
  stop(): Promise<void>;
  toJSON(): LivePreviewStatus;
}

export interface LivePreviewManagerOptions {
  executor?: CommandExecutor;
  spawner?: ProcessSpawner;
  environment?: Record<string, string | undefined>;
  platform?: NodeJS.Platform;
  architecture?: string;
  nodeExecutable?: string;
  sidecarPath?: string;
  temporaryRoot?: string;
  startTimeoutMs?: number;
  resolveServeSim?: () => ServeSimInstallation | null;
  randomCapability?: () => string;
}

interface ActiveLivePreview {
  id: string;
  child: SpawnedProcess;
  capability: string;
  deviceUdid: string;
  host: typeof LIVE_PREVIEW_HOST;
  port: number;
  temporaryDirectory: string;
  abortCleanup: (() => void) | null;
  stopping: Promise<void> | null;
}

interface StartingLivePreview {
  controller: AbortController;
  finished: Promise<void>;
  finish: () => void;
}

export interface SimulatorServiceOptions
  extends SimulatorDiagnosticsDependencies, RunShotDependencies {
  releasesDirectory?: string;
  livePreview?: LivePreviewManager;
  spawner?: ProcessSpawner;
  nodeExecutable?: string;
  sidecarPath?: string;
  temporaryRoot?: string;
  startTimeoutMs?: number;
  randomCapability?: () => string;
}

function interruptedError(): SimulatorError {
  return new SimulatorError("ABORTED", "The simulator operation was interrupted.");
}

function assertNotAborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw interruptedError();
}

function parseNodeVersion(value: string): { version: string; major: number } | null {
  const match = /^\s*v?(\d+)(?:\.(\d+))?(?:\.(\d+))?/u.exec(value);
  if (!match) return null;
  return {
    version: [match[1], match[2] ?? "0", match[3] ?? "0"].join("."),
    major: Number(match[1]),
  };
}

function inside(root: string, candidate: string): boolean {
  const fromRoot = relative(resolve(root), resolve(candidate));
  return fromRoot === "" || (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

export function canonicalSimulatorUdid(value: string): string {
  const canonical = value.trim().toUpperCase();
  if (!CANONICAL_UDID.test(canonical)) {
    throw new SimulatorError(
      "INVALID_DEVICE",
      "The Simulator device identifier is invalid.",
    );
  }
  return canonical;
}

function machineSimulatorUdid(value: string): string {
  try {
    return canonicalSimulatorUdid(value);
  } catch {
    throw new SimulatorError(
      "INVALID_MACHINE_RESPONSE",
      "The simulator returned an invalid device identifier.",
    );
  }
}

export function findExecutable(
  name: string,
  environment: Record<string, string | undefined> = process.env,
  cwd = process.cwd(),
): string | null {
  if (name.includes("/") || name.includes("\\")) {
    const candidate = isAbsolute(name) ? resolve(name) : resolve(cwd, name);
    try {
      accessSync(candidate, fsConstants.X_OK);
      return statSync(candidate).isFile() ? candidate : null;
    } catch {
      return null;
    }
  }
  for (const directory of (environment.PATH ?? "").split(delimiter).filter(Boolean)) {
    const candidate = resolve(isAbsolute(directory) ? directory : resolve(cwd, directory), name);
    try {
      accessSync(candidate, fsConstants.X_OK);
      if (statSync(candidate).isFile()) return candidate;
    } catch {
      // Keep looking.
    }
  }
  return null;
}

export function resolveServeSimInstallation(): ServeSimInstallation | null {
  let middlewarePath: string;
  try {
    middlewarePath = createRequire(import.meta.url).resolve("serve-sim/middleware");
  } catch {
    return null;
  }
  let directory = dirname(middlewarePath);
  while (true) {
    const packageJsonPath = join(directory, "package.json");
    if (existsSync(packageJsonPath)) {
      try {
        const value = JSON.parse(readFileSync(packageJsonPath, "utf8")) as {
          name?: unknown;
          version?: unknown;
          exports?: unknown;
        };
        if (value.name === "serve-sim" && typeof value.version === "string") {
          const exportsValue =
            typeof value.exports === "object" && value.exports !== null
              ? value.exports as Record<string, unknown>
              : {};
          return {
            packageJsonPath,
            middlewarePath,
            version: value.version,
            middlewareExport: exportsValue["./middleware"] !== undefined,
          };
        }
      } catch {
        return null;
      }
    }
    const parent = dirname(directory);
    if (parent === directory) return null;
    directory = parent;
  }
}

function appendBounded(
  current: string,
  chunk: string,
  limit: number,
): { value: string; overflow: boolean } {
  const nextLength = Buffer.byteLength(current) + Buffer.byteLength(chunk);
  if (nextLength <= limit) return { value: current + chunk, overflow: false };
  return { value: current, overflow: true };
}

export const executeCommand: CommandExecutor = async (argv, options) => {
  if (argv.length === 0 || !argv[0]) {
    throw new SimulatorError("MISSING_DEPENDENCY", "A required executable is unavailable.");
  }
  assertNotAborted(options.signal);
  const outputLimit = options.maxOutputBytes ?? COMMAND_OUTPUT_LIMIT;
  return await new Promise<CommandExecutionResult>((resolveResult, rejectResult) => {
    const ownsProcessGroup = process.platform !== "win32";
    const child = spawnChild(argv[0]!, [...argv.slice(1)], {
      cwd: options.cwd,
      env: options.environment as NodeJS.ProcessEnv,
      shell: false,
      detached: ownsProcessGroup,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    let outputOverflow = false;
    let interrupted = false;
    let timedOut = false;
    let settled = false;
    let terminationStarted = false;
    let timeout: ReturnType<typeof setTimeout> | undefined;
    let forceKill: ReturnType<typeof setTimeout> | undefined;

    const signalOwnedProcess = (signal: NodeJS.Signals): void => {
      if (ownsProcessGroup && child.pid !== undefined) {
        try {
          process.kill(-child.pid, signal);
          return;
        } catch {
          // The process may not have entered its new group before an early abort.
        }
      }
      child.kill(signal);
    };
    const terminateOwnedProcess = (): void => {
      if (terminationStarted) return;
      terminationStarted = true;
      signalOwnedProcess("SIGTERM");
      forceKill = setTimeout(() => {
        if (child.exitCode === null && child.signalCode === null) {
          signalOwnedProcess("SIGKILL");
        }
      }, 2_000);
      forceKill.unref?.();
    };
    const onAbort = (): void => {
      interrupted = true;
      terminateOwnedProcess();
    };
    const finish = (): void => {
      options.signal?.removeEventListener("abort", onAbort);
      if (timeout) clearTimeout(timeout);
      if (forceKill) clearTimeout(forceKill);
    };

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => {
      const appended = appendBounded(stdout, chunk, outputLimit);
      stdout = appended.value;
      if (appended.overflow && !outputOverflow) {
        outputOverflow = true;
        terminateOwnedProcess();
      }
    });
    child.stderr.on("data", (chunk: string) => {
      const appended = appendBounded(stderr, chunk, outputLimit);
      stderr = appended.value;
      if (appended.overflow && !outputOverflow) {
        outputOverflow = true;
        terminateOwnedProcess();
      }
    });
    child.once("error", () => {
      if (settled) return;
      settled = true;
      finish();
      if (interrupted) {
        rejectResult(interruptedError());
        return;
      }
      rejectResult(
        new SimulatorError("MISSING_DEPENDENCY", "A required executable could not be started."),
      );
    });
    child.once("close", (exitCode) => {
      if (settled) return;
      settled = true;
      finish();
      if (interrupted) {
        rejectResult(interruptedError());
        return;
      }
      if (timedOut) {
        rejectResult(
          new SimulatorError("MACHINE_FAILED", "A simulator operation timed out."),
        );
        return;
      }
      if (outputOverflow) {
        rejectResult(
          new SimulatorError("MACHINE_FAILED", "A simulator operation produced too much output."),
        );
        return;
      }
      resolveResult({ exitCode: exitCode ?? 1, stdout, stderr });
    });

    if (options.signal) {
      options.signal.addEventListener("abort", onAbort, { once: true });
      if (options.signal.aborted) onAbort();
    }
    if (options.timeoutMs !== undefined) {
      timeout = setTimeout(() => {
        timedOut = true;
        terminateOwnedProcess();
      }, options.timeoutMs);
      timeout.unref?.();
    }
  });
};

export const spawnProcess: ProcessSpawner = async (argv, options) => {
  if (argv.length === 0 || !argv[0]) {
    throw new SimulatorError("MISSING_DEPENDENCY", "A required executable is unavailable.");
  }
  assertNotAborted(options.signal);
  return await new Promise<SpawnedProcess>((resolveProcess, rejectProcess) => {
    const child = spawnChild(argv[0]!, [...argv.slice(1)], {
      cwd: options.cwd,
      env: options.environment as NodeJS.ProcessEnv,
      shell: false,
      detached: false,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let spawned = false;
    let settled = false;
    let exitResolved = false;
    let resolveExit!: (exit: SpawnedProcessExit) => void;
    const exited = new Promise<SpawnedProcessExit>((resolveValue) => {
      resolveExit = resolveValue;
    });
    const cleanup = (): void => {
      options.signal?.removeEventListener("abort", onAbort);
    };
    const onAbort = (): void => {
      child.kill("SIGTERM");
      if (settled) return;
      settled = true;
      cleanup();
      rejectProcess(interruptedError());
    };
    child.once("spawn", () => {
      spawned = true;
      if (settled) {
        child.kill("SIGTERM");
        return;
      }
      if (child.pid === undefined || child.stdout === null || child.stderr === null) {
        settled = true;
        cleanup();
        rejectProcess(
          new SimulatorError("LIVE_PREVIEW_FAILED", "The live preview helper could not start."),
        );
        return;
      }
      settled = true;
      cleanup();
      resolveProcess({
        pid: child.pid,
        stdout: child.stdout as Readable,
        stderr: child.stderr as Readable,
        exited,
        kill: (signal) => child.kill(signal),
      });
    });
    child.once("error", () => {
      if (!spawned && !settled) {
        settled = true;
        cleanup();
        rejectProcess(
          new SimulatorError("LIVE_PREVIEW_FAILED", "The live preview helper could not start."),
        );
      }
      if (!exitResolved) {
        exitResolved = true;
        resolveExit({ exitCode: null, signal: null });
      }
    });
    child.once("close", (exitCode, signal) => {
      if (exitResolved) return;
      exitResolved = true;
      resolveExit({
        exitCode,
        signal: signal as NodeJS.Signals | null,
      });
    });
    if (options.signal) {
      options.signal.addEventListener("abort", onAbort, { once: true });
      if (options.signal.aborted) onAbort();
    }
  });
};

function parseDevices(stdout: string): SimulatorDevice[] | null {
  let value: {
    devices?: Record<string, Array<{
      name?: unknown;
      udid?: unknown;
      state?: unknown;
      isAvailable?: unknown;
    }>>;
  };
  try {
    value = JSON.parse(stdout) as typeof value;
  } catch {
    return null;
  }
  if (
    typeof value.devices !== "object" ||
    value.devices === null ||
    Array.isArray(value.devices)
  ) {
    return null;
  }
  const devices: SimulatorDevice[] = [];
  for (const [runtime, candidates] of Object.entries(value.devices ?? {})) {
    if (!Array.isArray(candidates)) return null;
    if (!/SimRuntime\.iOS-/u.test(runtime)) continue;
    for (const candidate of candidates) {
      if (
        typeof candidate.name !== "string" ||
        !candidate.name.startsWith("iPhone") ||
        typeof candidate.udid !== "string" ||
        typeof candidate.state !== "string"
      ) {
        continue;
      }
      devices.push({
        name: candidate.name,
        udid: candidate.udid,
        state: candidate.state,
        runtime,
        available: candidate.isAvailable !== false,
      });
    }
  }
  return devices
    .filter((device) => device.available)
    .sort((left, right) =>
      Number(right.state === "Booted") - Number(left.state === "Booted") ||
      left.name.localeCompare(right.name) ||
      left.runtime.localeCompare(right.runtime)
    );
}

function diagnosticEnvironment(
  source: Record<string, string | undefined>,
): Record<string, string | undefined> {
  const allowed = new Set([
    "PATH",
    "HOME",
    "LANG",
    "SHELL",
    "USER",
    "LOGNAME",
    "TMPDIR",
    "TMP",
    "TEMP",
    "DEVELOPER_DIR",
    "TOHSENO_NODE",
  ]);
  const environment: Record<string, string | undefined> = {};
  for (const [key, value] of Object.entries(source)) {
    if (allowed.has(key) || key.startsWith("LC_")) environment[key] = value;
  }
  return environment;
}

async function safeExecute(
  executor: CommandExecutor,
  argv: readonly string[],
  options: CommandExecutionOptions,
): Promise<CommandExecutionResult | null> {
  try {
    return await executor(argv, options);
  } catch (error) {
    if (error instanceof SimulatorError && error.code === "ABORTED") throw error;
    return null;
  }
}

export async function simulatorDiagnostics(
  dependencies: SimulatorDiagnosticsDependencies = {},
  signal?: AbortSignal,
): Promise<SimulatorDiagnostics> {
  const executor = dependencies.executor ?? executeCommand;
  const environment = dependencies.environment ?? process.env;
  const cwd = resolve(dependencies.cwd ?? process.cwd());
  const platform = dependencies.platform ?? process.platform;
  const architecture = dependencies.architecture ?? process.arch;
  const executable = dependencies.findExecutable ?? findExecutable;
  const resolveServeSim = dependencies.resolveServeSim ?? resolveServeSimInstallation;
  const macos = platform === "darwin";
  const appleSilicon = architecture === "arm64";
  assertNotAborted(signal);

  const nodeExecutable = environment.TOHSENO_NODE
    ? executable(environment.TOHSENO_NODE, environment, cwd)
    : executable("node", environment, cwd);
  const xcodebuild = macos ? executable("xcodebuild", environment, cwd) : null;
  const xcrun = macos ? executable("xcrun", environment, cwd) : null;

  const commandOptions: CommandExecutionOptions = {
    cwd,
    environment: diagnosticEnvironment(environment),
    ...(signal === undefined ? {} : { signal }),
    timeoutMs: 10_000,
  };
  const [nodeResult, nodeArchitectureResult, xcodeResult, simctlResult] = await Promise.all([
    nodeExecutable
      ? safeExecute(executor, [nodeExecutable, "--version"], commandOptions)
      : Promise.resolve(null),
    nodeExecutable
      ? safeExecute(executor, [nodeExecutable, "--print", "process.arch"], commandOptions)
      : Promise.resolve(null),
    xcodebuild
      ? safeExecute(executor, [xcodebuild, "-version"], commandOptions)
      : Promise.resolve(null),
    xcrun
      ? safeExecute(
          executor,
          [xcrun, "simctl", "list", "devices", "available", "--json"],
          commandOptions,
        )
      : Promise.resolve(null),
  ]);
  const parsedNode =
    nodeResult?.exitCode === 0 ? parseNodeVersion(nodeResult.stdout.trim()) : null;
  const nodeSupported = parsedNode !== null && parsedNode.major >= MINIMUM_NODE_MAJOR;
  const nodeArchitecture =
    nodeArchitectureResult?.exitCode === 0
      ? nodeArchitectureResult.stdout.trim() || null
      : null;
  const nodeCompatible = nodeSupported && nodeArchitecture === "arm64";
  const parsedDevices =
    simctlResult?.exitCode === 0 ? parseDevices(simctlResult.stdout) : null;
  const devices = parsedDevices ?? [];
  const simctlHealthy = simctlResult?.exitCode === 0 && parsedDevices !== null;
  let serveSim: ServeSimInstallation | null = null;
  try {
    serveSim = resolveServeSim();
  } catch {
    // Package discovery is diagnostic and must not make doctor itself fail.
  }
  const exactServeSim = serveSim?.version === SERVE_SIM_VERSION;
  const middlewareExport = serveSim?.middlewareExport === true;
  const serveSimCompatible =
    macos &&
    appleSilicon &&
    nodeCompatible &&
    exactServeSim &&
    middlewareExport;

  const blockers: SimulatorReadinessBlocker[] = [];
  if (!macos) {
    blockers.push({
      code: "macos-required",
      message: "Interactive iOS preview requires macOS.",
    });
  }
  if (!appleSilicon) {
    blockers.push({
      code: "apple-silicon-required",
      message: "serve-sim 0.1.45 requires Apple Silicon.",
    });
  }
  if (!nodeExecutable || parsedNode === null) {
    blockers.push({
      code: "node-required",
      message: "A maintained Node.js installation is required for live preview.",
    });
  } else if (!nodeSupported) {
    blockers.push({
      code: "node-20-required",
      message: "Live preview requires Node.js 20 or newer.",
    });
  } else if (appleSilicon && nodeArchitecture !== "arm64") {
    blockers.push({
      code: "node-arm64-required",
      message: "Live preview requires an arm64 Node.js binary on Apple Silicon.",
    });
  }
  if (!xcodebuild || !xcrun || xcodeResult?.exitCode !== 0) {
    blockers.push({
      code: "xcode-tools-required",
      message: "Xcode command-line tools are required for iOS Simulator.",
    });
  }
  if (xcrun && !simctlHealthy) {
    blockers.push({
      code: "simctl-unhealthy",
      message: "xcrun simctl did not return a healthy device inventory.",
    });
  }
  if (simctlHealthy && devices.length === 0) {
    blockers.push({
      code: "simulator-required",
      message: "Install an available iPhone Simulator runtime in Xcode.",
    });
  }
  if (!serveSim) {
    blockers.push({
      code: "serve-sim-required",
      message: `serve-sim ${SERVE_SIM_VERSION} is required for interactive preview.`,
    });
  } else if (!exactServeSim || !middlewareExport) {
    blockers.push({
      code: "serve-sim-version",
      message: `Interactive preview requires the exact serve-sim ${SERVE_SIM_VERSION} package.`,
    });
  }

  return {
    platform: { current: platform, macos },
    cpu: { architecture, appleSilicon },
    node: {
      available: nodeExecutable !== null && parsedNode !== null,
      executable: nodeExecutable,
      version: parsedNode?.version ?? null,
      architecture: nodeArchitecture,
      supported: nodeSupported,
      compatible: nodeCompatible,
      minimumMajor: MINIMUM_NODE_MAJOR,
    },
    xcode: {
      available:
        xcodebuild !== null &&
        xcrun !== null &&
        xcodeResult?.exitCode === 0,
      xcodebuild,
      xcrun,
      version:
        xcodeResult?.exitCode === 0
          ? xcodeResult.stdout.trim().split(/\r?\n/u)[0] ?? null
          : null,
    },
    simctl: {
      healthy: simctlHealthy,
      availableDevice: devices.length > 0,
      devices,
    },
    serveSim: {
      available: serveSim !== null,
      version: serveSim?.version ?? null,
      expectedVersion: SERVE_SIM_VERSION,
      exactVersion: exactServeSim,
      middlewareExport,
      compatible: serveSimCompatible,
    },
    previewReady: blockers.length === 0,
    blockers,
  };
}

export const diagnoseSimulator = simulatorDiagnostics;

export function simulatorDoctorRecords(
  diagnostics: SimulatorDiagnostics,
): SimulatorDoctorRecord[] {
  const records: SimulatorDoctorRecord[] = [
    {
      id: "studio-platform",
      status: diagnostics.platform.macos ? "ok" : "warning",
      message: diagnostics.platform.macos
        ? "Studio iOS runtime: macOS"
        : "Studio live preview requires macOS",
    },
    {
      id: "studio-cpu",
      status: diagnostics.cpu.appleSilicon ? "ok" : "warning",
      message: diagnostics.cpu.appleSilicon
        ? `Studio CPU: ${diagnostics.cpu.architecture}`
        : "serve-sim 0.1.45 requires Apple Silicon",
    },
    {
      id: "studio-node",
      status: diagnostics.node.compatible ? "ok" : "warning",
      message: diagnostics.node.compatible
        ? `Studio Node.js ${diagnostics.node.version ?? "20+"} (${diagnostics.node.architecture})`
        : diagnostics.node.supported && diagnostics.node.architecture !== "arm64"
          ? "Studio live preview requires an arm64 Node.js binary on Apple Silicon"
          : "Studio live preview requires Node.js 20 or newer",
    },
    {
      id: "studio-xcode",
      status: diagnostics.xcode.available ? "ok" : "warning",
      message: diagnostics.xcode.available
        ? `Studio Xcode tools: ${diagnostics.xcode.version ?? "available"}`
        : "Xcode command-line tools are unavailable for Studio",
    },
    {
      id: "studio-simctl",
      status:
        diagnostics.simctl.healthy && diagnostics.simctl.availableDevice
          ? "ok"
          : "warning",
      message:
        diagnostics.simctl.healthy && diagnostics.simctl.availableDevice
          ? `${diagnostics.simctl.devices.length} available iPhone Simulator device${
              diagnostics.simctl.devices.length === 1 ? "" : "s"
            }`
          : diagnostics.simctl.healthy
            ? "No available iPhone Simulator device is installed"
            : "xcrun simctl is unavailable or unhealthy",
    },
    {
      id: "studio-serve-sim",
      status: diagnostics.serveSim.compatible ? "ok" : "warning",
      message: diagnostics.serveSim.compatible
        ? `serve-sim ${SERVE_SIM_VERSION}`
        : `Exact serve-sim ${SERVE_SIM_VERSION} compatibility is unavailable`,
    },
    {
      id: "studio-preview",
      status: diagnostics.previewReady ? "ok" : "warning",
      message: diagnostics.previewReady
        ? "Interactive Studio preview is ready"
        : "Interactive Studio preview is unavailable; contact sheet and creation remain available",
    },
  ];
  return records;
}

function requireShotRoot(value: string): { root: string; machine: string } {
  const requested = resolve(value);
  if (!existsSync(requested)) {
    throw new SimulatorError("INVALID_SHOT", "The requested shot does not exist.");
  }
  const requestedDetails = lstatSync(requested);
  if (requestedDetails.isSymbolicLink() || !requestedDetails.isDirectory()) {
    throw new SimulatorError("INVALID_SHOT", "The requested shot is not a real directory.");
  }
  const root = realpathSync(requested);
  if (readShotMetadata(root) === undefined) {
    throw new SimulatorError("INVALID_SHOT", "The requested directory is not a recognized shot.");
  }
  const local = join(root, ".tohseno");
  const machine = join(local, "machine.ts");
  for (const [path, kind] of [[local, "directory"], [machine, "file"]] as const) {
    if (!existsSync(path)) {
      throw new SimulatorError("INVALID_SHOT", "The shot has no pinned machine runtime.");
    }
    const details = lstatSync(path);
    if (
      details.isSymbolicLink() ||
      (kind === "directory" ? !details.isDirectory() : !details.isFile())
    ) {
      throw new SimulatorError("INVALID_SHOT", "The shot has an unsafe machine runtime.");
    }
    if (!inside(root, realpathSync(path))) {
      throw new SimulatorError("INVALID_SHOT", "The shot machine runtime escapes its shot.");
    }
  }
  return { root, machine };
}

function trustedMachine(
  shotRoot: string,
  releasesDirectory: string | undefined,
  resolver?: RunShotDependencies["resolveMachine"],
): { root: string; machine: string } {
  if (resolver !== undefined) {
    const expected = requireShotRoot(shotRoot);
    let resolvedMachine: { root: string; machine: string };
    try {
      resolvedMachine = resolver(expected.root, releasesDirectory);
      const candidateRoot = resolve(resolvedMachine.root);
      const candidateMachine = resolve(resolvedMachine.machine);
      if (
        !existsSync(candidateRoot) ||
        !existsSync(candidateMachine) ||
        lstatSync(candidateRoot).isSymbolicLink() ||
        !lstatSync(candidateRoot).isDirectory() ||
        lstatSync(candidateMachine).isSymbolicLink() ||
        !lstatSync(candidateMachine).isFile() ||
        realpathSync(candidateRoot) !== expected.root ||
        realpathSync(candidateMachine) !== realpathSync(expected.machine)
      ) {
        throw new Error("resolver returned an unsafe machine");
      }
    } catch {
      throw new SimulatorError(
        "INVALID_SHOT",
        "The requested shot has an unsafe pinned machine runtime.",
      );
    }
    return expected;
  }
  if (releasesDirectory === undefined) {
    throw new SimulatorError(
      "INVALID_SHOT",
      "The immutable factory release cache is required to run a shot. Run `tohseno doctor` and retry.",
    );
  }
  try {
    const trusted = trustedShotToolFromCache({
      shotRoot,
      releasesDirectory,
      tool: "machine",
    });
    return { root: trusted.root, machine: trusted.executable };
  } catch {
    throw new SimulatorError(
      "INVALID_SHOT",
      "The shot's pinned runtime does not match its immutable factory release. Run `tohseno verify <shot>` and restore or recreate the shot before running it.",
    );
  }
}

function parseMachineEnvelope(
  result: CommandExecutionResult,
  expectedOperation: string,
): MachineEnvelope {
  let value: unknown;
  try {
    value = JSON.parse(result.stdout) as unknown;
  } catch {
    throw new SimulatorError(
      "INVALID_MACHINE_RESPONSE",
      "The pinned shot machine returned an invalid response.",
    );
  }
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    (value as Partial<MachineEnvelope>).schemaVersion !== 1 ||
    typeof (value as Partial<MachineEnvelope>).ok !== "boolean" ||
    (value as Partial<MachineEnvelope>).operation !== expectedOperation
  ) {
    throw new SimulatorError(
      "INVALID_MACHINE_RESPONSE",
      "The pinned shot machine returned an invalid response.",
    );
  }
  return value as MachineEnvelope;
}

async function invokeMachine(
  executor: CommandExecutor,
  argv: readonly string[],
  options: CommandExecutionOptions,
  operation: string,
): Promise<MachineEnvelope> {
  const result = await executor(argv, options);
  const envelope = parseMachineEnvelope(result, operation);
  if (result.exitCode !== 0 || !envelope.ok) {
    const machineCode =
      typeof envelope.error?.code === "string" ? envelope.error.code : "UNKNOWN";
    throw new SimulatorError(
      "MACHINE_FAILED",
      machineFailureMessage(operation, machineCode),
      { operation, machineCode, exitCode: result.exitCode },
    );
  }
  return envelope;
}

function machineFailureMessage(operation: string, machineCode: string): string {
  if (machineCode === "MISSING_DEPENDENCY") {
    return "A required local development tool is unavailable. Run `tohseno doctor`, install the reported dependency, and retry.";
  }
  if (machineCode === "INVALID_CONFIGURATION") {
    return "The shot's pinned runtime rejected its configuration. Run `tohseno verify <shot>` and repair the reported shot files before retrying.";
  }
  if (machineCode === "UNHEALTHY_SERVICES") {
    return operation === "ios.launch"
      ? "The iOS build, Simulator boot, install, or launch failed. Run `tohseno doctor`, then inspect the shot's iOS development logs and retry."
      : "The shot's local development service did not become ready. Inspect its development logs, run `tohseno doctor`, and retry.";
  }
  return "The pinned shot machine could not complete the simulator operation. Run `tohseno doctor` and retry.";
}

async function parseIosLaunchResult(
  envelope: MachineEnvelope,
  shotRoot: string,
  executor: CommandExecutor,
  executable: NonNullable<RunShotDependencies["findExecutable"]>,
  environment: Record<string, string | undefined>,
  signal?: AbortSignal,
): Promise<IosLaunchResult> {
  const value = envelope.result;
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new SimulatorError(
      "INVALID_MACHINE_RESPONSE",
      "The pinned shot machine returned an invalid launch result.",
    );
  }
  const candidate = value as {
    launched?: unknown;
    device?: unknown;
    bundleId?: unknown;
    appPath?: unknown;
  };
  const device = candidate.device as Partial<SimulatorDevice> | undefined;
  if (
    candidate.launched !== true ||
    typeof candidate.bundleId !== "string" ||
    typeof candidate.appPath !== "string" ||
    !isAbsolute(candidate.appPath) ||
    typeof device?.name !== "string" ||
    typeof device.udid !== "string" ||
    typeof device.state !== "string"
  ) {
    throw new SimulatorError(
      "INVALID_MACHINE_RESPONSE",
      "The pinned shot machine returned an invalid launch result.",
    );
  }
  if (!existsSync(candidate.appPath)) {
    throw new SimulatorError(
      "INVALID_MACHINE_RESPONSE",
      "The pinned shot machine returned an invalid launch result.",
    );
  }
  const appDetails = lstatSync(candidate.appPath);
  const appPath = realpathSync(candidate.appPath);
  if (appDetails.isSymbolicLink() || !appDetails.isDirectory() || !inside(shotRoot, appPath)) {
    throw new SimulatorError(
      "INVALID_MACHINE_RESPONSE",
      "The pinned shot machine returned an invalid launch result.",
    );
  }
  const plistPath = join(appPath, "Info.plist");
  if (!existsSync(plistPath)) {
    throw new SimulatorError(
      "INVALID_MACHINE_RESPONSE",
      "The pinned shot machine returned an invalid launch result.",
    );
  }
  const plistDetails = lstatSync(plistPath);
  if (
    plistDetails.isSymbolicLink() ||
    !plistDetails.isFile() ||
    !inside(appPath, realpathSync(plistPath))
  ) {
    throw new SimulatorError(
      "INVALID_MACHINE_RESPONSE",
      "The pinned shot machine returned an invalid launch result.",
    );
  }
  const plutil =
    executable("/usr/bin/plutil", environment, shotRoot) ??
    executable("plutil", environment, shotRoot);
  if (plutil === null) {
    throw new SimulatorError(
      "MISSING_DEPENDENCY",
      "Property list tooling is required to validate the launched iOS app.",
    );
  }
  const extracted = await safeExecute(
    executor,
    [
      plutil,
      "-extract",
      "CFBundleIdentifier",
      "raw",
      "-o",
      "-",
      plistPath,
    ],
    {
      cwd: shotRoot,
      environment: sanitizedRuntimeEnvironment(environment),
      ...(signal === undefined ? {} : { signal }),
      timeoutMs: 10_000,
    },
  );
  const builtBundleId = extracted?.exitCode === 0 ? extracted.stdout.trim() : "";
  if (
    !BUNDLE_IDENTIFIER.test(candidate.bundleId) ||
    builtBundleId !== candidate.bundleId
  ) {
    throw new SimulatorError(
      "INVALID_MACHINE_RESPONSE",
      "The pinned shot machine returned an invalid launch result.",
    );
  }
  const runtime = typeof device.runtime === "string" ? device.runtime : "unknown";
  return {
    launched: true,
    device: {
      name: device.name,
      udid: machineSimulatorUdid(device.udid),
      state: device.state,
      runtime,
      available: device.available !== false,
    },
    bundleId: candidate.bundleId,
    appPath,
  };
}

function ensureArtifactDirectory(shotRoot: string): string {
  const local = join(shotRoot, ".tohseno");
  const artifacts = join(local, "artifacts");
  if (!existsSync(artifacts)) mkdirSync(artifacts, { mode: 0o700 });
  const details = lstatSync(artifacts);
  if (
    details.isSymbolicLink() ||
    !details.isDirectory() ||
    !inside(shotRoot, realpathSync(artifacts))
  ) {
    throw new SimulatorError(
      "SCREENSHOT_FAILED",
      "The shot screenshot directory is unsafe.",
    );
  }
  return artifacts;
}

export async function captureSimulatorScreenshot(
  options: {
    shotRoot: string;
    deviceUdid: string;
    environment?: Record<string, string | undefined>;
    signal?: AbortSignal;
  },
  dependencies: Pick<RunShotDependencies, "executor" | "findExecutable" | "randomId"> = {},
): Promise<string> {
  const executor = dependencies.executor ?? executeCommand;
  const executable = dependencies.findExecutable ?? findExecutable;
  const environment = options.environment ?? process.env;
  const { root } = requireShotRoot(options.shotRoot);
  const xcrun = executable("xcrun", environment, root);
  if (!xcrun) {
    throw new SimulatorError(
      "MISSING_DEPENDENCY",
      "Xcode command-line tools are required to capture a Simulator screenshot.",
    );
  }
  assertNotAborted(options.signal);
  const artifacts = ensureArtifactDirectory(root);
  const destination = join(artifacts, "screenshot.png");
  const temporary = join(
    artifacts,
    `.screenshot-${process.pid}-${dependencies.randomId?.() ?? randomUUID()}.png`,
  );
  try {
    const result = await executor(
      [
        xcrun,
        "simctl",
        "io",
        canonicalSimulatorUdid(options.deviceUdid),
        "screenshot",
        "--type=png",
        temporary,
      ],
      {
        cwd: root,
        environment: sanitizedRuntimeEnvironment(environment),
        ...(options.signal === undefined ? {} : { signal: options.signal }),
      },
    );
    if (result.exitCode !== 0 || !existsSync(temporary)) {
      throw new SimulatorError(
        "SCREENSHOT_FAILED",
        "The Simulator screenshot could not be captured.",
      );
    }
    const details = lstatSync(temporary);
    if (
      details.isSymbolicLink() ||
      !details.isFile() ||
      !inside(root, realpathSync(temporary))
    ) {
      throw new SimulatorError("SCREENSHOT_FAILED", "The Simulator screenshot is invalid.");
    }
    const signature = readFileSync(temporary).subarray(0, PNG_SIGNATURE.length);
    if (!signature.equals(PNG_SIGNATURE)) {
      throw new SimulatorError("SCREENSHOT_FAILED", "The Simulator screenshot is invalid.");
    }
    chmodSync(temporary, 0o600);
    renameSync(temporary, destination);
    return destination;
  } finally {
    rmSync(temporary, { force: true });
  }
}

async function emitProgress(
  onProgress: RunShotOptions["onProgress"],
  event: SimulatorProgressEvent,
): Promise<void> {
  await onProgress?.(event);
}

export async function runShotInSimulator(
  options: RunShotOptions,
  dependencies: RunShotDependencies = {},
): Promise<ShotRunResult> {
  const executor = dependencies.executor ?? executeCommand;
  const executable = dependencies.findExecutable ?? findExecutable;
  const environment = options.environment ?? process.env;
  const { root, machine } = trustedMachine(
    options.shotRoot,
    options.releasesDirectory,
    dependencies.resolveMachine,
  );
  const commandOptions: CommandExecutionOptions = {
    cwd: root,
    environment: sanitizedRuntimeEnvironment(environment),
    ...(options.signal === undefined ? {} : { signal: options.signal }),
  };
  try {
    assertNotAborted(options.signal);
    await emitProgress(options.onProgress, { type: "development-starting" });
    await invokeMachine(
      executor,
      [bunExecutable(environment), machine, "dev", "start", "--json"],
      commandOptions,
      "dev.start",
    );
    await emitProgress(options.onProgress, { type: "development-ready" });
    await emitProgress(options.onProgress, { type: "building" });
    await emitProgress(options.onProgress, { type: "simulator-launching" });
    const launchArguments = [
      bunExecutable(environment),
      machine,
      "ios",
      "launch",
      ...(options.deviceUdid === undefined
        ? []
        : ["--device", canonicalSimulatorUdid(options.deviceUdid)]),
      "--json",
    ];
    const launchEnvelope = await invokeMachine(
      executor,
      launchArguments,
      commandOptions,
      "ios.launch",
    );
    const launch = await parseIosLaunchResult(
      launchEnvelope,
      root,
      executor,
      executable,
      environment,
      options.signal,
    );
    await emitProgress(options.onProgress, {
      type: "simulator-launched",
      device: launch.device,
      bundleId: launch.bundleId,
    });
    await emitProgress(options.onProgress, { type: "screenshot-capturing" });
    let screenshotPath: string | null = null;
    let screenshotError: SimulatorError | null = null;
    try {
      screenshotPath = await captureSimulatorScreenshot(
        {
          shotRoot: root,
          deviceUdid: launch.device.udid,
          environment,
          ...(options.signal === undefined ? {} : { signal: options.signal }),
        },
        dependencies,
      );
    } catch (error) {
      if (error instanceof SimulatorError && error.code === "ABORTED") throw error;
      screenshotError = error instanceof SimulatorError
        ? error
        : new SimulatorError(
            "SCREENSHOT_FAILED",
            "The Simulator screenshot could not be captured.",
          );
    }
    if (screenshotPath !== null) {
      await emitProgress(options.onProgress, {
        type: "screenshot-captured",
        path: screenshotPath,
      });
    } else {
      await emitProgress(options.onProgress, {
        type: "screenshot-unavailable",
        code: screenshotError?.code ?? "SCREENSHOT_FAILED",
        message:
          screenshotError?.message ??
          "The Simulator screenshot could not be captured.",
      });
    }
    await emitProgress(options.onProgress, { type: "completed" });
    return {
      shotRoot: root,
      device: launch.device,
      bundleId: launch.bundleId,
      appPath: launch.appPath,
      screenshotPath,
    };
  } catch (error) {
    const simulatorError =
      error instanceof SimulatorError
        ? error
        : new SimulatorError("MACHINE_FAILED", "The simulator operation failed.");
    if (simulatorError.code === "ABORTED") {
      await emitProgress(options.onProgress, { type: "interrupted" });
    } else {
      await emitProgress(options.onProgress, {
        type: "failed",
        code: simulatorError.code,
        message: simulatorError.message,
      });
    }
    throw simulatorError;
  }
}

export const runSimulatorShot = runShotInSimulator;

function sidecarEnvironment(
  source: Record<string, string | undefined>,
  temporaryDirectory: string,
  deviceUdid: string,
  capability: string,
): Record<string, string | undefined> {
  const allowed = new Set([
    "PATH",
    "HOME",
    "LANG",
    "SHELL",
    "USER",
    "LOGNAME",
    "DEVELOPER_DIR",
  ]);
  const environment: Record<string, string | undefined> = {};
  for (const [key, value] of Object.entries(source)) {
    if (allowed.has(key) || key.startsWith("LC_")) environment[key] = value;
  }
  environment.TMPDIR = temporaryDirectory;
  environment.TMP = temporaryDirectory;
  environment.TEMP = temporaryDirectory;
  environment[SIDECAR_ENV_UDID] = deviceUdid;
  environment[SIDECAR_ENV_CAPABILITY] = capability;
  return environment;
}

function sidecarPath(): string {
  return fileURLToPath(new URL("./studio/serve-sim-sidecar.mjs", import.meta.url));
}

function privateTemporaryDirectory(rootValue: string): string {
  const root = resolve(rootValue);
  if (!existsSync(root) || !lstatSync(root).isDirectory()) {
    throw new SimulatorError(
      "LIVE_PREVIEW_FAILED",
      "The live preview temporary directory is unavailable.",
    );
  }
  const directory = mkdtempSync(join(root, SIDECAR_DIRECTORY_PREFIX));
  chmodSync(directory, 0o700);
  return directory;
}

function removePrivateTemporaryDirectory(rootValue: string, directoryValue: string): void {
  const root = resolve(rootValue);
  const directory = resolve(directoryValue);
  if (
    dirname(directory) !== root ||
    !basename(directory).startsWith(SIDECAR_DIRECTORY_PREFIX)
  ) {
    return;
  }
  rmSync(directory, { recursive: true, force: true });
}

async function readSidecarFailure(
  stream: AsyncIterable<Uint8Array | string>,
): Promise<string | null> {
  let buffer = "";
  try {
    for await (const chunk of stream) {
      buffer += typeof chunk === "string"
        ? chunk
        : Buffer.from(chunk).toString("utf8");
      if (Buffer.byteLength(buffer) > 8 * 1024) return null;
      let newline = buffer.indexOf("\n");
      while (newline !== -1) {
        const line = buffer.slice(0, newline).trim();
        buffer = buffer.slice(newline + 1);
        let value: unknown;
        try {
          value = JSON.parse(line) as unknown;
        } catch {
          newline = buffer.indexOf("\n");
          continue;
        }
        const failure = value as {
          schemaVersion?: unknown;
          event?: unknown;
          code?: unknown;
        };
        if (
          typeof value === "object" &&
          value !== null &&
          !Array.isArray(value) &&
          failure.schemaVersion === 1 &&
          failure.event === "failed" &&
          typeof failure.code === "string" &&
          /^[A-Z][A-Z0-9_]{0,63}$/u.test(failure.code)
        ) {
          return failure.code;
        }
        newline = buffer.indexOf("\n");
      }
    }
  } catch {
    // The owned child may close its pipes during teardown.
  }
  return null;
}

function sidecarExitError(code: string | null): SimulatorError {
  if (
    code === "SERVE_SIM_VERSION" ||
    code === "SERVE_SIM_IMPORT" ||
    code === "SERVE_SIM_API"
  ) {
    return new SimulatorError(
      "SERVE_SIM_UNAVAILABLE",
      `The pinned serve-sim ${SERVE_SIM_VERSION} helper could not load. Reinstall Tohseno, run \`tohseno doctor\`, and retry.`,
      { helperCode: code },
    );
  }
  if (code === "DEVICE_START") {
    return new SimulatorError(
      "LIVE_PREVIEW_FAILED",
      "The selected iPhone Simulator could not start its interactive stream. Confirm the device is booted, run `tohseno doctor`, and retry.",
      { helperCode: code },
    );
  }
  if (code === "UNSUPPORTED_PLATFORM") {
    return new SimulatorError(
      "UNSUPPORTED_PLATFORM",
      "Interactive iOS preview requires macOS.",
      { helperCode: code },
    );
  }
  if (code === "UNSUPPORTED_ARCHITECTURE") {
    return new SimulatorError(
      "UNSUPPORTED_ARCHITECTURE",
      `serve-sim ${SERVE_SIM_VERSION} requires Apple Silicon.`,
      { helperCode: code },
    );
  }
  if (code === "UNSUPPORTED_NODE") {
    return new SimulatorError(
      "UNSUPPORTED_NODE",
      "Live preview requires Node.js 20 or newer.",
      { helperCode: code },
    );
  }
  if (code === "SIDECAR_LISTEN" || code === "SIDECAR_ADDRESS") {
    return new SimulatorError(
      "LIVE_PREVIEW_FAILED",
      "The live preview helper could not bind its private localhost port. Check local security software and retry.",
      { helperCode: code },
    );
  }
  return new SimulatorError(
    "LIVE_PREVIEW_FAILED",
    "The live preview helper exited before it became ready. Run `tohseno doctor` and retry.",
    code === null ? {} : { helperCode: code },
  );
}

interface SidecarReady {
  schemaVersion: 1;
  event: "ready";
  host: typeof LIVE_PREVIEW_HOST;
  port: number;
  device: string;
}

async function readSidecarReady(
  stream: AsyncIterable<Uint8Array | string>,
): Promise<SidecarReady> {
  let buffer = "";
  for await (const chunk of stream) {
    buffer += typeof chunk === "string" ? chunk : Buffer.from(chunk).toString("utf8");
    if (Buffer.byteLength(buffer) > 8 * 1024) {
      throw new SimulatorError(
        "LIVE_PREVIEW_FAILED",
        "The live preview helper returned an invalid readiness response.",
      );
    }
    const newline = buffer.indexOf("\n");
    if (newline === -1) continue;
    const line = buffer.slice(0, newline).trim();
    let value: unknown;
    try {
      value = JSON.parse(line) as unknown;
    } catch {
      throw new SimulatorError(
        "LIVE_PREVIEW_FAILED",
        "The live preview helper returned an invalid readiness response.",
      );
    }
    if (
      typeof value !== "object" ||
      value === null ||
      Array.isArray(value) ||
      (value as Partial<SidecarReady>).schemaVersion !== 1 ||
      (value as Partial<SidecarReady>).event !== "ready" ||
      (value as Partial<SidecarReady>).host !== LIVE_PREVIEW_HOST ||
      !Number.isInteger((value as Partial<SidecarReady>).port) ||
      ((value as Partial<SidecarReady>).port ?? 0) < 1 ||
      ((value as Partial<SidecarReady>).port ?? 0) > 65_535 ||
      typeof (value as Partial<SidecarReady>).device !== "string"
    ) {
      throw new SimulatorError(
        "LIVE_PREVIEW_FAILED",
        "The live preview helper returned an invalid readiness response.",
      );
    }
    return value as SidecarReady;
  }
  // The owned-process exit branch pairs EOF with the helper's structured
  // stderr failure code. If the child stays alive without readiness, timeout
  // remains the authoritative failure.
  return await new Promise<SidecarReady>(() => {});
}

function timeoutFailure(milliseconds: number): {
  promise: Promise<never>;
  cancel: () => void;
} {
  let timer!: ReturnType<typeof setTimeout>;
  const promise = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => {
      reject(
        new SimulatorError(
          "LIVE_PREVIEW_FAILED",
          "The live preview helper did not become ready in time.",
        ),
      );
    }, milliseconds);
    timer.unref?.();
  });
  return { promise, cancel: () => clearTimeout(timer) };
}

function abortFailure(signal?: AbortSignal): {
  promise: Promise<never>;
  cancel: () => void;
} {
  if (!signal) return { promise: new Promise<never>(() => {}), cancel: () => {} };
  let onAbort!: () => void;
  const promise = new Promise<never>((_resolve, reject) => {
    onAbort = () => reject(interruptedError());
    signal.addEventListener("abort", onAbort, { once: true });
    if (signal.aborted) onAbort();
  });
  return {
    promise,
    cancel: () => signal.removeEventListener("abort", onAbort),
  };
}

async function stopExactChild(child: SpawnedProcess): Promise<void> {
  child.kill("SIGTERM");
  const graceful = await Promise.race([
    child.exited.then(() => true),
    new Promise<false>((resolveValue) => {
      const timer = setTimeout(() => resolveValue(false), 3_000);
      timer.unref?.();
    }),
  ]);
  if (graceful) return;
  child.kill("SIGKILL");
  await Promise.race([
    child.exited,
    new Promise<void>((resolveValue) => {
      const timer = setTimeout(resolveValue, 2_000);
      timer.unref?.();
    }),
  ]);
}

class ManagedLivePreviewHandle implements LivePreviewHandle {
  readonly deviceUdid: string;
  readonly host = LIVE_PREVIEW_HOST;
  readonly port: number;
  readonly #pid: number;
  readonly #id: string;
  readonly #url: string;
  readonly #manager: LivePreviewManager;

  constructor(
    manager: LivePreviewManager,
    id: string,
    deviceUdid: string,
    port: number,
    pid: number,
    capability: string,
  ) {
    this.#manager = manager;
    this.#id = id;
    this.deviceUdid = deviceUdid;
    this.port = port;
    this.#pid = pid;
    this.#url =
      `http://${LIVE_PREVIEW_HOST}:${port}/_tohseno/live/${capability}`;
  }

  iframeUrl(): string {
    return this.#url;
  }

  async stop(): Promise<void> {
    await this.#manager.stop(this.#id);
  }

  toJSON(): LivePreviewStatus {
    return {
      active: true,
      deviceUdid: this.deviceUdid,
      host: this.host,
      port: this.port,
      pid: this.#pid,
    };
  }
}

export class LivePreviewManager {
  readonly #executor: CommandExecutor;
  readonly #spawner: ProcessSpawner;
  readonly #environment: Record<string, string | undefined>;
  readonly #platform: NodeJS.Platform;
  readonly #architecture: string;
  readonly #nodeExecutable: string | undefined;
  readonly #sidecarPath: string;
  readonly #temporaryRoot: string;
  readonly #startTimeoutMs: number;
  readonly #resolveServeSim: () => ServeSimInstallation | null;
  readonly #randomCapability: () => string;
  #active: ActiveLivePreview | null = null;
  #starting: StartingLivePreview | null = null;
  #disposed = false;

  constructor(options: LivePreviewManagerOptions = {}) {
    this.#executor = options.executor ?? executeCommand;
    this.#spawner = options.spawner ?? spawnProcess;
    this.#environment = options.environment ?? process.env;
    this.#platform = options.platform ?? process.platform;
    this.#architecture = options.architecture ?? process.arch;
    this.#nodeExecutable = options.nodeExecutable;
    this.#sidecarPath = resolve(options.sidecarPath ?? sidecarPath());
    this.#temporaryRoot = resolve(options.temporaryRoot ?? tmpdir());
    this.#startTimeoutMs =
      options.startTimeoutMs ?? DEFAULT_SIDECAR_START_TIMEOUT_MS;
    this.#resolveServeSim = options.resolveServeSim ?? resolveServeSimInstallation;
    this.#randomCapability =
      options.randomCapability ?? (() => randomBytes(32).toString("base64url"));
  }

  status(): LivePreviewStatus {
    const active = this.#active;
    if (!active) {
      return {
        active: false,
        deviceUdid: null,
        host: null,
        port: null,
        pid: null,
      };
    }
    return {
      active: true,
      deviceUdid: active.deviceUdid,
      host: active.host,
      port: active.port,
      pid: active.child.pid,
    };
  }

  busy(): boolean {
    return this.#active !== null || this.#starting !== null;
  }

  async start(
    options: { deviceUdid: string; signal?: AbortSignal },
  ): Promise<LivePreviewHandle> {
    if (this.#disposed) {
      throw new SimulatorError(
        "LIVE_PREVIEW_FAILED",
        "The Studio live preview manager has already shut down.",
      );
    }
    if (this.#active || this.#starting) {
      throw new SimulatorError(
        "LIVE_PREVIEW_BUSY",
        "Only one Studio live preview can run at a time.",
      );
    }
    const controller = new AbortController();
    let finishStarting!: () => void;
    const starting: StartingLivePreview = {
      controller,
      finished: new Promise<void>((resolveFinished) => {
        finishStarting = resolveFinished;
      }),
      finish: () => finishStarting(),
    };
    this.#starting = starting;
    const forwardAbort = (): void => controller.abort();
    options.signal?.addEventListener("abort", forwardAbort, { once: true });
    if (options.signal?.aborted) forwardAbort();
    const signal = controller.signal;
    let child: SpawnedProcess | null = null;
    let temporaryDirectory: string | null = null;
    try {
      assertNotAborted(signal);
      if (this.#platform !== "darwin") {
        throw new SimulatorError(
          "UNSUPPORTED_PLATFORM",
          "Interactive iOS preview requires macOS.",
        );
      }
      if (this.#architecture !== "arm64") {
        throw new SimulatorError(
          "UNSUPPORTED_ARCHITECTURE",
          "serve-sim 0.1.45 requires Apple Silicon.",
        );
      }
      const node = this.#nodeExecutable ??
        (this.#environment.TOHSENO_NODE
          ? findExecutable(
              this.#environment.TOHSENO_NODE,
              this.#environment,
              process.cwd(),
            )
          : findExecutable("node", this.#environment, process.cwd()));
      if (!node) {
        throw new SimulatorError(
          "UNSUPPORTED_NODE",
          "A maintained Node.js installation is required for live preview.",
        );
      }
      const nodeResult = await this.#executor([node, "--version"], {
        cwd: process.cwd(),
        environment: diagnosticEnvironment(this.#environment),
        signal,
        timeoutMs: 10_000,
      });
      const nodeVersion =
        nodeResult.exitCode === 0 ? parseNodeVersion(nodeResult.stdout) : null;
      if (!nodeVersion || nodeVersion.major < MINIMUM_NODE_MAJOR) {
        throw new SimulatorError(
          "UNSUPPORTED_NODE",
          "Live preview requires Node.js 20 or newer.",
        );
      }
      const nodeArchitectureResult = await this.#executor(
        [node, "--print", "process.arch"],
        {
          cwd: process.cwd(),
          environment: diagnosticEnvironment(this.#environment),
          signal,
          timeoutMs: 10_000,
        },
      );
      if (
        nodeArchitectureResult.exitCode !== 0 ||
        nodeArchitectureResult.stdout.trim() !== "arm64"
      ) {
        throw new SimulatorError(
          "UNSUPPORTED_NODE",
          "Live preview requires an arm64 Node.js binary on Apple Silicon.",
        );
      }
      const installation = this.#resolveServeSim();
      if (
        !installation ||
        installation.version !== SERVE_SIM_VERSION ||
        !installation.middlewareExport
      ) {
        throw new SimulatorError(
          "SERVE_SIM_UNAVAILABLE",
          `Interactive preview requires the exact serve-sim ${SERVE_SIM_VERSION} package.`,
        );
      }
      if (
        !existsSync(this.#sidecarPath) ||
        lstatSync(this.#sidecarPath).isSymbolicLink() ||
        !lstatSync(this.#sidecarPath).isFile()
      ) {
        throw new SimulatorError(
          "LIVE_PREVIEW_FAILED",
          "The Studio live preview helper is unavailable.",
        );
      }
      const deviceUdid = canonicalSimulatorUdid(options.deviceUdid);
      const capability = this.#randomCapability();
      if (!/^[A-Za-z0-9_-]{43,128}$/u.test(capability)) {
        throw new SimulatorError(
          "LIVE_PREVIEW_FAILED",
          "The live preview capability could not be created.",
        );
      }
      temporaryDirectory = privateTemporaryDirectory(this.#temporaryRoot);
      child = await this.#spawner([node, this.#sidecarPath], {
        cwd: dirname(this.#sidecarPath),
        environment: sidecarEnvironment(
          this.#environment,
          temporaryDirectory,
          deviceUdid,
          capability,
        ),
        signal,
      });
      const sidecarFailure = readSidecarFailure(child.stderr);

      const timeout = timeoutFailure(this.#startTimeoutMs);
      const abort = abortFailure(signal);
      let ready: SidecarReady;
      try {
        ready = await Promise.race([
          readSidecarReady(child.stdout),
          child.exited.then(async () => {
            throw sidecarExitError(await sidecarFailure);
          }),
          timeout.promise,
          abort.promise,
        ]);
      } finally {
        timeout.cancel();
        abort.cancel();
      }
      let readyDevice: string;
      try {
        readyDevice = canonicalSimulatorUdid(ready.device);
      } catch {
        throw new SimulatorError(
          "LIVE_PREVIEW_FAILED",
          "The live preview helper returned an invalid device identifier.",
        );
      }
      if (readyDevice !== deviceUdid) {
        throw new SimulatorError(
          "LIVE_PREVIEW_FAILED",
          "The live preview helper selected an unexpected device.",
        );
      }
      assertNotAborted(signal);
      const id = randomUUID();
      const active: ActiveLivePreview = {
        id,
        child,
        capability,
        deviceUdid,
        host: LIVE_PREVIEW_HOST,
        port: ready.port,
        temporaryDirectory,
        abortCleanup: null,
        stopping: null,
      };
      this.#active = active;
      if (options.signal) {
        const onAbort = () => {
          void this.stop(id);
        };
        options.signal.addEventListener("abort", onAbort, { once: true });
        active.abortCleanup = () =>
          options.signal?.removeEventListener("abort", onAbort);
        if (options.signal.aborted) onAbort();
      }
      void child.exited.then(() => {
        this.#childExited(active);
      });
      return new ManagedLivePreviewHandle(
        this,
        id,
        deviceUdid,
        ready.port,
        child.pid,
        capability,
      );
    } catch (error) {
      if (child) await stopExactChild(child);
      if (temporaryDirectory) {
        removePrivateTemporaryDirectory(this.#temporaryRoot, temporaryDirectory);
      }
      if (error instanceof SimulatorError) throw error;
      throw new SimulatorError(
        "LIVE_PREVIEW_FAILED",
        "The Studio live preview could not start.",
      );
    } finally {
      options.signal?.removeEventListener("abort", forwardAbort);
      if (this.#starting === starting) this.#starting = null;
      starting.finish();
    }
  }

  async stop(expectedId?: string): Promise<void> {
    if (expectedId === undefined) {
      const starting = this.#starting;
      starting?.controller.abort();
      await starting?.finished;
    }
    const active = this.#active;
    if (!active || (expectedId !== undefined && active.id !== expectedId)) return;
    if (active.stopping) return await active.stopping;
    active.stopping = (async () => {
      active.abortCleanup?.();
      await stopExactChild(active.child);
      if (this.#active === active) this.#active = null;
      removePrivateTemporaryDirectory(
        this.#temporaryRoot,
        active.temporaryDirectory,
      );
    })();
    return await active.stopping;
  }

  async dispose(): Promise<void> {
    this.#disposed = true;
    const starting = this.#starting;
    starting?.controller.abort();
    await starting?.finished;
    await this.stop();
  }

  #childExited(active: ActiveLivePreview): void {
    active.abortCleanup?.();
    if (this.#active === active) this.#active = null;
    removePrivateTemporaryDirectory(
      this.#temporaryRoot,
      active.temporaryDirectory,
    );
  }
}

export class SimulatorService {
  readonly #diagnosticsDependencies: SimulatorDiagnosticsDependencies;
  readonly #runDependencies: RunShotDependencies;
  readonly #releasesDirectory: string | undefined;
  readonly livePreview: LivePreviewManager;

  constructor(options: SimulatorServiceOptions = {}) {
    this.#releasesDirectory = options.releasesDirectory;
    this.#diagnosticsDependencies = {
      ...(options.executor === undefined ? {} : { executor: options.executor }),
      ...(options.environment === undefined
        ? {}
        : { environment: options.environment }),
      ...(options.platform === undefined ? {} : { platform: options.platform }),
      ...(options.architecture === undefined
        ? {}
        : { architecture: options.architecture }),
      ...(options.cwd === undefined ? {} : { cwd: options.cwd }),
      ...(options.findExecutable === undefined
        ? {}
        : { findExecutable: options.findExecutable }),
      ...(options.resolveServeSim === undefined
        ? {}
        : { resolveServeSim: options.resolveServeSim }),
    };
    this.#runDependencies = {
      ...(options.executor === undefined ? {} : { executor: options.executor }),
      ...(options.findExecutable === undefined
        ? {}
        : { findExecutable: options.findExecutable }),
      ...(options.randomId === undefined ? {} : { randomId: options.randomId }),
      ...(options.resolveMachine === undefined
        ? {}
        : { resolveMachine: options.resolveMachine }),
    };
    this.livePreview = options.livePreview ?? new LivePreviewManager({
      ...(options.executor === undefined ? {} : { executor: options.executor }),
      ...(options.spawner === undefined ? {} : { spawner: options.spawner }),
      ...(options.environment === undefined
        ? {}
        : { environment: options.environment }),
      ...(options.platform === undefined ? {} : { platform: options.platform }),
      ...(options.architecture === undefined
        ? {}
        : { architecture: options.architecture }),
      ...(options.nodeExecutable === undefined
        ? {}
        : { nodeExecutable: options.nodeExecutable }),
      ...(options.sidecarPath === undefined
        ? {}
        : { sidecarPath: options.sidecarPath }),
      ...(options.temporaryRoot === undefined
        ? {}
        : { temporaryRoot: options.temporaryRoot }),
      ...(options.startTimeoutMs === undefined
        ? {}
        : { startTimeoutMs: options.startTimeoutMs }),
      ...(options.resolveServeSim === undefined
        ? {}
        : { resolveServeSim: options.resolveServeSim }),
      ...(options.randomCapability === undefined
        ? {}
        : { randomCapability: options.randomCapability }),
    });
  }

  async diagnostics(signal?: AbortSignal): Promise<SimulatorDiagnostics> {
    return await simulatorDiagnostics(this.#diagnosticsDependencies, signal);
  }

  async runShot(options: RunShotOptions): Promise<ShotRunResult> {
    if (this.livePreview.busy()) {
      throw new SimulatorError(
        "LIVE_PREVIEW_BUSY",
        "Stop the active Studio live preview before running another shot.",
      );
    }
    return await runShotInSimulator({
      ...options,
      ...(options.releasesDirectory !== undefined
        ? {}
        : this.#releasesDirectory === undefined
          ? {}
          : { releasesDirectory: this.#releasesDirectory }),
    }, this.#runDependencies);
  }

  async startLivePreview(
    options: { deviceUdid: string; signal?: AbortSignal },
  ): Promise<LivePreviewHandle> {
    return await this.livePreview.start(options);
  }

  async runAndPreview(
    options: RunShotOptions,
  ): Promise<{ run: ShotRunResult; preview: LivePreviewHandle }> {
    const run = await this.runShot(options);
    const preview = await this.startLivePreview({
      deviceUdid: run.device.udid,
      ...(options.signal === undefined ? {} : { signal: options.signal }),
    });
    return { run, preview };
  }

  creationRunner(): CreationRunner {
    return createSimulatorCreationRunner(this);
  }

  async dispose(): Promise<void> {
    await this.livePreview.dispose();
  }
}

/**
 * Adapter for the application-level factory in creation.ts. Simulator support
 * is optional for creation: a missing runtime or failed local build leaves the
 * already-verified shot intact and reports preview unavailability. Explicit
 * cancellation still propagates so the factory records an interruption.
 */
export function createSimulatorCreationRunner(
  service: SimulatorService = new SimulatorService(),
): CreationRunner {
  return {
    async runShot(shotRoot, options): Promise<CreationRunnerResult> {
      try {
        const run = await service.runShot({
          shotRoot,
          ...(options.signal === undefined ? {} : { signal: options.signal }),
          onProgress: async (event) => {
            if (event.type === "building" || event.type === "simulator-launching") {
              await options.onProgress?.(event.type);
            }
          },
        });
        const diagnostics = await service.diagnostics(options.signal);
        const message = diagnostics.previewReady
          ? undefined
          : diagnostics.blockers[0]?.message ??
            "Interactive Studio preview is unavailable on this machine.";
        return {
          screenshotPath: run.screenshotPath,
          previewAvailable: diagnostics.previewReady,
          ...(message === undefined ? {} : { message }),
        };
      } catch (error) {
        if (error instanceof SimulatorError && error.code === "ABORTED") throw error;
        const message = error instanceof SimulatorError
          ? error.message
          : "The local Simulator preview is unavailable.";
        return {
          screenshotPath: null,
          previewAvailable: false,
          message,
        };
      }
    },
  };
}

export const simulatorCreationRunner = createSimulatorCreationRunner;
