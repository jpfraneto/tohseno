#!/usr/bin/env bun
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  readdirSync,
  readlinkSync,
  realpathSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
// @ts-ignore This factory template is copied beside its pinned manifest directory.
import { APP_MANIFEST_SCHEMA_VERSION, validateAppManifest } from "./manifest/app.ts";
import {
  type CanonicalShotMetadata,
  MachineError,
  PRE_RELEASE_UNSUPPORTED,
  readBoundedRegularFile,
  readBoundedUtf8,
  readCanonicalShotMetadata,
  runCaptured,
  safeEnvironment,
  validateCanonicalLocalShotState,
  validateCanonicalShotMetadata,
} from "./runtime/shared.ts";

function resolvedShotRoot(): string {
  let candidate = resolve(process.cwd());
  while (true) {
    if (existsSync(join(candidate, ".tohseno", "shot.json"))) {
      return realpathSync(candidate);
    }
    const parent = dirname(candidate);
    if (parent === candidate) break;
    candidate = parent;
  }
  return realpathSync(resolve(import.meta.dir, ".."));
}

const SHOT_ROOT = resolvedShotRoot();
let SHOT_METADATA: CanonicalShotMetadata | null = null;
let PINNED_RELEASE: Record<string, unknown> | null = null;
const REQUIRED_SHOT_FILES = [
  "App/ShotApp.swift",
  "App/Design/DesignTokens.swift",
  "Config/App.xcconfig",
  "Config/Debug.xcconfig",
  "Config/Release.xcconfig",
  "Tests/KernelTests.swift",
  "Shot.xcodeproj/project.pbxproj",
  "Shot.xcodeproj/xcshareddata/xcschemes/Shot.xcscheme",
  "app.manifest.json",
  "SHOT.md",
  "DONE.md",
  "tohseno.skills.json",
  "tohseno.skills.lock",
  ".tohseno/shot-plan.json",
  "project.yml",
  "site/index.html",
] as const;
const PRIVATE_TRACKED_FILE = /(?:^|\/)(?:(?:MASTER_(?:EVOLUTIONARY_)?|TOHSENO_EVOLUTION_)PROMPT\.md|Local\.xcconfig|\.env(?:\..*)?)$|(?:^|\/)\.tohseno\/(?:artifacts|data|provenance|run)(?:\/|$)|\.(?:p8|p12|pem|pfx|mobileprovision)$/iu;
const MAX_JSON_BYTES = 1_048_576;
const MAX_INTENTION_BYTES = 1_048_576;
const MAX_REFERENCE_BYTES = 12 * 1_048_576;
const MAX_REFERENCES = 8;
const MAX_WORKTREE_FILE_BYTES = 64 * 1_048_576;
const MAX_WORKTREE_BYTES = 512 * 1_048_576;
const MAX_WORKTREE_ENTRIES = 20_000;
const MIN_EMBEDDED_INTENTION_BYTES = 24;
const PRIVATE_LOCAL_DIRECTORY =
  /(?:^|\/)\.tohseno\/(?:artifacts|data|provenance|run)(?:\/|$)/u;
const GENERATED_DIRECTORY =
  /(?:^|\/)(?:node_modules|DerivedData|build|\.build)(?:\/|$)/u;

interface CommandResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

interface PrivateLeakMaterial {
  intentionSha256: string | null;
  intentionNeedle: Buffer | null;
  referenceHashes: Set<string>;
}

async function run(command: readonly string[]): Promise<CommandResult> {
  const environment = {
    ...safeEnvironment(),
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_TERMINAL_PROMPT: "0",
    GIT_OPTIONAL_LOCKS: "0",
  };
  const hardened = command[0] === "git"
    ? [
      "git",
      "-c", "core.fsmonitor=false",
      "-c", "core.hooksPath=/dev/null",
      "-c", "core.excludesFile=/dev/null",
      ...command.slice(1),
    ]
    : command;
  try {
    return await runCaptured(hardened, {
      cwd: SHOT_ROOT,
      environment,
    });
  } catch (error) {
    const detail = error instanceof MachineError
      ? error.message
      : "repository inspection subprocess failed";
    fail(detail);
  }
}

function fail(message: string): never {
  console.error(`✗ ${message}`);
  process.exit(1);
}

function failObsoleteState(message: string): never {
  fail(`${PRE_RELEASE_UNSUPPORTED}: ${message}`);
}

function formatIssues(
  issues: ReadonlyArray<{
    severity: string;
    path: string;
    code: string;
    message: string;
  }>,
): string {
  return issues
    .map(
      (issue) =>
        `${issue.severity.toUpperCase()} ${issue.path} [${issue.code}]: ${issue.message}`,
    )
    .join("\n");
}

function insideShot(path: string): boolean {
  const fromRoot = relative(SHOT_ROOT, path);
  return fromRoot === "" || (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

function readJsonFile(path: string, label: string): unknown {
  try {
    return JSON.parse(readBoundedUtf8(path, MAX_JSON_BYTES, label)) as unknown;
  } catch {
    fail(`${label} must be valid JSON in a single-link regular file no larger than ${MAX_JSON_BYTES} bytes`);
  }
}

function readCanonicalStateFile(path: string, label: string): unknown {
  try {
    return JSON.parse(readBoundedUtf8(path, MAX_JSON_BYTES, label)) as unknown;
  } catch {
    failObsoleteState(`${label} is missing or unreadable`);
  }
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

function validateLocalProtocolState(
  metadata: CanonicalShotMetadata,
): void {
  const pointer = metadata.protocol;
  const stateValue = readJsonFile(
    join(SHOT_ROOT, ".tohseno", "protocol-state.json"),
    "local Shot protocol state",
  );
  let state;
  try {
    state = validateCanonicalLocalShotState(stateValue, pointer.shotId);
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
  }
  console.log(
    `✓ local Shot metadata · ${state.lifecycle} · evolution ${state.evolution} · no public record claimed`,
  );
}

function validateMetadata(): void {
  const releasePath = join(SHOT_ROOT, ".tohseno", "factory-release.json");
  let metadata: CanonicalShotMetadata;
  try {
    metadata = readCanonicalShotMetadata(SHOT_ROOT);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    fail(
      message.includes(PRE_RELEASE_UNSUPPORTED)
        ? message
        : `${PRE_RELEASE_UNSUPPORTED}: canonical Shot metadata is unreadable`,
    );
  }
  SHOT_METADATA = metadata;
  validateLocalProtocolState(metadata);
  const releaseValue = readCanonicalStateFile(
    releasePath,
    "factory release record",
  );
  if (typeof releaseValue !== "object" || releaseValue === null || Array.isArray(releaseValue)) {
    failObsoleteState("factory release record must be an object");
  }
  const release = releaseValue as Record<string, unknown>;
  const source =
    typeof release.source === "object" &&
      release.source !== null &&
      !Array.isArray(release.source)
      ? release.source as Record<string, unknown>
      : null;
  if (
    !exactKeys(release, [
      "schemaVersion",
      "releaseId",
      "cliVersion",
      "templateVersion",
      "manifestSchemaVersion",
      "platform",
      "source",
      "bundleDigest",
      "files",
    ]) ||
    release.schemaVersion !== 1 ||
    release.cliVersion !== "0.5.0" ||
    release.templateVersion !== "ios-kernel-v1" ||
    release.manifestSchemaVersion !== APP_MANIFEST_SCHEMA_VERSION ||
    release.platform !== "ios" ||
    typeof release.releaseId !== "string" ||
    !/^(?:git-[0-9a-f]{40}(?:-dirty)?-[0-9a-f]{16}|content-[0-9a-f]{32})$/u
      .test(release.releaseId) ||
    typeof release.bundleDigest !== "string" ||
    !/^[a-f0-9]{64}$/u.test(release.bundleDigest) ||
    source === null ||
    !exactKeys(source, ["kind", "commit", "dirty"]) ||
    (source.kind !== "git" && source.kind !== "content") ||
    typeof source.dirty !== "boolean" ||
    !Array.isArray(release.files) ||
    release.files.length > 4_096
  ) {
    failObsoleteState("factory release record is not canonical 0.5");
  }
  if (
    (
      source.kind === "git" &&
      (
        typeof source.commit !== "string" ||
        !/^[0-9a-f]{40}$/u.test(source.commit)
      )
    ) ||
    (
      source.kind === "content" &&
      (source.commit !== null || source.dirty)
    )
  ) {
    failObsoleteState("factory release source provenance is invalid");
  }
  PINNED_RELEASE = release;
  const provenance = metadata.factory;
  if (
    release.releaseId !== provenance.releaseId ||
    release.cliVersion !== provenance.cliVersion ||
    release.templateVersion !== provenance.templateVersion ||
    release.manifestSchemaVersion !== provenance.manifestSchemaVersion ||
    release.bundleDigest !== provenance.bundleDigest ||
    source.commit !== provenance.sourceCommit ||
    source.dirty !== provenance.sourceDirty
  ) {
    failObsoleteState(
      "Shot provenance does not match its pinned factory release record",
    );
  }
  const files = releaseFiles();
  const digest = releaseBundleDigest(files);
  const expectedReleaseId = source.kind === "git"
    ? `git-${source.commit as string}${
      source.dirty ? "-dirty" : ""
    }-${digest.slice(0, 16)}`
    : `content-${digest.slice(0, 32)}`;
  if (
    digest !== release.bundleDigest ||
    expectedReleaseId !== release.releaseId
  ) {
    failObsoleteState(
      "factory release identity does not match its canonical file inventory",
    );
  }
}

function releaseFiles(): Array<{
  path: string;
  sha256: string;
  size: number;
  executable: boolean;
}> {
  const files = PINNED_RELEASE?.files;
  if (!Array.isArray(files)) {
    failObsoleteState("factory release record has no canonical file inventory");
  }
  const parsed = files.map((value) => {
    if (
      typeof value !== "object" ||
      value === null ||
      Array.isArray(value)
    ) {
      failObsoleteState("factory release record has an invalid file inventory");
    }
    const file = value as Record<string, unknown>;
    if (
      !exactKeys(file, ["path", "sha256", "size", "executable"]) ||
      typeof file.path !== "string" ||
      isAbsolute(file.path) ||
      file.path.split(/[\\/]/u).some((part) =>
        part === "" || part === "." || part === ".."
      ) ||
      typeof file.sha256 !== "string" ||
      !/^[a-f0-9]{64}$/u.test(file.sha256) ||
      !Number.isSafeInteger(file.size) ||
      (file.size as number) < 0 ||
      typeof file.executable !== "boolean"
    ) {
      failObsoleteState("factory release record has an invalid file inventory");
    }
    return file as {
      path: string;
      sha256: string;
      size: number;
      executable: boolean;
    };
  });
  if (
    new Set(parsed.map((file) => file.path)).size !== parsed.length ||
    parsed.some(
      (file, index) =>
        index > 0 && parsed[index - 1]!.path.localeCompare(file.path) >= 0,
    )
  ) {
    failObsoleteState(
      "factory release file inventory must be unique and sorted",
    );
  }
  return parsed;
}

function releaseBundleDigest(
  files: ReturnType<typeof releaseFiles>,
): string {
  const hash = createHash("sha256");
  for (const file of files) {
    hash.update(file.path);
    hash.update("\0");
    hash.update(file.sha256);
    hash.update("\0");
    hash.update(String(file.size));
    hash.update("\0");
    hash.update(file.executable ? "x" : "-");
    hash.update("\0");
  }
  return hash.digest("hex");
}

function releaseSubtreeDigest(prefix: string): string {
  const files = releaseFiles()
    .filter((file) => file.path.startsWith(prefix))
    .map((file) => ({
      ...file,
      path: file.path.slice(prefix.length),
    }))
    .sort((left, right) => left.path.localeCompare(right.path));
  if (files.length === 0) fail(`factory release is missing catalog subtree ${prefix}`);
  const hash = createHash("sha256");
  for (const file of files) {
    hash.update(file.path);
    hash.update("\0");
    hash.update(file.sha256);
    hash.update("\0");
  }
  return hash.digest("hex");
}

function appLock(): {
  factoryReleaseId: string;
  kernel: { id: string; version: string; digest: string };
  template: { id: string; version: string; digest: string };
  skills: Array<{ id: string; version: string; digest: string }>;
  resolvedOrder: string[];
  files: Array<{ path: string; owner: string; sha256: string }>;
} {
  const value = readJsonFile(
    join(SHOT_ROOT, "tohseno.skills.lock"),
    "app skill lock",
  );
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    fail("app skill lock must be an object");
  }
  const lock = value as Record<string, unknown>;
  const kernel = lock.kernel;
  const template = lock.template;
  if (
    !exactKeys(lock, [
      "schemaVersion",
      "factoryReleaseId",
      "kernel",
      "template",
      "skills",
      "resolvedOrder",
      "files",
    ]) ||
    lock.schemaVersion !== 1 ||
    typeof lock.factoryReleaseId !== "string" ||
    typeof kernel !== "object" ||
    kernel === null ||
    Array.isArray(kernel) ||
    typeof template !== "object" ||
    template === null ||
    Array.isArray(template) ||
    !Array.isArray(lock.skills) ||
    !Array.isArray(lock.resolvedOrder) ||
    !Array.isArray(lock.files) ||
    !lock.resolvedOrder.every((value) => typeof value === "string")
  ) {
    fail("app skill lock has an unsupported shape");
  }
  const validateItem = (item: unknown, label: string): {
    id: string;
    version: string;
    digest: string;
  } => {
    if (typeof item !== "object" || item === null || Array.isArray(item)) {
      fail(`${label} lock entry must be an object`);
    }
    const entry = item as Record<string, unknown>;
    if (
      !exactKeys(entry, ["id", "version", "digest"]) ||
      typeof entry.id !== "string" ||
      !/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(entry.id) ||
      typeof entry.version !== "string" ||
      !/^[0-9]+\.[0-9]+\.[0-9]+$/u.test(entry.version) ||
      typeof entry.digest !== "string" ||
      !/^[a-f0-9]{64}$/u.test(entry.digest)
    ) {
      fail(`${label} lock entry is invalid`);
    }
    return entry as { id: string; version: string; digest: string };
  };
  const skills = lock.skills.map((item, index) =>
    validateItem(item, `skill ${index + 1}`));
  const files = lock.files.map((item, index) => {
    if (typeof item !== "object" || item === null || Array.isArray(item)) {
      fail(`locked composition file ${index + 1} must be an object`);
    }
    const file = item as Record<string, unknown>;
    if (
      !exactKeys(file, ["path", "owner", "sha256"]) ||
      typeof file.path !== "string" ||
      isAbsolute(file.path) ||
      file.path.split(/[\\/]/u).some((part) =>
        part === "" || part === "." || part === ".."
      ) ||
      typeof file.owner !== "string" ||
      typeof file.sha256 !== "string" ||
      !/^[a-f0-9]{64}$/u.test(file.sha256)
    ) {
      fail(`locked composition file ${index + 1} is invalid`);
    }
    return file as { path: string; owner: string; sha256: string };
  });
  if (new Set(skills.map((skill) => skill.id)).size !== skills.length) {
    fail("app skill lock contains duplicate skill ids");
  }
  if (
    !lock.resolvedOrder.every((value) =>
      /^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(value)
    ) ||
    new Set(lock.resolvedOrder).size !== lock.resolvedOrder.length
  ) {
    fail("app skill lock resolvedOrder must contain unique app skill ids");
  }
  if (new Set(files.map((file) => file.path)).size !== files.length) {
    fail("app skill lock contains duplicate composition file paths");
  }
  return {
    factoryReleaseId: lock.factoryReleaseId,
    kernel: validateItem(kernel, "kernel"),
    template: validateItem(template, "template"),
    skills,
    resolvedOrder: lock.resolvedOrder as string[],
    files,
  };
}

function validateComposition(): void {
  const declaredValue = readJsonFile(
    join(SHOT_ROOT, "tohseno.skills.json"),
    "declared app composition",
  );
  if (
    typeof declaredValue !== "object" ||
    declaredValue === null ||
    Array.isArray(declaredValue)
  ) {
    fail("declared app composition must be an object");
  }
  const declared = declaredValue as Record<string, unknown>;
  if (
    !exactKeys(declared, ["schemaVersion", "template", "skills"]) ||
    declared.schemaVersion !== 1 ||
    typeof declared.template !== "string" ||
    !/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(declared.template) ||
    !Array.isArray(declared.skills) ||
    !declared.skills.every((skill) =>
      typeof skill === "string" &&
      /^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(skill)
    ) ||
    new Set(declared.skills).size !== declared.skills.length
  ) {
    fail("declared app composition is not canonical");
  }
  const lock = appLock();
  const plan = readJsonFile(
    join(SHOT_ROOT, ".tohseno", "shot-plan.json"),
    "sanitized shot plan",
  );
  const manifest = readJsonFile(
    join(SHOT_ROOT, "app.manifest.json"),
    "app.manifest.json",
  );
  if (
    typeof plan !== "object" ||
    plan === null ||
    Array.isArray(plan) ||
    typeof manifest !== "object" ||
    manifest === null ||
    Array.isArray(manifest)
  ) {
    fail("Shot plan and app manifest must be objects");
  }
  const planRecord = plan as Record<string, unknown>;
  const manifestRecord = manifest as Record<string, unknown>;
  const metadata = SHOT_METADATA;
  if (metadata === null) fail("canonical Shot metadata is unavailable");
  const metadataComposition = metadata.composition;
  const metadataApp = metadata.app;
  const manifestComposition = manifestRecord.composition;
  const manifestApplication = manifestRecord.application;
  const planApp = planRecord.app;
  const planSkills = Array.isArray(planRecord.skills)
    ? planRecord.skills.map((value) =>
        typeof value === "object" && value !== null && !Array.isArray(value)
          ? (value as Record<string, unknown>).id
          : null)
    : null;
  const releaseId = PINNED_RELEASE?.releaseId;
  if (
    declared.template !== lock.template.id ||
    JSON.stringify(declared.skills) !==
      JSON.stringify(lock.skills.map((skill) => skill.id)) ||
    lock.factoryReleaseId !== releaseId ||
    JSON.stringify(lock.resolvedOrder) !==
      JSON.stringify(lock.skills.map((skill) => skill.id)) ||
    createHash("sha256").update(JSON.stringify(plan)).digest("hex") !==
      metadata.sanitizedPlanDigest ||
    JSON.stringify(metadataComposition.kernel) !==
      JSON.stringify(lock.kernel) ||
    JSON.stringify(metadataComposition.template) !==
      JSON.stringify(lock.template) ||
    JSON.stringify(metadataComposition.skills) !==
      JSON.stringify(lock.skills) ||
    typeof planApp !== "object" ||
    planApp === null ||
    Array.isArray(planApp) ||
    (planApp as Record<string, unknown>).name !==
      metadataApp.name ||
    (planApp as Record<string, unknown>).bundleId !==
      metadataApp.bundleId ||
    planRecord.template !== lock.template.id ||
    JSON.stringify(planSkills) !==
      JSON.stringify(lock.skills.map((skill) => skill.id)) ||
    typeof manifestComposition !== "object" ||
    manifestComposition === null ||
    Array.isArray(manifestComposition) ||
    (manifestComposition as Record<string, unknown>).kernel !== lock.kernel.id ||
    (manifestComposition as Record<string, unknown>).template !== lock.template.id ||
    JSON.stringify((manifestComposition as Record<string, unknown>).skills) !==
      JSON.stringify(lock.skills.map((skill) => skill.id)) ||
    typeof manifestApplication !== "object" ||
    manifestApplication === null ||
    Array.isArray(manifestApplication) ||
    (manifestApplication as Record<string, unknown>).id !==
      (planApp as Record<string, unknown>).bundleId ||
    (manifestApplication as Record<string, unknown>).name !==
      (planApp as Record<string, unknown>).name
  ) {
    fail("Shot plan, app manifest, metadata, and exact lock do not agree");
  }
  const expectedDigests = [
    [
      lock.kernel,
      `catalog/kernels/${lock.kernel.id}/`,
    ],
    [
      lock.template,
      `catalog/templates/${lock.template.id}/`,
    ],
    ...lock.skills.map((skill) => [
      skill,
      `catalog/skills/${skill.id}/`,
    ] as const),
  ] as const;
  for (const [item, prefix] of expectedDigests) {
    if (releaseSubtreeDigest(prefix) !== item.digest) {
      fail(`${item.id} content digest does not match the pinned factory release`);
    }
  }
  for (const file of lock.files) {
    const path = resolve(SHOT_ROOT, file.path);
    if (!insideShot(path) || !existsSync(path)) {
      fail(`locked composition file is missing: ${file.path}`);
    }
    if (
      sha256File(path, MAX_WORKTREE_FILE_BYTES, "locked composition file") !==
        file.sha256
    ) {
      fail(`locked composition file changed: ${file.path} (${file.owner})`);
    }
  }
  for (const skill of lock.skills) {
    const descriptorValue = readJsonFile(
      join(
        SHOT_ROOT,
        ".tohseno",
        "app-skills",
        skill.id,
        "skill.json",
      ),
      `${skill.id} app skill descriptor`,
    );
    if (
      typeof descriptorValue !== "object" ||
      descriptorValue === null ||
      Array.isArray(descriptorValue)
    ) {
      fail(`${skill.id} app skill descriptor must be an object`);
    }
    const descriptor = descriptorValue as Record<string, unknown>;
    if (
      descriptor.schemaVersion !== 1 ||
      descriptor.id !== skill.id ||
      descriptor.version !== skill.version ||
      !Array.isArray(descriptor.acceptanceChecks)
    ) {
      fail(`${skill.id} app skill descriptor does not match the lock`);
    }
    for (const checkValue of descriptor.acceptanceChecks) {
      if (
        typeof checkValue !== "object" ||
        checkValue === null ||
        Array.isArray(checkValue)
      ) {
        fail(`${skill.id} has invalid acceptance metadata`);
      }
      const check = checkValue as Record<string, unknown>;
      if (
        check.type !== "file" ||
        typeof check.path !== "string" ||
        isAbsolute(check.path) ||
        check.path.split(/[\\/]/u).some((part) =>
          part === "" || part === "." || part === ".."
        ) ||
        !existsSync(join(SHOT_ROOT, check.path))
      ) {
        fail(`${skill.id} acceptance check failed`);
      }
    }
  }
  console.log(
    `✓ skills · ${lock.skills.length}/${lock.skills.length} acceptance sets · exact lock and digests valid`,
  );
}

function sha256File(path: string, maximumBytes: number, label: string): string {
  return createHash("sha256")
    .update(readBoundedRegularFile(path, maximumBytes, label))
    .digest("hex");
}

function sha256Text(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function privateProvenanceFile(
  root: string,
  relativePath: string,
  maximumBytes: number,
): { path: string; bytes: number } {
  if (
    relativePath === "" ||
    isAbsolute(relativePath) ||
    relativePath.split(/[\\/]/u).some((part) => part === "" || part === "." || part === "..")
  ) {
    fail("private creation provenance contains an unsafe relative path");
  }
  const path = resolve(root, relativePath);
  if (!insideShot(path) || relative(root, path).startsWith(`..${sep}`)) {
    fail("private creation provenance leaves its local directory");
  }
  if (!existsSync(path)) fail("private creation provenance is missing a recorded input");
  const details = lstatSync(path);
  if (
    details.isSymbolicLink() ||
    !details.isFile() ||
    details.nlink !== 1 ||
    (details.mode & 0o077) !== 0 ||
    details.size > maximumBytes
  ) {
    fail("private creation provenance input is not a regular file");
  }
  const canonical = realpathSync(path);
  if (!insideShot(canonical) || !insideShot(root) || !insideRoot(root, canonical)) {
    fail("private creation provenance input leaves its local directory");
  }
  return { path: canonical, bytes: details.size };
}

function insideRoot(root: string, candidate: string): boolean {
  const fromRoot = relative(root, candidate);
  return fromRoot === "" ||
    (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

function validatePrivateCreationProvenance(): PrivateLeakMaterial | null {
  const requestedRoot = join(SHOT_ROOT, ".tohseno", "provenance");
  const path = join(requestedRoot, "provenance.json");
  if (!existsSync(path)) {
    console.error("WARNING local creation inputs are unavailable; Git intentionally does not carry private provenance");
    return null;
  }
  if (!existsSync(requestedRoot)) {
    fail("private creation provenance directory is missing");
  }
  const rootDetails = lstatSync(requestedRoot);
  if (
    rootDetails.isSymbolicLink() ||
    !rootDetails.isDirectory() ||
    (rootDetails.mode & 0o777) !== 0o700
  ) {
    fail("private creation provenance directory is not a real directory");
  }
  const root = realpathSync(requestedRoot);
  if (!insideShot(root) || root === SHOT_ROOT) {
    fail("private creation provenance directory leaves the shot");
  }
  const value = readJsonFile(path, "private creation provenance record");
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    fail("private creation provenance record must be an object");
  }
  const provenance = value as Record<string, unknown>;
  const privateFactory =
    typeof provenance.factory === "object" &&
      provenance.factory !== null &&
      !Array.isArray(provenance.factory)
      ? provenance.factory as Record<string, unknown>
      : null;
  const privateOptions =
    typeof provenance.options === "object" &&
      provenance.options !== null &&
      !Array.isArray(provenance.options)
      ? provenance.options as Record<string, unknown>
      : null;
  if (
    !exactKeys(provenance, [
      "schemaVersion",
      "createdAt",
      "door",
      "factory",
      "intention",
      "references",
      "inputDigest",
      "options",
      "events",
    ]) ||
    provenance.schemaVersion !== 1 ||
    (provenance.door !== "cli" && provenance.door !== "studio") ||
    typeof provenance.createdAt !== "string" ||
    typeof provenance.inputDigest !== "string" ||
    !/^[a-f0-9]{64}$/u.test(provenance.inputDigest) ||
    !Array.isArray(provenance.references) ||
    provenance.events !== "events.jsonl" ||
    privateFactory === null ||
    !exactKeys(privateFactory, [
      "releaseId",
      "cliVersion",
      "templateVersion",
      "manifestSchemaVersion",
      "bundleDigest",
    ]) ||
    privateOptions === null ||
    !exactKeys(privateOptions, [
      "selectedAgent",
      "agentMode",
      "verifyAfterAgent",
      "runAfterCreate",
    ])
  ) {
    fail("private creation provenance record is not canonical");
  }
  const metadata = SHOT_METADATA;
  if (metadata === null) fail("canonical Shot metadata is unavailable");
  const creationSummary = metadata.creation;
  const factorySummary = metadata.factory;
  if (
    creationSummary.door !== provenance.door ||
    creationSummary.inputDigest !== provenance.inputDigest ||
    metadata.createdAt !== provenance.createdAt ||
    privateFactory.releaseId !== factorySummary.releaseId ||
    privateFactory.cliVersion !== factorySummary.cliVersion ||
    privateFactory.templateVersion !== factorySummary.templateVersion ||
    privateFactory.manifestSchemaVersion !==
      factorySummary.manifestSchemaVersion ||
    privateFactory.bundleDigest !== factorySummary.bundleDigest ||
    JSON.stringify(privateOptions) !== JSON.stringify(creationSummary.options)
  ) {
    fail("private creation provenance does not match the immutable shot summary");
  }
  const intention = provenance.intention;
  let intentionSha256: string | null = null;
  let intentionNeedle: Buffer | null = null;
  if (intention !== null) {
    if (typeof intention !== "object" || Array.isArray(intention)) {
      fail("private creation intention record must be an object or null");
    }
    const record = intention as Record<string, unknown>;
    if (
      !exactKeys(record, [
        "path",
        "sha256",
        "bytes",
        "components",
      ]) ||
      !Array.isArray(record.components)
    ) {
      fail("private creation intention record is not canonical");
    }
    const intentionFile = typeof record.path === "string"
      ? privateProvenanceFile(root, record.path, MAX_INTENTION_BYTES)
      : null;
    const intentionBytes = intentionFile === null
      ? null
      : readBoundedRegularFile(
        intentionFile.path,
        MAX_INTENTION_BYTES,
        "private creation intention",
      );
    if (
      intentionFile === null ||
      intentionBytes === null ||
      typeof record.sha256 !== "string" ||
      !/^[0-9a-f]{64}$/u.test(record.sha256) ||
      !Number.isSafeInteger(record.bytes) ||
      (record.bytes as number) < 1 ||
      intentionFile.bytes !== record.bytes ||
      createHash("sha256").update(intentionBytes).digest("hex") !== record.sha256
    ) {
      fail("private creation intention checksum does not match");
    }
    for (const componentValue of record.components as unknown[]) {
      if (
        typeof componentValue !== "object" ||
        componentValue === null ||
        Array.isArray(componentValue)
      ) {
        fail("private creation intention component is not canonical");
      }
      const component = componentValue as Record<string, unknown>;
      const keys = Object.hasOwn(component, "originalName")
        ? [
            "kind",
            "originalName",
            "sha256",
            "bytes",
            "byteOffset",
            "byteLength",
          ]
        : ["kind", "sha256", "bytes", "byteOffset", "byteLength"];
      if (
        !exactKeys(component, keys) ||
        (component.kind !== "textarea" && component.kind !== "markdown") ||
        (
          Object.hasOwn(component, "originalName") &&
          (
            component.kind !== "markdown" ||
            typeof component.originalName !== "string" ||
            component.originalName.length < 1
          )
        ) ||
        typeof component.sha256 !== "string" ||
        !/^[a-f0-9]{64}$/u.test(component.sha256) ||
        !Number.isSafeInteger(component.bytes) ||
        !Number.isSafeInteger(component.byteOffset) ||
        !Number.isSafeInteger(component.byteLength) ||
        (component.bytes as number) < 1 ||
        component.bytes !== component.byteLength ||
        (component.byteOffset as number) < 0 ||
        (component.byteLength as number) < 1 ||
        (component.byteOffset as number) +
          (component.byteLength as number) >
          intentionBytes.length
      ) {
        fail("private creation intention component is not canonical");
      }
      const componentBytes = intentionBytes.subarray(
        component.byteOffset as number,
        (component.byteOffset as number) + (component.byteLength as number),
      );
      if (
        createHash("sha256").update(componentBytes).digest("hex") !==
          component.sha256
      ) {
        fail("private creation intention component checksum does not match");
      }
    }
    intentionSha256 = record.sha256;
    let end = intentionBytes.length;
    while (
      end > 0 &&
      (
        intentionBytes[end - 1] === 0x09 ||
        intentionBytes[end - 1] === 0x0a ||
        intentionBytes[end - 1] === 0x0d ||
        intentionBytes[end - 1] === 0x20
      )
    ) {
      end -= 1;
    }
    if (end >= MIN_EMBEDDED_INTENTION_BYTES) {
      intentionNeedle = intentionBytes.subarray(0, end);
    }
  }
  if (!Array.isArray(provenance.references)) {
    fail("private creation references must be an array");
  }
  if (provenance.references.length > MAX_REFERENCES) {
    fail(`private creation references exceed the ${MAX_REFERENCES}-file limit`);
  }
  const referenceHashes: string[] = [];
  for (const reference of provenance.references) {
    if (typeof reference !== "object" || reference === null || Array.isArray(reference)) {
      fail("private creation reference record must be an object");
    }
    const record = reference as Record<string, unknown>;
    if (
      !exactKeys(record, [
        "path",
        "originalName",
        "mediaType",
        "bytes",
        "sha256",
      ]) ||
      typeof record.originalName !== "string" ||
      typeof record.mediaType !== "string"
    ) {
      fail("private creation reference record is not canonical");
    }
    const referenceFile = typeof record.path === "string"
      ? privateProvenanceFile(root, record.path, MAX_REFERENCE_BYTES)
      : null;
    if (
      referenceFile === null ||
      typeof record.sha256 !== "string" ||
      !/^[0-9a-f]{64}$/u.test(record.sha256) ||
      !Number.isSafeInteger(record.bytes) ||
      (record.bytes as number) < 1 ||
      referenceFile.bytes !== record.bytes ||
      sha256File(
        referenceFile.path,
        MAX_REFERENCE_BYTES,
        "private creation reference",
      ) !== record.sha256
    ) {
      fail("private creation reference checksum does not match");
    }
    referenceHashes.push(record.sha256);
  }
  const expectedInputDigest = sha256Text(JSON.stringify({
    intentionSha256,
    references: referenceHashes,
  }));
  if (
    provenance.inputDigest !== expectedInputDigest ||
    creationSummary.hasIntention !== (intention !== null) ||
    creationSummary.referenceCount !== referenceHashes.length
  ) {
    fail("private creation input digest does not match its normalized inputs");
  }
  console.log("✓ provenance · local private inputs · checksums valid");
  return {
    intentionSha256,
    intentionNeedle,
    referenceHashes: new Set(referenceHashes),
  };
}

function normalizedRelativePath(path: string): string {
  return relative(SHOT_ROOT, path).split(sep).join("/");
}

function privateLocalPath(path: string): boolean {
  const relativePath = normalizedRelativePath(path);
  return relativePath !== "" && PRIVATE_TRACKED_FILE.test(relativePath);
}

function validateWorktreePrivacy(material: PrivateLeakMaterial | null): void {
  let entriesSeen = 0;
  let bytesRead = 0;

  function visit(directory: string): void {
    let canonicalDirectory: string;
    let entries;
    try {
      const details = lstatSync(directory);
      canonicalDirectory = realpathSync(directory);
      if (
        details.isSymbolicLink() ||
        !details.isDirectory() ||
        !insideShot(canonicalDirectory) ||
        privateLocalPath(canonicalDirectory)
      ) {
        fail("shot worktree contains an unsafe directory boundary");
      }
      entries = readdirSync(directory, { withFileTypes: true });
    } catch {
      fail("shot worktree cannot be inspected safely");
    }

    for (const entry of entries) {
      entriesSeen += 1;
      if (entriesSeen > MAX_WORKTREE_ENTRIES) {
        fail(`shot worktree exceeds the ${MAX_WORKTREE_ENTRIES}-entry verification limit`);
      }
      const path = join(directory, entry.name);
      const relativePath = normalizedRelativePath(path);
      if (relativePath === ".git" || relativePath.startsWith(".git/")) continue;

      let details;
      try {
        details = lstatSync(path);
      } catch {
        fail("shot worktree changed while it was being inspected");
      }
      if (details.isSymbolicLink()) {
        let target: string;
        try {
          target = realpathSync(path);
        } catch {
          fail("shot worktree contains an unsafe symbolic link");
        }
        if (!insideShot(target) || privateLocalPath(target)) {
          fail("shot worktree contains a symbolic link across a private boundary");
        }
        continue;
      }
      if (details.isDirectory()) {
        if (
          PRIVATE_LOCAL_DIRECTORY.test(relativePath) ||
          GENERATED_DIRECTORY.test(relativePath)
        ) {
          continue;
        }
        visit(path);
        continue;
      }
      if (!details.isFile()) {
        fail("shot worktree contains an unsupported filesystem entry");
      }
      if (PRIVATE_TRACKED_FILE.test(relativePath)) continue;
      if (details.size > MAX_WORKTREE_FILE_BYTES) {
        fail(`shot worktree contains a file larger than ${MAX_WORKTREE_FILE_BYTES} bytes`);
      }

      let bytes: Buffer;
      try {
        bytes = readBoundedRegularFile(
          path,
          MAX_WORKTREE_FILE_BYTES,
          "shot worktree file",
        );
      } catch {
        fail("shot worktree contains an unsafe or oversized file");
      }
      bytesRead += bytes.length;
      if (bytesRead > MAX_WORKTREE_BYTES) {
        fail(`shot worktree exceeds the ${MAX_WORKTREE_BYTES}-byte verification limit`);
      }
      if (material === null) continue;

      const digest = createHash("sha256").update(bytes).digest("hex");
      if (
        digest === material.intentionSha256 ||
        material.referenceHashes.has(digest) ||
        (
          material.intentionNeedle !== null &&
          bytes.indexOf(material.intentionNeedle) !== -1
        )
      ) {
        fail("private creation input appears outside its protected local directory");
      }
    }
  }

  visit(SHOT_ROOT);
  console.log("✓ privacy · worktree contains no copied private creation input or unsafe links");
}

function validateStructure(): void {
  for (const path of REQUIRED_SHOT_FILES) {
    if (!existsSync(join(SHOT_ROOT, path))) {
      fail(`missing required canonical Shot file ${path}`);
    }
  }
}

function validateManifestFile(): void {
  const path = join(SHOT_ROOT, "app.manifest.json");
  const value = readJsonFile(path, "app.manifest.json");
  const result = validateAppManifest(value);
  if (result.warnings.length > 0) {
    console.error(formatIssues(result.warnings));
  }
  if (!result.valid) {
    console.error(formatIssues(result.errors));
    fail(
      `app.manifest ${APP_MANIFEST_SCHEMA_VERSION} has ${result.errors.length} error${result.errors.length === 1 ? "" : "s"}`,
    );
  }
  console.log(
    `✓ manifest · app.manifest ${APP_MANIFEST_SCHEMA_VERSION} · valid`,
  );
}

async function validateGitAndLinks(): Promise<void> {
  const top = await run(["git", "rev-parse", "--show-toplevel"]);
  if (top.exitCode !== 0 || realpathSync(resolve(top.stdout.trim())) !== SHOT_ROOT) {
    fail("shot is not the root of an independent Git repository");
  }
  const metadata = SHOT_METADATA;
  if (metadata === null) fail("canonical Shot metadata is unavailable");
  const protocol = metadata.protocol;
  const additions = await run([
    "git",
    "log",
    "--format=%H",
    "--reverse",
    "--diff-filter=A",
    "--",
    ".tohseno/shot.json",
  ]);
  const anchorCommit = additions.stdout.trim().split(/\r?\n/u)[0] ?? "";
  if (additions.exitCode !== 0) {
    fail("cannot inspect the local Shot identity baseline");
  }
  if (!/^[0-9a-f]{40}$/u.test(anchorCommit)) {
    fail("canonical Shot identity has no factory baseline Git anchor");
  }
  const [baselineMetadata, baselineState] = await Promise.all([
    run([
      "git",
      "show",
      `${anchorCommit}:.tohseno/shot.json`,
    ]),
    run([
      "git",
      "show",
      `${anchorCommit}:.tohseno/protocol-state.json`,
    ]),
  ]);
  let baselineShotId: unknown;
  let baselineStateShotId: unknown;
  try {
    const baselineMetadataValue = validateCanonicalShotMetadata(
      JSON.parse(baselineMetadata.stdout) as unknown,
    );
    const baselineStateValue = JSON.parse(
      baselineState.stdout,
    ) as Record<string, unknown>;
    if (
      !exactKeys(baselineStateValue, [
        "protocolVersion",
        "shotId",
        "lifecycle",
        "evolution",
      ]) ||
      baselineStateValue.protocolVersion !== 1 ||
      baselineStateValue.lifecycle !== "EVOLVING" ||
      baselineStateValue.evolution !== 0
    ) {
      fail("canonical Shot identity baseline state is invalid");
    }
    baselineShotId = baselineMetadataValue.protocol.shotId;
    baselineStateShotId = baselineStateValue.shotId;
  } catch {
    fail("canonical Shot identity baseline is unreadable");
  }
  if (
    baselineMetadata.exitCode !== 0 ||
    baselineState.exitCode !== 0 ||
    baselineShotId !== protocol.shotId ||
    baselineStateShotId !== protocol.shotId
  ) {
    fail("local Shot ID differs from its factory baseline Git anchor");
  }
  console.log("✓ identity · local Shot ID matches its factory baseline Git anchor");
  const listed = await run(["git", "ls-files", "-z"]);
  if (listed.exitCode !== 0) fail("cannot inspect tracked files");
  for (const trackedPath of listed.stdout.split("\0").filter(Boolean)) {
    if (PRIVATE_TRACKED_FILE.test(trackedPath)) {
      fail("a private or credential-bearing file is tracked");
    }
    const path = join(SHOT_ROOT, trackedPath);
    if (!existsSync(path)) continue;
    if (lstatSync(path).isSymbolicLink()) {
      const target = readlinkSync(path);
      const resolved = isAbsolute(target) ? resolve(target) : resolve(dirname(path), target);
      if (!insideShot(resolved)) {
        fail("a tracked symbolic link leaves the shot");
      }
    }
  }
  for (const ignoredPath of [
    "MASTER_PROMPT.md",
    "MASTER_EVOLUTIONARY_PROMPT.md",
    "TOHSENO_EVOLUTION_PROMPT.md",
    "Config/Local.xcconfig",
    ".tohseno/data/",
    ".tohseno/run/logs/ios.log",
    ".tohseno/provenance/provenance.json",
    ".tohseno/artifacts/screenshot.png",
    ".env",
    "credential.p8",
  ]) {
    const ignored = await run(["git", "check-ignore", "--quiet", "--no-index", ignoredPath]);
    if (ignored.exitCode !== 0) fail(`runtime artifact is not gitignored: ${ignoredPath}`);
  }
  console.log("✓ structure · independent Git repository · no tracked private files or external links");
}

validateMetadata();
const privateLeakMaterial = validatePrivateCreationProvenance();
validateStructure();
validateManifestFile();
validateComposition();
await validateGitAndLinks();
validateWorktreePrivacy(privateLeakMaterial);
