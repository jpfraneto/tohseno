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
  type CreationRunner,
  createShot as createShotThroughFactory,
  factoryReleaseFor,
  MaterializedShotCreationError,
} from "./creation.ts";
import { CliError, errorMessage } from "./errors.ts";
import { readBoundedJson } from "./files.ts";
import type { CliIo } from "./io.ts";
import type { CreationInput } from "./provenance.ts";
import { normalizeCreationInput } from "./provenance.ts";
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
import { readShotMetadata } from "./shot.ts";
import { locateFactorySourceRoot } from "./source.ts";
import {
  bundleIdForSlug,
  displayNameForSlug,
  slugForShotName,
  validateShotSlug,
} from "./slug.ts";
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
import {
  blankPlan,
  planIntention,
  type ShotPlan,
  type ValidatedShotPlan,
} from "./planning.ts";
import { loadCatalog, resolveComposition, type AppCatalog } from "../../skills/index.ts";
import { handoffForShot, renderHandoff } from "./handoff.ts";
import {
  acquireShotEvolutionLock,
  advanceLocalShotEvolution,
  readLocalShotProtocolState,
  releaseShotEvolutionLock,
} from "./protocol-state.ts";

export type { DiscoveredShot } from "./workspace.ts";

export interface CommandContext {
  config: ResolvedConfig;
  cwd: string;
  environment: Record<string, string | undefined>;
  io: CliIo;
  sourceRoot?: string | undefined;
  creationRunner?: CreationRunner | undefined;
}

export interface CreateArguments {
  slug?: string | undefined;
  agent?: string | undefined;
  file?: string | undefined;
  references?: readonly string[] | undefined;
  text?: string | undefined;
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

function renderPlan(plan: ValidatedShotPlan, io: CliIo): void {
  io.out("TOHSENO / NEW SHOT");
  io.out();
  io.out(`APP            ${plan.plan.app.name}`);
  io.out(`SLUG           ${plan.plan.app.slug}`);
  io.out(`BUNDLE ID      ${plan.plan.app.bundleId}`);
  io.out(`STARTING SHAPE ${plan.composition.template.descriptor.title}`);
  io.out();
  io.out("APP SKILLS");
  if (plan.plan.skills.length === 0) {
    io.out("  None — neutral kernel only");
  } else {
    for (const skill of plan.plan.skills) {
      const descriptor = plan.composition.skills.find(
        (item) => item.descriptor.id === skill.id,
      )?.descriptor;
      io.out(`  ✓ ${descriptor?.title ?? skill.id} — ${skill.reason}`);
    }
  }
  io.out();
  io.out(`DATA           ${plan.plan.data.strategy} — ${plan.plan.data.reason}`);
  io.out(`RUNTIME IDENTITY ${plan.plan.identity.strategy} — ${plan.plan.identity.reason}`);
  io.out();
  io.out("FIRST DEFINITION OF DONE");
  plan.plan.definitionOfDone.forEach((item, index) =>
    io.out(`  ${index + 1}. ${item}`));
  if (plan.fallback) {
    io.out();
    io.out(
      `PLAN STATUS    safe Blank fallback (${plan.fallbackReason ?? "planning unavailable"})`,
    );
  }
}

async function approvePlan(
  planned: ValidatedShotPlan,
  catalog: AppCatalog,
  io: CliIo,
): Promise<ValidatedShotPlan | null> {
  let current = planned;
  while (true) {
    renderPlan(current, io);
    io.out();
    const answer = (
      await io.prompt(
        "Press Enter to build this. [E] Edit composition  [B] Start blank  [Q] Cancel: ",
      )
    ).trim().toLowerCase();
    if (answer === "") return current;
    if (answer === "q") return null;
    if (answer === "b") {
      current = blankPlan(catalog, current.plan.app.name);
      continue;
    }
    if (answer !== "e") {
      io.error("Enter E, B, Q, or press Enter.");
      continue;
    }

    const nameInput = (
      await io.prompt(`App name [${current.plan.app.name}]: `)
    ).trim();
    const name = nameInput || current.plan.app.name;
    const suggestedSlug = slugForShotName(name);
    const slugInput = (
      await io.prompt(`Slug [${suggestedSlug}]: `)
    ).trim();
    const slug = validateShotSlug(slugInput || suggestedSlug);
    const templates = [...catalog.templates.values()].sort((left, right) =>
      left.descriptor.title.localeCompare(right.descriptor.title));
    io.out("Templates:");
    templates.forEach((template, index) =>
      io.out(`  ${index + 1}. ${template.descriptor.title} — ${template.descriptor.summary}`));
    const selectedTemplate = templates[
      await chooseNumber(io, templates.length, "Template")
    ]!;
    const availableSkills = [...catalog.skills.values()].sort((left, right) =>
      left.descriptor.id.localeCompare(right.descriptor.id));
    io.out(
      `Bundled skills: ${availableSkills.map((skill) => skill.descriptor.id).join(", ")}`,
    );
    const baseComposition = resolveComposition(catalog, {
      schemaVersion: 1,
      template: selectedTemplate.descriptor.id,
      skills: [],
    });
    const baseSkillIds = new Set(
      baseComposition.skills.map((skill) => skill.descriptor.id),
    );
    const installedOutsideTemplate = current.plan.template ===
        selectedTemplate.descriptor.id
      ? current.plan.skills
        .map((skill) => skill.id)
        .filter((id) => !baseSkillIds.has(id))
      : [];
    const dependencyIds = new Set<string>();
    const collectDependencies = (id: string): void => {
      const skill = catalog.skills.get(id);
      if (skill === undefined) return;
      for (const dependency of skill.descriptor.requires) {
        if (dependencyIds.has(dependency)) continue;
        dependencyIds.add(dependency);
        collectDependencies(dependency);
      }
    };
    installedOutsideTemplate.forEach(collectDependencies);
    const currentExtraSkills = installedOutsideTemplate
      .filter((id) => !dependencyIds.has(id))
      .sort();
    const skillsInput = (
      await io.prompt(
        `Extra skill IDs, comma-separated [${currentExtraSkills.join(",")}]: `,
      )
    ).trim();
    const requestedSkills = (
      skillsInput === ""
        ? currentExtraSkills
        : skillsInput.split(",").map((value) => value.trim()).filter(Boolean)
    );
    const composition = resolveComposition(catalog, {
      schemaVersion: 1,
      template: selectedTemplate.descriptor.id,
      skills: requestedSkills,
    });
    const plan: ShotPlan = {
      ...current.plan,
      app: {
        name,
        slug,
        bundleId: bundleIdForSlug(slug),
      },
      template: composition.template.descriptor.id,
      skills: composition.skills.map((skill) => ({
        id: skill.descriptor.id,
        reason: current.plan.skills.find((item) =>
          item.id === skill.descriptor.id)?.reason ??
          `Selected for this ${composition.template.descriptor.title} starting shape.`,
      })),
      definitionOfDone: [...composition.template.descriptor.definitionOfDone],
    };
    current = { plan, composition, fallback: false };
  }
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
  if (
    slug !== undefined &&
    existsSync(join(context.config.shotsDirectory, slug))
  ) {
    throw new CliError(
      `target already exists; refusing to overwrite: ${
        join(context.config.shotsDirectory, slug)
      }`,
    );
  }
  const installed = detectInstalledAgents(context.environment.PATH ?? "", context.cwd);
  const selectedAgent = await selectAgent(
    arguments_,
    installed,
    context.io,
    nonInteractive,
    context.config.defaultAgent,
  );
  const input: CreationInput = {
    ...(arguments_.text === undefined ? {} : { text: arguments_.text }),
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
  const release = await factoryReleaseFor({
    config: context.config,
    environment: context.environment,
    ...(context.sourceRoot === undefined
      ? {}
      : { sourceRoot: context.sourceRoot }),
  });
  const catalog = loadCatalog(join(release.directory, "catalog"));
  const normalized = normalizeCreationInput(input);
  let planned = normalized.intention === null
    ? blankPlan(catalog, slug)
    : await planIntention({
        intention: normalized.intention,
        catalog,
        agent: selectedAgent,
        environment: context.environment,
      });
  if (slug !== undefined) {
    const name = displayNameForSlug(slug);
    planned = {
      ...planned,
      plan: {
        ...planned.plan,
        app: {
          name,
          slug,
          bundleId: bundleIdForSlug(slug),
        },
      },
    };
  }
  if (!nonInteractive) {
    const approved = await approvePlan(planned, catalog, context.io);
    if (approved === null) {
      context.io.out("Shot cancelled; no repository was created.");
      return 0;
    }
    planned = approved;
  }

  context.io.out(
    slug === undefined
      ? `Creating the next shot in ${context.config.shotsDirectory}…`
      : `Creating ${join(context.config.shotsDirectory, slug)}…`,
  );
  let created;
  try {
    created = await createShotThroughFactory({
      config: context.config,
      cwd: context.cwd,
      environment: context.environment,
      ...(context.sourceRoot === undefined ? {} : { sourceRoot: context.sourceRoot }),
      slug: slug ?? planned.plan.app.slug,
      door: "cli",
      input,
      agent: selectedAgent,
      noLaunch: arguments_.noLaunch,
      io: context.io,
      runner: context.creationRunner ?? new SimulatorService({
          environment: context.environment,
          cwd: context.cwd,
          releasesDirectory: context.config.cacheDirectory,
        }).creationRunner(),
      plan: planned.plan,
    });
  } catch (error) {
    if (!(error instanceof MaterializedShotCreationError)) throw error;
    context.io.error(error.message);
    renderHandoff(handoffForShot({
      ...error.shot,
      verificationPassed: false,
      agentExitCode: null,
      buildState: "not-attempted",
      simulatorState: "not-attempted",
      captureState: "not-attempted",
    }), context.io, context.environment);
    return error.exitCode;
  }
  if (created.gitIdentityMissing) {
    context.io.out("Git author identity was not configured; the neutral baseline succeeded, but configure Git before later commits.");
  }
  const simulatorAttempted =
    created.agentMode === "automated" &&
    created.agentExitCode === 0;
  renderHandoff(handoffForShot({
    name: created.metadata.app.name,
    slug: created.metadata.slug,
    path: created.path,
    sequence: created.metadata.sequence,
    skillCount: created.metadata.composition.skills.length,
    verificationPassed: created.verified,
    agentExitCode: created.agentExitCode,
    buildState: created.simulatorLaunched
      ? "completed"
      : simulatorAttempted
        ? "failed"
        : "not-attempted",
    simulatorState: created.simulatorLaunched
      ? "completed"
      : simulatorAttempted
        ? "failed"
        : "not-attempted",
    captureState: created.screenshotPath !== null
      ? "completed"
      : created.simulatorLaunched
        ? "failed"
        : "not-attempted",
    ...(created.simulatorMessage === null
      ? {}
      : { simulatorReason: created.simulatorMessage }),
  }), context.io, context.environment);
  return created.agentExitCode ?? 0;
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

async function chooseEvolutionAgent(
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

export async function evolveCommand(
  value: string,
  options: { agent?: string; noInteractive: boolean },
  context: CommandContext,
): Promise<number> {
  const root = requireRecognizedShot(resolveShotArgument(value, context));
  const metadata = readShotMetadata(root)!;
  const lock = acquireShotEvolutionLock(root);
  try {
    const preferred = context.config.defaultAgent ?? metadata.selectedAgent;
    const selected = await chooseEvolutionAgent(
      options.agent,
      preferred,
      context,
      options.noInteractive || !context.io.interactive,
    );
    context.io.out(`Evolving ${metadata.slug} at ${root}`);
    context.io.out(`Launching ${selected.label}…`);
    const exitCode = await runInherited(
      [selected.executable, ...selected.launchArguments],
      { cwd: root, env: sanitizedAgentEnvironment(context.environment) },
    );
    const trusted = trustedShotToolFromCache({
      shotRoot: root,
      releasesDirectory: context.config.cacheDirectory,
      tool: "verify",
    });
    const verification = await runCaptured(
      [process.execPath, trusted.executable],
      {
        cwd: trusted.root,
        env: sanitizedAgentEnvironment(context.environment),
      },
    );
    if (verification.stdout.trim()) context.io.out(verification.stdout.trim());
    if (verification.stderr.trim()) context.io.error(verification.stderr.trim());
    if (exitCode === 0 && verification.exitCode === 0) {
      const state = advanceLocalShotEvolution(
        root,
        metadata.protocol,
        lock,
      );
      context.io.out(
        `Evolution ${state.evolution} recorded locally for ${state.shotId}.`,
      );
    }
    renderHandoff(handoffForShot({
      name: metadata.app.name,
      slug: metadata.slug,
      path: root,
      sequence: metadata.sequence,
      skillCount: metadata.composition.skills.length,
      verificationPassed: verification.exitCode === 0,
      agentExitCode: exitCode,
      buildState: "not-attempted",
      simulatorState: "not-attempted",
      captureState: "not-attempted",
    }), context.io, context.environment);
    if (exitCode !== 0) return exitCode;
    return verification.exitCode === 0 ? 0 : 1;
  } finally {
    releaseShotEvolutionLock(lock);
  }
}

function resolveShotArgument(value: string | undefined, context: CommandContext): string {
  if (value === undefined) return resolve(context.cwd);
  const looksLikePath = isAbsolute(value) || value.startsWith(".") || value.includes(sep) || value.includes("/");
  return looksLikePath ? resolve(context.cwd, value) : join(context.config.shotsDirectory, validateShotSlug(value));
}

function requireRecognizedShot(path: string): string {
  if (!existsSync(path) || !lstatSync(path).isDirectory()) throw new CliError(`shot does not exist: ${path}`);
  if (readShotMetadata(path) === undefined) {
    throw new CliError(
      `not a recognized Shot: ${path}; create a fresh Shot with \`tohseno\``,
    );
  }
  return path;
}

export function openCommand(slug: string, context: CommandContext): number {
  const path = requireRecognizedShot(join(context.config.shotsDirectory, validateShotSlug(slug)));
  context.io.out(path);
  return 0;
}

export async function statusCommand(
  value: string | undefined,
  context: CommandContext,
): Promise<number> {
  const root = requireRecognizedShot(resolveShotArgument(value, context));
  const metadata = readShotMetadata(root)!;
  const trusted = trustedShotToolFromCache({
    shotRoot: root,
    releasesDirectory: context.config.cacheDirectory,
    tool: "verify",
  });
  const verification = await runCaptured(
    [process.execPath, trusted.executable],
    {
      cwd: trusted.root,
      env: sanitizedAgentEnvironment(context.environment),
    },
  );
  if (verification.exitCode !== 0) {
    throw new CliError(
      `shot verification failed with status ${verification.exitCode}`,
      verification.exitCode,
    );
  }
  context.io.out("TOHSENO / SHOT STATUS");
  context.io.out(`SHOT         ${metadata.slug}`);
  context.io.out(`PATH         ${root}`);
  const state = readLocalShotProtocolState(root);
  if (state === null || state.shotId !== metadata.protocol.shotId) {
    throw new CliError("local Shot protocol state does not match shot metadata");
  }
  context.io.out(`SHOT ID      ${state.shotId}`);
  context.io.out(`LIFECYCLE    ${state.lifecycle}`);
  context.io.out(`EVOLUTION    ${state.evolution}`);
  context.io.out(
    "SOURCE       factory-baseline-bound local metadata; no public record claimed",
  );
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
  let release: Awaited<ReturnType<typeof factoryReleaseFor>> | undefined;
  try {
    const sourceRoot = sourceRootFor(context);
    sourceAvailable = true;
    ok(`factory source ${sourceRoot}`);
    release = await factoryReleaseFor({
      config: context.config,
      environment: context.environment,
      sourceRoot,
    });
  } catch (error) {
    if (sourceAvailable) {
      fail(`current factory release preparation failed: ${errorMessage(error)}`);
    } else {
      try {
        release = useActiveCachedRelease(context.config.cacheDirectory);
        warn(
          `factory source unavailable; using verified cached release ${release.metadata.releaseId}`,
        );
      } catch (cacheError) {
        fail(`${errorMessage(error)}; cached fallback failed: ${errorMessage(cacheError)}`);
      }
    }
  }

  if (release !== undefined) {
    const manifestTool = join(release.directory, "manifest", "cli.ts");
    const appManifest = join(
      release.directory,
      "catalog",
      "kernels",
      "ios-kernel",
      "overlay",
      "app.manifest.json",
    );
    const appManifestCheck = await runCaptured(
      [process.execPath, manifestTool, appManifest],
      { cwd: release.directory, env: context.environment },
    );
    if (appManifestCheck.exitCode !== 0) {
      fail(
        `app manifest validation failed: ${
          appManifestCheck.stderr.trim() ||
          appManifestCheck.stdout.trim() ||
          `status ${appManifestCheck.exitCode}`
        }`,
      );
    } else {
      try {
        const value = readBoundedJson<{
          composition?: {
            kernel?: unknown;
            template?: unknown;
            skills?: unknown;
          };
        }>(appManifest, undefined, "neutral app manifest");
        if (
          value.composition?.kernel !== "ios-kernel" ||
          value.composition.template !== "blank" ||
          !Array.isArray(value.composition.skills) ||
          value.composition.skills.length !== 0
        ) {
          throw new CliError(
            "neutral app manifest must declare ios-kernel, blank, and no app skills",
          );
        }
        ok("app manifest · neutral ios-kernel");
      } catch (error) {
        fail(`app manifest validation failed: ${errorMessage(error)}`);
      }
    }

    try {
      const composition = resolveComposition(
        loadCatalog(join(release.directory, "catalog")),
        { schemaVersion: 1, template: "blank", skills: [] },
      );
      if (
        composition.kernel.descriptor.id !== "ios-kernel" ||
        composition.template.descriptor.id !== "blank" ||
        composition.skills.length !== 0
      ) {
        throw new CliError(
          "neutral catalog composition did not resolve to blank on ios-kernel",
        );
      }
      ok("app catalog · blank resolves to ios-kernel");
    } catch (error) {
      fail(`app catalog validation failed: ${errorMessage(error)}`);
    }

  }

  const cached = listCachedReleaseDirectories(context.config.cacheDirectory);
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

  context.io.out();
  context.io.out(`Doctor: ${failures} required failure${failures === 1 ? "" : "s"}, ${warnings} warning${warnings === 1 ? "" : "s"}.`);
  return failures === 0 ? 0 : 1;
}

export function launchContract(): string {
  return AGENT_INSTRUCTION;
}

function simulatorProgress(
  context: CommandContext,
): (event: { type: string }) => void {
  const labels: Record<string, string> = {
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
    context.io.out(`TOHSENO Studio: ${studio.url} (private session required)`);
    context.io.out(`Workspace: ${context.config.shotsDirectory}`);
    context.io.out("Binding: 127.0.0.1 only");
    context.io.out(
      studio.selectedAgent === null
        ? "Coding agent: unavailable (viewing works; creation will explain how to install one)"
        : `Coding agent: ${studio.selectedAgent.label}`,
    );
    if (!arguments_.noOpen) {
      await studio.open();
    } else {
      context.io.out(`Private browser launcher: ${studio.launcherPath}`);
      context.io.out("Open that owner-only file in a browser to enter Studio.");
    }
    context.io.out("Press Ctrl-C to stop Studio.");
    await (dependencies.wait ?? waitForStudioSignal)();
    return 0;
  } finally {
    await studio.stop();
  }
}
