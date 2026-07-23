import { createHash, randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  statSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { CliError, errorMessage } from "./errors.ts";
import {
  copyRegularFile,
  makeTreeReadOnly,
  removeTreeEvenIfReadOnly,
} from "./files.ts";
import {
  releasePathWithinCache,
  verifyReleaseDirectory,
  type FactoryRelease,
  type PreparedRelease,
  type ReleaseFileRecord,
} from "./release.ts";
import { readShotMetadata } from "./shot.ts";

export type TrustedShotToolName = "machine" | "verify";

export interface TrustedShotTool {
  root: string;
  executable: string;
  release: PreparedRelease | null;
}

export class LegacyShotToolError extends CliError {
  constructor(message: string) {
    super(message, 2);
    this.name = "LegacyShotToolError";
  }
}

interface TrustedShotContext {
  root: string;
  local: string;
  releaseRecord: string;
  metadata: FactoryRelease;
}

interface TrustedSource {
  kind: "cache" | "bundled";
  root: string;
  releaseRecord: string;
}

const CLI_PACKAGE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const BUNDLED_SOURCE_ROOT = resolve(CLI_PACKAGE_ROOT, "../..");

function integrityError(message: string): CliError {
  return new CliError(message, 2);
}

function inside(root: string, candidate: string): boolean {
  const fromRoot = relative(resolve(root), resolve(candidate));
  return fromRoot === "" ||
    (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

function canonicalDirectory(
  path: string,
  boundary: string | null,
  label: string,
): string {
  if (!existsSync(path)) throw integrityError(`${label} is missing`);
  const details = lstatSync(path);
  if (details.isSymbolicLink() || !details.isDirectory()) {
    throw integrityError(`${label} is not a real directory`);
  }
  const canonical = realpathSync(path);
  if (
    boundary !== null &&
    (!inside(boundary, canonical) || canonical === boundary)
  ) {
    throw integrityError(`${label} leaves its trusted boundary`);
  }
  return canonical;
}

function canonicalRegularFile(
  root: string,
  path: string,
  label: string,
): string {
  if (!existsSync(path)) throw integrityError(`${label} is missing`);
  const details = lstatSync(path);
  if (details.isSymbolicLink() || !details.isFile()) {
    throw integrityError(`${label} must be a regular file`);
  }
  const canonical = realpathSync(path);
  if (!inside(root, canonical) || canonical === root) {
    throw integrityError(`${label} leaves its trusted boundary`);
  }
  return canonical;
}

function sha256File(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function digestRecords(records: readonly ReleaseFileRecord[]): string {
  const hash = createHash("sha256");
  for (const record of records) {
    hash.update(record.path);
    hash.update("\0");
    hash.update(record.sha256);
    hash.update("\0");
    hash.update(String(record.size));
    hash.update("\0");
    hash.update(record.executable ? "x" : "-");
    hash.update("\0");
  }
  return hash.digest("hex");
}

function releaseIdFor(metadata: FactoryRelease, digest: string): string {
  if (metadata.source.kind === "git" && metadata.source.commit !== null) {
    return `git-${metadata.source.commit}${
      metadata.source.dirty ? "-dirty" : ""
    }-${digest.slice(0, 16)}`;
  }
  return `content-${digest.slice(0, 32)}`;
}

function safeReleasePath(value: string): boolean {
  return value !== "" &&
    !isAbsolute(value) &&
    !value.includes("\\") &&
    value.split("/").every((part) =>
      part !== "" && part !== "." && part !== ".."
    );
}

function readEmbeddedRelease(
  path: string,
  expected?: { releaseId: string; bundleDigest: string },
): FactoryRelease {
  let value: unknown;
  try {
    value = JSON.parse(readFileSync(path, "utf8")) as unknown;
  } catch (error) {
    throw integrityError(
      `shot-local factory release record is unreadable: ${errorMessage(error)}`,
    );
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw integrityError("shot-local factory release record must be an object");
  }
  const candidate = value as Partial<FactoryRelease>;
  const source = candidate.source;
  if (
    candidate.schemaVersion !== 1 ||
    candidate.platform !== "ios" ||
    typeof candidate.releaseId !== "string" ||
    typeof candidate.cliVersion !== "string" ||
    typeof candidate.templateVersion !== "string" ||
    typeof candidate.manifestSchemaVersion !== "string" ||
    typeof candidate.bundleDigest !== "string" ||
    !/^[a-f0-9]{64}$/u.test(candidate.bundleDigest) ||
    typeof source !== "object" ||
    source === null ||
    (source.kind !== "git" && source.kind !== "content") ||
    typeof source.dirty !== "boolean" ||
    !Array.isArray(candidate.files)
  ) {
    throw integrityError(
      "shot-local factory release record has an unsupported shape",
    );
  }
  if (
    (source.kind === "git" &&
      (typeof source.commit !== "string" ||
        !/^[a-f0-9]{40}$/u.test(source.commit))) ||
    (source.kind === "content" &&
      (source.commit !== null || source.dirty))
  ) {
    throw integrityError(
      "shot-local factory release record has invalid source provenance",
    );
  }

  const records: ReleaseFileRecord[] = [];
  const seen = new Set<string>();
  for (const entry of candidate.files) {
    if (typeof entry !== "object" || entry === null || Array.isArray(entry)) {
      throw integrityError(
        "shot-local factory release record has an invalid file inventory",
      );
    }
    const record = entry as Partial<ReleaseFileRecord>;
    if (
      typeof record.path !== "string" ||
      !safeReleasePath(record.path) ||
      seen.has(record.path) ||
      typeof record.sha256 !== "string" ||
      !/^[a-f0-9]{64}$/u.test(record.sha256) ||
      typeof record.size !== "number" ||
      !Number.isSafeInteger(record.size) ||
      record.size < 0 ||
      typeof record.executable !== "boolean"
    ) {
      throw integrityError(
        "shot-local factory release record has an invalid file inventory",
      );
    }
    seen.add(record.path);
    records.push(record as ReleaseFileRecord);
  }
  const sorted = [...records].sort((left, right) =>
    left.path.localeCompare(right.path)
  );
  if (JSON.stringify(records) !== JSON.stringify(sorted)) {
    throw integrityError(
      "shot-local factory release inventory is not canonical",
    );
  }
  const required = [
    "manifest/types.ts",
    "manifest/validate.ts",
    "shot/runtime/production.ts",
    "shot/runtime/shared.ts",
    "shot/verify.ts",
  ];
  if (required.some((requiredPath) => !seen.has(requiredPath))) {
    throw integrityError(
      "shot-local factory release record is missing required verification rails",
    );
  }

  const metadata = {
    ...candidate,
    files: records,
  } as FactoryRelease;
  const digest = digestRecords(records);
  if (
    digest !== metadata.bundleDigest ||
    releaseIdFor(metadata, digest) !== metadata.releaseId ||
    (expected !== undefined &&
      (metadata.releaseId !== expected.releaseId ||
        metadata.bundleDigest !== expected.bundleDigest))
  ) {
    throw integrityError(
      "shot-local factory release record does not match its content-addressed identity",
    );
  }
  return metadata;
}

function localPathForRecord(record: ReleaseFileRecord): string | null {
  if (record.path.startsWith("manifest/")) return record.path;
  if (record.path.startsWith("shot/runtime/")) {
    return `runtime/${record.path.slice("shot/runtime/".length)}`;
  }
  if (record.path === "shot/verify.ts") return "verify.ts";
  if (record.path === "shot/machine.ts") return "machine.ts";
  return null;
}

function expectedPinnedFiles(
  metadata: FactoryRelease,
): Array<{ local: string; record: ReleaseFileRecord }> {
  return metadata.files
    .flatMap((record) => {
      const local = localPathForRecord(record);
      return local === null ? [] : [{ local, record }];
    })
    .sort((left, right) => left.local.localeCompare(right.local));
}

function pinnedLabel(path: string): string {
  if (path === "verify.ts") return "shot-local verifier";
  if (path === "machine.ts") return "shot-local machine runtime";
  return `shot-local pinned file ${path}`;
}

function assertLocalRecord(
  localRoot: string,
  localRelativePath: string,
  record: ReleaseFileRecord,
): void {
  const local = canonicalRegularFile(
    localRoot,
    join(localRoot, localRelativePath),
    pinnedLabel(localRelativePath),
  );
  const details = statSync(local);
  if (
    details.size !== record.size ||
    ((details.mode & 0o111) !== 0) !== record.executable ||
    sha256File(local) !== record.sha256
  ) {
    throw integrityError(
      `shot-local pinned file ${localRelativePath} differs from its immutable factory release`,
    );
  }
}

function regularTreePaths(root: string): string[] {
  const paths: string[] = [];
  const visit = (directory: string): void => {
    for (const entry of readdirSync(directory, { withFileTypes: true })
      .sort((left, right) => left.name.localeCompare(right.name))) {
      const path = join(directory, entry.name);
      const relativePath = relative(root, path).split(sep).join("/");
      if (entry.isSymbolicLink()) {
        throw integrityError(
          `shot-local pinned tree contains a symbolic link: ${relativePath}`,
        );
      }
      if (entry.isDirectory()) {
        canonicalDirectory(path, root, "shot-local pinned directory");
        visit(path);
      } else if (entry.isFile()) {
        canonicalRegularFile(root, path, "shot-local pinned file");
        paths.push(relativePath);
      } else {
        throw integrityError(
          `shot-local pinned tree contains an unsupported entry: ${relativePath}`,
        );
      }
    }
  };
  visit(root);
  return paths;
}

function assertExactTree(
  localRoot: string,
  prefix: "manifest" | "runtime",
  expected: readonly { local: string }[],
): void {
  const root = canonicalDirectory(
    join(localRoot, prefix),
    localRoot,
    `shot-local ${prefix} tree`,
  );
  const actual = regularTreePaths(root)
    .map((path) => `${prefix}/${path}`)
    .sort();
  const wanted = expected
    .map((file) => file.local)
    .filter((path) => path.startsWith(`${prefix}/`))
    .sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw integrityError(
      `shot-local ${prefix} tree differs from its immutable factory release`,
    );
  }
}

function assertPinnedLayout(
  local: string,
  metadata: FactoryRelease,
): void {
  const expected = expectedPinnedFiles(metadata);
  for (const file of expected) {
    assertLocalRecord(local, file.local, file.record);
  }
  assertExactTree(local, "manifest", expected);
  assertExactTree(local, "runtime", expected);

  const machineRecord = metadata.files.find(
    (record) => record.path === "shot/machine.ts",
  );
  if (machineRecord === undefined && existsSync(join(local, "machine.ts"))) {
    throw integrityError(
      "legacy shot has an unpinned machine runtime",
    );
  }
}

function shotContext(shotRoot: string): TrustedShotContext {
  const root = canonicalDirectory(resolve(shotRoot), null, "shot root");
  const local = canonicalDirectory(
    join(root, ".tohseno"),
    root,
    "shot-local metadata directory",
  );
  canonicalRegularFile(local, join(local, "shot.json"), "shot metadata");
  const shot = readShotMetadata(root);
  if (shot === undefined) {
    throw integrityError("the directory is not a recognized shot");
  }
  const releaseRecord = canonicalRegularFile(
    local,
    join(local, "factory-release.json"),
    "shot-local factory release record",
  );
  const metadata = readEmbeddedRelease(releaseRecord, {
    releaseId: shot.factory.releaseId,
    bundleDigest: shot.factory.bundleDigest,
  });
  assertPinnedLayout(local, metadata);
  return { root, local, releaseRecord, metadata };
}

function validatedRelease(
  release: PreparedRelease,
  expected: { releaseId: string; bundleDigest: string },
): PreparedRelease {
  const directory = canonicalDirectory(
    resolve(release.directory),
    null,
    "cached factory release",
  );
  let metadata: FactoryRelease;
  try {
    metadata = verifyReleaseDirectory(directory, expected.releaseId);
  } catch (error) {
    throw integrityError(errorMessage(error));
  }
  if (
    metadata.bundleDigest !== release.metadata.bundleDigest ||
    metadata.releaseId !== expected.releaseId ||
    metadata.bundleDigest !== expected.bundleDigest
  ) {
    throw integrityError(
      "the cached factory release does not match this shot's immutable pin",
    );
  }
  return { directory, metadata, reused: true };
}

function trustedCacheRoot(releasesDirectory: string): string {
  const cache = resolve(dirname(resolve(releasesDirectory)));
  mkdirSync(cache, { recursive: true, mode: 0o700 });
  const canonicalCache = canonicalDirectory(
    cache,
    null,
    "factory cache directory",
  );
  const requested = join(canonicalCache, "trusted-tools");
  if (!existsSync(requested)) mkdirSync(requested, { mode: 0o700 });
  return canonicalDirectory(
    requested,
    canonicalCache,
    "trusted tool cache",
  );
}

function sourcePathForRecord(
  source: TrustedSource,
  record: ReleaseFileRecord,
): string {
  if (source.kind === "cache") return join(source.root, record.path);
  if (record.path.startsWith("manifest/")) {
    return join(source.root, "packages", record.path);
  }
  if (record.path.startsWith("shot/runtime/")) {
    return join(
      source.root,
      "packages",
      "cli",
      "factory",
      record.path.slice("shot/".length),
    );
  }
  if (record.path === "shot/verify.ts") {
    return join(source.root, "packages", "cli", "factory", "shot-verify.ts");
  }
  if (record.path === "shot/machine.ts") {
    return join(source.root, "packages", "cli", "factory", "shot-machine.ts");
  }
  throw integrityError(
    `factory release record ${record.path} is not an executable verification rail`,
  );
}

function trustedSourceFile(
  source: TrustedSource,
  record: ReleaseFileRecord,
): string {
  let path: string;
  try {
    path = canonicalRegularFile(
      source.root,
      sourcePathForRecord(source, record),
      source.kind === "cache"
        ? `cached factory release file ${record.path}`
        : `installed CLI factory file ${record.path}`,
    );
  } catch (error) {
    if (source.kind === "cache") throw error;
    throw integrityError(
      "this shot's factory release is absent from the trusted cache and " +
        `cannot be authenticated by the installed CLI: ${errorMessage(error)}`,
    );
  }
  const details = statSync(path);
  if (
    details.size !== record.size ||
    ((details.mode & 0o111) !== 0) !== record.executable ||
    sha256File(path) !== record.sha256
  ) {
    if (source.kind === "cache") {
      throw integrityError(
        `cached factory release file ${record.path} differs from its immutable record`,
      );
    }
    throw integrityError(
      "this shot's factory release is absent from the trusted cache and does " +
        "not match the installed CLI; restore its original factory cache or " +
        "install the CLI release that created it",
    );
  }
  return path;
}

function verifySnapshot(
  destination: string,
  cacheRoot: string,
  metadata: FactoryRelease,
  releaseRecord: string,
  tool: TrustedShotToolName,
): string {
  const root = canonicalDirectory(
    destination,
    cacheRoot,
    "trusted tool snapshot",
  );
  const local = canonicalDirectory(
    join(root, ".tohseno"),
    root,
    "trusted tool snapshot metadata",
  );
  const snapshotRecord = canonicalRegularFile(
    local,
    join(local, "factory-release.json"),
    "trusted tool snapshot release record",
  );
  if (sha256File(snapshotRecord) !== sha256File(releaseRecord)) {
    throw integrityError(
      "trusted tool snapshot differs from its immutable factory release",
    );
  }
  readEmbeddedRelease(snapshotRecord, {
    releaseId: metadata.releaseId,
    bundleDigest: metadata.bundleDigest,
  });
  assertPinnedLayout(local, metadata);
  return canonicalRegularFile(
    local,
    join(local, tool === "verify" ? "verify.ts" : "machine.ts"),
    `trusted ${tool} snapshot`,
  );
}

function prepareTrustedSnapshot(options: {
  releasesDirectory: string;
  metadata: FactoryRelease;
  source: TrustedSource;
  tool: TrustedShotToolName;
}): string {
  const cacheRoot = trustedCacheRoot(options.releasesDirectory);
  const destination = join(cacheRoot, options.metadata.releaseId);
  const authenticatedFiles = expectedPinnedFiles(options.metadata).map(
    (file) => ({
      ...file,
      source: trustedSourceFile(options.source, file.record),
    }),
  );
  if (existsSync(destination)) {
    return verifySnapshot(
      destination,
      cacheRoot,
      options.metadata,
      options.source.releaseRecord,
      options.tool,
    );
  }

  const staging = join(
    cacheRoot,
    `.build-${process.pid}-${randomUUID()}`,
  );
  mkdirSync(join(staging, ".tohseno"), {
    recursive: true,
    mode: 0o700,
  });
  const local = join(staging, ".tohseno");
  try {
    for (const file of authenticatedFiles) {
      copyRegularFile(
        file.source,
        join(local, file.local),
        file.record.executable,
      );
    }
    copyRegularFile(
      options.source.releaseRecord,
      join(local, "factory-release.json"),
      false,
    );
    assertPinnedLayout(local, options.metadata);
    makeTreeReadOnly(staging);
    try {
      renameSync(staging, destination);
    } catch (error) {
      if (!existsSync(destination)) throw error;
      removeTreeEvenIfReadOnly(staging);
    }
    return verifySnapshot(
      destination,
      cacheRoot,
      options.metadata,
      options.source.releaseRecord,
      options.tool,
    );
  } catch (error) {
    if (existsSync(staging)) removeTreeEvenIfReadOnly(staging);
    if (error instanceof CliError) throw error;
    throw integrityError(
      `could not prepare trusted shot tools: ${errorMessage(error)}`,
    );
  }
}

function requireToolDeclared(
  metadata: FactoryRelease,
  tool: TrustedShotToolName,
): void {
  const path = tool === "verify" ? "shot/verify.ts" : "shot/machine.ts";
  if (!metadata.files.some((record) => record.path === path)) {
    if (tool === "machine") {
      throw new LegacyShotToolError(
        "this legacy shot has no pinned machine runtime",
      );
    }
    throw integrityError("this shot has no pinned verifier");
  }
}

export function trustedShotToolFromRelease(options: {
  shotRoot: string;
  release: PreparedRelease;
  tool: TrustedShotToolName;
  expectedRelease?: { releaseId: string; bundleDigest: string };
}): TrustedShotTool {
  const context = shotContext(options.shotRoot);
  const expected = options.expectedRelease ?? {
    releaseId: context.metadata.releaseId,
    bundleDigest: context.metadata.bundleDigest,
  };
  if (
    context.metadata.releaseId !== expected.releaseId ||
    context.metadata.bundleDigest !== expected.bundleDigest
  ) {
    throw integrityError(
      "shot metadata no longer matches its immutable factory release",
    );
  }
  requireToolDeclared(context.metadata, options.tool);
  const release = validatedRelease(options.release, expected);
  const cachedRecord = canonicalRegularFile(
    release.directory,
    join(release.directory, "release.json"),
    "cached factory release record",
  );
  if (sha256File(context.releaseRecord) !== sha256File(cachedRecord)) {
    throw integrityError(
      "shot-local factory release record differs from its immutable cache",
    );
  }
  const executable = prepareTrustedSnapshot({
    releasesDirectory: dirname(release.directory),
    metadata: context.metadata,
    source: {
      kind: "cache",
      root: release.directory,
      releaseRecord: cachedRecord,
    },
    tool: options.tool,
  });
  return { root: context.root, executable, release };
}

export function trustedShotToolFromCache(options: {
  shotRoot: string;
  releasesDirectory: string;
  tool: TrustedShotToolName;
}): TrustedShotTool {
  const context = shotContext(options.shotRoot);
  requireToolDeclared(context.metadata, options.tool);
  const directory = releasePathWithinCache(
    options.releasesDirectory,
    context.metadata.releaseId,
  );
  let release: PreparedRelease | null = null;
  let source: TrustedSource = {
    kind: "bundled",
    root: BUNDLED_SOURCE_ROOT,
    releaseRecord: context.releaseRecord,
  };
  if (existsSync(directory)) {
    const canonicalReleaseDirectory = canonicalDirectory(
      directory,
      null,
      "cached factory release",
    );
    let metadata: FactoryRelease;
    try {
      metadata = verifyReleaseDirectory(
        canonicalReleaseDirectory,
        context.metadata.releaseId,
      );
    } catch (error) {
      throw integrityError(errorMessage(error));
    }
    if (metadata.bundleDigest !== context.metadata.bundleDigest) {
      throw integrityError(
        "the cached factory release does not match this shot's immutable pin",
      );
    }
    const cachedRecord = canonicalRegularFile(
      canonicalReleaseDirectory,
      join(canonicalReleaseDirectory, "release.json"),
      "cached factory release record",
    );
    if (sha256File(context.releaseRecord) !== sha256File(cachedRecord)) {
      throw integrityError(
        "shot-local factory release record differs from its immutable cache",
      );
    }
    release = {
      directory: canonicalReleaseDirectory,
      metadata,
      reused: true,
    };
    source = {
      kind: "cache",
      root: release.directory,
      releaseRecord: cachedRecord,
    };
  }
  const executable = prepareTrustedSnapshot({
    releasesDirectory: options.releasesDirectory,
    metadata: context.metadata,
    source,
    tool: options.tool,
  });
  return { root: context.root, executable, release };
}
