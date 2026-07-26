import type { SignatureVerifier } from "../../packages/signer/src/index.ts";
import {
  localEd25519VerifierSet,
} from "../../packages/signer/src/index.ts";
import {
  createReferenceNodeApplication,
  type ReferenceNodeLogger,
} from "./src/application.ts";
import { loadReferenceNodeConfig } from "./src/config.ts";
import {
  openReferenceNodeDatabase,
  REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
} from "./src/database.ts";
import { MAX_RECORD_BODY_BYTES } from "./src/http.ts";
import { SqlitePublicRecordRegistry } from "./src/registry.ts";

export interface StartReferenceNodeOptions {
  hostname?: string;
  port?: number;
  databasePath?: string;
  verifier?: SignatureVerifier;
  logger?: ReferenceNodeLogger;
  now?: () => Date;
}

export interface RunningReferenceNode {
  origin: string;
  port: number;
  server: Bun.Server<undefined>;
  registry: SqlitePublicRecordRegistry;
  stop(): Promise<void>;
}

function structuredRequestLogger(
  entry: Parameters<ReferenceNodeLogger>[0],
): void {
  console.info(JSON.stringify(entry));
}

export function startReferenceNode(
  options: StartReferenceNodeOptions = {},
): RunningReferenceNode {
  const configured = loadReferenceNodeConfig();
  const hostname = options.hostname ?? configured.hostname;
  const port = options.port ?? configured.port;
  const databasePath = options.databasePath ?? configured.databasePath;
  if (
    hostname.length < 1 ||
    hostname.length > 253 ||
    /[\s/\\@,]/u.test(hostname)
  ) {
    throw new Error("reference node hostname is invalid");
  }
  if (!Number.isSafeInteger(port) || port < 0 || port > 65_535) {
    throw new Error("reference node port is invalid");
  }

  const opened = openReferenceNodeDatabase(databasePath);
  const registry = new SqlitePublicRecordRegistry(
    opened.database,
    options.verifier ?? localEd25519VerifierSet(),
    options.now ?? (() => new Date()),
  );
  const application = createReferenceNodeApplication({
    registry,
    databaseSchemaVersion: REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
    logger: options.logger ?? structuredRequestLogger,
  });

  let server: Bun.Server<undefined>;
  try {
    server = Bun.serve({
      hostname,
      port,
      maxRequestBodySize: MAX_RECORD_BODY_BYTES,
      fetch: application,
    });
  } catch (error) {
    opened.database.close();
    throw error;
  }
  if (server.port === undefined) {
    void server.stop(true);
    opened.database.close();
    throw new Error("reference node did not receive a bound port");
  }

  let stopping: Promise<void> | undefined;
  const stop = (): Promise<void> => {
    stopping ??= (async () => {
      try {
        await server.stop(true);
      } finally {
        opened.database.close();
      }
    })();
    return stopping;
  };
  return {
    origin: `http://${hostname.includes(":") ? `[${hostname}]` : hostname}:${server.port}`,
    port: server.port,
    server,
    registry,
    stop,
  };
}

if (import.meta.main) {
  try {
    const running = startReferenceNode();
    console.info(JSON.stringify({
      event: "startup",
      service: "tohseno-reference-node",
      port: running.port,
    }));
    let shutdownStarted = false;
    const shutdown = (): void => {
      if (shutdownStarted) return;
      shutdownStarted = true;
      void running.stop().finally(() => process.exit(0));
    };
    process.once("SIGINT", shutdown);
    process.once("SIGTERM", shutdown);
  } catch {
    console.error(JSON.stringify({
      event: "startup-failed",
      service: "tohseno-reference-node",
    }));
    process.exit(1);
  }
}
