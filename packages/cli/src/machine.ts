import { existsSync, lstatSync } from "node:fs";
import { dirname, isAbsolute, join, resolve, sep } from "node:path";
import type { CommandContext } from "./commands.ts";
import { CliError, errorMessage, MACHINE_EXIT } from "./errors.ts";
import { bunExecutable, runCaptured, runInherited, sanitizedRuntimeEnvironment } from "./process.ts";
import { readShotMetadata } from "./shot.ts";
import { validateShotSlug } from "./slug.ts";

interface GlobalMachineArguments {
  shotValue?: string;
  localArguments: string[];
  json: boolean;
}

function parseGlobalArguments(arguments_: readonly string[]): GlobalMachineArguments {
  let shotValue: string | undefined;
  const localArguments: string[] = [];
  let json = false;
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index]!;
    if (argument === "--shot") {
      const value = arguments_[index + 1];
      if (value === undefined || value.startsWith("--")) throw new CliError("--shot requires a path or slug", 2);
      if (shotValue !== undefined) throw new CliError("--shot may be provided only once", 2);
      shotValue = value;
      index += 1;
    } else if (argument === "--json") {
      json = true;
    } else {
      localArguments.push(argument);
    }
  }
  if (json) localArguments.push("--json");
  return shotValue === undefined ? { localArguments, json } : { shotValue, localArguments, json };
}

function nearestShot(start: string): string | null {
  let candidate = resolve(start);
  while (true) {
    if (readShotMetadata(candidate) !== undefined) return candidate;
    const parent = dirname(candidate);
    if (parent === candidate) return null;
    candidate = parent;
  }
}

function resolveExplicitShot(value: string, context: CommandContext): string {
  const looksLikePath = isAbsolute(value) || value.startsWith(".") || value.includes(sep) || value.includes("/");
  return looksLikePath
    ? resolve(context.cwd, value)
    : join(context.config.shotsDirectory, validateShotSlug(value));
}

function requireShot(parsed: GlobalMachineArguments, context: CommandContext): string {
  const path = parsed.shotValue === undefined
    ? nearestShot(context.cwd)
    : resolveExplicitShot(parsed.shotValue, context);
  if (path === null) {
    throw new CliError("machine operations require the current shot or an explicit --shot <path-or-slug>", 2);
  }
  if (!existsSync(path) || !lstatSync(path).isDirectory()) throw new CliError(`shot does not exist: ${path}`, 2);
  if (readShotMetadata(path) === undefined) throw new CliError(`not a recognized shot: ${path}`, 2);
  return path;
}

function operationName(arguments_: readonly string[]): string {
  const values = arguments_.filter((argument) => argument !== "--json");
  if ((values[0] === "dev" || values[0] === "ios" || values[0] === "production") && values[1]) {
    return `${values[0]}.${values[1]}`;
  }
  return values[0] ?? "unknown";
}

function jsonFailure(
  operation: string,
  shot: string | null,
  code: "INVALID_CONFIGURATION" | "MISSING_DEPENDENCY" | "UNHEALTHY_SERVICES" | "INTERNAL_FAILURE",
  message: string,
): string {
  return JSON.stringify({
    schemaVersion: 1,
    ok: false,
    operation,
    shot,
    error: { code, message },
  });
}

async function legacyVerify(
  root: string,
  operation: string,
  json: boolean,
  context: CommandContext,
): Promise<number> {
  const verifier = join(root, ".tohseno", "verify.ts");
  if (!existsSync(verifier)) {
    throw new CliError(`shot is missing its pinned verifier: ${verifier}`, 2);
  }
  const verifierDetails = lstatSync(verifier);
  if (verifierDetails.isSymbolicLink() || !verifierDetails.isFile()) {
    throw new CliError(`shot-local verifier is not a regular file: ${verifier}`, 2);
  }
  const environment = sanitizedRuntimeEnvironment(context.environment);
  const result = await runCaptured([bunExecutable(context.environment), verifier], { cwd: root, env: environment });
  for (const line of [result.stdout.trim(), result.stderr.trim()].filter(Boolean)) context.io.error(line);
  if (json) {
    if (result.exitCode === 0) {
      context.io.out(JSON.stringify({
        schemaVersion: 1,
        ok: true,
        operation,
        shot: root,
        result: { valid: true, verifier, compatibility: "legacy-shot" },
      }));
    } else {
      context.io.out(jsonFailure(operation, root, "INVALID_CONFIGURATION", `shot verification failed with status ${result.exitCode}`));
    }
  }
  return result.exitCode === 0 ? 0 : MACHINE_EXIT.invalidConfiguration;
}

export async function machineCommand(arguments_: readonly string[], context: CommandContext): Promise<number> {
  const parsed = parseGlobalArguments(arguments_);
  const operation = operationName(parsed.localArguments);
  let root: string | null = null;
  try {
    root = requireShot(parsed, context);
    const machine = join(root, ".tohseno", "machine.ts");
    if (!existsSync(machine)) {
      const localArguments = parsed.localArguments.filter((argument) => argument !== "--json");
      if (localArguments.length === 1 && localArguments[0] === "verify") {
        return await legacyVerify(root, operation, parsed.json, context);
      }
      throw new CliError(
        "this legacy shot has no pinned machine runtime; its existing create/list/open/doctor/verify behavior remains supported",
        2,
      );
    }
    const machineDetails = lstatSync(machine);
    if (machineDetails.isSymbolicLink() || !machineDetails.isFile()) {
      throw new CliError(`shot-local machine runtime is not a regular file: ${machine}`, 2);
    }
    const command = [bunExecutable(context.environment), machine, ...parsed.localArguments];
    const environment = sanitizedRuntimeEnvironment(context.environment);
    if (!parsed.json) {
      return await runInherited(command, { cwd: root, env: environment });
    }
    const result = await runCaptured(command, { cwd: root, env: environment });
    if (result.stderr.trim()) context.io.error(result.stderr.trim());
    let output: unknown;
    try {
      output = JSON.parse(result.stdout) as unknown;
    } catch {
      context.io.out(jsonFailure(operation, root, "INTERNAL_FAILURE", "shot-local machine operation emitted invalid JSON"));
      return MACHINE_EXIT.internalFailure;
    }
    context.io.out(JSON.stringify(output));
    return result.exitCode;
  } catch (error) {
    const exitCode = error instanceof CliError ? error.exitCode : MACHINE_EXIT.internalFailure;
    if (parsed.json) {
      const code = exitCode === MACHINE_EXIT.invalidConfiguration
        ? "INVALID_CONFIGURATION"
        : exitCode === MACHINE_EXIT.missingDependency
          ? "MISSING_DEPENDENCY"
          : exitCode === MACHINE_EXIT.unhealthyServices
            ? "UNHEALTHY_SERVICES"
            : "INTERNAL_FAILURE";
      context.io.out(jsonFailure(operation, root, code, errorMessage(error)));
    } else {
      context.io.error(`tohseno: ${errorMessage(error)}`);
    }
    return exitCode;
  }
}
