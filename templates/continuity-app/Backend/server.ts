import { randomUUID } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
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

function readShotMetadata(root: string): ShotMetadata {
  try {
    return JSON.parse(readFileSync(join(root, ".tohseno", "shot.json"), "utf8")) as ShotMetadata;
  } catch {
    return {};
  }
}

function atomicJson(path: string, value: unknown): void {
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  const temporary = `${path}.writing-${process.pid}-${randomUUID()}`;
  writeFileSync(temporary, `${JSON.stringify(value)}\n`, { mode: 0o600 });
  renameSync(temporary, path);
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
  const readyFile = options.readyFile ?? environment.TOHSENO_API_READY_FILE;
  const stopRequestFile = options.stopRequestFile ?? environment.TOHSENO_STOP_REQUEST_FILE;
  const instanceId = options.instanceId ?? environment.TOHSENO_INSTANCE_ID ?? "standalone";
  const log = options.log ?? ((record: Record<string, unknown>) => console.log(JSON.stringify(record)));
  const initialized = initializeDatabase(databasePathFor(root, environment));
  const metadata = readShotMetadata(root);
  const startedAt = new Date().toISOString();

  let server: ReturnType<typeof Bun.serve>;
  try {
    server = Bun.serve({
      hostname,
      port,
      async fetch(request) {
        const requestStarted = performance.now();
        const pathname = new URL(request.url).pathname;
        const route = pathname === "/health" || pathname === "/ready" ? pathname : "unmatched";
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
                slug: typeof metadata.slug === "string" ? metadata.slug : "unknown",
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
            method: request.method,
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
        const request = JSON.parse(readFileSync(stopRequestFile, "utf8")) as {
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
      message: error instanceof Error ? error.message : "unknown startup failure",
    }));
    process.exit(1);
  }
}
