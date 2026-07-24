import { createHash, randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmdirSync,
  writeFileSync,
} from "node:fs";
import { basename, join, resolve } from "node:path";
import { isAgentId, type AgentId } from "./agents.ts";
import {
  GENERIC_SHOT_SCHEMA_VERSION,
  SHOT_SCHEMA_VERSION,
} from "./constants.ts";
import { CliError, errorMessage } from "./errors.ts";
import {
  assertNoExternalSymlinks,
  copyRegularFile,
  copyTree,
  readBoundedJson,
  readBoundedUtf8,
  removeTreeEvenIfReadOnly,
} from "./files.ts";
import { runCaptured } from "./process.ts";
import {
  writeCreationProvenance,
  type CreationProvenance,
  type NormalizedCreationInput,
} from "./provenance.ts";
import type {
  CreationDoor,
  ShotProgressInput,
} from "./progress.ts";
import type { FactoryRelease, PreparedRelease } from "./release.ts";
import {
  bundleIdForSlug,
  displayNameForSlug,
  slugForShotName,
  validateShotSlug,
} from "./slug.ts";
import {
  applyComposition,
  loadCatalog,
  resolveComposition,
  type AppSkillsLock,
} from "../../skills/index.ts";
import type { ShotPlan } from "./planning.ts";

export interface ShotMetadata {
  schemaVersion:
    | typeof SHOT_SCHEMA_VERSION
    | typeof GENERIC_SHOT_SCHEMA_VERSION;
  slug: string;
  platform: "ios";
  adopted: boolean;
  createdAt: string;
  sequence?: number;
  selectedAgent: AgentId | null;
  baselineAuthor: "factory" | "existing-history";
  creation?: {
    door: CreationDoor;
    inputDigest: string;
    hasIntention: boolean;
    referenceCount: number;
    provenancePath: ".tohseno/provenance/provenance.json";
    options: CreationProvenance["options"];
  };
  factory: {
    releaseId: string;
    cliVersion: string;
    templateVersion: string;
    manifestSchemaVersion: string;
    sourceCommit: string | null;
    sourceDirty: boolean;
    bundleDigest: string;
  };
  architecture?: "generic-app-v1";
  app?: {
    name: string;
    bundleId: string;
  };
  composition?: {
    kernel: { id: string; version: string; digest: string };
    template: { id: string; version: string; digest: string };
    skills: Array<{ id: string; version: string; digest: string }>;
  };
  sanitizedPlanDigest?: string;
}

export interface CreatedShot {
  path: string;
  metadata: ShotMetadata;
  gitIdentityMissing: boolean;
}

function metadataFor(
  slug: string,
  release: FactoryRelease,
  options: {
    adopted: boolean;
    selectedAgent: AgentId | null;
    baselineAuthor: ShotMetadata["baselineAuthor"];
    now: Date;
    sequence?: number;
    creation?: ShotMetadata["creation"];
  },
): ShotMetadata {
  return {
    schemaVersion: SHOT_SCHEMA_VERSION,
    slug,
    platform: "ios",
    adopted: options.adopted,
    createdAt: options.now.toISOString(),
    ...(options.sequence === undefined ? {} : { sequence: options.sequence }),
    selectedAgent: options.selectedAgent,
    baselineAuthor: options.baselineAuthor,
    ...(options.creation === undefined ? {} : { creation: options.creation }),
    factory: {
      releaseId: release.releaseId,
      cliVersion: release.cliVersion,
      templateVersion: release.templateVersion,
      manifestSchemaVersion: release.manifestSchemaVersion,
      sourceCommit: release.source.commit,
      sourceDirty: release.source.dirty,
      bundleDigest: release.bundleDigest,
    },
  };
}

function genericMetadataFor(
  slug: string,
  release: FactoryRelease,
  options: {
    selectedAgent: AgentId | null;
    now: Date;
    sequence: number;
    creation: ShotMetadata["creation"];
    plan: ShotPlan;
    lock: AppSkillsLock;
  },
): ShotMetadata {
  const legacy = metadataFor(slug, release, {
    adopted: false,
    selectedAgent: options.selectedAgent,
    baselineAuthor: "factory",
    now: options.now,
    sequence: options.sequence,
    creation: options.creation,
  });
  return {
    ...legacy,
    schemaVersion: GENERIC_SHOT_SCHEMA_VERSION,
    architecture: "generic-app-v1",
    app: {
      name: options.plan.app.name,
      bundleId: options.plan.app.bundleId,
    },
    composition: {
      kernel: options.lock.kernel,
      template: options.lock.template,
      skills: options.lock.skills,
    },
    sanitizedPlanDigest: createHash("sha256")
      .update(JSON.stringify(options.plan))
      .digest("hex"),
  };
}

function writeJson(path: string, value: unknown): void {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o644 });
}

function customizeManifest(root: string, slug: string): void {
  const path = join(root, "continuity.manifest.json");
  const value = readBoundedJson<Record<string, unknown>>(
    path,
    1_048_576,
    "base continuity manifest",
  );
  const application = value.application;
  if (typeof application !== "object" || application === null || Array.isArray(application)) {
    throw new CliError("base manifest has no application object");
  }
  const app = application as Record<string, unknown>;
  app.id = bundleIdForSlug(slug);
  app.name = displayNameForSlug(slug);
  writeJson(path, value);
}

function customizeXcconfig(root: string, slug: string): void {
  const path = join(root, "Config", "App.xcconfig");
  let source = readBoundedUtf8(path, 65_536, "base app configuration");
  source = source.replace(/^APP_DISPLAY_NAME\s*=.*$/mu, `APP_DISPLAY_NAME = ${displayNameForSlug(slug)}`);
  source = source.replace(/^APP_BUNDLE_ID\s*=.*$/mu, `APP_BUNDLE_ID = ${bundleIdForSlug(slug)}`);
  writeFileSync(path, source, { mode: 0o644 });
}

function addVerifyScript(root: string): void {
  const path = join(root, "package.json");
  const value = readBoundedJson<Record<string, unknown>>(
    path,
    65_536,
    "base package manifest",
  );
  const scriptsValue = value.scripts;
  const scripts = typeof scriptsValue === "object" && scriptsValue !== null && !Array.isArray(scriptsValue)
    ? scriptsValue as Record<string, unknown>
    : {};
  scripts.verify = "bun .tohseno/verify.ts";
  scripts.machine = "bun .tohseno/machine.ts";
  value.scripts = scripts;
  writeJson(path, value);
}

function installPinnedShotFiles(
  root: string,
  release: PreparedRelease,
  metadata: ShotMetadata,
  includeAgentInstructions: boolean,
): void {
  const local = join(root, ".tohseno");
  mkdirSync(join(local, "manifest"), { recursive: true });
  copyTree(join(release.directory, "manifest"), join(local, "manifest"));
  copyRegularFile(join(release.directory, "shot", "verify.ts"), join(local, "verify.ts"), true);
  copyRegularFile(join(release.directory, "shot", "machine.ts"), join(local, "machine.ts"), true);
  copyTree(join(release.directory, "shot", "runtime"), join(local, "runtime"));
  copyRegularFile(join(release.directory, "shot", "OPERATIONS.md"), join(local, "OPERATIONS.md"), false);
  copyRegularFile(join(release.directory, "release.json"), join(local, "factory-release.json"), false);
  writeJson(join(local, "shot.json"), metadata);
  if (includeAgentInstructions) {
    mkdirSync(join(root, "skills", "continuity-app"), { recursive: true });
    copyRegularFile(
      join(release.directory, "agent", "continuity-app", "SKILL.md"),
      join(root, "skills", "continuity-app", "SKILL.md"),
      false,
    );
    copyRegularFile(join(release.directory, "shot", "AGENTS.md"), join(root, "AGENTS.md"), false);
    copyRegularFile(join(release.directory, "shot", "CLAUDE.md"), join(root, "CLAUDE.md"), false);
    if (!existsSync(join(root, "LICENSE"))) {
      copyRegularFile(join(release.directory, "legal", "LICENSE"), join(root, "LICENSE"), false);
    }
  }
}

function installGenericPinnedShotFiles(
  root: string,
  release: PreparedRelease,
  metadata: ShotMetadata,
  lock: AppSkillsLock,
): void {
  installPinnedShotFiles(root, release, metadata, false);
  copyRegularFile(
    join(release.directory, "shot", "AGENTS.md"),
    join(root, "AGENTS.md"),
    false,
  );
  copyRegularFile(
    join(release.directory, "shot", "CLAUDE.md"),
    join(root, "CLAUDE.md"),
    false,
  );
  for (const skill of lock.skills) {
    const source = join(
      release.directory,
      "catalog",
      "skills",
      skill.id,
      "SKILL.md",
    );
    const destination = join(root, "skills", skill.id, "SKILL.md");
    copyRegularFile(source, destination, false);
    copyRegularFile(
      join(
        release.directory,
        "catalog",
        "skills",
        skill.id,
        "skill.json",
      ),
      join(root, ".tohseno", "app-skills", skill.id, "skill.json"),
      false,
    );
  }
  if (!existsSync(join(root, "LICENSE"))) {
    copyRegularFile(
      join(release.directory, "legal", "LICENSE"),
      join(root, "LICENSE"),
      false,
    );
  }
}

async function requireSuccessful(
  command: readonly string[],
  cwd: string,
  label: string,
  environment?: Record<string, string | undefined>,
): Promise<string> {
  const result = await runCaptured(command, environment === undefined ? { cwd } : { cwd, env: environment });
  if (result.exitCode !== 0) {
    const detail = result.stderr.trim() || result.stdout.trim() || `exit ${result.exitCode}`;
    throw new CliError(`${label} failed: ${detail}`);
  }
  return result.stdout;
}

async function configuredGitIdentity(root: string, environment?: Record<string, string | undefined>): Promise<boolean> {
  const name = await runCaptured(["git", "config", "user.name"], environment === undefined ? { cwd: root } : { cwd: root, env: environment });
  const email = await runCaptured(["git", "config", "user.email"], environment === undefined ? { cwd: root } : { cwd: root, env: environment });
  return name.exitCode === 0 && email.exitCode === 0 && name.stdout.trim() !== "" && email.stdout.trim() !== "";
}

async function initializeGit(
  root: string,
  releaseId: string,
  environment?: Record<string, string | undefined>,
): Promise<boolean> {
  await requireSuccessful(
    ["git", "-c", "init.templateDir=", "init", "--quiet", "--initial-branch=main"],
    root,
    "Git initialization",
    environment,
  );
  const hasIdentity = await configuredGitIdentity(root, environment);
  await requireSuccessful(["git", "add", "-A"], root, "Git staging", environment);
  const commit = [
    "git", "-c", "commit.gpgSign=false", "-c", "user.name=TOHSENO Factory",
    "-c", "user.email=factory@tohseno.local",
  ];
  commit.push("commit", "--quiet", "--no-verify", "-m", `chore: create shot from ${releaseId}`);
  await requireSuccessful(commit, root, "baseline commit", environment);
  return !hasIdentity;
}

async function validateManifestWithPinnedTool(root: string, environment?: Record<string, string | undefined>): Promise<void> {
  const manifest = existsSync(join(root, "app.manifest.json"))
    ? "app.manifest.json"
    : "continuity.manifest.json";
  await requireSuccessful(
    [process.execPath, ".tohseno/manifest/cli.ts", manifest],
    root,
    "manifest validation",
    environment,
  );
}

function customizeGenericApp(
  root: string,
  plan: ShotPlan,
): void {
  const manifestPath = join(root, "app.manifest.json");
  const manifest = readBoundedJson<Record<string, unknown>>(
    manifestPath,
    1_048_576,
    "generic app manifest",
  );
  const application = manifest.application;
  if (
    typeof application !== "object" ||
    application === null ||
    Array.isArray(application)
  ) {
    throw new CliError("generic app manifest has no application object");
  }
  (application as Record<string, unknown>).id = plan.app.bundleId;
  (application as Record<string, unknown>).name = plan.app.name;
  writeJson(manifestPath, manifest);

  const configPath = join(root, "Config", "App.xcconfig");
  let config = readBoundedUtf8(
    configPath,
    65_536,
    "generic app configuration",
  );
  config = config.replace(
    /^APP_DISPLAY_NAME\s*=.*$/mu,
    `APP_DISPLAY_NAME = ${plan.app.name}`,
  );
  config = config.replace(
    /^APP_BUNDLE_ID\s*=.*$/mu,
    `APP_BUNDLE_ID = ${plan.app.bundleId}`,
  );
  writeFileSync(configPath, config, { mode: 0o644 });
}

function shotMarkdown(plan: ShotPlan): string {
  const skills = plan.skills.length === 0
    ? "- None. Start from the neutral kernel."
    : plan.skills.map((skill) => `- **${skill.id}** — ${skill.reason}`).join("\n");
  const assumptions = plan.assumptions.length === 0
    ? "- None recorded."
    : plan.assumptions.map((item) => `- ${item}`).join("\n");
  return `# ${plan.app.name}

## Intention

${plan.summary}

## Starting shape

- Template: \`${plan.template}\`
- Platform: native iOS

## Installed app skills

${skills}

## Data and identity

- Data: ${plan.data.strategy} — ${plan.data.reason}
- Identity: ${plan.identity.strategy} — ${plan.identity.reason}

## Assumptions

${assumptions}

## Boundaries

- Raw intention and references remain under private, gitignored provenance.
- The first shot does not add undeclared accounts, remote data movement, deployment, purchases, publishing, or irreversible operations.
- Implement unresolved product behavior against \`DONE.md\`; preserve the kernel and installed capabilities as deliberate starting points.
`;
}

function doneMarkdown(plan: ShotPlan): string {
  return `# First definition of done

${plan.definitionOfDone.map((item, index) => `${index + 1}. ${item}`).join("\n")}
`;
}

export async function materializeGenericShot(options: {
  slug: string;
  shotsDirectory: string;
  release: PreparedRelease;
  selectedAgent: AgentId | null;
  sequence: number;
  door: CreationDoor;
  input: NormalizedCreationInput;
  plan: ShotPlan;
  agentMode: CreationProvenance["options"]["agentMode"];
  verifyAfterAgent: boolean;
  runAfterCreate: boolean;
  environment?: Record<string, string | undefined>;
  now?: Date;
  signal?: AbortSignal;
  emit?: (event: ShotProgressInput) => void | Promise<void>;
}): Promise<CreatedShot> {
  const destination = resolve(options.shotsDirectory, options.slug);
  if (existsSync(destination)) {
    throw new CliError(`target already exists; refusing to overwrite: ${destination}`);
  }
  mkdirSync(options.shotsDirectory, { recursive: true });
  const staging = join(
    options.shotsDirectory,
    `.${options.slug}.creating-${process.pid}-${randomUUID()}`,
  );
  mkdirSync(staging, { mode: 0o700 });
  const createdAt = options.now ?? new Date();
  const progress = async (event: ShotProgressInput): Promise<void> => {
    await options.emit?.(event);
  };
  try {
    throwIfAborted(options.signal);
    await progress({
      type: "preparing-shot",
      slug: options.slug,
      sequence: options.sequence,
    });
    const catalog = loadCatalog(join(options.release.directory, "catalog"));
    const composition = resolveComposition(catalog, {
      schemaVersion: 1,
      template: options.plan.template,
      skills: options.plan.skills.map((skill) => skill.id),
    });
    const applied = applyComposition({
      composition,
      target: staging,
      factoryReleaseId: options.release.metadata.releaseId,
    });
    customizeGenericApp(staging, options.plan);
    writeJson(join(staging, "tohseno.skills.json"), {
      schemaVersion: 1,
      template: options.plan.template,
      skills: options.plan.skills.map((skill) => skill.id),
    });
    writeJson(join(staging, "tohseno.skills.lock"), applied.lock);
    mkdirSync(join(staging, ".tohseno"), { recursive: true });
    writeJson(join(staging, ".tohseno", "shot-plan.json"), options.plan);
    writeFileSync(join(staging, "SHOT.md"), shotMarkdown(options.plan), {
      mode: 0o644,
    });
    writeFileSync(join(staging, "DONE.md"), doneMarkdown(options.plan), {
      mode: 0o644,
    });

    const creationOptions: CreationProvenance["options"] = {
      selectedAgent: options.selectedAgent,
      agentMode: options.agentMode,
      verifyAfterAgent: options.verifyAfterAgent,
      runAfterCreate: options.runAfterCreate,
    };
    const metadata = genericMetadataFor(
      options.slug,
      options.release.metadata,
      {
        selectedAgent: options.selectedAgent,
        now: createdAt,
        sequence: options.sequence,
        creation: {
          door: options.door,
          inputDigest: options.input.inputDigest,
          hasIntention: options.input.intention !== null,
          referenceCount: options.input.references.length,
          provenancePath: ".tohseno/provenance/provenance.json",
          options: creationOptions,
        },
        plan: options.plan,
        lock: applied.lock,
      },
    );
    installGenericPinnedShotFiles(
      staging,
      options.release,
      metadata,
      applied.lock,
    );
    writeCreationProvenance({
      shotRoot: staging,
      createdAt,
      door: options.door,
      release: options.release.metadata,
      input: options.input,
      selectedAgent: options.selectedAgent,
      agentMode: options.agentMode,
      verifyAfterAgent: options.verifyAfterAgent,
      runAfterCreate: options.runAfterCreate,
    });
    await progress({
      type: "provenance-written",
      slug: options.slug,
      sequence: options.sequence,
    });
    assertNoExternalSymlinks(staging);
    await validateManifestWithPinnedTool(staging, options.environment);
    await progress({
      type: "manifest-validated",
      slug: options.slug,
      sequence: options.sequence,
    });
    const gitIdentityMissing = await initializeGit(
      staging,
      options.release.metadata.releaseId,
      options.environment,
    );
    await progress({
      type: "baseline-committed",
      slug: options.slug,
      sequence: options.sequence,
    });
    await validateShotWithPinnedTool(staging, options.environment);
    publishStagedShot(staging, destination);
    await progress({
      type: "published",
      slug: options.slug,
      sequence: options.sequence,
    });
    return { path: destination, metadata, gitIdentityMissing };
  } catch (error) {
    removeTreeEvenIfReadOnly(staging);
    throw new CliError(
      `shot creation failed before publication: ${errorMessage(error)}`,
    );
  }
}

async function validateShotWithPinnedTool(root: string, environment?: Record<string, string | undefined>): Promise<void> {
  await requireSuccessful([process.execPath, ".tohseno/verify.ts"], root, "shot verification", environment);
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) {
    throw new CliError("shot creation was interrupted");
  }
}

export function publishStagedShot(
  staging: string,
  destination: string,
): void {
  let reservation;
  try {
    mkdirSync(destination, { mode: 0o700 });
    reservation = lstatSync(destination);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "EEXIST") {
      throw new CliError(
        `target appeared during creation; refusing to overwrite: ${destination}`,
      );
    }
    throw error;
  }
  try {
    renameSync(staging, destination);
  } catch (error) {
    try {
      const current = lstatSync(destination);
      if (
        current.dev === reservation.dev &&
        current.ino === reservation.ino &&
        current.isDirectory() &&
        readdirSync(destination).length === 0
      ) {
        rmdirSync(destination);
      }
    } catch {
      // Never remove a path that replaced or populated our empty reservation.
    }
    throw error;
  }
}

export async function materializeShot(options: {
  slug: string;
  shotsDirectory: string;
  release: PreparedRelease;
  selectedAgent: AgentId | null;
  sequence: number;
  door: CreationDoor;
  input: NormalizedCreationInput;
  agentMode: CreationProvenance["options"]["agentMode"];
  verifyAfterAgent: boolean;
  runAfterCreate: boolean;
  environment?: Record<string, string | undefined>;
  now?: Date;
  signal?: AbortSignal;
  emit?: (event: ShotProgressInput) => void | Promise<void>;
}): Promise<CreatedShot> {
  const destination = resolve(options.shotsDirectory, options.slug);
  if (existsSync(destination)) throw new CliError(`target already exists; refusing to overwrite: ${destination}`);
  mkdirSync(options.shotsDirectory, { recursive: true });
  const staging = join(options.shotsDirectory, `.${options.slug}.creating-${process.pid}-${randomUUID()}`);
  mkdirSync(staging, { mode: 0o700 });
  const createdAt = options.now ?? new Date();
  const progress = async (event: ShotProgressInput): Promise<void> => {
    await options.emit?.(event);
  };
  try {
    throwIfAborted(options.signal);
    await progress({
      type: "preparing-shot",
      slug: options.slug,
      sequence: options.sequence,
    });
    copyTree(join(options.release.directory, "platforms", "ios", "base"), staging);
    customizeManifest(staging, options.slug);
    customizeXcconfig(staging, options.slug);
    addVerifyScript(staging);
    const creationOptions: CreationProvenance["options"] = {
      selectedAgent: options.selectedAgent,
      agentMode: options.agentMode,
      verifyAfterAgent: options.verifyAfterAgent,
      runAfterCreate: options.runAfterCreate,
    };
    const provisionalMetadata = metadataFor(options.slug, options.release.metadata, {
      adopted: false,
      selectedAgent: options.selectedAgent,
      baselineAuthor: "factory",
      now: createdAt,
      sequence: options.sequence,
      creation: {
        door: options.door,
        inputDigest: options.input.inputDigest,
        hasIntention: options.input.intention !== null,
        referenceCount: options.input.references.length,
        provenancePath: ".tohseno/provenance/provenance.json",
        options: creationOptions,
      },
    });
    installPinnedShotFiles(staging, options.release, provisionalMetadata, true);
    writeCreationProvenance({
      shotRoot: staging,
      createdAt,
      door: options.door,
      release: options.release.metadata,
      input: options.input,
      selectedAgent: options.selectedAgent,
      agentMode: options.agentMode,
      verifyAfterAgent: options.verifyAfterAgent,
      runAfterCreate: options.runAfterCreate,
    });
    await progress({
      type: "provenance-written",
      slug: options.slug,
      sequence: options.sequence,
    });
    throwIfAborted(options.signal);
    assertNoExternalSymlinks(staging);
    await validateManifestWithPinnedTool(staging, options.environment);
    await progress({
      type: "manifest-validated",
      slug: options.slug,
      sequence: options.sequence,
    });
    throwIfAborted(options.signal);
    const gitIdentityMissing = await initializeGit(
      staging,
      options.release.metadata.releaseId,
      options.environment,
    );
    await progress({
      type: "baseline-committed",
      slug: options.slug,
      sequence: options.sequence,
    });
    throwIfAborted(options.signal);
    await validateShotWithPinnedTool(staging, options.environment);
    publishStagedShot(staging, destination);
    try {
      await progress({
        type: "published",
        slug: options.slug,
        sequence: options.sequence,
      });
    } catch {
      // Publication is already atomic and durable. A presentation-layer
      // progress failure cannot turn a completed shot into a failed one.
    }
    return { path: destination, metadata: provisionalMetadata, gitIdentityMissing };
  } catch (error) {
    removeTreeEvenIfReadOnly(staging);
    throw new CliError(`shot creation failed before publication: ${errorMessage(error)}`);
  }
}

export function readShotMetadata(root: string): ShotMetadata | undefined {
  const path = join(root, ".tohseno", "shot.json");
  if (!existsSync(path)) return undefined;
  try {
    const value = readBoundedJson<Partial<ShotMetadata>>(
      path,
      65_536,
      "shot metadata",
    );
    const factory = value.factory;
    const selectedAgentValid = value.selectedAgent === null ||
      (typeof value.selectedAgent === "string" &&
        isAgentId(value.selectedAgent));
    const sequenceValid = value.sequence === undefined ||
      (Number.isSafeInteger(value.sequence) && (value.sequence ?? 0) > 0);
    const createdAtValid = typeof value.createdAt === "string" &&
      Number.isFinite(Date.parse(value.createdAt)) &&
      new Date(value.createdAt).toISOString() === value.createdAt;
    if (
      (value.schemaVersion !== SHOT_SCHEMA_VERSION &&
        value.schemaVersion !== GENERIC_SHOT_SCHEMA_VERSION) ||
      value.platform !== "ios" ||
      typeof value.slug !== "string" ||
      validateShotSlug(value.slug) !== value.slug ||
      typeof value.adopted !== "boolean" ||
      !createdAtValid ||
      !sequenceValid ||
      !selectedAgentValid ||
      (value.baselineAuthor !== "factory" &&
        value.baselineAuthor !== "existing-history") ||
      typeof factory !== "object" ||
      factory === null ||
      typeof factory.releaseId !== "string" ||
      !/^(?:git-[0-9a-f]{40}(?:-dirty)?-[0-9a-f]{16}|content-[0-9a-f]{32})$/u
        .test(factory.releaseId) ||
      typeof factory.cliVersion !== "string" ||
      !/^[0-9]+\.[0-9]+\.[0-9]+$/u.test(factory.cliVersion) ||
      typeof factory.templateVersion !== "string" ||
      !/^[a-z0-9][a-z0-9.-]{0,127}$/u.test(factory.templateVersion) ||
      typeof factory.manifestSchemaVersion !== "string" ||
      !/^[0-9]+\.[0-9]+\.[0-9]+$/u.test(factory.manifestSchemaVersion) ||
      (factory.sourceCommit !== null &&
        (typeof factory.sourceCommit !== "string" ||
          !/^[0-9a-f]{40}$/u.test(factory.sourceCommit))) ||
      typeof factory.sourceDirty !== "boolean" ||
      typeof factory.bundleDigest !== "string" ||
      !/^[0-9a-f]{64}$/u.test(factory.bundleDigest)
    ) {
      return undefined;
    }
    if (value.schemaVersion === GENERIC_SHOT_SCHEMA_VERSION) {
      const app = value.app;
      const composition = value.composition;
      if (
        value.architecture !== "generic-app-v1" ||
        typeof app !== "object" ||
        app === null ||
        typeof app.name !== "string" ||
        app.name.trim() === "" ||
        app.name.length > 80 ||
        typeof app.bundleId !== "string" ||
        !/^[A-Za-z0-9]+(?:\.[A-Za-z0-9-]+)+$/u.test(app.bundleId) ||
        typeof composition !== "object" ||
        composition === null ||
        typeof composition.kernel !== "object" ||
        composition.kernel === null ||
        typeof composition.template !== "object" ||
        composition.template === null ||
        !Array.isArray(composition.skills) ||
        typeof value.sanitizedPlanDigest !== "string" ||
        !/^[a-f0-9]{64}$/u.test(value.sanitizedPlanDigest)
      ) {
        return undefined;
      }
      for (const item of [
        composition.kernel,
        composition.template,
        ...composition.skills,
      ]) {
        if (
          typeof item !== "object" ||
          item === null ||
          typeof item.id !== "string" ||
          !/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(item.id) ||
          typeof item.version !== "string" ||
          !/^[0-9]+\.[0-9]+\.[0-9]+$/u.test(item.version) ||
          typeof item.digest !== "string" ||
          !/^[a-f0-9]{64}$/u.test(item.digest)
        ) {
          return undefined;
        }
      }
    }
    if (value.creation !== undefined) {
      const creation = value.creation;
      const options = creation.options;
      if (
        typeof creation !== "object" ||
        creation === null ||
        (creation.door !== "cli" && creation.door !== "studio") ||
        typeof creation.inputDigest !== "string" ||
        !/^[0-9a-f]{64}$/u.test(creation.inputDigest) ||
        typeof creation.hasIntention !== "boolean" ||
        !Number.isSafeInteger(creation.referenceCount) ||
        creation.referenceCount < 0 ||
        creation.referenceCount > 8 ||
        creation.provenancePath !==
          ".tohseno/provenance/provenance.json" ||
        typeof options !== "object" ||
        options === null ||
        (options.selectedAgent !== null &&
          (typeof options.selectedAgent !== "string" ||
            !isAgentId(options.selectedAgent))) ||
        !["interactive", "automated", "none"].includes(options.agentMode) ||
        typeof options.verifyAfterAgent !== "boolean" ||
        typeof options.runAfterCreate !== "boolean"
      ) {
        return undefined;
      }
    }
    return value as ShotMetadata;
  } catch {
    return undefined;
  }
}

export async function adoptShot(options: {
  path: string;
  release: PreparedRelease;
  environment?: Record<string, string | undefined>;
  now?: Date;
}): Promise<ShotMetadata> {
  const requestedRoot = resolve(options.path);
  const root = existsSync(requestedRoot) ? realpathSync(requestedRoot) : requestedRoot;
  if (!existsSync(root) || !lstatSync(root).isDirectory()) throw new CliError(`adoption path is not a directory: ${root}`);
  if (existsSync(join(root, ".tohseno"))) throw new CliError(`${root} already has .tohseno metadata; refusing to overwrite it`);
  for (const path of ["continuity.manifest.json", "project.yml", "App/AppConfig.swift", "Writing.xcodeproj/project.pbxproj"]) {
    if (!existsSync(join(root, path))) throw new CliError(`project is not a compatible iOS base: missing ${path}`);
  }
  const top = await requireSuccessful(["git", "rev-parse", "--show-toplevel"], root, "Git repository check", options.environment);
  if (realpathSync(resolve(top.trim())) !== root) {
    throw new CliError("adopt requires the path to be the root of its independent Git repository");
  }
  await requireSuccessful(
    [process.execPath, join(options.release.directory, "manifest", "cli.ts"), join(root, "continuity.manifest.json")],
    root,
    "manifest validation",
    options.environment,
  );

  const slug = slugForShotName(basename(root));
  const metadata = metadataFor(slug, options.release.metadata, {
    adopted: true,
    selectedAgent: null,
    baselineAuthor: "existing-history",
    now: options.now ?? new Date(),
  });
  const temporary = join(root, `.tohseno-adopting-${process.pid}-${randomUUID()}`);
  mkdirSync(temporary, { mode: 0o700 });
  try {
    mkdirSync(join(temporary, "manifest"));
    copyTree(join(options.release.directory, "manifest"), join(temporary, "manifest"));
    copyRegularFile(join(options.release.directory, "shot", "verify.ts"), join(temporary, "verify.ts"), true);
    copyRegularFile(join(options.release.directory, "shot", "machine.ts"), join(temporary, "machine.ts"), true);
    copyTree(join(options.release.directory, "shot", "runtime"), join(temporary, "runtime"));
    copyRegularFile(join(options.release.directory, "shot", "OPERATIONS.md"), join(temporary, "OPERATIONS.md"), false);
    copyRegularFile(join(options.release.directory, "release.json"), join(temporary, "factory-release.json"), false);
    writeJson(join(temporary, "shot.json"), metadata);
    renameSync(temporary, join(root, ".tohseno"));
    try {
      await validateShotWithPinnedTool(root, options.environment);
    } catch (error) {
      removeTreeEvenIfReadOnly(join(root, ".tohseno"));
      throw error;
    }
    return metadata;
  } catch (error) {
    removeTreeEvenIfReadOnly(temporary);
    throw new CliError(`adoption failed without changing the app: ${errorMessage(error)}`);
  }
}
