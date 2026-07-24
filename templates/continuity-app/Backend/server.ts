import { randomUUID } from "node:crypto";
import {
  closeSync,
  constants,
  existsSync,
  fchmodSync,
  fstatSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  readSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { DATABASE_SCHEMA_VERSION, initializeDatabase } from "./database.ts";

const API_SCHEMA_VERSION = 1 as const;

interface ShotMetadata {
  slug?: unknown;
  platform?: unknown;
  factory?: {
    releaseId?: unknown;
    templateVersion?: unknown;
  };
}

export interface ShotApiOptions {
  root?: string;
  environment?: Record<string, string | undefined>;
  port?: number;
  hostname?: string;
  readyFile?: string;
  instanceId?: string;
  stopRequestFile?: string;
  log?: (record: Record<string, unknown>) => void;
}

export interface RunningShotApi {
  hostname: string;
  port: number;
  origin: string;
  databasePath: string;
  stop(): Promise<void>;
}

function asPort(value: string | undefined): number {
  if (value === undefined || value === "") return 0;
  if (!/^\d{1,5}$/u.test(value)) throw new Error("TOHSENO_API_PORT must be a number from 0 to 65535");
  const port = Number(value);
  if (!Number.isInteger(port) || port < 0 || port > 65_535) {
    throw new Error("TOHSENO_API_PORT must be a number from 0 to 65535");
  }
  return port;
}

function runtimeRoot(rootValue: string): string {
  const root = resolve(rootValue);
  return existsSync(root) ? realpathSync(root) : root;
}

function databasePathFor(root: string, environment: Record<string, string | undefined>): string {
  const configured = environment.TOHSENO_DATABASE_PATH;
  if (environment.NODE_ENV === "production") {
    if (!configured || !isAbsolute(configured)) {
      throw new Error("production requires an absolute TOHSENO_DATABASE_PATH");
    }
    return resolve(configured);
  }
  return configured ? resolve(root, configured) : join(root, ".tohseno", "data", "development.sqlite3");
}

function hostFor(environment: Record<string, string | undefined>, explicit?: string): string {
  const hostname = explicit ?? environment.TOHSENO_API_HOST ?? "127.0.0.1";
  if (environment.NODE_ENV !== "production" && hostname !== "127.0.0.1" && hostname !== "localhost") {
    throw new Error("development API must bind to localhost (127.0.0.1)");
  }
  return hostname;
}

function readBoundedJson(path: string, maximumBytes: number): unknown {
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
      throw new Error("unsafe JSON file");
    }
    const chunks: Buffer[] = [];
    const buffer = Buffer.allocUnsafe(65_536);
    let total = 0;
    while (true) {
      const length = readSync(descriptor, buffer, 0, buffer.length, null);
      if (length === 0) break;
      total += length;
      if (total > maximumBytes) throw new Error("JSON file grew past its limit");
      chunks.push(Buffer.from(buffer.subarray(0, length)));
    }
    const source = new TextDecoder("utf-8", { fatal: true }).decode(
      Buffer.concat(chunks, total),
    );
    return JSON.parse(source) as unknown;
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
}

function readShotMetadata(root: string): ShotMetadata {
  try {
    const value = readBoundedJson(
      join(root, ".tohseno", "shot.json"),
      65_536,
    ) as ShotMetadata;
    if (
      typeof value.slug !== "string" ||
      !/^[a-z0-9](?:[a-z0-9]|-(?=[a-z0-9])){0,62}$/u.test(value.slug) ||
      value.platform !== "ios" ||
      typeof value.factory !== "object" ||
      value.factory === null ||
      typeof value.factory.releaseId !== "string" ||
      !/^(?:git-[0-9a-f]{40}(?:-dirty)?-[0-9a-f]{16}|content-[0-9a-f]{32})$/u
        .test(value.factory.releaseId) ||
      typeof value.factory.templateVersion !== "string" ||
      !/^[a-z0-9][a-z0-9.-]{0,127}$/u.test(value.factory.templateVersion)
    ) {
      return {};
    }
    return value;
  } catch {
    return {};
  }
}

function atomicJson(path: string, value: unknown): void {
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
      0o600,
    );
    writeFileSync(descriptor, `${JSON.stringify(value)}\n`, {
      encoding: "utf8",
    });
    fchmodSync(descriptor, 0o600);
    fsyncSync(descriptor);
    closeSync(descriptor);
    descriptor = undefined;
    renameSync(temporary, path);
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
    rmSync(temporary, { force: true });
  }
}

function responseJson(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "Cache-Control": "no-store",
    },
  });
}

export async function startShotApi(options: ShotApiOptions = {}): Promise<RunningShotApi> {
  const environment = options.environment ?? process.env;
  const root = runtimeRoot(options.root ?? environment.TOHSENO_SHOT_ROOT ?? resolve(import.meta.dir, ".."));
  const hostname = hostFor(environment, options.hostname);
  const port = options.port ?? asPort(environment.TOHSENO_API_PORT);
  if (!Number.isSafeInteger(port) || port < 0 || port > 65_535) {
    throw new Error("API port must be a number from 0 to 65535");
  }
  const readyFile = options.readyFile ?? environment.TOHSENO_API_READY_FILE;
  const stopRequestFile = options.stopRequestFile ?? environment.TOHSENO_STOP_REQUEST_FILE;
  const instanceId = options.instanceId ?? environment.TOHSENO_INSTANCE_ID ?? "standalone";
  if (!/^[A-Za-z0-9_-]{1,128}$/u.test(instanceId)) {
    throw new Error("TOHSENO instance id is invalid");
  }
  const log = options.log ?? ((record: Record<string, unknown>) => console.log(JSON.stringify(record)));
  const initialized = initializeDatabase(
    databasePathFor(root, environment),
    environment.NODE_ENV === "production" ? {} : { boundary: root },
  );
  const metadata = readShotMetadata(root);
  const startedAt = new Date().toISOString();

  let server: ReturnType<typeof Bun.serve>;
  try {
    server = Bun.serve({
      hostname,
      port,
      maxRequestBodySize: 1_024,
      async fetch(request) {
        const requestStarted = performance.now();
        const pathname = new URL(request.url).pathname;
        const route = pathname === "/health" || pathname === "/ready" ? pathname : "unmatched";
        const method = request.method === "GET" ? "GET" : "OTHER";
        let status = 404;
        try {
          if (request.method === "GET" && (pathname === "/health" || pathname === "/ready")) {
            status = 200;
            return responseJson({
              schemaVersion: API_SCHEMA_VERSION,
              status: "ok",
              ready: true,
              service: "shot-api",
              shot: {
                platform: typeof metadata.platform === "string" ? metadata.platform : "ios",
              },
              build: {
                factoryReleaseId: typeof metadata.factory?.releaseId === "string"
                  ? metadata.factory.releaseId
                  : "unreleased-template",
                templateVersion: typeof metadata.factory?.templateVersion === "string"
                  ? metadata.factory.templateVersion
                  : "unreleased-template",
              },
              runtime: {
                apiSchemaVersion: API_SCHEMA_VERSION,
                databaseSchemaVersion: DATABASE_SCHEMA_VERSION,
                persistence: "sqlite",
                startedAt,
              },
            });
          }
          if (request.method !== "GET") {
            status = 405;
            return responseJson({ error: "method_not_allowed" }, status);
          }
          return responseJson({ error: "not_found" }, status);
        } finally {
          log({
            event: "request",
            method,
            route,
            status,
            durationMs: Math.round((performance.now() - requestStarted) * 100) / 100,
          });
        }
      },
    });
  } catch (error) {
    initialized.database.close();
    throw error;
  }

  const boundPort = server.port;
  if (typeof boundPort !== "number") {
    server.stop(true);
    initialized.database.close();
    throw new Error("the API server did not report its bound port");
  }
  const origin = `http://${hostname}:${boundPort}`;
  if (readyFile) {
    atomicJson(readyFile, {
      schemaVersion: 1,
      instanceId,
      pid: process.pid,
      hostname,
      port: boundPort,
      origin,
      readyAt: new Date().toISOString(),
    });
  }
  log({ event: "startup", service: "shot-api", hostname, port: boundPort, databaseSchemaVersion: 1 });

  let stopped = false;
  let stopRequestTimer: ReturnType<typeof setInterval> | undefined;
  const running: RunningShotApi = {
    hostname,
    port: boundPort,
    origin,
    databasePath: initialized.path,
    async stop() {
      if (stopped) return;
      stopped = true;
      if (stopRequestTimer) clearInterval(stopRequestTimer);
      server.stop(true);
      initialized.database.close();
      if (readyFile) rmSync(readyFile, { force: true });
      log({ event: "shutdown", service: "shot-api" });
    },
  };
  if (stopRequestFile) {
    stopRequestTimer = setInterval(() => {
      if (!existsSync(stopRequestFile)) return;
      try {
        const details = lstatSync(stopRequestFile);
        if (
          details.isSymbolicLink() ||
          !details.isFile() ||
          details.size > 4_096
        ) return;
        const request = readBoundedJson(stopRequestFile, 4_096) as {
          schemaVersion?: unknown;
          instanceId?: unknown;
        };
        if (request.schemaVersion === 1 && request.instanceId === instanceId) void running.stop();
      } catch {
        // Ignore incomplete or malformed requests; the supervisor owns the file.
      }
    }, 50);
  }
  return running;
}

function instanceFromArguments(arguments_: readonly string[]): string | undefined {
  if (arguments_.length === 0) return undefined;
  if (arguments_.length === 2 && arguments_[0] === "--tohseno-instance") return arguments_[1];
  throw new Error("unsupported API arguments");
}

if (import.meta.main) {
  try {
    const instanceId = instanceFromArguments(Bun.argv.slice(2));
    const running = await startShotApi(instanceId === undefined ? {} : { instanceId });
    const shutdown = async (): Promise<void> => {
      await running.stop();
      process.exit(0);
    };
    process.on("SIGINT", shutdown);
    process.on("SIGTERM", shutdown);
  } catch (error) {
    console.error(JSON.stringify({
      event: "startup_failed",
      service: "shot-api",
      errorType: error instanceof Error ? error.constructor.name : "Unknown",
    }));
    process.exit(1);
  }
}
