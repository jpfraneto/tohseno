import type { SpawnOptions } from "bun";
import { basename } from "node:path";

const MAX_CAPTURED_OUTPUT_BYTES = 8 * 1_048_576;

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
    "LANG", "DEVELOPER_DIR", "TOHSENO_BUN",
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
  return {
    ...sanitizedRuntimeEnvironment(environment),
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_TERMINAL_PROMPT: "0",
    GIT_OPTIONAL_LOCKS: "0",
  };
}

export async function runCaptured(
  command: readonly string[],
  options: { cwd?: string; env?: Record<string, string | undefined> } = {},
): Promise<CommandResult> {
  const effectiveCommand = command[0] === "git"
    ? [
      "git",
      "-c", "core.hooksPath=/dev/null",
      "-c", "core.fsmonitor=false",
      "-c", "core.attributesFile=/dev/null",
      "-c", "core.excludesFile=/dev/null",
      ...command.slice(1),
    ]
    : [...command];
  const spawnOptions: SpawnOptions.OptionsObject<"ignore", "pipe", "pipe"> = {
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  };
  if (options.cwd !== undefined) spawnOptions.cwd = options.cwd;
  if (command[0] === "git") spawnOptions.env = sanitizedGitEnvironment(options.env);
  else if (options.env !== undefined) spawnOptions.env = options.env;
  const process = Bun.spawn(effectiveCommand, spawnOptions);
  let outputExceeded = false;
  const stopForOutputLimit = (): void => {
    if (outputExceeded) return;
    outputExceeded = true;
    try {
      process.kill("SIGKILL");
    } catch {
      // It exited between the oversized chunk and the kill request.
    }
  };
  const [exitCode, stdout, stderr] = await Promise.all([
    process.exited,
    boundedStreamText(
      process.stdout,
      MAX_CAPTURED_OUTPUT_BYTES,
      stopForOutputLimit,
    ),
    boundedStreamText(
      process.stderr,
      MAX_CAPTURED_OUTPUT_BYTES,
      stopForOutputLimit,
    ),
  ]);
  if (outputExceeded) {
    throw new Error(
      `${basename(command[0] ?? "subprocess")} output exceeded the ${MAX_CAPTURED_OUTPUT_BYTES}-byte safety limit`,
    );
  }
  return { exitCode, stdout, stderr };
}

async function boundedStreamText(
  stream: ReadableStream<Uint8Array>,
  maximumBytes: number,
  onLimit: () => void,
): Promise<string> {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    while (true) {
      const next = await reader.read();
      if (next.done) break;
      const remaining = maximumBytes - length;
      if (next.value.byteLength > remaining) {
        if (remaining > 0) chunks.push(next.value.subarray(0, remaining));
        onLimit();
        break;
      }
      chunks.push(next.value);
      length += next.value.byteLength;
    }
  } finally {
    try {
      await reader.cancel();
    } catch {
      // The process closing its pipe first is expected.
    }
  }
  return Buffer.concat(chunks.map((chunk) => Buffer.from(chunk))).toString("utf8");
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
