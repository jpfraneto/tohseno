import { accessSync, constants as fsConstants, existsSync, lstatSync, statSync } from "node:fs";
import { basename, delimiter, dirname, isAbsolute, join, resolve, sep } from "node:path";
import {
  detectInstalledAgents,
  requireInstalledAgent,
  sanitizedAgentEnvironment,
  type AgentId,
  type AgentAdapter,
} from "./agents.ts";
import { AGENT_INSTRUCTION } from "./constants.ts";
import type { ResolvedConfig } from "./config.ts";
import {
  createShot as createShotThroughFactory,
  factoryReleaseFor,
} from "./creation.ts";
import { CliError, errorMessage } from "./errors.ts";
import type { CliIo } from "./io.ts";
import type { CreationInput } from "./provenance.ts";
import { runCaptured, runInherited } from "./process.ts";
import {
  listCachedReleaseDirectories,
  useActiveCachedRelease,
  verifyReleaseDirectory,
} from "./release.ts";
import {
  SimulatorService,
  simulatorDoctorRecords,
  type LivePreviewHandle,
} from "./simulator.ts";
import { adoptShot, readShotMetadata } from "./shot.ts";
import { locateFactorySourceRoot } from "./source.ts";
import { validateShotSlug } from "./slug.ts";
import {
  discoverShotsInDirectory,
  resolveRecognizedShot,
  type DiscoveredShot,
} from "./workspace.ts";
import { trustedShotToolFromCache } from "./trusted-tools.ts";
import {
  startStudioServer,
  waitForStudioSignal,
  type StudioServerHandle,
} from "./studio/server.ts";

export type { DiscoveredShot } from "./workspace.ts";

export interface CommandContext {
  config: ResolvedConfig;
  cwd: string;
  environment: Record<string, string | undefined>;
  io: CliIo;
  sourceRoot?: string | undefined;
}

export interface CreateArguments {
  slug?: string | undefined;
  platform?: string | undefined;
  agent?: string | undefined;
  file?: string | undefined;
  references?: readonly string[] | undefined;
  noLaunch: boolean;
  noInteractive: boolean;
}

function sourceRootFor(context: CommandContext): string {
  return context.sourceRoot ?? locateFactorySourceRoot(context.environment);
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

export async function createCommand(arguments_: CreateArguments, context: CommandContext): Promise<number> {
  const slug = arguments_.slug === undefined
    ? undefined
    : validateShotSlug(arguments_.slug);
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
  const input: CreationInput = {
    ...(arguments_.file === undefined
      ? {}
      : {
          markdown: {
            path: resolve(context.cwd, arguments_.file),
            originalName: basename(arguments_.file),
          },
        }),
    references: (arguments_.references ?? []).map((path) => ({
      path: resolve(context.cwd, path),
      originalName: basename(path),
    })),
  };

  context.io.out(
    slug === undefined
      ? `Creating the next shot in ${context.config.shotsDirectory}…`
      : `Creating ${join(context.config.shotsDirectory, slug)}…`,
  );
  const created = await createShotThroughFactory({
    config: context.config,
    cwd: context.cwd,
    environment: context.environment,
    ...(context.sourceRoot === undefined ? {} : { sourceRoot: context.sourceRoot }),
    ...(slug === undefined ? {} : { slug }),
    door: "cli",
    input,
    agent: selectedAgent,
    noLaunch: arguments_.noLaunch,
    io: context.io,
    runner: new SimulatorService({
      environment: context.environment,
      cwd: context.cwd,
      releasesDirectory: context.config.cacheDirectory,
    }).creationRunner(),
  });
  if (created.gitIdentityMissing) {
    context.io.out("Git author identity was not configured; the neutral baseline succeeded, but configure Git before later commits.");
  }
  context.io.out(`Shot ready at ${created.path}`);
  context.io.out(
    `Factory release: ${created.release.metadata.releaseId}${created.release.reused ? " (cached)" : " (cached now)"}`,
  );

  if (arguments_.noLaunch || selectedAgent === null) {
    context.io.out("Launch skipped. Run your coding agent there and tell it: Read the local AGENTS.md and begin.");
  }
  return 0;
}

export function discoverShots(context: CommandContext): DiscoveredShot[] {
  return discoverShotsInDirectory(context.config.shotsDirectory)
    .sort((left, right) => left.metadata.slug.localeCompare(right.metadata.slug));
}

export function listCommand(context: CommandContext): number {
  const shots = discoverShots(context);
  if (shots.length === 0) {
    context.io.out(`No shots yet in ${context.config.shotsDirectory}. Run tohseno and take the first one.`);
    return 0;
  }
  context.io.out("SHOT\tPLATFORM\tFACTORY RELEASE\tPATH");
  for (const shot of shots) {
    const metadata = shot.metadata;
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
  const trusted = trustedShotToolFromCache({
    shotRoot: root,
    releasesDirectory: context.config.cacheDirectory,
    tool: "verify",
  });
  const exitCode = await runInherited(
    [process.execPath, trusted.executable],
    {
      cwd: trusted.root,
      env: sanitizedAgentEnvironment(context.environment),
    },
  );
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
    if (!statSync(parent).isDirectory()) fail(`shots path has no directory parent: ${context.config.shotsDirectory}`);
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

  if (commandOnPath("xcodegen", context)) ok("XcodeGen");
  else warn("xcodegen not found; install it before changing project.yml or the Swift file layout");
  const simulator = new SimulatorService({
    environment: context.environment,
    cwd: context.cwd,
    releasesDirectory: context.config.cacheDirectory,
  });
  const simulatorReadiness = await simulator.diagnostics();
  for (const record of simulatorDoctorRecords(simulatorReadiness)) {
    if (record.status === "ok") ok(record.message);
    else warn(record.message);
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
  const release = await factoryReleaseFor({
    config: context.config,
    environment: context.environment,
    ...(context.sourceRoot === undefined ? {} : { sourceRoot: context.sourceRoot }),
  });
  const metadata = await adoptShot({ path: root, release, environment: context.environment });
  context.io.out(`Adopted ${root} as an iOS shot.`);
  context.io.out(`Pinned verifier: bun ${join(root, ".tohseno", "verify.ts")}`);
  context.io.out(`Factory release: ${metadata.factory.releaseId}`);
  return 0;
}

export function launchContract(): string {
  return AGENT_INSTRUCTION;
}

function simulatorProgress(
  context: CommandContext,
): (event: { type: string }) => void {
  const labels: Record<string, string> = {
    "development-starting": "Starting the shot's local development service…",
    "development-ready": "Local development service ready.",
    building: "Building the shot for iOS Simulator…",
    "simulator-launching": "Installing and launching in iOS Simulator…",
    "simulator-launched": "Shot launched in iOS Simulator.",
    "screenshot-capturing": "Capturing the Simulator contact-sheet frame…",
    "screenshot-captured": "Simulator screenshot captured.",
    "screenshot-unavailable":
      "Simulator screenshot unavailable; the app remains running.",
    completed: "Simulator run complete.",
  };
  return (event) => {
    const label = labels[event.type];
    if (label !== undefined) context.io.out(label);
  };
}

export interface CommandCancellation {
  signal: AbortSignal;
  close(): void;
}

export function createCommandCancellation(): CommandCancellation {
  const controller = new AbortController();
  const interrupt = (): void => controller.abort();
  let closed = false;
  process.once("SIGINT", interrupt);
  process.once("SIGTERM", interrupt);
  return {
    signal: controller.signal,
    close() {
      if (closed) return;
      closed = true;
      process.off("SIGINT", interrupt);
      process.off("SIGTERM", interrupt);
    },
  };
}

export interface RunCommandDependencies {
  cancellation?: () => CommandCancellation;
}

export async function runCommand(
  value: string,
  context: CommandContext,
  service = new SimulatorService({
    environment: context.environment,
    cwd: context.cwd,
    releasesDirectory: context.config.cacheDirectory,
  }),
  dependencies: RunCommandDependencies = {},
): Promise<number> {
  const cancellation =
    (dependencies.cancellation ?? createCommandCancellation)();
  try {
    const shot = resolveRecognizedShot(value, context);
    context.io.out(`Running ${shot.metadata.slug} in the native iOS Simulator…`);
    const result = await service.runShot({
      shotRoot: shot.path,
      environment: context.environment,
      signal: cancellation.signal,
      onProgress: simulatorProgress(context),
    });
    if (result.screenshotPath !== null) {
      context.io.out(`Screenshot: ${result.screenshotPath}`);
    }
    return 0;
  } finally {
    cancellation.close();
  }
}

async function openLocalPreview(
  url: string,
  context: CommandContext,
): Promise<void> {
  const open = "/usr/bin/open";
  if (!existsSync(open)) {
    throw new CliError(
      "the macOS browser launcher is unavailable; run `tohseno studio --no-open` and open its local URL manually",
      3,
    );
  }
  const result = await runCaptured([open, url], {
    cwd: context.cwd,
    env: context.environment,
  });
  if (result.exitCode !== 0) {
    throw new CliError("the interactive Simulator preview could not be opened");
  }
}

async function waitForPreviewShutdown(
  service: SimulatorService,
  signal: AbortSignal,
): Promise<void> {
  await new Promise<void>((resolveWait) => {
    let settled = false;
    const finish = (): void => {
      if (settled) return;
      settled = true;
      clearInterval(poll);
      signal.removeEventListener("abort", finish);
      resolveWait();
    };
    const poll = setInterval(() => {
      if (!service.livePreview.status().active) finish();
    }, 250);
    poll.unref?.();
    signal.addEventListener("abort", finish, { once: true });
    if (signal.aborted) finish();
  });
}

export interface PreviewCommandDependencies {
  service?: SimulatorService;
  openUrl?: (url: string, context: CommandContext) => Promise<void>;
  wait?: (service: SimulatorService, signal: AbortSignal) => Promise<void>;
  cancellation?: () => CommandCancellation;
}

export async function previewCommand(
  value: string,
  context: CommandContext,
  dependencies: PreviewCommandDependencies = {},
): Promise<number> {
  const service = dependencies.service ?? new SimulatorService({
    environment: context.environment,
    cwd: context.cwd,
    releasesDirectory: context.config.cacheDirectory,
  });
  const cancellation =
    (dependencies.cancellation ?? createCommandCancellation)();
  let preview: LivePreviewHandle | null = null;
  try {
    const shot = resolveRecognizedShot(value, context);
    context.io.out(
      `Running ${shot.metadata.slug} and starting its interactive Simulator stream…`,
    );
    const result = await service.runAndPreview({
      shotRoot: shot.path,
      environment: context.environment,
      signal: cancellation.signal,
      onProgress: simulatorProgress(context),
    });
    preview = result.preview;
    await (dependencies.openUrl ?? openLocalPreview)(
      preview.iframeUrl(),
      context,
    );
    context.io.out(
      "Interactive preview opened from this Mac. Press Ctrl-C here to stop the stream.",
    );
    await (dependencies.wait ?? waitForPreviewShutdown)(
      service,
      cancellation.signal,
    );
    return 0;
  } finally {
    try {
      try {
        await preview?.stop();
      } finally {
        await service.dispose();
      }
    } finally {
      cancellation.close();
    }
  }
}

export interface StudioCommandArguments {
  port: number;
  noOpen: boolean;
}

export interface StudioCommandDependencies {
  start?: (options: Parameters<typeof startStudioServer>[0]) => StudioServerHandle;
  wait?: () => Promise<unknown>;
}

export async function studioCommand(
  arguments_: StudioCommandArguments,
  context: CommandContext,
  dependencies: StudioCommandDependencies = {},
): Promise<number> {
  const studio = (dependencies.start ?? startStudioServer)({
    config: context.config,
    cwd: context.cwd,
    environment: context.environment,
    ...(context.sourceRoot === undefined
      ? {}
      : { sourceRoot: context.sourceRoot }),
    port: arguments_.port,
  });
  try {
    context.io.out(`TOHSENO Studio: ${studio.url}`);
    context.io.out(`Workspace: ${context.config.shotsDirectory}`);
    context.io.out("Binding: 127.0.0.1 only");
    context.io.out(
      studio.selectedAgent === null
        ? "Coding agent: unavailable (viewing works; creation will explain how to install one)"
        : `Coding agent: ${studio.selectedAgent.label}`,
    );
    if (!arguments_.noOpen) await studio.open();
    context.io.out("Press Ctrl-C to stop Studio.");
    await (dependencies.wait ?? waitForStudioSignal)();
    return 0;
  } finally {
    await studio.stop();
  }
}
