import { accessSync, constants as fsConstants, existsSync, lstatSync, readFileSync, readdirSync, statSync } from "node:fs";
import { delimiter, dirname, isAbsolute, join, resolve, sep } from "node:path";
import {
  detectInstalledAgents,
  requireInstalledAgent,
  sanitizedAgentEnvironment,
  type AgentId,
  type AgentAdapter,
} from "./agents.ts";
import { AGENT_INSTRUCTION } from "./constants.ts";
import type { ResolvedConfig } from "./config.ts";
import { CliError, errorMessage } from "./errors.ts";
import type { CliIo } from "./io.ts";
import { runCaptured, runInherited } from "./process.ts";
import {
  listCachedReleaseDirectories,
  prepareFactoryRelease,
  useActiveCachedRelease,
  verifyReleaseDirectory,
  type PreparedRelease,
} from "./release.ts";
import { adoptShot, createShot, readShotMetadata } from "./shot.ts";
import { locateFactorySourceRoot } from "./source.ts";
import { validateShotSlug } from "./slug.ts";

export interface CommandContext {
  config: ResolvedConfig;
  cwd: string;
  environment: Record<string, string | undefined>;
  io: CliIo;
  sourceRoot?: string | undefined;
}

export interface CreateArguments {
  slug: string;
  platform?: string | undefined;
  agent?: string | undefined;
  noLaunch: boolean;
  noInteractive: boolean;
}

function sourceRootFor(context: CommandContext): string {
  return context.sourceRoot ?? locateFactorySourceRoot(context.environment);
}

async function factoryReleaseFor(context: CommandContext): Promise<PreparedRelease> {
  let sourceRoot: string;
  try {
    sourceRoot = sourceRootFor(context);
  } catch (sourceError) {
    try {
      return useActiveCachedRelease(context.config.cacheDirectory);
    } catch (cacheError) {
      throw new CliError(
        `factory source is unavailable (${errorMessage(sourceError)}) and cached fallback failed: ${errorMessage(cacheError)}`,
      );
    }
  }
  return await prepareFactoryRelease(sourceRoot, context.config.cacheDirectory);
}

export async function chooseNumber(
  io: CliIo,
  count: number,
  label: string,
  defaultIndex?: number,
): Promise<number> {
  while (true) {
    const defaultLabel = defaultIndex === undefined ? "" : ` (default ${defaultIndex + 1})`;
    const answer = (await io.prompt(`${label} [1-${count}]${defaultLabel}: `)).trim();
    if (answer === "" && defaultIndex !== undefined) return defaultIndex;
    const selection = Number(answer);
    if (Number.isInteger(selection) && selection >= 1 && selection <= count) return selection - 1;
    io.error(`Enter a number from 1 to ${count}.`);
  }
}

async function selectPlatform(arguments_: CreateArguments, io: CliIo, nonInteractive: boolean): Promise<"ios"> {
  if (arguments_.platform !== undefined) {
    if (arguments_.platform !== "ios") {
      throw new CliError(
        `unsupported platform ${JSON.stringify(arguments_.platform)}; this factory release implements ios only`,
        2,
      );
    }
    return "ios";
  }
  if (nonInteractive) {
    throw new CliError("non-interactive creation requires --platform ios", 2);
  }
  io.out("Platform:");
  io.out("  1. iOS");
  await chooseNumber(io, 1, "Select platform");
  io.out();
  return "ios";
}

async function selectAgent(
  arguments_: CreateArguments,
  installed: readonly AgentAdapter[],
  io: CliIo,
  nonInteractive: boolean,
  configuredDefault?: AgentId,
): Promise<AgentAdapter | null> {
  if (arguments_.agent !== undefined) return requireInstalledAgent(arguments_.agent, installed);
  if (arguments_.noLaunch) return null;
  if (installed.length === 0) {
    throw new CliError(
      "no supported coding agent found on PATH; install Codex or Claude Code, or create with --no-launch",
      3,
    );
  }
  const preferred = configuredDefault === undefined
    ? undefined
    : installed.find((candidate) => candidate.id === configuredDefault);
  if (configuredDefault !== undefined && preferred === undefined) {
    if (nonInteractive) {
      throw new CliError(`configured default agent ${configuredDefault} is not installed`, 3);
    }
    io.error(`Configured default ${configuredDefault} is not installed; choose an available agent.`);
  }
  if (nonInteractive) {
    if (preferred !== undefined) return preferred;
    throw new CliError("non-interactive creation requires --agent codex or --agent claude (or --no-launch)", 2);
  }
  io.out("Coding agents found:");
  installed.forEach((agent, index) => io.out(
    `  ${index + 1}. ${agent.label}${agent.id === configuredDefault ? " (configured default)" : ""}`,
  ));
  if (installed.length === 1) {
    io.out(`Using ${installed[0]!.label}, the only supported agent found.`);
    io.out();
    return installed[0]!;
  }
  const defaultIndex = preferred === undefined ? undefined : installed.indexOf(preferred);
  const selected = installed[await chooseNumber(io, installed.length, "Select coding agent", defaultIndex)]!;
  io.out();
  return selected;
}

async function requireGit(context: CommandContext): Promise<void> {
  try {
    const result = await runCaptured(["git", "--version"], { cwd: context.cwd, env: context.environment });
    if (result.exitCode !== 0) throw new Error(result.stderr.trim());
  } catch {
    throw new CliError("Git is required to create an independent shot; install Git and retry", 3);
  }
}

export async function createCommand(arguments_: CreateArguments, context: CommandContext): Promise<number> {
  const slug = validateShotSlug(arguments_.slug);
  const destination = join(context.config.shotsDirectory, slug);
  if (existsSync(destination)) throw new CliError(`target already exists; refusing to overwrite: ${destination}`);
  const nonInteractive = arguments_.noInteractive || !context.io.interactive;
  await selectPlatform(arguments_, context.io, nonInteractive);
  const installed = detectInstalledAgents(context.environment.PATH ?? "", context.cwd);
  const selectedAgent = await selectAgent(
    arguments_,
    installed,
    context.io,
    nonInteractive,
    context.config.defaultAgent,
  );
  await requireGit(context);

  context.io.out(`Creating ${destination}…`);
  const release = await factoryReleaseFor(context);
  const created = await createShot({
    slug,
    shotsDirectory: context.config.shotsDirectory,
    release,
    selectedAgent: selectedAgent?.id ?? null,
    environment: context.environment,
  });
  context.io.out("Manifest valid.");
  context.io.out("Baseline committed.");
  if (created.gitIdentityMissing) {
    context.io.out("Git author identity was not configured; the neutral baseline succeeded, but configure Git before later commits.");
  }
  context.io.out(`Shot ready at ${created.path}`);
  context.io.out(`Factory release: ${release.metadata.releaseId}${release.reused ? " (cached)" : " (cached now)"}`);

  if (arguments_.noLaunch || selectedAgent === null) {
    context.io.out("Launch skipped. Run your coding agent there and tell it: Read the local AGENTS.md and begin.");
    return 0;
  }

  context.io.out(`Launching ${selectedAgent.label}…`);
  const exitCode = await runInherited(
    [selectedAgent.executable, ...selectedAgent.launchArguments],
    { cwd: created.path, env: sanitizedAgentEnvironment(context.environment) },
  );
  if (exitCode !== 0) {
    throw new CliError(
      `${selectedAgent.label} exited with status ${exitCode}; the validated shot remains at ${created.path}`,
      exitCode,
    );
  }
  return 0;
}

export interface DiscoveredShot {
  path: string;
  metadata: NonNullable<ReturnType<typeof readShotMetadata>>;
  name: string;
}

export function discoverShots(context: CommandContext): DiscoveredShot[] {
  if (!existsSync(context.config.shotsDirectory)) {
    return [];
  }
  return readdirSync(context.config.shotsDirectory, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith("."))
    .map((entry) => {
      const path = join(context.config.shotsDirectory, entry.name);
      const metadata = readShotMetadata(path);
      let name = metadata?.slug ?? entry.name;
      try {
        const manifest = JSON.parse(readFileSync(join(path, "continuity.manifest.json"), "utf8")) as {
          application?: { name?: unknown };
        };
        if (typeof manifest.application?.name === "string") name = manifest.application.name;
      } catch {
        // A malformed manifest is surfaced by status/verification; discovery still works.
      }
      return { path, metadata, name };
    })
    .filter((entry) => entry.metadata !== undefined)
    .sort((left, right) => left.metadata!.slug.localeCompare(right.metadata!.slug)) as DiscoveredShot[];
}

export function listCommand(context: CommandContext): number {
  const shots = discoverShots(context);
  if (shots.length === 0) {
    context.io.out(`No shots yet in ${context.config.shotsDirectory}. Run tohseno and take the first one.`);
    return 0;
  }
  context.io.out("SHOT\tPLATFORM\tFACTORY RELEASE\tPATH");
  for (const shot of shots) {
    const metadata = shot.metadata!;
    context.io.out(`${metadata.slug}\t${metadata.platform}\t${metadata.factory.releaseId}\t${shot.path}`);
  }
  return 0;
}

async function chooseContinuationAgent(
  requested: string | undefined,
  preferredId: AgentId | null | undefined,
  context: CommandContext,
  nonInteractive: boolean,
): Promise<AgentAdapter> {
  const installed = detectInstalledAgents(context.environment.PATH ?? "", context.cwd);
  if (requested !== undefined) return requireInstalledAgent(requested, installed);
  if (installed.length === 0) {
    throw new CliError("no supported coding agent found on PATH; install Codex or Claude Code", 3);
  }
  const preferred = preferredId === null || preferredId === undefined
    ? undefined
    : installed.find((agent) => agent.id === preferredId);
  if (preferredId && preferred === undefined) {
    if (nonInteractive) throw new CliError(`preferred agent ${preferredId} is not installed`, 3);
    context.io.error(`Previously selected ${preferredId} is not installed; choose an available agent.`);
  }
  if (installed.length === 1) {
    context.io.out(`Using ${installed[0]!.label}, the only supported agent found.`);
    return installed[0]!;
  }
  if (nonInteractive) {
    if (preferred) return preferred;
    throw new CliError("multiple coding agents are installed; select one with --agent codex or --agent claude", 2);
  }
  context.io.out("Coding agents found:");
  installed.forEach((agent, index) => context.io.out(
    `  ${index + 1}. ${agent.label}${agent.id === preferredId ? " (shot preference)" : ""}`,
  ));
  const defaultIndex = preferred ? installed.indexOf(preferred) : undefined;
  return installed[await chooseNumber(context.io, installed.length, "Select coding agent", defaultIndex)]!;
}

export async function continueCommand(
  value: string,
  options: { agent?: string; noInteractive: boolean },
  context: CommandContext,
): Promise<number> {
  const root = requireRecognizedShot(resolveShotArgument(value, context));
  const metadata = readShotMetadata(root)!;
  const preferred = context.config.defaultAgent ?? metadata.selectedAgent;
  const selected = await chooseContinuationAgent(
    options.agent,
    preferred,
    context,
    options.noInteractive || !context.io.interactive,
  );
  context.io.out(`Continuing ${metadata.slug} at ${root}`);
  context.io.out(`Launching ${selected.label}…`);
  const exitCode = await runInherited(
    [selected.executable, ...selected.launchArguments],
    { cwd: root, env: sanitizedAgentEnvironment(context.environment) },
  );
  if (exitCode !== 0) {
    throw new CliError(
      `${selected.label} exited with status ${exitCode}; the shot remains at ${root}`,
      exitCode,
    );
  }
  return 0;
}

function resolveShotArgument(value: string | undefined, context: CommandContext): string {
  if (value === undefined) return resolve(context.cwd);
  const looksLikePath = isAbsolute(value) || value.startsWith(".") || value.includes(sep) || value.includes("/");
  return looksLikePath ? resolve(context.cwd, value) : join(context.config.shotsDirectory, validateShotSlug(value));
}

function requireRecognizedShot(path: string): string {
  if (!existsSync(path) || !lstatSync(path).isDirectory()) throw new CliError(`shot does not exist: ${path}`);
  if (readShotMetadata(path) === undefined) {
    throw new CliError(`not a recognized shot: ${path}; compatible existing projects can use tohseno adopt <path>`);
  }
  return path;
}

export function openCommand(slug: string, context: CommandContext): number {
  const path = requireRecognizedShot(join(context.config.shotsDirectory, validateShotSlug(slug)));
  context.io.out(path);
  return 0;
}

export async function verifyCommand(value: string | undefined, context: CommandContext): Promise<number> {
  const root = requireRecognizedShot(resolveShotArgument(value, context));
  const verifier = join(root, ".tohseno", "verify.ts");
  if (!existsSync(verifier)) throw new CliError(`shot is missing its pinned verifier: ${verifier}`);
  const verifierDetails = lstatSync(verifier);
  if (verifierDetails.isSymbolicLink() || !verifierDetails.isFile()) {
    throw new CliError(`shot-local verifier is not a regular file: ${verifier}`);
  }
  const exitCode = await runInherited([process.execPath, verifier], { cwd: root, env: context.environment });
  if (exitCode !== 0) throw new CliError(`shot verification failed with status ${exitCode}`, exitCode);
  return 0;
}

function nearestExistingParent(path: string): string {
  let candidate = path;
  while (!existsSync(candidate)) {
    const parent = dirname(candidate);
    if (parent === candidate) return candidate;
    candidate = parent;
  }
  return candidate;
}

function commandOnPath(name: string, context: CommandContext): boolean {
  const pathValue = context.environment.PATH ?? "";
  return pathValue.split(delimiter).filter(Boolean).some((directory) => {
    try {
      accessSync(join(directory, name), fsConstants.X_OK);
      return statSync(join(directory, name)).isFile();
    } catch {
      return false;
    }
  });
}

export async function doctorCommand(context: CommandContext): Promise<number> {
  let failures = 0;
  let warnings = 0;
  const ok = (message: string): void => context.io.out(`✓ ${message}`);
  const warn = (message: string): void => { warnings += 1; context.io.out(`! ${message}`); };
  const fail = (message: string): void => { failures += 1; context.io.out(`✗ ${message}`); };

  const bunVersion = Bun.version.split(".").map((part) => Number(part));
  const bunSupported = (bunVersion[0] ?? 0) > 1 ||
    ((bunVersion[0] ?? 0) === 1 && (
      (bunVersion[1] ?? 0) > 2 ||
      ((bunVersion[1] ?? 0) === 2 && (bunVersion[2] ?? 0) >= 18)
    ));
  if (bunSupported) ok(`Bun ${Bun.version}`);
  else fail(`Bun ${Bun.version} is too old; version 1.2.18 or newer is required`);
  if (context.config.configExists) ok(`config ${context.config.configPath}`);
  else warn(`config absent; using defaults (${context.config.configPath} is optional)`);

  try {
    const parent = nearestExistingParent(context.config.shotsDirectory);
    if (!lstatSync(parent).isDirectory()) fail(`shots path has no directory parent: ${context.config.shotsDirectory}`);
    else {
      accessSync(parent, fsConstants.W_OK);
      ok(`shots directory ${context.config.shotsDirectory}`);
    }
  } catch (error) {
    fail(`shots directory is not writable: ${errorMessage(error)}`);
  }

  try {
    const git = await runCaptured(["git", "--version"], { cwd: context.cwd, env: context.environment });
    if (git.exitCode !== 0) fail("Git is unavailable");
    else {
      ok(git.stdout.trim());
      const [name, email] = await Promise.all([
        runCaptured(["git", "config", "--get", "user.name"], { cwd: context.cwd, env: context.environment }),
        runCaptured(["git", "config", "--get", "user.email"], { cwd: context.cwd, env: context.environment }),
      ]);
      if (name.exitCode !== 0 || email.exitCode !== 0 || name.stdout.trim() === "" || email.stdout.trim() === "") {
        warn("Git author identity is not configured; TOHSENO will use a local-only factory identity for the baseline commit");
      } else ok("Git author identity configured");
    }
  } catch {
    fail("Git is unavailable");
  }

  const agents = detectInstalledAgents(context.environment.PATH ?? "", context.cwd);
  if (agents.length === 0) warn("no supported coding agent found (install Codex or Claude Code, or use --no-launch)");
  else ok(`coding agents: ${agents.map((agent) => agent.label).join(", ")}`);

  let sourceAvailable = false;
  try {
    const sourceRoot = sourceRootFor(context);
    sourceAvailable = true;
    ok(`factory source ${sourceRoot}`);
    const manifestCheck = await runCaptured(
      [process.execPath, join(sourceRoot, "packages", "manifest", "cli.ts"), join(sourceRoot, "templates", "continuity-app", "continuity.manifest.json")],
      { cwd: sourceRoot, env: context.environment },
    );
    if (manifestCheck.exitCode === 0) ok("manifest tooling and iOS base");
    else fail(`manifest tooling failed: ${manifestCheck.stderr.trim()}`);
  } catch (error) {
    try {
      const active = useActiveCachedRelease(context.config.cacheDirectory);
      warn(`factory source unavailable; using verified cached release ${active.metadata.releaseId}`);
    } catch (cacheError) {
      fail(`${errorMessage(error)}; cached fallback failed: ${errorMessage(cacheError)}`);
    }
  }

  const cached = listCachedReleaseDirectories(context.config.cacheDirectory);
  if (cached.length === 0 && sourceAvailable) ok("release cache is empty and ready to populate");
  for (const directory of cached) {
    try {
      const metadata = verifyReleaseDirectory(directory);
      ok(`cached release ${metadata.releaseId}`);
    } catch (error) {
      fail(errorMessage(error));
    }
  }

  if (commandOnPath("xcodebuild", context)) ok("Xcode command-line tools");
  else warn("xcodebuild not found; shots can be created but iOS builds cannot run here");
  if (commandOnPath("xcodegen", context)) ok("XcodeGen");
  else warn("xcodegen not found; install it before changing project.yml or the Swift file layout");
  if (commandOnPath("xcrun", context)) {
    const simulators = await runCaptured(["xcrun", "simctl", "list", "devices", "available"], {
      cwd: context.cwd,
      env: context.environment,
    });
    if (simulators.exitCode === 0 && /iPhone/u.test(simulators.stdout)) ok("available iPhone simulator");
    else warn("no available iPhone simulator found");
  }

  if (commandOnPath("bankr", context) || commandOnPath("npx", context)) ok("Bankr CLI reachable (bankr or npx @bankr/cli)");
  else warn("Bankr CLI not found; optional, only needed for token launches (npm i -g @bankr/cli)");
  const home = context.environment.HOME;
  if ((home !== undefined && existsSync(join(home, ".bankr", "config.json"))) || context.environment.BANKR_API_KEY) {
    ok("Bankr credentials present");
  } else {
    warn("no Bankr credentials (~/.bankr/config.json or BANKR_API_KEY); optional, run `npx @bankr/cli login email` before launching a token");
  }

  context.io.out();
  context.io.out(`Doctor: ${failures} required failure${failures === 1 ? "" : "s"}, ${warnings} warning${warnings === 1 ? "" : "s"}.`);
  return failures === 0 ? 0 : 1;
}

export async function adoptCommand(
  pathValue: string,
  options: { yes: boolean; noInteractive: boolean },
  context: CommandContext,
): Promise<number> {
  const root = resolve(context.cwd, pathValue);
  const existing = readShotMetadata(root);
  if (existing !== undefined) {
    context.io.out(`Already a recognized shot: ${root}`);
    return 0;
  }
  context.io.out(`Adopt existing project in place: ${root}`);
  context.io.out("This adds only .tohseno/ with pinned metadata and validation tools.");
  context.io.out("It does not move, rewrite, stage, or commit the app.");
  if (!options.yes) {
    if (options.noInteractive || !context.io.interactive) {
      throw new CliError("adopt requires explicit confirmation; rerun with --yes", 2);
    }
    const answer = (await context.io.prompt("Type yes to adopt: ")).trim().toLowerCase();
    if (answer !== "yes") {
      context.io.out("Adoption cancelled; no project files changed.");
      return 0;
    }
  }
  const release = await factoryReleaseFor(context);
  const metadata = await adoptShot({ path: root, release, environment: context.environment });
  context.io.out(`Adopted ${root} as an iOS shot.`);
  context.io.out(`Pinned verifier: bun ${join(root, ".tohseno", "verify.ts")}`);
  context.io.out(`Factory release: ${metadata.factory.releaseId}`);
  return 0;
}

export function launchContract(): string {
  return AGENT_INSTRUCTION;
}
