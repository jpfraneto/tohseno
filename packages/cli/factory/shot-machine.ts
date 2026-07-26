#!/usr/bin/env bun
import { realpathSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { inspectIos, launchIos } from "./runtime/ios.ts";
import {
  errorExitCode,
  failure,
  MachineError,
  publicErrorMessage,
  readCanonicalShotMetadata,
  requireRegularFile,
  runCaptured,
  safeEnvironment,
  shotRoot,
  success,
} from "./runtime/shared.ts";

interface Parsed {
  values: Map<string, string>;
  positionals: string[];
}

const MACHINE_PATH = realpathSync(fileURLToPath(import.meta.url));
const TRUSTED_LOCAL = dirname(MACHINE_PATH);

function parse(
  arguments_: readonly string[],
  valueOptions: readonly string[],
): Parsed {
  const values = new Map<string, string>();
  const positionals: string[] = [];
  const allowedValues = new Set(valueOptions);
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index]!;
    if (allowedValues.has(argument)) {
      const value = arguments_[index + 1];
      if (value === undefined || value.startsWith("--")) {
        throw new MachineError(
          "INVALID_CONFIGURATION",
          `${argument} requires a value`,
        );
      }
      if (values.has(argument)) {
        throw new MachineError(
          "INVALID_CONFIGURATION",
          `${argument} may be provided only once`,
        );
      }
      values.set(argument, value);
      index += 1;
    } else if (argument.startsWith("--")) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        `unknown option ${argument}`,
      );
    } else {
      positionals.push(argument);
    }
  }
  return { values, positionals };
}

function operationName(arguments_: readonly string[]): string {
  if (arguments_[0] === "ios" && arguments_[1]) {
    return `ios.${arguments_[1]}`;
  }
  return arguments_[0] ?? "unknown";
}

function operationInventory(): unknown {
  return {
    protocolVersion: 1,
    commands: [
      { operation: "operations", mutation: false },
      { operation: "ios.inspect", mutation: false },
      {
        operation: "ios.launch",
        mutation: true,
        options: ["--device"],
      },
      { operation: "verify", mutation: false },
    ],
    exitCodes: {
      success: 0,
      invalidConfiguration: 2,
      missingDependency: 3,
      unhealthyServices: 4,
      internalFailure: 5,
    },
    json: {
      stdout: "exactly one JSON document",
      diagnostics: "stderr",
    },
  };
}

async function verify(root: string, json: boolean): Promise<unknown> {
  const verifier = join(TRUSTED_LOCAL, "verify.ts");
  requireRegularFile(verifier, "Shot-local verifier");
  const result = await runCaptured(
    [process.execPath, verifier],
    { cwd: root, environment: safeEnvironment() },
  );
  const diagnostics = [result.stdout.trim(), result.stderr.trim()]
    .filter(Boolean)
    .join("\n");
  if (diagnostics) {
    if (json) console.error(diagnostics);
    else console.log(diagnostics);
  }
  if (result.exitCode !== 0) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `Shot verification failed with status ${result.exitCode}`,
      { verifier, exitCode: result.exitCode },
    );
  }
  return { valid: true, verifier, exitCode: 0 };
}

async function dispatch(
  arguments_: readonly string[],
  root: string,
  json: boolean,
): Promise<unknown> {
  readCanonicalShotMetadata(root);
  const first = arguments_[0];
  if (first === "operations") {
    const parsed = parse(arguments_.slice(1), []);
    if (parsed.positionals.length > 0) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        "operations accepts no arguments",
      );
    }
    return operationInventory();
  }
  if (first === "verify") {
    const parsed = parse(arguments_.slice(1), []);
    if (parsed.positionals.length > 0) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        "verify accepts no arguments",
      );
    }
    return await verify(root, json);
  }
  if (first === "ios") {
    const action = arguments_[1];
    const rest = arguments_.slice(2);
    if (action === "inspect") {
      const parsed = parse(rest, []);
      if (parsed.positionals.length > 0) {
        throw new MachineError(
          "INVALID_CONFIGURATION",
          "ios inspect accepts no arguments",
        );
      }
      return await inspectIos(root);
    }
    if (action === "launch") {
      const parsed = parse(rest, ["--device"]);
      if (parsed.positionals.length > 0) {
        throw new MachineError(
          "INVALID_CONFIGURATION",
          "ios launch accepts no positional arguments",
        );
      }
      return await launchIos(root, parsed.values.get("--device"));
    }
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "ios operation must be inspect or launch",
    );
  }
  throw new MachineError(
    "INVALID_CONFIGURATION",
    "operation must be operations, ios inspect|launch, or verify",
  );
}

export async function main(arguments_: readonly string[]): Promise<number> {
  const json = arguments_.includes("--json");
  const operationArguments = arguments_.filter(
    (argument) => argument !== "--json",
  );
  const operation = operationName(operationArguments);
  let root: string | null = null;
  try {
    root = shotRoot();
    const result = await dispatch(operationArguments, root, json);
    const output = success(operation, root, result);
    console.log(json ? JSON.stringify(output) : JSON.stringify(output, null, 2));
    return 0;
  } catch (error) {
    const output = failure(operation, root, error);
    if (json) console.log(JSON.stringify(output));
    else console.error(`tohseno machine: ${publicErrorMessage(error)}`);
    return errorExitCode(error);
  }
}

if (import.meta.main) {
  process.exitCode = await main(Bun.argv.slice(2));
}
