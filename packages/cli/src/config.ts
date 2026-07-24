import {
  existsSync,
  lstatSync,
  mkdirSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { CONFIG_SCHEMA_VERSION } from "./constants.ts";
import { isAgentId, type AgentId } from "./agents.ts";
import { CliError, errorMessage } from "./errors.ts";
import { readBoundedJson } from "./files.ts";

interface ConfigFile {
  schemaVersion: typeof CONFIG_SCHEMA_VERSION;
  shotsDirectory?: string;
  defaultAgent?: AgentId;
  onboardingVersion?: number;
}

export interface ResolvedConfig {
  factoryHome: string;
  cacheDirectory: string;
  configPath: string;
  shotsDirectory: string;
  configExists: boolean;
  defaultAgent: AgentId | undefined;
  onboardingVersion: number | undefined;
}

export interface ConfigOptions {
  cwd?: string;
  environment?: Record<string, string | undefined>;
  home?: string;
  shotsDirectoryOverride?: string | undefined;
}

function expandHome(value: string, home: string): string {
  if (value === "~") return home;
  if (value.startsWith("~/")) return join(home, value.slice(2));
  if (value.startsWith("~")) {
    throw new CliError(`unsupported home path ${JSON.stringify(value)}; use ~ or ~/path`);
  }
  return value;
}

function absolutePath(value: string, base: string, home: string): string {
  const expanded = expandHome(value, home);
  return isAbsolute(expanded) ? resolve(expanded) : resolve(base, expanded);
}

function parseConfig(path: string): ConfigFile {
  let value: unknown;
  try {
    value = readBoundedJson<unknown>(path, 65_536, "TOHSENO configuration");
  } catch (error) {
    throw new CliError(`cannot read ${path}: ${errorMessage(error)}`);
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new CliError(`${path} must contain a JSON object`);
  }
  const record = value as Record<string, unknown>;
  const allowed = new Set([
    "schemaVersion",
    "shotsDirectory",
    "defaultAgent",
    "onboardingVersion",
  ]);
  const unknown = Object.keys(record).filter((key) => !allowed.has(key));
  if (unknown.length > 0) {
    throw new CliError(`${path} contains unsupported field ${JSON.stringify(unknown[0])}`);
  }
  if (record.schemaVersion !== CONFIG_SCHEMA_VERSION) {
    throw new CliError(`${path} schemaVersion must be ${CONFIG_SCHEMA_VERSION}`);
  }
  if (record.shotsDirectory !== undefined && typeof record.shotsDirectory !== "string") {
    throw new CliError(`${path} shotsDirectory must be a string`);
  }
  if (
    record.defaultAgent !== undefined &&
    (typeof record.defaultAgent !== "string" || !isAgentId(record.defaultAgent))
  ) {
    throw new CliError(`${path} defaultAgent must be codex or claude`);
  }
  if (
    record.onboardingVersion !== undefined &&
    (!Number.isSafeInteger(record.onboardingVersion) ||
      (record.onboardingVersion as number) < 0)
  ) {
    throw new CliError(`${path} onboardingVersion must be a non-negative integer`);
  }
  return record as unknown as ConfigFile;
}

export function resolveConfig(options: ConfigOptions = {}): ResolvedConfig {
  const environment = options.environment ?? process.env;
  const cwd = options.cwd ?? process.cwd();
  const home = resolve(options.home ?? environment.HOME ?? homedir());
  const configuredFactoryHome = environment.TOHSENO_HOME ?? join(home, ".tohseno");
  const factoryHome = absolutePath(configuredFactoryHome, cwd, home);
  const configPath = join(factoryHome, "config.json");
  const configExists = lstatSync(configPath, {
    throwIfNoEntry: false,
  }) !== undefined;
  const config = configExists ? parseConfig(configPath) : undefined;

  let shotsDirectory: string;
  if (options.shotsDirectoryOverride !== undefined) {
    shotsDirectory = absolutePath(options.shotsDirectoryOverride, cwd, home);
  } else if (environment.TOHSENO_SHOTS_DIR !== undefined) {
    shotsDirectory = absolutePath(environment.TOHSENO_SHOTS_DIR, cwd, home);
  } else if (config?.shotsDirectory !== undefined) {
    shotsDirectory = absolutePath(config.shotsDirectory, dirname(configPath), home);
  } else {
    shotsDirectory = join(home, "tohseno", "shots");
  }

  return {
    factoryHome,
    cacheDirectory: join(factoryHome, "cache", "releases"),
    configPath,
    shotsDirectory,
    configExists,
    defaultAgent: config?.defaultAgent,
    onboardingVersion: config?.onboardingVersion,
  };
}

export function writeOnboardingVersion(
  config: ResolvedConfig,
  version: number,
): void {
  if (!Number.isSafeInteger(version) || version < 1) {
    throw new CliError("onboarding version must be a positive integer");
  }
  mkdirSync(config.factoryHome, { recursive: true, mode: 0o700 });
  const existing = config.configExists
    ? parseConfig(config.configPath)
    : {
        schemaVersion: CONFIG_SCHEMA_VERSION,
        shotsDirectory: config.shotsDirectory,
      };
  const temporary = `${config.configPath}.writing-${process.pid}`;
  try {
    writeFileSync(
      temporary,
      `${JSON.stringify({ ...existing, onboardingVersion: version }, null, 2)}\n`,
      { flag: "wx", mode: 0o600 },
    );
    renameSync(temporary, config.configPath);
  } finally {
    if (existsSync(temporary)) {
      unlinkSync(temporary);
    }
  }
}
