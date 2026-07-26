import { createHash, randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  renameSync,
  rmdirSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { isAgentId, type AgentId } from "./agents.ts";
import {
  CLI_VERSION,
  IOS_TEMPLATE_VERSION,
  MANIFEST_SCHEMA_VERSION,
  SHOT_SCHEMA_VERSION,
} from "./constants.ts";
import { CliError, errorMessage } from "./errors.ts";
import {
  assertNoExternalSymlinks,
  copyRegularFile,
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
import {
  createShotProtocolPointer,
  initialShotProtocolState,
  readLocalShotProtocolState,
  validateShotProtocolPointer,
  type ShotProtocolPointer,
} from "./protocol-state.ts";
import type { FactoryRelease, PreparedRelease } from "./release.ts";
import { bundleIdForSlug, validateShotSlug } from "./slug.ts";
import {
  applyComposition,
  loadCatalog,
  resolveInstalledComposition,
  type AppSkillsLock,
} from "../../skills/index.ts";
import {
  type AppManifest,
  validateAppManifest,
} from "../../manifest/app.ts";
import type { ShotPlan } from "./planning.ts";

export const UNSUPPORTED_SHOT_STATE_MESSAGE =
  "pre-release compatibility is unsupported; create a fresh Shot with `tohseno`" as const;

interface CompositionPin {
  id: string;
  version: string;
  digest: string;
}

export interface ShotMetadata {
  schemaVersion: typeof SHOT_SCHEMA_VERSION;
  slug: string;
  platform: "ios";
  createdAt: string;
  sequence: number;
  selectedAgent: AgentId | null;
  creation: {
    door: CreationDoor;
    inputDigest: string;
    hasIntention: boolean;
    referenceCount: number;
    provenancePath: ".tohseno/provenance/provenance.json";
    options: CreationProvenance["options"];
  };
  factory: {
    releaseId: string;
    cliVersion: typeof CLI_VERSION;
    templateVersion: typeof IOS_TEMPLATE_VERSION;
    manifestSchemaVersion: typeof MANIFEST_SCHEMA_VERSION;
    sourceCommit: string | null;
    sourceDirty: boolean;
    bundleDigest: string;
  };
  app: {
    name: string;
    bundleId: string;
  };
  composition: {
    kernel: CompositionPin;
    template: CompositionPin;
    skills: CompositionPin[];
  };
  sanitizedPlanDigest: string;
  protocol: ShotProtocolPointer;
}

export interface CreatedShot {
  path: string;
  metadata: ShotMetadata;
  gitIdentityMissing: boolean;
}

function writeJson(path: string, value: unknown): void {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o644 });
}

function metadataFor(
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
  return {
    schemaVersion: SHOT_SCHEMA_VERSION,
    slug,
    platform: "ios",
    createdAt: options.now.toISOString(),
    sequence: options.sequence,
    selectedAgent: options.selectedAgent,
    creation: options.creation,
    factory: {
      releaseId: release.releaseId,
      cliVersion: CLI_VERSION,
      templateVersion: IOS_TEMPLATE_VERSION,
      manifestSchemaVersion: MANIFEST_SCHEMA_VERSION,
      sourceCommit: release.source.commit,
      sourceDirty: release.source.dirty,
      bundleDigest: release.bundleDigest,
    },
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
    protocol: createShotProtocolPointer(),
  };
}

function installPinnedShotFiles(
  root: string,
  release: PreparedRelease,
  metadata: ShotMetadata,
  lock: AppSkillsLock,
): void {
  const local = join(root, ".tohseno");
  mkdirSync(join(local, "manifest"), { recursive: true });
  for (const file of [
    "app.ts",
    "app.manifest.schema.json",
    "cli.ts",
  ]) {
    copyRegularFile(
      join(release.directory, "manifest", file),
      join(local, "manifest", file),
      false,
    );
  }
  copyRegularFile(
    join(release.directory, "shot", "verify.ts"),
    join(local, "verify.ts"),
    true,
  );
  copyRegularFile(
    join(release.directory, "shot", "machine.ts"),
    join(local, "machine.ts"),
    true,
  );
  for (const file of ["ios.ts", "shared.ts"]) {
    copyRegularFile(
      join(release.directory, "shot", "runtime", file),
      join(local, "runtime", file),
      false,
    );
  }
  copyRegularFile(
    join(release.directory, "shot", "OPERATIONS.md"),
    join(local, "OPERATIONS.md"),
    false,
  );
  copyRegularFile(
    join(release.directory, "release.json"),
    join(local, "factory-release.json"),
    false,
  );
  writeJson(join(local, "shot.json"), metadata);
  writeJson(
    join(root, metadata.protocol.statePath),
    initialShotProtocolState(metadata.protocol.shotId),
  );
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
    copyRegularFile(
      join(
        release.directory,
        "catalog",
        "skills",
        skill.id,
        "SKILL.md",
      ),
      join(root, "skills", skill.id, "SKILL.md"),
      false,
    );
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
  const result = await runCaptured(
    command,
    environment === undefined ? { cwd } : { cwd, env: environment },
  );
  if (result.exitCode !== 0) {
    const detail = result.stderr.trim() ||
      result.stdout.trim() ||
      `exit ${result.exitCode}`;
    throw new CliError(`${label} failed: ${detail}`);
  }
  return result.stdout;
}

async function configuredGitIdentity(
  root: string,
  environment?: Record<string, string | undefined>,
): Promise<boolean> {
  const options = environment === undefined
    ? { cwd: root }
    : { cwd: root, env: environment };
  const name = await runCaptured(["git", "config", "user.name"], options);
  const email = await runCaptured(["git", "config", "user.email"], options);
  return name.exitCode === 0 &&
    email.exitCode === 0 &&
    name.stdout.trim() !== "" &&
    email.stdout.trim() !== "";
}

async function initializeGit(
  root: string,
  releaseId: string,
  environment?: Record<string, string | undefined>,
): Promise<boolean> {
  await requireSuccessful(
    [
      "git",
      "-c",
      "init.templateDir=",
      "init",
      "--quiet",
      "--initial-branch=main",
    ],
    root,
    "Git initialization",
    environment,
  );
  const hasIdentity = await configuredGitIdentity(root, environment);
  await requireSuccessful(
    ["git", "add", "-A"],
    root,
    "Git staging",
    environment,
  );
  await requireSuccessful(
    [
      "git",
      "-c",
      "commit.gpgSign=false",
      "-c",
      "user.name=TOHSENO Factory",
      "-c",
      "user.email=factory@tohseno.local",
      "commit",
      "--quiet",
      "--no-verify",
      "-m",
      `chore: create shot from ${releaseId}`,
    ],
    root,
    "baseline commit",
    environment,
  );
  return !hasIdentity;
}

async function validateManifestWithPinnedTool(
  root: string,
  environment?: Record<string, string | undefined>,
): Promise<void> {
  await requireSuccessful(
    [process.execPath, ".tohseno/manifest/cli.ts", "app.manifest.json"],
    root,
    "manifest validation",
    environment,
  );
}

function customizeApp(root: string, plan: ShotPlan): void {
  const manifestPath = join(root, "app.manifest.json");
  const manifest = readBoundedJson<Record<string, unknown>>(
    manifestPath,
    1_048_576,
    "app manifest",
  );
  const application = manifest.application;
  if (
    typeof application !== "object" ||
    application === null ||
    Array.isArray(application)
  ) {
    throw new CliError("app manifest has no application object");
  }
  (application as Record<string, unknown>).id = plan.app.bundleId;
  (application as Record<string, unknown>).name = plan.app.name;
  writeJson(manifestPath, manifest);

  const configPath = join(root, "Config", "App.xcconfig");
  let config = readBoundedUtf8(
    configPath,
    65_536,
    "app configuration",
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
    : plan.skills
      .map((skill) => `- **${skill.id}** — ${skill.reason}`)
      .join("\n");
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
- Runtime identity: ${plan.identity.strategy} — ${plan.identity.reason}

## Assumptions

${assumptions}

## Boundaries

- This repository is one Shot. Later changes are Evolutions of this same Shot;
  use \`tohseno status\` for its stable local identity and tracked starting
  state. Public lifecycle claims require a separately verified signed chain.
- Raw intention and references remain under private, gitignored provenance.
- The first Shot does not add undeclared accounts, remote data movement,
  deployment, purchases, publishing, or irreversible operations.
- Implement unresolved product behavior against \`DONE.md\`; preserve the
  kernel and installed capabilities as deliberate starting points.
`;
}

function doneMarkdown(plan: ShotPlan): string {
  return `# First definition of done

${plan.definitionOfDone
    .map((item, index) => `${index + 1}. ${item}`)
    .join("\n")}
`;
}

async function validateShotWithPinnedTool(
  root: string,
  environment?: Record<string, string | undefined>,
): Promise<void> {
  await requireSuccessful(
    [process.execPath, ".tohseno/verify.ts"],
    root,
    "shot verification",
    environment,
  );
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) {
    throw new CliError("shot creation was interrupted");
  }
}

export function materializeStagedShot(
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
    throw new CliError(
      `target already exists; refusing to overwrite: ${destination}`,
    );
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
    const composition = resolveInstalledComposition(catalog, {
      schemaVersion: 1,
      template: options.plan.template,
      skills: options.plan.skills.map((skill) => skill.id),
    });
    const applied = applyComposition({
      composition,
      target: staging,
      factoryReleaseId: options.release.metadata.releaseId,
    });
    customizeApp(staging, options.plan);
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
    const metadata = metadataFor(
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
    installPinnedShotFiles(
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
    materializeStagedShot(staging, destination);
    try {
      await progress({
        type: "repository-created",
        slug: options.slug,
        sequence: options.sequence,
      });
    } catch {
      // Creation is already durable; UI progress cannot invalidate the Shot.
    }
    return { path: destination, metadata, gitIdentityMissing };
  } catch (error) {
    removeTreeEvenIfReadOnly(staging);
    throw new CliError(
      `shot creation failed before repository creation: ${errorMessage(error)}`,
    );
  }
}

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" &&
      value !== null &&
      !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function exactKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
): boolean {
  const actual = Object.keys(value).sort();
  const canonical = [...expected].sort();
  return actual.length === canonical.length &&
    actual.every((key, index) => key === canonical[index]);
}

function validTimestamp(value: unknown): value is string {
  return typeof value === "string" &&
    Number.isFinite(Date.parse(value)) &&
    new Date(value).toISOString() === value;
}

function validPin(value: unknown): value is CompositionPin {
  const candidate = record(value);
  return candidate !== null &&
    exactKeys(candidate, ["id", "version", "digest"]) &&
    typeof candidate.id === "string" &&
    /^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(candidate.id) &&
    typeof candidate.version === "string" &&
    /^[0-9]+\.[0-9]+\.[0-9]+$/u.test(candidate.version) &&
    typeof candidate.digest === "string" &&
    /^[a-f0-9]{64}$/u.test(candidate.digest);
}

function unsupportedShotState(root: string): CliError {
  return new CliError(
    `unsupported Shot state at ${resolve(root)}: ${UNSUPPORTED_SHOT_STATE_MESSAGE}`,
    2,
  );
}

function parseShotMetadata(value: unknown): ShotMetadata | null {
  const candidate = record(value);
  if (
    candidate === null ||
    !exactKeys(candidate, [
      "schemaVersion",
      "slug",
      "platform",
      "createdAt",
      "sequence",
      "selectedAgent",
      "creation",
      "factory",
      "app",
      "composition",
      "sanitizedPlanDigest",
      "protocol",
    ]) ||
    candidate.schemaVersion !== SHOT_SCHEMA_VERSION ||
    candidate.platform !== "ios" ||
    typeof candidate.slug !== "string" ||
    validateShotSlug(candidate.slug) !== candidate.slug ||
    !validTimestamp(candidate.createdAt) ||
    !Number.isSafeInteger(candidate.sequence) ||
    (candidate.sequence as number) < 1 ||
    (
      candidate.selectedAgent !== null &&
      (
        typeof candidate.selectedAgent !== "string" ||
        !isAgentId(candidate.selectedAgent)
      )
    ) ||
    typeof candidate.sanitizedPlanDigest !== "string" ||
    !/^[a-f0-9]{64}$/u.test(candidate.sanitizedPlanDigest)
  ) {
    return null;
  }

  const creation = record(candidate.creation);
  const creationOptions = record(creation?.options);
  if (
    creation === null ||
    !exactKeys(creation, [
      "door",
      "inputDigest",
      "hasIntention",
      "referenceCount",
      "provenancePath",
      "options",
    ]) ||
    (creation.door !== "cli" && creation.door !== "studio") ||
    typeof creation.inputDigest !== "string" ||
    !/^[a-f0-9]{64}$/u.test(creation.inputDigest) ||
    typeof creation.hasIntention !== "boolean" ||
    !Number.isSafeInteger(creation.referenceCount) ||
    (creation.referenceCount as number) < 0 ||
    (creation.referenceCount as number) > 8 ||
    creation.provenancePath !== ".tohseno/provenance/provenance.json" ||
    creationOptions === null ||
    !exactKeys(creationOptions, [
      "selectedAgent",
      "agentMode",
      "verifyAfterAgent",
      "runAfterCreate",
    ]) ||
    creationOptions.selectedAgent !== candidate.selectedAgent ||
    (
      creationOptions.selectedAgent !== null &&
      (
        typeof creationOptions.selectedAgent !== "string" ||
        !isAgentId(creationOptions.selectedAgent)
      )
    ) ||
    !["interactive", "automated", "none"].includes(
      creationOptions.agentMode as string,
    ) ||
    typeof creationOptions.verifyAfterAgent !== "boolean" ||
    typeof creationOptions.runAfterCreate !== "boolean"
  ) {
    return null;
  }

  const factory = record(candidate.factory);
  if (
    factory === null ||
    !exactKeys(factory, [
      "releaseId",
      "cliVersion",
      "templateVersion",
      "manifestSchemaVersion",
      "sourceCommit",
      "sourceDirty",
      "bundleDigest",
    ]) ||
    typeof factory.releaseId !== "string" ||
    !/^(?:git-[0-9a-f]{40}(?:-dirty)?-[0-9a-f]{16}|content-[0-9a-f]{32})$/u
      .test(factory.releaseId) ||
    factory.cliVersion !== CLI_VERSION ||
    factory.templateVersion !== IOS_TEMPLATE_VERSION ||
    factory.manifestSchemaVersion !== MANIFEST_SCHEMA_VERSION ||
    (
      factory.sourceCommit !== null &&
      (
        typeof factory.sourceCommit !== "string" ||
        !/^[0-9a-f]{40}$/u.test(factory.sourceCommit)
      )
    ) ||
    typeof factory.sourceDirty !== "boolean" ||
    typeof factory.bundleDigest !== "string" ||
    !/^[a-f0-9]{64}$/u.test(factory.bundleDigest)
  ) {
    return null;
  }
  if (
    (
      factory.releaseId.startsWith("content-") &&
      (factory.sourceCommit !== null || factory.sourceDirty)
    ) ||
    (
      factory.releaseId.startsWith("git-") &&
      (
        factory.sourceCommit === null ||
        !factory.releaseId.startsWith(`git-${factory.sourceCommit}`) ||
        factory.releaseId.includes("-dirty-") !== factory.sourceDirty
      )
    )
  ) {
    return null;
  }

  const app = record(candidate.app);
  if (
    app === null ||
    !exactKeys(app, ["name", "bundleId"]) ||
    typeof app.name !== "string" ||
    app.name.length < 1 ||
    app.name.length > 80 ||
    app.name !== app.name.trim() ||
    /[\u0000-\u001f\u007f-\u009f]/u.test(app.name) ||
    app.bundleId !== bundleIdForSlug(candidate.slug)
  ) {
    return null;
  }

  const composition = record(candidate.composition);
  if (
    composition === null ||
    !exactKeys(composition, ["kernel", "template", "skills"]) ||
    !validPin(composition.kernel) ||
    composition.kernel.id !== "ios-kernel" ||
    composition.kernel.version !== "1.0.0" ||
    !validPin(composition.template) ||
    !Array.isArray(composition.skills) ||
    !composition.skills.every(validPin) ||
    new Set(
      (composition.skills as CompositionPin[]).map((skill) => skill.id),
    ).size !== composition.skills.length
  ) {
    return null;
  }

  try {
    validateShotProtocolPointer(candidate.protocol);
  } catch {
    return null;
  }
  return candidate as unknown as ShotMetadata;
}

export function readShotMetadata(root: string): ShotMetadata | undefined {
  const path = join(root, ".tohseno", "shot.json");
  if (!existsSync(path)) {
    if (
      existsSync(join(root, ".tohseno")) ||
      existsSync(join(root, "app.manifest.json")) ||
      existsSync(join(root, "continuity.manifest.json"))
    ) {
      throw unsupportedShotState(root);
    }
    return undefined;
  }
  let metadata: ShotMetadata | null;
  try {
    metadata = parseShotMetadata(
      readBoundedJson<unknown>(path, 65_536, "shot metadata"),
    );
  } catch {
    throw unsupportedShotState(root);
  }
  if (metadata === null) throw unsupportedShotState(root);
  let state;
  try {
    state = readLocalShotProtocolState(root);
  } catch {
    throw unsupportedShotState(root);
  }
  if (state === null || state.shotId !== metadata.protocol.shotId) {
    throw unsupportedShotState(root);
  }
  try {
    const manifestValue = readBoundedJson<unknown>(
      join(root, "app.manifest.json"),
      1_048_576,
      "Shot app manifest",
    );
    if (!validateAppManifest(manifestValue).valid) {
      throw unsupportedShotState(root);
    }
    const manifest = manifestValue as AppManifest;
    if (
      manifest.application.id !== metadata.app.bundleId ||
      manifest.application.name !== metadata.app.name ||
      manifest.composition.kernel !== metadata.composition.kernel.id ||
      manifest.composition.template !== metadata.composition.template.id ||
      JSON.stringify(manifest.composition.skills) !==
        JSON.stringify(metadata.composition.skills.map((skill) => skill.id))
    ) {
      throw unsupportedShotState(root);
    }
  } catch {
    throw unsupportedShotState(root);
  }
  return metadata;
}
