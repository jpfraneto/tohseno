#!/usr/bin/env bun
import {
  existsSync,
  lstatSync,
  readFileSync,
  readlinkSync,
  realpathSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
// @ts-ignore This factory template is copied beside its pinned manifest directory.
import { formatManifestIssues, validateManifest } from "./manifest/validate.ts";
// @ts-ignore This factory template is copied beside its pinned manifest directory.
import { CONTINUITY_MANIFEST_SCHEMA_VERSION } from "./manifest/types.ts";
import { configuredProductionEndpoint, inspectEndpoint, inspectProduction } from "./runtime/production.ts";

const SHOT_ROOT = realpathSync(resolve(import.meta.dir, ".."));
const REQUIRED_IOS_FILES = [
  "App/AppConfig.swift",
  "App/Identity/BIP39.swift",
  "App/Resources/bip39-english.txt",
  "App/WritingApp.swift",
  "Config/App.xcconfig",
  "Config/Debug.xcconfig",
  "Config/Production.xcconfig",
  "Config/Release.xcconfig",
  "Backend/database.ts",
  "Backend/server.ts",
  "operations/production.json",
  "scripts/validate-production-endpoint.sh",
  "Tests/BIP39Tests.swift",
  "Writing.xcodeproj/project.pbxproj",
  "Writing.xcodeproj/xcshareddata/xcschemes/Writing.xcscheme",
  "continuity.manifest.json",
  "project.yml",
  "site/index.html",
] as const;
const PRIVATE_TRACKED_FILE = /(?:^|\/)(?:MASTER_PROMPT\.md|Local\.xcconfig|DevelopmentEndpoint\.xcconfig|app\.config\.json|\.env(?:\..*)?)$|(?:^|\/)\.tohseno\/(?:data|run)(?:\/|$)|\.(?:p8|p12|pem|pfx|mobileprovision)$/iu;

interface CommandResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

function gitEnvironment(): Record<string, string | undefined> {
  const environment: Record<string, string | undefined> = { ...process.env };
  const exact = new Set([
    "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR", "GIT_NAMESPACE",
    "GIT_QUARANTINE_PATH", "GIT_PREFIX", "GIT_INTERNAL_SUPER_PREFIX",
    "GIT_TEMPLATE_DIR", "GIT_CEILING_DIRECTORIES", "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS",
  ]);
  for (const key of Object.keys(environment)) {
    if (
      exact.has(key) || key.startsWith("GIT_AUTHOR_") || key.startsWith("GIT_COMMITTER_") ||
      key.startsWith("GIT_CONFIG_KEY_") || key.startsWith("GIT_CONFIG_VALUE_")
    ) delete environment[key];
  }
  return environment;
}

async function run(command: readonly string[]): Promise<CommandResult> {
  const child = Bun.spawn([...command], {
    cwd: SHOT_ROOT,
    env: gitEnvironment(),
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  });
  const [exitCode, stdout, stderr] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ]);
  return { exitCode, stdout, stderr };
}

function fail(message: string): never {
  console.error(`✗ ${message}`);
  process.exit(1);
}

function insideShot(path: string): boolean {
  const fromRoot = relative(SHOT_ROOT, path);
  return fromRoot === "" || (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

function validateMetadata(): void {
  const path = join(SHOT_ROOT, ".tohseno", "shot.json");
  const releasePath = join(SHOT_ROOT, ".tohseno", "factory-release.json");
  if (!existsSync(path)) fail("missing .tohseno/shot.json; this project is not a recognized shot");
  let value: unknown;
  try {
    value = JSON.parse(readFileSync(path, "utf8")) as unknown;
  } catch (error) {
    fail(`cannot read .tohseno/shot.json: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) fail("shot metadata must be an object");
  const metadata = value as Record<string, unknown>;
  if (metadata.schemaVersion !== 1 || metadata.platform !== "ios") {
    fail("shot metadata has an unsupported schema or platform");
  }
  const factory = metadata.factory;
  if (typeof factory !== "object" || factory === null || Array.isArray(factory)) fail("shot metadata is missing factory provenance");
  const provenance = factory as Record<string, unknown>;
  if (
    typeof provenance.releaseId !== "string" ||
    typeof provenance.bundleDigest !== "string" ||
    provenance.manifestSchemaVersion !== CONTINUITY_MANIFEST_SCHEMA_VERSION
  ) {
    fail("shot metadata has incomplete or incompatible factory provenance");
  }
  let releaseValue: unknown;
  try {
    releaseValue = JSON.parse(readFileSync(releasePath, "utf8")) as unknown;
  } catch (error) {
    fail(`cannot read .tohseno/factory-release.json: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (typeof releaseValue !== "object" || releaseValue === null || Array.isArray(releaseValue)) {
    fail("factory release record must be an object");
  }
  const release = releaseValue as Record<string, unknown>;
  if (
    release.schemaVersion !== 1 ||
    release.releaseId !== provenance.releaseId ||
    release.bundleDigest !== provenance.bundleDigest ||
    release.manifestSchemaVersion !== provenance.manifestSchemaVersion
  ) {
    fail("shot provenance does not match its pinned factory release record");
  }
}

function validateStructure(): void {
  for (const path of REQUIRED_IOS_FILES) {
    if (!existsSync(join(SHOT_ROOT, path))) fail(`missing required iOS file ${path}`);
  }
}

function validateManifestFile(): void {
  const path = join(SHOT_ROOT, "continuity.manifest.json");
  let value: unknown;
  try {
    value = JSON.parse(readFileSync(path, "utf8")) as unknown;
  } catch (error) {
    fail(`continuity.manifest.json is not valid JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
  const result = validateManifest(value);
  if (result.warnings.length > 0) console.error(formatManifestIssues(result.warnings));
  if (!result.valid) {
    console.error(formatManifestIssues(result.errors));
    fail(`continuity.manifest ${CONTINUITY_MANIFEST_SCHEMA_VERSION} has ${result.errors.length} error${result.errors.length === 1 ? "" : "s"}`);
  }
  console.log(`✓ manifest · continuity.manifest ${CONTINUITY_MANIFEST_SCHEMA_VERSION} · valid`);
}

function validateProductionEndpoint(): void {
  const endpoint = configuredProductionEndpoint(SHOT_ROOT);
  const inspection = inspectEndpoint(endpoint);
  if (!inspection.configured) {
    console.error("WARNING production API endpoint is not configured; production inspection will report a blocker");
  } else if (!inspection.valid) {
    fail(`invalid production API endpoint: ${inspection.issues.join("; ")}`);
  } else {
    console.log("✓ production endpoint · stable HTTPS · no development transport");
  }
  try {
    const production = inspectProduction(SHOT_ROOT);
    console.log(`✓ production contract · inspected · ${production.blockers.length} blocker${production.blockers.length === 1 ? "" : "s"}`);
  } catch (error) {
    fail(`invalid production contract: ${error instanceof Error ? error.message : String(error)}`);
  }
}

async function validateGitAndLinks(): Promise<void> {
  const top = await run(["git", "rev-parse", "--show-toplevel"]);
  if (top.exitCode !== 0 || realpathSync(resolve(top.stdout.trim())) !== SHOT_ROOT) {
    fail("shot is not the root of an independent Git repository");
  }
  const listed = await run(["git", "ls-files", "-z"]);
  if (listed.exitCode !== 0) fail(`cannot inspect tracked files: ${listed.stderr.trim()}`);
  for (const trackedPath of listed.stdout.split("\0").filter(Boolean)) {
    if (PRIVATE_TRACKED_FILE.test(trackedPath)) fail(`private or credential-bearing file is tracked: ${trackedPath}`);
    const path = join(SHOT_ROOT, trackedPath);
    if (!existsSync(path)) continue;
    if (lstatSync(path).isSymbolicLink()) {
      const target = readlinkSync(path);
      const resolved = isAbsolute(target) ? resolve(target) : resolve(dirname(path), target);
      if (!insideShot(resolved)) fail(`tracked symbolic link leaves the shot: ${trackedPath}`);
    }
  }
  for (const ignoredPath of [
    ".tohseno/data/development.sqlite3",
    ".tohseno/run/state.json",
    ".tohseno/run/logs/api.log",
    "Config/DevelopmentEndpoint.xcconfig",
  ]) {
    const ignored = await run(["git", "check-ignore", "--quiet", "--no-index", ignoredPath]);
    if (ignored.exitCode !== 0) fail(`runtime artifact is not gitignored: ${ignoredPath}`);
  }
  console.log("✓ structure · independent Git repository · no tracked private files or external links");
}

validateMetadata();
validateStructure();
validateManifestFile();
validateProductionEndpoint();
await validateGitAndLinks();
