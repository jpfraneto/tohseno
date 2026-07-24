import { accessSync, constants as fsConstants, statSync } from "node:fs";
import { delimiter, isAbsolute, join, resolve } from "node:path";
import { AGENT_INSTRUCTION } from "./constants.ts";
import { CliError } from "./errors.ts";

export type AgentId = "codex" | "claude";

export interface AgentAdapter {
  id: AgentId;
  label: string;
  binary: string;
  executable: string;
  launchArguments: readonly string[];
}

const SUPPORTED_AGENTS: ReadonlyArray<Omit<AgentAdapter, "executable">> = [
  {
    id: "codex",
    label: "Codex",
    binary: "codex",
    launchArguments: [AGENT_INSTRUCTION],
  },
  {
    id: "claude",
    label: "Claude Code",
    binary: "claude",
    launchArguments: [AGENT_INSTRUCTION],
  },
];

function executableOnPath(binary: string, pathValue: string, cwd: string): string | undefined {
  const candidates = binary.includes("/")
    ? [isAbsolute(binary) ? binary : resolve(cwd, binary)]
    : pathValue.split(delimiter).filter(Boolean).map((directory) => join(directory, binary));
  for (const candidate of candidates) {
    try {
      accessSync(candidate, fsConstants.X_OK);
      if (statSync(candidate).isFile()) return resolve(candidate);
    } catch {
      // Continue to the next PATH entry.
    }
  }
  return undefined;
}

export function detectInstalledAgents(
  pathValue = process.env.PATH ?? "",
  cwd = process.cwd(),
): AgentAdapter[] {
  const found: AgentAdapter[] = [];
  for (const adapter of SUPPORTED_AGENTS) {
    const executable = executableOnPath(adapter.binary, pathValue, cwd);
    if (executable !== undefined) found.push({ ...adapter, executable });
  }
  return found;
}

export function isAgentId(value: string): value is AgentId {
  return value === "codex" || value === "claude";
}

export function requireInstalledAgent(
  id: string,
  installed: readonly AgentAdapter[],
): AgentAdapter {
  if (!isAgentId(id)) {
    throw new CliError(`unsupported coding agent ${JSON.stringify(id)}; supported agents are codex and claude`, 2);
  }
  const agent = installed.find((candidate) => candidate.id === id);
  if (agent === undefined) {
    throw new CliError(`${id} is not installed or is not executable on PATH`, 3);
  }
  return agent;
}

/**
 * Agent subprocesses receive terminal and user-location context, but not the
 * caller's arbitrary environment. In particular, provider keys and unrelated
 * application secrets are never forwarded merely because `tohseno` inherited
 * them.
 */
export function sanitizedAgentEnvironment(
  environment: Record<string, string | undefined> = process.env,
): Record<string, string | undefined> {
  const exact = new Set([
    "PATH",
    "HOME",
    "SHELL",
    "USER",
    "LOGNAME",
    "TMPDIR",
    "TMP",
    "TEMP",
    "TERM",
    "COLORTERM",
    "LANG",
    "CODEX_HOME",
    "CLAUDE_CONFIG_DIR",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "XDG_DATA_HOME",
  ]);
  const result: Record<string, string | undefined> = {};
  for (const [key, value] of Object.entries(environment)) {
    if (exact.has(key) || key.startsWith("LC_")) result[key] = value;
  }
  return result;
}
