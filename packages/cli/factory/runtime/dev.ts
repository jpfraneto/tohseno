import { spawn, type ChildProcess } from "node:child_process";
import {
  accessSync,
  appendFileSync,
  closeSync,
  constants as fsConstants,
  existsSync,
  mkdirSync,
  openSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
} from "node:fs";
import { basename, delimiter, join, resolve } from "node:path";
import {
  atomicJson,
  atomicWrite,
  delay,
  encodeXcconfigUrl,
  ensureRuntimeDirectories,
  isOwnedProcess,
  isProcessAlive,
  MachineError,
  parseQuickTunnelUrl,
  publicErrorMessage,
  readDevelopmentState,
  readJson,
  runtimePaths,
  safeEnvironment,
  tailLines,
  terminateOwnedProcess,
  type DevelopmentState,
  type OwnedProcess,
  type RuntimePaths,
} from "./shared.ts";

const DEFAULT_READINESS_TIMEOUT_MS = 15_000;
const MAX_READINESS_TIMEOUT_MS = 120_000;

export interface DevStartOptions {
  tunnel: boolean;
  port?: number;
  readinessTimeoutMs?: number;
  cloudflaredPath?: string;
}

export interface DevStatus {
  state: "stopped" | "starting" | "running" | "unhealthy";
  healthy: boolean;
  instanceId: string | null;
  startedAt: string | null;
  api: {
    running: boolean;
    healthy: boolean;
    url: string | null;
    healthUrl: string | null;
    port: number | null;
    pid: number | null;
    log: string;
  };
  tunnel: {
    requested: boolean;
    running: boolean;
    url: string | null;
    pid: number | null;
    developmentOnly: true;
    constraints: string[];
    log: string;
  };
  endpoint: {
    configured: boolean;
    url: string | null;
    file: string;
  };
  issues: string[];
}

interface ApiReady {
  schemaVersion: 1;
  instanceId: string;
  pid: number;
  hostname: "127.0.0.1";
  port: number;
  origin: string;
}

interface StartupResult {
  ok: boolean;
  error?: {
    code?: string;
    message?: string;
  };
}

interface StartLock {
  schemaVersion?: unknown;
  pid?: unknown;
  attemptId?: unknown;
  instanceId?: unknown;
  commandContains?: unknown;
}

interface SupervisorOptions {
  root: string;
  machinePath: string;
  instanceId: string;
  resultPath: string;
  readinessTimeoutMs: number;
  port: number;
  cloudflaredPath?: string;
}

function commandOnPath(name: string, pathValue = process.env.PATH ?? ""): string | null {
  for (const directory of pathValue.split(delimiter).filter(Boolean)) {
    const candidate = resolve(directory, name);
    try {
      accessSync(candidate, fsConstants.X_OK);
      if (statSync(candidate).isFile()) return candidate;
    } catch {
      // Try the next PATH entry.
    }
  }
  return null;
}

function validateStartOptions(options: DevStartOptions): Required<Omit<DevStartOptions, "cloudflaredPath">> & { cloudflaredPath?: string } {
  const port = options.port ?? 0;
  if (!Number.isInteger(port) || port < 0 || port > 65_535) {
    throw new MachineError("INVALID_CONFIGURATION", "--port must be a number from 0 to 65535");
  }
  const readinessTimeoutMs = options.readinessTimeoutMs ?? DEFAULT_READINESS_TIMEOUT_MS;
  if (
    !Number.isInteger(readinessTimeoutMs) ||
    readinessTimeoutMs < 250 ||
    readinessTimeoutMs > MAX_READINESS_TIMEOUT_MS
  ) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `--readiness-timeout-ms must be from 250 to ${MAX_READINESS_TIMEOUT_MS}`,
    );
  }
  let cloudflaredPath = options.cloudflaredPath;
  if (options.tunnel) {
    cloudflaredPath ??= commandOnPath("cloudflared") ?? undefined;
    if (!cloudflaredPath) {
      throw new MachineError(
        "MISSING_DEPENDENCY",
        "cloudflared is required for --tunnel; rerun the TOHSENO installer or install cloudflared and retry",
        { dependency: "cloudflared", transport: "quick-tunnel", developmentOnly: true },
      );
    }
  }
  const value: Required<Omit<DevStartOptions, "cloudflaredPath">> & { cloudflaredPath?: string } = {
    tunnel: options.tunnel,
    port,
    readinessTimeoutMs,
  };
  if (cloudflaredPath !== undefined) value.cloudflaredPath = resolve(cloudflaredPath);
  return value;
}

async function health(url: string): Promise<boolean> {
  try {
    const response = await fetch(url, {
      headers: { Accept: "application/json" },
      signal: AbortSignal.timeout(1_500),
    });
    if (!response.ok) return false;
    const value = await response.json() as { status?: unknown; ready?: unknown; service?: unknown };
    return value.status === "ok" && value.ready === true && value.service === "shot-api";
  } catch {
    return false;
  }
}

function stoppedStatus(paths: RuntimePaths, issues: string[] = []): DevStatus {
  if (existsSync(paths.endpoint)) issues.push("generated development endpoint exists without a running stack");
  return {
    state: "stopped",
    healthy: false,
    instanceId: null,
    startedAt: null,
    api: {
      running: false,
      healthy: false,
      url: null,
      healthUrl: null,
      port: null,
      pid: null,
      log: paths.apiLog,
    },
    tunnel: {
      requested: false,
      running: false,
      url: null,
      pid: null,
      developmentOnly: true,
      constraints: quickTunnelConstraints(),
      log: paths.tunnelLog,
    },
    endpoint: { configured: false, url: null, file: paths.endpoint },
    issues,
  };
}

export function quickTunnelConstraints(): string[] {
  return [
    "development and testing only",
    "random public hostname with no uptime SLA",
    "limited concurrent requests",
    "server-sent events are unsupported",
    "never use as a production endpoint",
  ];
}

export async function developmentStatus(root: string): Promise<DevStatus> {
  const paths = runtimePaths(root);
  const state = readDevelopmentState(paths);
  if (!state) {
    if (existsSync(paths.lockMetadata)) {
      try {
        const lock = readJson<StartLock>(paths.lockMetadata);
        const owner = startLockProcess(lock);
        if (owner && await isOwnedProcess(owner)) {
          const stopped = stoppedStatus(paths);
          stopped.state = "starting";
          stopped.issues = [];
          return stopped;
        }
      } catch {
        // A malformed lock is reported as stale below rather than trusted.
      }
      return stoppedStatus(paths, ["stale development start lock detected"]);
    }
    return stoppedStatus(paths);
  }

  const [supervisorRunning, apiRunning, tunnelRunning] = await Promise.all([
    isOwnedProcess(state.supervisor),
    isOwnedProcess(state.api),
    state.tunnel ? isOwnedProcess(state.tunnel) : Promise.resolve(false),
  ]);
  const apiHealthy = apiRunning && await health(state.api.healthUrl);
  const issues: string[] = [];
  if (!supervisorRunning) issues.push("the shot-owned supervisor is not running");
  if (!apiRunning) issues.push("the shot-owned API process is not running");
  else if (!apiHealthy) issues.push("the API health check failed");
  if (state.tunnel && !tunnelRunning) issues.push("the requested Quick Tunnel process is not running");
  if (!existsSync(paths.endpoint)) issues.push("the generated development endpoint is missing");
  const healthy = issues.length === 0 && state.status === "running";
  return {
    state: healthy ? "running" : "unhealthy",
    healthy,
    instanceId: state.instanceId,
    startedAt: state.startedAt,
    api: {
      running: apiRunning,
      healthy: apiHealthy,
      url: state.api.url,
      healthUrl: state.api.healthUrl,
      port: state.api.port,
      pid: state.api.pid,
      log: state.api.log,
    },
    tunnel: {
      requested: state.tunnel !== null,
      running: tunnelRunning,
      url: state.tunnel?.url ?? null,
      pid: state.tunnel?.pid ?? null,
      developmentOnly: true,
      constraints: quickTunnelConstraints(),
      log: state.tunnel?.log ?? paths.tunnelLog,
    },
    endpoint: {
      configured: existsSync(paths.endpoint),
      url: state.endpoint.url,
      file: state.endpoint.configuration,
    },
    issues: state.issue ? [...issues, state.issue] : issues,
  };
}

async function removeStaleArtifacts(paths: RuntimePaths): Promise<void> {
  const state = readDevelopmentState(paths);
  if (state) {
    for (const record of [state.supervisor, state.api, state.tunnel].filter(Boolean) as OwnedProcess[]) {
      await terminateOwnedProcess(record);
    }
  }
  rmSync(paths.state, { force: true });
  rmSync(paths.stopRequest, { force: true });
  rmSync(paths.apiReady, { force: true });
  rmSync(paths.endpoint, { force: true });
  if (existsSync(paths.lockMetadata)) {
    try {
      const lock = readJson<StartLock>(paths.lockMetadata);
      const owner = startLockProcess(lock);
      if (!owner || !(await isOwnedProcess(owner))) {
        rmSync(paths.lock, { recursive: true, force: true });
      }
    } catch {
      rmSync(paths.lock, { recursive: true, force: true });
    }
  }
}

function removeLockForAttempt(paths: RuntimePaths, attemptId: string): void {
  if (!existsSync(paths.lockMetadata)) return;
  try {
    const lock = readJson<StartLock>(paths.lockMetadata);
    if (lock.attemptId === attemptId) rmSync(paths.lock, { recursive: true, force: true });
  } catch {
    // A caller must not remove a lock whose ownership it cannot prove.
  }
}

function removeLockForInstance(paths: RuntimePaths, instanceId: string): void {
  if (!existsSync(paths.lockMetadata)) return;
  try {
    const lock = readJson<StartLock>(paths.lockMetadata);
    if (lock.instanceId === instanceId) rmSync(paths.lock, { recursive: true, force: true });
  } catch {
    // A supervisor must not remove a lock whose ownership it cannot prove.
  }
}

function startLockProcess(lock: StartLock): OwnedProcess | null {
  if (
    typeof lock.pid !== "number" ||
    !Array.isArray(lock.commandContains) ||
    lock.commandContains.length === 0 ||
    !lock.commandContains.every((fragment) => typeof fragment === "string" && fragment !== "")
  ) return null;
  return {
    pid: lock.pid,
    role: "supervisor",
    commandContains: lock.commandContains as string[],
  };
}

async function anotherStartOwnsLock(paths: RuntimePaths, attemptId: string): Promise<boolean> {
  if (!existsSync(paths.lockMetadata)) return false;
  try {
    const lock = readJson<StartLock>(paths.lockMetadata);
    const owner = startLockProcess(lock);
    return lock.attemptId !== attemptId && owner !== null && await isOwnedProcess(owner);
  } catch {
    return false;
  }
}

async function acquireStartLock(
  paths: RuntimePaths,
  timeoutMs: number,
  attemptId: string,
  machinePath: string,
): Promise<"acquired" | "running"> {
  const deadline = Date.now() + timeoutMs;
  while (true) {
    try {
      mkdirSync(paths.lock, { mode: 0o700 });
      atomicJson(paths.lockMetadata, {
        schemaVersion: 1,
        pid: process.pid,
        attemptId,
        commandContains: [machinePath, "dev", "start"],
        createdAt: new Date().toISOString(),
      });
      return "acquired";
    } catch (error) {
      const code = error instanceof Error && "code" in error ? String(error.code) : "";
      if (code !== "EEXIST") throw error;
      const current = await developmentStatus(paths.root);
      if (current.healthy) return "running";
      let ownerAlive = false;
      try {
        const lock = readJson<StartLock>(paths.lockMetadata);
        const owner = startLockProcess(lock);
        ownerAlive = owner !== null && await isOwnedProcess(owner);
      } catch {
        ownerAlive = false;
      }
      if (!ownerAlive) {
        let recent = false;
        try {
          recent = Date.now() - statSync(paths.lock).mtimeMs < Math.min(timeoutMs, 1_000);
        } catch {
          recent = false;
        }
        if (recent) {
          await delay(25);
          continue;
        }
        rmSync(paths.lock, { recursive: true, force: true });
        continue;
      }
      if (Date.now() >= deadline) {
        throw new MachineError("UNHEALTHY_SERVICES", "another development start is still in progress");
      }
      await delay(50);
    }
  }
}

function appendLog(path: string, record: Record<string, unknown>): void {
  appendFileSync(path, `${JSON.stringify({ at: new Date().toISOString(), ...record })}\n`, { mode: 0o600 });
}

function writeStopRequest(paths: RuntimePaths, instanceId: string): void {
  atomicJson(paths.stopRequest, {
    schemaVersion: 1,
    instanceId,
    requestedAt: new Date().toISOString(),
  });
}

function stopRequested(paths: RuntimePaths, instanceId: string): boolean {
  if (!existsSync(paths.stopRequest)) return false;
  try {
    const request = JSON.parse(readFileSync(paths.stopRequest, "utf8")) as {
      schemaVersion?: unknown;
      instanceId?: unknown;
    };
    return request.schemaVersion === 1 && request.instanceId === instanceId;
  } catch {
    return false;
  }
}

export async function startDevelopment(
  root: string,
  requested: DevStartOptions,
  trustedMachinePath = join(root, ".tohseno", "machine.ts"),
): Promise<DevStatus> {
  const options = validateStartOptions(requested);
  const paths = runtimePaths(root);
  ensureRuntimeDirectories(paths);
  const attemptId = crypto.randomUUID();
  const machinePath = realpathSync(resolve(trustedMachinePath));

  let current = await developmentStatus(root);
  if (current.healthy) {
    if (options.tunnel && !current.tunnel.requested) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        "development is already running without a tunnel; stop it before restarting with --tunnel",
      );
    }
    return current;
  }
  const lockResult = await acquireStartLock(
    paths,
    options.readinessTimeoutMs,
    attemptId,
    machinePath,
  );
  current = await developmentStatus(root);
  if (current.healthy) {
    if (lockResult === "acquired") removeLockForAttempt(paths, attemptId);
    return current;
  }
  if (lockResult === "running") return current;
  if (current.state === "unhealthy" || current.state === "stopped") await removeStaleArtifacts(paths);

  const instanceId = crypto.randomUUID();
  const resultPath = join(paths.runtime, `startup-${instanceId}.json`);
  const arguments_ = [
    machinePath,
    "__supervise",
    "--instance", instanceId,
    "--result", resultPath,
    "--readiness-timeout-ms", String(options.readinessTimeoutMs),
    "--port", String(options.port),
  ];
  if (options.tunnel && options.cloudflaredPath) {
    arguments_.push("--cloudflared", options.cloudflaredPath);
  }
  const logDescriptor = openSync(paths.supervisorLog, "a", 0o600);
  let child: ChildProcess;
  try {
    appendLog(paths.supervisorLog, { event: "supervisor_start_requested", instanceId, tunnel: options.tunnel });
    child = spawn(process.execPath, arguments_, {
      cwd: root,
      env: safeEnvironment(),
      detached: true,
      stdio: ["ignore", logDescriptor, logDescriptor],
    });
  } finally {
    closeSync(logDescriptor);
  }
  if (!child.pid) {
    removeLockForAttempt(paths, attemptId);
    throw new MachineError("INTERNAL_FAILURE", "could not start the shot-owned supervisor");
  }
  child.unref();
  atomicJson(paths.lockMetadata, {
    schemaVersion: 1,
    pid: child.pid,
    attemptId,
    instanceId,
    commandContains: [machinePath, "__supervise", "--instance", instanceId],
    createdAt: new Date().toISOString(),
  });
  const supervisorRecord: OwnedProcess = {
    pid: child.pid,
    role: "supervisor",
    commandContains: [machinePath, "__supervise", "--instance", instanceId],
  };

  const deadline = Date.now() + options.readinessTimeoutMs + 2_000;
  try {
    while (Date.now() < deadline) {
      const state = readDevelopmentState(paths);
      if (state?.instanceId === instanceId && state.status === "running") {
        const status = await developmentStatus(root);
        if (status.healthy) return status;
      }
      if (existsSync(resultPath)) {
        const result = readJson<StartupResult>(resultPath);
        if (!result.ok) {
          const code = result.error?.code === "MISSING_DEPENDENCY"
            ? "MISSING_DEPENDENCY"
            : "UNHEALTHY_SERVICES";
          throw new MachineError(code, result.error?.message ?? "development startup failed");
        }
      }
      if (!isProcessAlive(child.pid)) {
        throw new MachineError("UNHEALTHY_SERVICES", "the development supervisor exited during startup");
      }
      await delay(50);
    }
    writeStopRequest(paths, instanceId);
    const stopDeadline = Date.now() + 2_000;
    while (Date.now() < stopDeadline && await isOwnedProcess(supervisorRecord)) await delay(50);
    if (await isOwnedProcess(supervisorRecord)) await terminateOwnedProcess(supervisorRecord);
    throw new MachineError(
      "UNHEALTHY_SERVICES",
      `development readiness timed out after ${options.readinessTimeoutMs}ms`,
      { readinessTimeoutMs: options.readinessTimeoutMs },
    );
  } finally {
    rmSync(resultPath, { force: true });
    const finalStatus = await developmentStatus(root);
    const state = readDevelopmentState(paths);
    if (
      !finalStatus.healthy &&
      !(await anotherStartOwnsLock(paths, attemptId)) &&
      (state === null || state.instanceId === instanceId)
    ) {
      await removeStaleArtifacts(paths);
    }
  }
}

async function childExit(child: ChildProcess, role: string): Promise<{ role: string; code: number | null; signal: string | null }> {
  return new Promise((resolveExit) => {
    child.once("exit", (code, signal) => resolveExit({ role, code, signal }));
  });
}

async function waitForApiReady(
  paths: RuntimePaths,
  child: ChildProcess,
  instanceId: string,
  timeoutMs: number,
): Promise<ApiReady> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (stopRequested(paths, instanceId)) {
      throw new MachineError("UNHEALTHY_SERVICES", "development startup was stopped before API readiness");
    }
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new MachineError("UNHEALTHY_SERVICES", "the local API exited before readiness");
    }
    if (existsSync(paths.apiReady)) {
      const ready = readJson<ApiReady>(paths.apiReady);
      if (
        ready.schemaVersion === 1 &&
        ready.instanceId === instanceId &&
        ready.pid === child.pid &&
        ready.hostname === "127.0.0.1" &&
        Number.isInteger(ready.port) &&
        ready.port > 0 &&
        ready.origin === `http://127.0.0.1:${ready.port}`
      ) return ready;
      throw new MachineError("UNHEALTHY_SERVICES", "the local API wrote invalid readiness metadata");
    }
    await delay(40);
  }
  throw new MachineError("UNHEALTHY_SERVICES", `the local API did not become ready within ${timeoutMs}ms`);
}

async function waitForTunnel(
  paths: RuntimePaths,
  child: ChildProcess,
  instanceId: string,
  offset: number,
  timeoutMs: number,
): Promise<string> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (stopRequested(paths, instanceId)) {
      throw new MachineError("UNHEALTHY_SERVICES", "development startup was stopped before tunnel readiness");
    }
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new MachineError("UNHEALTHY_SERVICES", "cloudflared exited before publishing a Quick Tunnel URL");
    }
    if (existsSync(paths.tunnelLog)) {
      const source = readFileSync(paths.tunnelLog, "utf8").slice(offset);
      const url = parseQuickTunnelUrl(source);
      if (url) return url;
    }
    await delay(50);
  }
  throw new MachineError("UNHEALTHY_SERVICES", `cloudflared did not publish a Quick Tunnel URL within ${timeoutMs}ms`);
}

async function stopChild(child: ChildProcess | undefined): Promise<void> {
  if (!child || !child.pid || child.exitCode !== null || child.signalCode !== null) return;
  const exitPromise = childExit(child, "cleanup").then(() => true);
  try {
    child.kill("SIGTERM");
  } catch {
    return;
  }
  const exited = await Promise.race([
    exitPromise,
    delay(1_500).then(() => false),
  ]);
  if (!exited) {
    try {
      child.kill("SIGKILL");
    } catch {
      // It exited between timeout and signal.
    }
  }
}

async function waitForStopRequest(
  paths: RuntimePaths,
  instanceId: string,
  active: () => boolean,
): Promise<{ role: "stop-request"; signal: null }> {
  while (active()) {
    if (existsSync(paths.stopRequest)) {
      try {
        const request = JSON.parse(readFileSync(paths.stopRequest, "utf8")) as {
          schemaVersion?: unknown;
          instanceId?: unknown;
        };
        if (request.schemaVersion === 1 && request.instanceId === instanceId) {
          return { role: "stop-request", signal: null };
        }
      } catch {
        // Only a valid request for this exact supervisor instance can stop it.
      }
    }
    await delay(50);
  }
  return new Promise(() => undefined);
}

export async function runSupervisor(options: SupervisorOptions): Promise<number> {
  const paths = runtimePaths(options.root);
  ensureRuntimeDirectories(paths);
  let api: ChildProcess | undefined;
  let tunnel: ChildProcess | undefined;
  const startedAt = new Date().toISOString();
  try {
    rmSync(paths.apiReady, { force: true });
    rmSync(paths.endpoint, { force: true });
    const apiLog = openSync(paths.apiLog, "a", 0o600);
    try {
      const environment = {
        ...safeEnvironment(),
        NODE_ENV: "development",
        TOHSENO_SHOT_ROOT: options.root,
        TOHSENO_API_HOST: "127.0.0.1",
        TOHSENO_API_PORT: String(options.port),
        TOHSENO_API_READY_FILE: paths.apiReady,
        TOHSENO_STOP_REQUEST_FILE: paths.stopRequest,
        TOHSENO_DATABASE_PATH: join(paths.data, "development.sqlite3"),
        TOHSENO_INSTANCE_ID: options.instanceId,
      };
      api = spawn(
        process.execPath,
        [join(options.root, "Backend", "server.ts"), "--tohseno-instance", options.instanceId],
        { cwd: options.root, env: environment, stdio: ["ignore", apiLog, apiLog] },
      );
    } finally {
      closeSync(apiLog);
    }
    if (!api.pid) throw new MachineError("UNHEALTHY_SERVICES", "the local API process did not start");
    const ready = await waitForApiReady(paths, api, options.instanceId, options.readinessTimeoutMs);
    const healthUrl = `${ready.origin}/health`;
    const healthDeadline = Date.now() + options.readinessTimeoutMs;
    while (!(await health(healthUrl))) {
      if (stopRequested(paths, options.instanceId)) {
        throw new MachineError("UNHEALTHY_SERVICES", "development startup was stopped during API health checking");
      }
      if (api.exitCode !== null || api.signalCode !== null || Date.now() >= healthDeadline) {
        throw new MachineError("UNHEALTHY_SERVICES", "the local API failed its readiness health check");
      }
      await delay(50);
    }

    let tunnelUrl: string | null = null;
    if (options.cloudflaredPath) {
      appendLog(paths.tunnelLog, { event: "quick_tunnel_start", instanceId: options.instanceId, developmentOnly: true });
      const offset = statSync(paths.tunnelLog).size;
      const tunnelLog = openSync(paths.tunnelLog, "a", 0o600);
      try {
        tunnel = spawn(options.cloudflaredPath, [
          "tunnel",
          "--url", ready.origin,
          "--no-autoupdate",
          "--loglevel", "info",
          "--logfile", paths.tunnelLog,
        ], {
          cwd: options.root,
          env: safeEnvironment(),
          stdio: ["ignore", tunnelLog, tunnelLog],
        });
      } finally {
        closeSync(tunnelLog);
      }
      if (!tunnel.pid) throw new MachineError("UNHEALTHY_SERVICES", "cloudflared did not start");
      tunnelUrl = await waitForTunnel(paths, tunnel, options.instanceId, offset, options.readinessTimeoutMs);
    }

    const endpointUrl = tunnelUrl ?? ready.origin;
    atomicWrite(paths.endpoint, [
      "// Generated by the shot-local TOHSENO runtime. Development only.",
      "// This file is gitignored and removed by `machine dev stop`.",
      `TOHSENO_API_BASE_URL = ${encodeXcconfigUrl(endpointUrl)}`,
      "",
    ].join("\n"));
    const apiRecord: DevelopmentState["api"] = {
      pid: api.pid,
      role: "api",
      commandContains: [join(options.root, "Backend", "server.ts"), "--tohseno-instance", options.instanceId],
      hostname: "127.0.0.1",
      port: ready.port,
      url: ready.origin,
      healthUrl,
      log: paths.apiLog,
    };
    const tunnelRecord: DevelopmentState["tunnel"] = tunnel && tunnel.pid && tunnelUrl && options.cloudflaredPath
      ? {
        pid: tunnel.pid,
        role: "tunnel",
        commandContains: [basename(options.cloudflaredPath), "tunnel", ready.origin, paths.tunnelLog],
        url: tunnelUrl,
        log: paths.tunnelLog,
        developmentOnly: true,
      }
      : null;
    const state: DevelopmentState = {
      schemaVersion: 1,
      instanceId: options.instanceId,
      status: "running",
      startedAt,
      updatedAt: new Date().toISOString(),
      shotRoot: options.root,
      supervisor: {
        pid: process.pid,
        role: "supervisor",
        commandContains: [options.machinePath, "__supervise", "--instance", options.instanceId],
      },
      api: apiRecord,
      tunnel: tunnelRecord,
      endpoint: {
        url: endpointUrl,
        transport: tunnelUrl ? "quick-tunnel" : "localhost",
        configuration: paths.endpoint,
      },
    };
    atomicJson(paths.state, state);
    atomicJson(options.resultPath, { ok: true });
    removeLockForInstance(paths, options.instanceId);

    let resolveSignal: ((signal: string) => void) | undefined;
    const signal = new Promise<{ role: "signal"; signal: string }>((resolveSignalPromise) => {
      resolveSignal = (name) => resolveSignalPromise({ role: "signal", signal: name });
    });
    const onTerm = (): void => resolveSignal?.("SIGTERM");
    const onInt = (): void => resolveSignal?.("SIGINT");
    process.once("SIGTERM", onTerm);
    process.once("SIGINT", onInt);
    let monitoring = true;
    const waits: Array<Promise<{ role: string; code?: number | null; signal: string | null }>> = [
      childExit(api, "api"),
      signal,
      waitForStopRequest(paths, options.instanceId, () => monitoring),
    ];
    if (tunnel) waits.push(childExit(tunnel, "tunnel"));
    const event = await Promise.race(waits);
    monitoring = false;
    process.off("SIGTERM", onTerm);
    process.off("SIGINT", onInt);

    if (event.role !== "signal" && event.role !== "stop-request") {
      writeStopRequest(paths, options.instanceId);
      const unhealthy: DevelopmentState = {
        ...state,
        status: "unhealthy",
        updatedAt: new Date().toISOString(),
        issue: `${event.role} exited unexpectedly`,
      };
      atomicJson(paths.state, unhealthy);
      rmSync(paths.endpoint, { force: true });
      await Promise.all([stopChild(api), stopChild(tunnel)]);
      return 4;
    }

    writeStopRequest(paths, options.instanceId);
    await Promise.all([stopChild(tunnel), stopChild(api)]);
    rmSync(paths.state, { force: true });
    rmSync(paths.stopRequest, { force: true });
    rmSync(paths.apiReady, { force: true });
    rmSync(paths.endpoint, { force: true });
    return 0;
  } catch (error) {
    writeStopRequest(paths, options.instanceId);
    await Promise.all([stopChild(tunnel), stopChild(api)]);
    rmSync(paths.state, { force: true });
    rmSync(paths.stopRequest, { force: true });
    rmSync(paths.apiReady, { force: true });
    rmSync(paths.endpoint, { force: true });
    removeLockForInstance(paths, options.instanceId);
    atomicJson(options.resultPath, {
      ok: false,
      error: {
        code: error instanceof MachineError ? error.code : "UNHEALTHY_SERVICES",
        message: publicErrorMessage(error),
      },
    });
    appendLog(paths.supervisorLog, { event: "startup_failed", errorType: error instanceof Error ? error.constructor.name : "Unknown" });
    return error instanceof MachineError ? error.exitCode : 4;
  }
}

export async function stopDevelopment(root: string): Promise<{
  stopped: boolean;
  stoppedPids: number[];
  refusedPids: number[];
  preservedData: string;
  logs: string;
}> {
  const paths = runtimePaths(root);
  const state = readDevelopmentState(paths);
  let starting: { instanceId: string; supervisor: OwnedProcess } | null = null;
  if (!state && existsSync(paths.lockMetadata)) {
    const deadline = Date.now() + 250;
    while (Date.now() <= deadline) {
      try {
        const lock = readJson<StartLock>(paths.lockMetadata);
        if (
          typeof lock.pid === "number" &&
          typeof lock.instanceId === "string"
        ) {
          const commandContains =
            Array.isArray(lock.commandContains) &&
            lock.commandContains.every((value) => typeof value === "string")
              ? lock.commandContains
              : [
                  join(paths.local, "machine.ts"),
                  "__supervise",
                  "--instance",
                  lock.instanceId,
                ];
          starting = {
            instanceId: lock.instanceId,
            supervisor: {
              pid: lock.pid,
              role: "supervisor",
              commandContains,
            },
          };
          break;
        }
        if (typeof lock.pid !== "number" || !isProcessAlive(lock.pid)) break;
      } catch {
        break;
      }
      await delay(25);
    }
  }
  const stoppedPids: number[] = [];
  const refusedPids: number[] = [];
  if (state || starting) {
    const supervisor = state?.supervisor ?? starting!.supervisor;
    const instanceId = state?.instanceId ?? starting!.instanceId;
    const records = state
      ? [state.supervisor, state.tunnel, state.api].filter(Boolean) as OwnedProcess[]
      : [supervisor];
    const initiallyOwned = new Map<number, boolean>();
    for (const record of records) initiallyOwned.set(record.pid, await isOwnedProcess(record));

    if (initiallyOwned.get(supervisor.pid)) {
      writeStopRequest(paths, instanceId);
      const deadline = Date.now() + 4_000;
      while (Date.now() < deadline && await isOwnedProcess(supervisor)) await delay(50);
    }

    for (const record of records) {
      if (!initiallyOwned.get(record.pid)) {
        if (isProcessAlive(record.pid)) refusedPids.push(record.pid);
        continue;
      }
      if (await isOwnedProcess(record)) await terminateOwnedProcess(record);
      stoppedPids.push(record.pid);
    }
  }
  rmSync(paths.state, { force: true });
  rmSync(paths.stopRequest, { force: true });
  rmSync(paths.apiReady, { force: true });
  rmSync(paths.endpoint, { force: true });
  if (existsSync(paths.lockMetadata)) {
    try {
      const lock = readJson<StartLock>(paths.lockMetadata);
      const owner = startLockProcess(lock);
      if (!owner || !(await isOwnedProcess(owner))) {
        rmSync(paths.lock, { recursive: true, force: true });
      }
    } catch {
      rmSync(paths.lock, { recursive: true, force: true });
    }
  }
  return {
    stopped: state !== null || starting !== null,
    stoppedPids,
    refusedPids,
    preservedData: paths.data,
    logs: paths.logs,
  };
}

export function developmentLogs(
  root: string,
  options: { service: "api" | "tunnel" | "supervisor" | "ios" | "all"; lines: number },
): { service: string; lines: number; logs: Record<string, string[]> } {
  if (!Number.isInteger(options.lines) || options.lines < 1 || options.lines > 2_000) {
    throw new MachineError("INVALID_CONFIGURATION", "--lines must be a number from 1 to 2000");
  }
  const paths = runtimePaths(root);
  const selected: Array<[string, string]> = options.service === "all"
    ? [["api", paths.apiLog], ["tunnel", paths.tunnelLog], ["supervisor", paths.supervisorLog], ["ios", paths.iosLog]]
    : [[options.service, options.service === "api"
      ? paths.apiLog
      : options.service === "tunnel"
        ? paths.tunnelLog
        : options.service === "supervisor"
          ? paths.supervisorLog
          : paths.iosLog]];
  return {
    service: options.service,
    lines: options.lines,
    logs: Object.fromEntries(selected.map(([name, path]) => [name, tailLines(path, options.lines)])),
  };
}
