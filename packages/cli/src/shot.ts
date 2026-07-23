import { randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { basename, join, resolve } from "node:path";
import type { AgentId } from "./agents.ts";
import { SHOT_SCHEMA_VERSION } from "./constants.ts";
import { CliError, errorMessage } from "./errors.ts";
import {
  assertNoExternalSymlinks,
  copyRegularFile,
  copyTree,
  removeTreeEvenIfReadOnly,
} from "./files.ts";
import { runCaptured } from "./process.ts";
import type { FactoryRelease, PreparedRelease } from "./release.ts";
import { bundleIdForSlug, displayNameForSlug } from "./slug.ts";

export interface ShotMetadata {
  schemaVersion: typeof SHOT_SCHEMA_VERSION;
  slug: string;
  platform: "ios";
  adopted: boolean;
  createdAt: string;
  selectedAgent: AgentId | null;
  baselineAuthor: "factory" | "existing-history";
  factory: {
    releaseId: string;
    cliVersion: string;
    templateVersion: string;
    manifestSchemaVersion: string;
    sourceCommit: string | null;
    sourceDirty: boolean;
    bundleDigest: string;
  };
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
  },
): ShotMetadata {
  return {
    schemaVersion: SHOT_SCHEMA_VERSION,
    slug,
    platform: "ios",
    adopted: options.adopted,
    createdAt: options.now.toISOString(),
    selectedAgent: options.selectedAgent,
    baselineAuthor: options.baselineAuthor,
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

function writeJson(path: string, value: unknown): void {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o644 });
}

function customizeManifest(root: string, slug: string): void {
  const path = join(root, "continuity.manifest.json");
  const value = JSON.parse(readFileSync(path, "utf8")) as Record<string, unknown>;
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
  let source = readFileSync(path, "utf8");
  source = source.replace(/^APP_DISPLAY_NAME\s*=.*$/mu, `APP_DISPLAY_NAME = ${displayNameForSlug(slug)}`);
  source = source.replace(/^APP_BUNDLE_ID\s*=.*$/mu, `APP_BUNDLE_ID = ${bundleIdForSlug(slug)}`);
  writeFileSync(path, source, { mode: 0o644 });
}

function addVerifyScript(root: string): void {
  const path = join(root, "package.json");
  const value = JSON.parse(readFileSync(path, "utf8")) as Record<string, unknown>;
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
  await requireSuccessful(
    [process.execPath, ".tohseno/manifest/cli.ts", "continuity.manifest.json"],
    root,
    "manifest validation",
    environment,
  );
}

async function validateShotWithPinnedTool(root: string, environment?: Record<string, string | undefined>): Promise<void> {
  await requireSuccessful([process.execPath, ".tohseno/verify.ts"], root, "shot verification", environment);
}

export async function createShot(options: {
  slug: string;
  shotsDirectory: string;
  release: PreparedRelease;
  selectedAgent: AgentId | null;
  environment?: Record<string, string | undefined>;
  now?: Date;
}): Promise<CreatedShot> {
  const destination = resolve(options.shotsDirectory, options.slug);
  if (existsSync(destination)) throw new CliError(`target already exists; refusing to overwrite: ${destination}`);
  mkdirSync(options.shotsDirectory, { recursive: true });
  const staging = join(options.shotsDirectory, `.${options.slug}.creating-${process.pid}-${randomUUID()}`);
  mkdirSync(staging, { mode: 0o700 });
  try {
    copyTree(join(options.release.directory, "platforms", "ios", "base"), staging);
    customizeManifest(staging, options.slug);
    customizeXcconfig(staging, options.slug);
    addVerifyScript(staging);
    const provisionalMetadata = metadataFor(options.slug, options.release.metadata, {
      adopted: false,
      selectedAgent: options.selectedAgent,
      baselineAuthor: "factory",
      now: options.now ?? new Date(),
    });
    installPinnedShotFiles(staging, options.release, provisionalMetadata, true);
    assertNoExternalSymlinks(staging);
    await validateManifestWithPinnedTool(staging, options.environment);
    const gitIdentityMissing = await initializeGit(
      staging,
      options.release.metadata.releaseId,
      options.environment,
    );
    await validateShotWithPinnedTool(staging, options.environment);
    if (existsSync(destination)) throw new CliError(`target appeared during creation; refusing to overwrite: ${destination}`);
    renameSync(staging, destination);
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
    const value = JSON.parse(readFileSync(path, "utf8")) as Partial<ShotMetadata>;
    const factory = value.factory;
    if (
      value.schemaVersion !== SHOT_SCHEMA_VERSION ||
      value.platform !== "ios" ||
      typeof value.slug !== "string" ||
      typeof factory !== "object" ||
      factory === null ||
      typeof factory.releaseId !== "string"
    ) {
      return undefined;
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

  const metadata = metadataFor(basename(root), options.release.metadata, {
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
