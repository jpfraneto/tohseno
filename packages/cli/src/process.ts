import type { SpawnOptions } from "bun";

export interface CommandResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export function bunExecutable(
  environment: Record<string, string | undefined> = process.env,
): string {
  return environment.TOHSENO_BUN || process.execPath;
}

/** Minimal environment for deterministic shot-local machinery. */
export function sanitizedRuntimeEnvironment(
  environment: Record<string, string | undefined> = process.env,
): Record<string, string | undefined> {
  const exact = new Set([
    "PATH", "HOME", "SHELL", "USER", "LOGNAME", "TMPDIR", "TMP", "TEMP",
    "LANG", "TOHSENO_BUN",
    // Bankr auth for token ops; forwarded only to the spawned bankr process
    // by runtime/token.ts, never to other shot children.
    "BANKR_API_KEY",
  ]);
  const result: Record<string, string | undefined> = {};
  for (const [key, value] of Object.entries(environment)) {
    if (exact.has(key) || key.startsWith("LC_")) result[key] = value;
  }
  return result;
}

export function sanitizedGitEnvironment(
  environment: Record<string, string | undefined> = process.env,
): Record<string, string | undefined> {
  const sanitized = { ...environment };
  const exact = new Set([
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_QUARANTINE_PATH",
    "GIT_PREFIX",
    "GIT_INTERNAL_SUPER_PREFIX",
    "GIT_TEMPLATE_DIR",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
  ]);
  for (const key of Object.keys(sanitized)) {
    if (
      exact.has(key) ||
      key.startsWith("GIT_AUTHOR_") ||
      key.startsWith("GIT_COMMITTER_") ||
      key.startsWith("GIT_CONFIG_KEY_") ||
      key.startsWith("GIT_CONFIG_VALUE_")
    ) {
      delete sanitized[key];
    }
  }
  return sanitized;
}

export async function runCaptured(
  command: readonly string[],
  options: { cwd?: string; env?: Record<string, string | undefined> } = {},
): Promise<CommandResult> {
  const spawnOptions: SpawnOptions.OptionsObject<"ignore", "pipe", "pipe"> = {
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  };
  if (options.cwd !== undefined) spawnOptions.cwd = options.cwd;
  if (command[0] === "git") spawnOptions.env = sanitizedGitEnvironment(options.env);
  else if (options.env !== undefined) spawnOptions.env = options.env;
  const process = Bun.spawn([...command], spawnOptions);
  const [exitCode, stdout, stderr] = await Promise.all([
    process.exited,
    new Response(process.stdout).text(),
    new Response(process.stderr).text(),
  ]);
  return { exitCode, stdout, stderr };
}

export async function runInherited(
  command: readonly string[],
  options: { cwd: string; env?: Record<string, string | undefined> },
): Promise<number> {
  const spawnOptions: SpawnOptions.OptionsObject<"inherit", "inherit", "inherit"> = {
    cwd: options.cwd,
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  };
  if (options.env !== undefined) spawnOptions.env = options.env;
  return Bun.spawn([...command], spawnOptions).exited;
}
