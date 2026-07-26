import { resolve } from "node:path";

export const DEFAULT_REFERENCE_NODE_HOST = "127.0.0.1" as const;
export const DEFAULT_REFERENCE_NODE_PORT = 8787;

export interface ReferenceNodeConfig {
  hostname: string;
  port: number;
  databasePath: string;
}

type Environment = Record<string, string | undefined>;

function port(value: string | undefined): number {
  const source = value ?? String(DEFAULT_REFERENCE_NODE_PORT);
  if (!/^\d{1,5}$/u.test(source)) {
    throw new Error("TOHSENO_NODE_PORT must be a whole number from 1 to 65535");
  }
  const parsed = Number(source);
  if (!Number.isSafeInteger(parsed) || parsed < 1 || parsed > 65_535) {
    throw new Error("TOHSENO_NODE_PORT must be a whole number from 1 to 65535");
  }
  return parsed;
}

function hostname(value: string | undefined): string {
  const source = value ?? DEFAULT_REFERENCE_NODE_HOST;
  if (
    source === "" ||
    source.length > 253 ||
    /[\s/\\@]/u.test(source) ||
    source.includes(",")
  ) {
    throw new Error("TOHSENO_NODE_HOST is invalid");
  }
  return source;
}

export function loadReferenceNodeConfig(
  environment: Environment = process.env,
): ReferenceNodeConfig {
  const configuredPath = environment.TOHSENO_NODE_DATABASE_PATH;
  return {
    hostname: hostname(environment.TOHSENO_NODE_HOST),
    port: port(environment.TOHSENO_NODE_PORT),
    databasePath: configuredPath === undefined || configuredPath === ""
      ? resolve(import.meta.dir, "..", "data", "reference-node.sqlite")
      : resolve(configuredPath),
  };
}
