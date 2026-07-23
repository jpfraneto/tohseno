#!/usr/bin/env bun
import { join } from "node:path";
import {
  developmentLogs,
  developmentStatus,
  runSupervisor,
  startDevelopment,
  stopDevelopment,
} from "./runtime/dev.ts";
import { inspectIos, launchIos } from "./runtime/ios.ts";
import { inspectProduction } from "./runtime/production.ts";
import { launchToken, tokenFees, tokenStatus } from "./runtime/token.ts";
import {
  errorExitCode,
  failure,
  MachineError,
  publicErrorMessage,
  requireRegularFile,
  runCaptured,
  safeEnvironment,
  shotRoot,
  success,
} from "./runtime/shared.ts";

interface Parsed {
  flags: Set<string>;
  values: Map<string, string>;
  positionals: string[];
}

function parse(
  arguments_: readonly string[],
  valueOptions: readonly string[],
  flagOptions: readonly string[],
): Parsed {
  const values = new Map<string, string>();
  const flags = new Set<string>();
  const positionals: string[] = [];
  const allowedValues = new Set(valueOptions);
  const allowedFlags = new Set(flagOptions);
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index]!;
    if (allowedValues.has(argument)) {
      const value = arguments_[index + 1];
      if (value === undefined || value.startsWith("--")) {
        throw new MachineError("INVALID_CONFIGURATION", `${argument} requires a value`);
      }
      if (values.has(argument)) {
        throw new MachineError("INVALID_CONFIGURATION", `${argument} may be provided only once`);
      }
      values.set(argument, value);
      index += 1;
    } else if (allowedFlags.has(argument)) {
      flags.add(argument);
    } else if (argument.startsWith("--")) {
      throw new MachineError("INVALID_CONFIGURATION", `unknown option ${argument}`);
    } else {
      positionals.push(argument);
    }
  }
  return { flags, values, positionals };
}

function integer(value: string | undefined, option: string): number | undefined {
  if (value === undefined) return undefined;
  if (!/^\d+$/u.test(value)) throw new MachineError("INVALID_CONFIGURATION", `${option} must be a whole number`);
  return Number(value);
}

function operationName(arguments_: readonly string[]): string {
  if (arguments_[0] === "dev" && arguments_[1]) return `dev.${arguments_[1]}`;
  if (arguments_[0] === "production" && arguments_[1]) return `production.${arguments_[1]}`;
  if (arguments_[0] === "ios" && arguments_[1]) return `ios.${arguments_[1]}`;
  if (arguments_[0] === "token" && arguments_[1]) return `token.${arguments_[1]}`;
  return arguments_[0] ?? "unknown";
}

function operationInventory(): unknown {
  return {
    protocolVersion: 1,
    commands: [
      { operation: "operations", mutation: false },
      { operation: "dev.start", mutation: true, idempotent: true, options: ["--tunnel", "--port", "--readiness-timeout-ms"] },
      { operation: "dev.status", mutation: false },
      { operation: "dev.logs", mutation: false, options: ["--service", "--lines"] },
      { operation: "dev.stop", mutation: true, idempotent: true },
      { operation: "ios.inspect", mutation: false },
      { operation: "ios.launch", mutation: true, options: ["--device"] },
      { operation: "token.status", mutation: false },
      { operation: "token.launch", mutation: true, options: ["--name", "--symbol", "--chain", "--image", "--website", "--fee-recipient", "--fee-type", "--yes"] },
      { operation: "token.fees", mutation: false },
      { operation: "verify", mutation: false },
      { operation: "production.inspect", mutation: false },
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
  const verifier = join(root, ".tohseno", "verify.ts");
  requireRegularFile(verifier, "shot-local verifier");
  const result = await runCaptured([process.execPath, verifier], { cwd: root, environment: safeEnvironment() });
  const diagnostics = [result.stdout.trim(), result.stderr.trim()].filter(Boolean).join("\n");
  if (diagnostics) {
    if (json) console.error(diagnostics);
    else console.log(diagnostics);
  }
  if (result.exitCode !== 0) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      `shot verification failed with status ${result.exitCode}`,
      { verifier, exitCode: result.exitCode },
    );
  }
  return { valid: true, verifier, exitCode: 0 };
}

async function dispatch(arguments_: readonly string[], root: string, json: boolean): Promise<unknown> {
  const first = arguments_[0];
  if (first === "operations") {
    const parsed = parse(arguments_.slice(1), [], []);
    if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "operations accepts no arguments");
    return operationInventory();
  }
  if (first === "verify") {
    const parsed = parse(arguments_.slice(1), [], []);
    if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "verify accepts no arguments");
    return await verify(root, json);
  }
  if (first === "production" && arguments_[1] === "inspect") {
    const parsed = parse(arguments_.slice(2), [], []);
    if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "production inspect accepts no arguments");
    return inspectProduction(root);
  }
  if (first === "dev") {
    const action = arguments_[1];
    const rest = arguments_.slice(2);
    if (action === "start") {
      const parsed = parse(rest, ["--port", "--readiness-timeout-ms", "--cloudflared"], ["--tunnel"]);
      if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "dev start accepts no positional arguments");
      const cloudflaredPath = parsed.values.get("--cloudflared");
      const port = integer(parsed.values.get("--port"), "--port");
      const readinessTimeoutMs = integer(parsed.values.get("--readiness-timeout-ms"), "--readiness-timeout-ms");
      return await startDevelopment(root, {
        tunnel: parsed.flags.has("--tunnel") || cloudflaredPath !== undefined,
        ...(port === undefined ? {} : { port }),
        ...(readinessTimeoutMs === undefined ? {} : { readinessTimeoutMs }),
        ...(cloudflaredPath === undefined ? {} : { cloudflaredPath }),
      });
    }
    if (action === "status") {
      const parsed = parse(rest, [], []);
      if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "dev status accepts no arguments");
      const status = await developmentStatus(root);
      if (status.state === "unhealthy") {
        throw new MachineError(
          "UNHEALTHY_SERVICES",
          "one or more shot-owned development services are unhealthy",
          { status },
        );
      }
      return status;
    }
    if (action === "stop") {
      const parsed = parse(rest, [], []);
      if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "dev stop accepts no arguments");
      return await stopDevelopment(root);
    }
    if (action === "logs") {
      const parsed = parse(rest, ["--service", "--lines"], []);
      if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "dev logs accepts no positional arguments");
      const serviceValue = parsed.values.get("--service") ?? "all";
      if (!["api", "tunnel", "supervisor", "ios", "all"].includes(serviceValue)) {
        throw new MachineError("INVALID_CONFIGURATION", "--service must be api, tunnel, supervisor, ios, or all");
      }
      return developmentLogs(root, {
        service: serviceValue as "api" | "tunnel" | "supervisor" | "ios" | "all",
        lines: integer(parsed.values.get("--lines"), "--lines") ?? 100,
      });
    }
    throw new MachineError("INVALID_CONFIGURATION", "dev operation must be start, status, logs, or stop");
  }
  if (first === "ios") {
    const action = arguments_[1];
    const rest = arguments_.slice(2);
    if (action === "inspect") {
      const parsed = parse(rest, [], []);
      if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "ios inspect accepts no arguments");
      return await inspectIos(root);
    }
    if (action === "launch") {
      const parsed = parse(rest, ["--device"], []);
      if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "ios launch accepts no positional arguments");
      return await launchIos(root, parsed.values.get("--device"));
    }
    throw new MachineError("INVALID_CONFIGURATION", "ios operation must be inspect or launch");
  }
  if (first === "token") {
    const action = arguments_[1];
    const rest = arguments_.slice(2);
    if (action === "status") {
      const parsed = parse(rest, [], []);
      if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "token status accepts no arguments");
      return await tokenStatus(root);
    }
    if (action === "launch") {
      const parsed = parse(
        rest,
        ["--name", "--symbol", "--chain", "--image", "--website", "--fee-recipient", "--fee-type"],
        ["--yes"],
      );
      if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "token launch accepts no positional arguments");
      const chain = requiredValue(parsed, "--chain");
      if (chain !== "base" && chain !== "robinhood") {
        throw new MachineError("INVALID_CONFIGURATION", "--chain must be base or robinhood");
      }
      const feeType = parsed.values.get("--fee-type");
      if (feeType !== undefined && !["x", "farcaster", "ens", "wallet"].includes(feeType)) {
        throw new MachineError("INVALID_CONFIGURATION", "--fee-type must be x, farcaster, ens, or wallet");
      }
      return await launchToken(root, {
        name: requiredValue(parsed, "--name"),
        symbol: requiredValue(parsed, "--symbol"),
        chain,
        ...(parsed.values.has("--image") ? { image: parsed.values.get("--image")! } : {}),
        ...(parsed.values.has("--website") ? { website: parsed.values.get("--website")! } : {}),
        ...(parsed.values.has("--fee-recipient") ? { feeRecipient: parsed.values.get("--fee-recipient")! } : {}),
        ...(feeType === undefined ? {} : { feeType: feeType as "x" | "farcaster" | "ens" | "wallet" }),
        yes: parsed.flags.has("--yes"),
        json,
      });
    }
    if (action === "fees") {
      const parsed = parse(rest, [], []);
      if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "token fees accepts no arguments");
      return await tokenFees(root);
    }
    throw new MachineError("INVALID_CONFIGURATION", "token operation must be status, launch, or fees");
  }
  throw new MachineError(
    "INVALID_CONFIGURATION",
    "operation must be operations, dev start|status|logs|stop, ios inspect|launch, token status|launch|fees, verify, or production inspect",
  );
}

function requiredValue(parsed: Parsed, option: string): string {
  const value = parsed.values.get(option);
  if (value === undefined) throw new MachineError("INVALID_CONFIGURATION", `${option} is required`);
  return value;
}

async function supervisorMain(arguments_: readonly string[]): Promise<number> {
  const parsed = parse(
    arguments_,
    ["--instance", "--result", "--readiness-timeout-ms", "--port", "--cloudflared"],
    [],
  );
  if (parsed.positionals.length > 0) throw new MachineError("INVALID_CONFIGURATION", "invalid supervisor arguments");
  const root = shotRoot();
  const readinessTimeoutMs = integer(requiredValue(parsed, "--readiness-timeout-ms"), "--readiness-timeout-ms")!;
  const port = integer(requiredValue(parsed, "--port"), "--port")!;
  return await runSupervisor({
    root,
    machinePath: join(root, ".tohseno", "machine.ts"),
    instanceId: requiredValue(parsed, "--instance"),
    resultPath: requiredValue(parsed, "--result"),
    readinessTimeoutMs,
    port,
    ...(parsed.values.has("--cloudflared") ? { cloudflaredPath: parsed.values.get("--cloudflared")! } : {}),
  });
}

export async function main(arguments_: readonly string[]): Promise<number> {
  if (arguments_[0] === "__supervise") {
    try {
      return await supervisorMain(arguments_.slice(1));
    } catch (error) {
      console.error(JSON.stringify({ event: "supervisor_failed", errorType: error instanceof Error ? error.constructor.name : "Unknown" }));
      return errorExitCode(error);
    }
  }

  const json = arguments_.includes("--json");
  const operationArguments = arguments_.filter((argument) => argument !== "--json");
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

if (import.meta.main) process.exitCode = await main(Bun.argv.slice(2));
