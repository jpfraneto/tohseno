import { CLI_VERSION } from "./constants.ts";
import {
  adoptCommand,
  createCommand,
  doctorCommand,
  continueCommand,
  listCommand,
  openCommand,
  previewCommand,
  runCommand,
  studioCommand,
  verifyCommand,
} from "./commands.ts";
import { resolveConfig } from "./config.ts";
import { CliError, errorMessage } from "./errors.ts";
import { defaultIo, type CliIo } from "./io.ts";
import { interactiveLauncher } from "./launcher.ts";
import { machineCommand } from "./machine.ts";
import { validateShotSlug } from "./slug.ts";

const HELP = `TOHSENO ${CLI_VERSION} — agent-first app factory
Take another one.

Usage:
  tohseno
  tohseno <shot-slug>
  tohseno studio [--port 4747] [--no-open] [--shots-dir <path>]

Agent/automation operations:
  tohseno machine operations --json [--shot <path-or-slug>]
  tohseno machine dev start|status|logs|stop --json [--shot <path-or-slug>]
  tohseno machine ios inspect|launch --json [--shot <path-or-slug>]
  tohseno machine verify --json [--shot <path-or-slug>]
  tohseno machine production inspect --json [--shot <path-or-slug>]

Advanced compatibility commands:
  tohseno create <slug> [--platform ios] [--agent codex|claude] [--no-launch]
  tohseno create --file <intention.md> [--reference <image> ...]
  tohseno list [--shots-dir <path>]
  tohseno open <slug> [--shots-dir <path>]
  tohseno doctor [--shots-dir <path>]
  tohseno verify [slug-or-path] [--shots-dir <path>]
  tohseno adopt <path> [--yes] [--no-interactive]
  tohseno run <slug-or-path> [--shots-dir <path>]
  tohseno preview <slug-or-path> [--shots-dir <path>]

Create options:
  --platform ios       iOS is the only implemented platform
  --agent <agent>      codex or claude; both must already be installed
  --file <path>        use a UTF-8 Markdown file as creation input
  --reference <path>   attach image context to --file; repeat up to eight times
  --shots-dir <path>   override config/default (default: ~/tohseno/shots)
  --no-launch          create and verify without launching the selected agent
  --no-interactive     never prompt; required selections must be flags

Studio options:
  --port <number>       local Studio port (default: 4747)
  --no-open             start Studio without opening the browser
  --shots-dir <path>    override config/default (default: ~/tohseno/shots)

Run TOHSENO with no arguments, choose create or continue, and tell the launched coding agent what you want.
Open prints the absolute shot path; for example: cd "$(tohseno open my-shot)"
Config: ~/.tohseno/config.json (override factory home with TOHSENO_HOME).`;

interface ParsedOptions {
  positionals: string[];
  values: Map<string, string>;
  multiValues: Map<string, string[]>;
  flags: Set<string>;
}

export interface CliMainOptions {
  cwd?: string;
  environment?: Record<string, string | undefined>;
  io?: CliIo;
  sourceRoot?: string;
}

function parseOptions(
  arguments_: readonly string[],
  valueOptions: readonly string[],
  flagOptions: readonly string[],
  multiValueOptions: readonly string[] = [],
): ParsedOptions {
  const values = new Map<string, string>();
  const multiValues = new Map<string, string[]>();
  const flags = new Set<string>();
  const positionals: string[] = [];
  const valuesAllowed = new Set(valueOptions);
  const multiValuesAllowed = new Set(multiValueOptions);
  const flagsAllowed = new Set(flagOptions);
  let positionalOnly = false;
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index]!;
    if (!positionalOnly && argument === "--") {
      positionalOnly = true;
    } else if (!positionalOnly && argument.startsWith("--")) {
      if (valuesAllowed.has(argument) || multiValuesAllowed.has(argument)) {
        const value = arguments_[index + 1];
        if (value === undefined || value.startsWith("--")) throw new CliError(`${argument} requires a value`, 2);
        if (multiValuesAllowed.has(argument)) {
          const collected = multiValues.get(argument) ?? [];
          collected.push(value);
          multiValues.set(argument, collected);
        } else {
          if (values.has(argument)) throw new CliError(`${argument} may be provided only once`, 2);
          values.set(argument, value);
        }
        index += 1;
      } else if (flagsAllowed.has(argument)) {
        flags.add(argument);
      } else {
        throw new CliError(`unknown option ${argument}`, 2);
      }
    } else {
      positionals.push(argument);
    }
  }
  return { positionals, values, multiValues, flags };
}

function onePositional(parsed: ParsedOptions, name: string, optional = false): string | undefined {
  if (parsed.positionals.length > 1) throw new CliError(`expected one ${name}`, 2);
  const value = parsed.positionals[0];
  if (!optional && value === undefined) throw new CliError(`${name} is required`, 2);
  return value;
}

function portOption(value: string | undefined): number {
  if (value === undefined) return 4747;
  if (!/^[0-9]{1,5}$/u.test(value)) {
    throw new CliError("--port must be an integer from 1 to 65535", 2);
  }
  const port = Number(value);
  if (port < 1 || port > 65_535) {
    throw new CliError("--port must be an integer from 1 to 65535", 2);
  }
  return port;
}

function extractValueOption(
  arguments_: readonly string[],
  option: string,
): { value?: string; remaining: string[] } {
  let value: string | undefined;
  const remaining: string[] = [];
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index]!;
    if (argument !== option) {
      remaining.push(argument);
      continue;
    }
    const candidate = arguments_[index + 1];
    if (candidate === undefined || candidate.startsWith("--")) throw new CliError(`${option} requires a value`, 2);
    if (value !== undefined) throw new CliError(`${option} may be provided only once`, 2);
    value = candidate;
    index += 1;
  }
  return value === undefined ? { remaining } : { value, remaining };
}

export async function main(arguments_: readonly string[], options: CliMainOptions = {}): Promise<number> {
  const io = options.io ?? defaultIo();
  const environment = options.environment ?? process.env;
  const cwd = options.cwd ?? process.cwd();
  try {
    const command = arguments_[0];
    if (command === undefined) {
      const config = resolveConfig({ cwd, environment });
      return await interactiveLauncher({ config, cwd, environment, io, sourceRoot: options.sourceRoot });
    }
    if (command === "--help" || command === "-h" || command === "help") {
      io.out(HELP);
      return 0;
    }
    if (command === "--version" || command === "-v") {
      io.out(CLI_VERSION);
      return 0;
    }

    const rest = arguments_.slice(1);
    if (rest.includes("--help") || rest.includes("-h")) {
      io.out(HELP);
      return 0;
    }
    if (command === "create") {
      const parsed = parseOptions(
        rest,
        ["--platform", "--agent", "--shots-dir", "--file"],
        ["--no-launch", "--no-interactive"],
        ["--reference"],
      );
      const slug = onePositional(parsed, "shot slug", true);
      const file = parsed.values.get("--file");
      const references = parsed.multiValues.get("--reference") ?? [];
      if (slug === undefined && file === undefined) {
        if (references.length > 0) {
          throw new CliError(
            "--reference cannot supply an intention; add --file <intention.md> when creating without a shot slug",
            2,
          );
        }
        throw new CliError("shot slug is required unless --file supplies creation input", 2);
      }
      const config = resolveConfig({ cwd, environment, shotsDirectoryOverride: parsed.values.get("--shots-dir") });
      return await createCommand({
        ...(slug === undefined ? {} : { slug }),
        platform: parsed.values.get("--platform"),
        agent: parsed.values.get("--agent"),
        file,
        references,
        noLaunch: parsed.flags.has("--no-launch"),
        noInteractive: parsed.flags.has("--no-interactive"),
      }, { config, cwd, environment, io, sourceRoot: options.sourceRoot });
    }
    if (command === "machine") {
      const extracted = extractValueOption(rest, "--shots-dir");
      const config = resolveConfig({ cwd, environment, shotsDirectoryOverride: extracted.value });
      return await machineCommand(extracted.remaining, { config, cwd, environment, io, sourceRoot: options.sourceRoot });
    }
    if (command === "studio") {
      const parsed = parseOptions(
        rest,
        ["--port", "--shots-dir"],
        ["--no-open"],
      );
      if (parsed.positionals.length > 0) {
        throw new CliError("studio accepts no positional arguments", 2);
      }
      const config = resolveConfig({
        cwd,
        environment,
        shotsDirectoryOverride: parsed.values.get("--shots-dir"),
      });
      return await studioCommand({
        port: portOption(parsed.values.get("--port")),
        noOpen: parsed.flags.has("--no-open"),
      }, {
        config,
        cwd,
        environment,
        io,
        sourceRoot: options.sourceRoot,
      });
    }
    if (command === "list" || command === "doctor") {
      const parsed = parseOptions(rest, ["--shots-dir"], []);
      if (parsed.positionals.length > 0) throw new CliError(`${command} accepts no positional arguments`, 2);
      const config = resolveConfig({ cwd, environment, shotsDirectoryOverride: parsed.values.get("--shots-dir") });
      const context = { config, cwd, environment, io, sourceRoot: options.sourceRoot };
      return command === "list" ? listCommand(context) : await doctorCommand(context);
    }
    if (command === "open") {
      const parsed = parseOptions(rest, ["--shots-dir"], []);
      const slug = onePositional(parsed, "shot slug") ?? "";
      const config = resolveConfig({ cwd, environment, shotsDirectoryOverride: parsed.values.get("--shots-dir") });
      return openCommand(slug, { config, cwd, environment, io, sourceRoot: options.sourceRoot });
    }
    if (command === "verify") {
      const parsed = parseOptions(rest, ["--shots-dir"], []);
      const value = onePositional(parsed, "shot slug or path", true);
      const config = resolveConfig({ cwd, environment, shotsDirectoryOverride: parsed.values.get("--shots-dir") });
      return await verifyCommand(value, { config, cwd, environment, io, sourceRoot: options.sourceRoot });
    }
    if (command === "run" || command === "preview") {
      const parsed = parseOptions(rest, ["--shots-dir"], []);
      const value = onePositional(parsed, "shot slug or path") ?? "";
      const config = resolveConfig({
        cwd,
        environment,
        shotsDirectoryOverride: parsed.values.get("--shots-dir"),
      });
      const context = {
        config,
        cwd,
        environment,
        io,
        sourceRoot: options.sourceRoot,
      };
      return command === "run"
        ? await runCommand(value, context)
        : await previewCommand(value, context);
    }
    if (command === "adopt") {
      const parsed = parseOptions(rest, ["--shots-dir"], ["--yes", "--no-interactive"]);
      const path = onePositional(parsed, "project path") ?? "";
      const config = resolveConfig({ cwd, environment, shotsDirectoryOverride: parsed.values.get("--shots-dir") });
      return await adoptCommand(path, {
        yes: parsed.flags.has("--yes"),
        noInteractive: parsed.flags.has("--no-interactive"),
      }, { config, cwd, environment, io, sourceRoot: options.sourceRoot });
    }
    if (/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(command)) {
      const parsed = parseOptions(rest, ["--agent", "--shots-dir"], ["--no-interactive"]);
      if (parsed.positionals.length > 0) throw new CliError("shot shortcut accepts no positional arguments", 2);
      const slug = validateShotSlug(command);
      const config = resolveConfig({ cwd, environment, shotsDirectoryOverride: parsed.values.get("--shots-dir") });
      const agent = parsed.values.get("--agent");
      return await continueCommand(slug, {
        ...(agent === undefined ? {} : { agent }),
        noInteractive: parsed.flags.has("--no-interactive"),
      }, { config, cwd, environment, io, sourceRoot: options.sourceRoot });
    }
    throw new CliError(`unknown command ${JSON.stringify(command)}; run tohseno --help`, 2);
  } catch (error) {
    const exitCode = error instanceof CliError ? error.exitCode : 1;
    io.error(`tohseno: ${errorMessage(error)}`);
    return exitCode;
  }
}
