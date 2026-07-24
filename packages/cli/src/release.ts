import { createHash, randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  realpathSync,
  readdirSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import {
  CLI_VERSION,
  FACTORY_RELEASE_SCHEMA_VERSION,
  IOS_TEMPLATE_VERSION,
  MANIFEST_SCHEMA_VERSION,
  MAX_FACTORY_RELEASE_BYTES,
  MAX_FACTORY_RELEASE_FILE_BYTES,
  MAX_FACTORY_RELEASE_FILES,
  RELEASE_SOURCE_FILES,
  REQUIRED_RELEASE_FILES,
} from "./constants.ts";
import { CliError, errorMessage } from "./errors.ts";
import {
  assertSafeBundlePath,
  copyRegularFile,
  listRegularFiles,
  makeTreeReadOnly,
  readBoundedJson,
  readBoundedRegularFile,
  removeTreeEvenIfReadOnly,
} from "./files.ts";
import { runCaptured } from "./process.ts";

export interface ReleaseFileRecord {
  path: string;
  sha256: string;
  size: number;
  executable: boolean;
}

export interface ReleaseSource {
  kind: "git" | "content";
  commit: string | null;
  dirty: boolean;
}

export interface FactoryRelease {
  schemaVersion: typeof FACTORY_RELEASE_SCHEMA_VERSION;
  releaseId: string;
  cliVersion: string;
  templateVersion: string;
  manifestSchemaVersion: string;
  platform: "ios";
  source: ReleaseSource;
  bundleDigest: string;
  files: ReleaseFileRecord[];
}

export interface PreparedRelease {
  directory: string;
  metadata: FactoryRelease;
  reused: boolean;
}

interface ActiveReleasePointer {
  schemaVersion: 1;
  releaseId: string;
}

interface SourceEntry {
  source: string;
  destination: string;
  executable: boolean;
}

function sha256File(path: string): string {
  return createHash("sha256")
    .update(
      readBoundedRegularFile(
        path,
        MAX_FACTORY_RELEASE_FILE_BYTES,
        "factory release file",
      ),
    )
    .digest("hex");
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

function releaseIdFor(source: ReleaseSource, digest: string): string {
  if (source.kind === "git" && source.commit !== null) {
    return `git-${source.commit}${source.dirty ? "-dirty" : ""}-${digest.slice(0, 16)}`;
  }
  return `content-${digest.slice(0, 32)}`;
}

function mapSourcePath(sourceRoot: string, path: string): SourceEntry {
  let destination: string;
  if (path.startsWith("templates/continuity-app/")) {
    destination = `platforms/ios/base/${path.slice("templates/continuity-app/".length)}`;
  } else if (path === "skills/continuity-app/SKILL.md") {
    destination = "agent/continuity-app/SKILL.md";
  } else if (path.startsWith("packages/manifest/")) {
    destination = `manifest/${basename(path)}`;
  } else if (path === "packages/cli/factory/AGENTS.md") {
    destination = "shot/AGENTS.md";
  } else if (path === "packages/cli/factory/CLAUDE.md") {
    destination = "shot/CLAUDE.md";
  } else if (path === "packages/cli/factory/OPERATIONS.md") {
    destination = "shot/OPERATIONS.md";
  } else if (path === "packages/cli/factory/shot-machine.ts") {
    destination = "shot/machine.ts";
  } else if (path.startsWith("packages/cli/factory/runtime/")) {
    destination = `shot/runtime/${path.slice("packages/cli/factory/runtime/".length)}`;
  } else if (path === "packages/cli/factory/shot-verify.ts") {
    destination = "shot/verify.ts";
  } else if (path === "packages/cli/package.json") {
    destination = "factory/cli/package.json";
  } else if (path.startsWith("packages/cli/src/")) {
    destination = `factory/cli/src/${path.slice("packages/cli/src/".length)}`;
  } else if (path === "LICENSE") {
    destination = "legal/LICENSE";
  } else {
    throw new CliError(`unsupported release source path: ${path}`);
  }
  assertSafeBundlePath(destination);
  const source = join(sourceRoot, path);
  const details = lstatSync(source);
  if (
    details.isSymbolicLink() ||
    !details.isFile() ||
    details.nlink !== 1
  ) {
    throw new CliError(
      `factory release input must be a single-link regular file: ${path}`,
    );
  }
  return { source, destination, executable: (details.mode & 0o111) !== 0 };
}

async function gitListedFiles(sourceRoot: string, path: string): Promise<string[] | undefined> {
  const topLevel = await runCaptured(["git", "-C", sourceRoot, "rev-parse", "--show-toplevel"]);
  if (topLevel.exitCode !== 0 || resolve(topLevel.stdout.trim()) !== resolve(sourceRoot)) return undefined;
  const listed = await runCaptured([
    "git", "-C", sourceRoot, "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--",
    path,
  ]);
  if (listed.exitCode !== 0) return undefined;
  return listed.stdout.split("\0").filter(Boolean).sort();
}

async function sourceEntries(sourceRoot: string): Promise<SourceEntry[]> {
  const listed = await gitListedFiles(sourceRoot, "templates/continuity-app");
  const templatePaths = listed ?? listRegularFiles(join(sourceRoot, "templates", "continuity-app"))
    .map((file) => `templates/continuity-app/${file.relativePath}`);
  const listedCli = await gitListedFiles(sourceRoot, "packages/cli/src");
  const cliPaths = listedCli ?? listRegularFiles(join(sourceRoot, "packages", "cli", "src"))
    .map((file) => `packages/cli/src/${file.relativePath}`);
  const paths = [...templatePaths, ...cliPaths, ...RELEASE_SOURCE_FILES];
  const unique = [...new Set(paths)].sort();
  const entries = unique.map((path) => {
    const absolute = join(sourceRoot, path);
    if (!existsSync(absolute)) throw new CliError(`factory source is missing required file ${path}`);
    return mapSourcePath(sourceRoot, path);
  });
  const destinations = new Set<string>();
  for (const entry of entries) {
    if (destinations.has(entry.destination)) {
      throw new CliError(`duplicate factory release path ${entry.destination}`);
    }
    destinations.add(entry.destination);
  }
  return entries;
}

async function sourceProvenance(sourceRoot: string): Promise<ReleaseSource> {
  const commitResult = await runCaptured(["git", "-C", sourceRoot, "rev-parse", "HEAD"]);
  const topResult = await runCaptured(["git", "-C", sourceRoot, "rev-parse", "--show-toplevel"]);
  if (
    commitResult.exitCode !== 0 ||
    topResult.exitCode !== 0 ||
    resolve(topResult.stdout.trim()) !== resolve(sourceRoot) ||
    !/^[0-9a-f]{40}$/u.test(commitResult.stdout.trim())
  ) {
    return { kind: "content", commit: null, dirty: false };
  }
  const status = await runCaptured([
    "git", "-C", sourceRoot, "status", "--porcelain=v1", "--untracked-files=all",
  ]);
  return {
    kind: "git",
    commit: commitResult.stdout.trim(),
    dirty: status.exitCode !== 0 || status.stdout.length > 0,
  };
}

function recordsForDirectory(root: string): ReleaseFileRecord[] {
  const files = listRegularFiles(root)
    .filter((file) => file.relativePath !== "release.json");
  if (files.length > MAX_FACTORY_RELEASE_FILES) {
    throw new CliError("factory release contains too many files");
  }
  let totalBytes = 0;
  return files
    .map((file) => {
      const size = lstatSync(file.absolutePath).size;
      if (size > MAX_FACTORY_RELEASE_FILE_BYTES) {
        throw new CliError(
          `factory release file exceeds the size limit: ${file.relativePath}`,
        );
      }
      totalBytes += size;
      if (totalBytes > MAX_FACTORY_RELEASE_BYTES) {
        throw new CliError("factory release exceeds the total size limit");
      }
      return {
      path: file.relativePath,
      sha256: sha256File(file.absolutePath),
      size,
      executable: file.executable,
      };
    })
    .sort((left, right) => left.path.localeCompare(right.path));
}

function parseReleaseMetadata(path: string): FactoryRelease {
  let value: unknown;
  try {
    value = readBoundedJson<unknown>(
      path,
      2 * 1_048_576,
      "factory release metadata",
    );
  } catch (error) {
    throw new CliError(`release metadata is unreadable: ${errorMessage(error)}`);
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new CliError("release metadata must be a JSON object");
  }
  const record = value as Partial<FactoryRelease>;
  const source = record.source;
  if (
    record.schemaVersion !== FACTORY_RELEASE_SCHEMA_VERSION ||
    typeof record.releaseId !== "string" ||
    !/^(?:git-[0-9a-f]{40}(?:-dirty)?-[0-9a-f]{16}|content-[0-9a-f]{32})$/u
      .test(record.releaseId) ||
    typeof record.cliVersion !== "string" ||
    !/^[0-9]+\.[0-9]+\.[0-9]+$/u.test(record.cliVersion) ||
    typeof record.templateVersion !== "string" ||
    typeof record.manifestSchemaVersion !== "string" ||
    typeof record.bundleDigest !== "string" ||
    !/^[0-9a-f]{64}$/u.test(record.bundleDigest) ||
    record.platform !== "ios" ||
    typeof source !== "object" ||
    source === null ||
    (source.kind !== "git" && source.kind !== "content") ||
    typeof source.dirty !== "boolean" ||
    !Array.isArray(record.files) ||
    record.files.length > MAX_FACTORY_RELEASE_FILES
  ) {
    throw new CliError("release metadata has an unsupported or incomplete shape");
  }
  if (
    (source.kind === "git" &&
      (typeof source.commit !== "string" ||
        !/^[0-9a-f]{40}$/u.test(source.commit))) ||
    (source.kind === "content" &&
      (source.commit !== null || source.dirty))
  ) {
    throw new CliError("release metadata has invalid source provenance");
  }
  return record as FactoryRelease;
}

export function verifyReleaseDirectory(directory: string, expectedId?: string): FactoryRelease {
  const absolute = resolve(directory);
  if (!existsSync(absolute) || !lstatSync(absolute).isDirectory()) {
    throw new CliError(`factory release is absent: ${absolute}`);
  }
  const metadataPath = join(absolute, "release.json");
  if (!existsSync(metadataPath)) throw new CliError(`factory release is corrupt: missing release.json in ${absolute}`);
  let metadata: FactoryRelease;
  try {
    metadata = parseReleaseMetadata(metadataPath);
    if (expectedId !== undefined && metadata.releaseId !== expectedId) {
      throw new CliError(`release id is ${metadata.releaseId}, expected ${expectedId}`);
    }
    if (basename(absolute) !== metadata.releaseId) {
      throw new CliError(`release directory name does not match ${metadata.releaseId}`);
    }
    const actual = recordsForDirectory(absolute);
    if (JSON.stringify(actual) !== JSON.stringify(metadata.files)) {
      throw new CliError("release file inventory or checksum does not match release.json");
    }
    const digest = digestRecords(actual);
    if (digest !== metadata.bundleDigest) throw new CliError("release bundle digest does not match release.json");
    if (releaseIdFor(metadata.source, digest) !== metadata.releaseId) {
      throw new CliError("release id does not match its provenance and bundle digest");
    }
    for (const path of REQUIRED_RELEASE_FILES) {
      if (!actual.some((file) => file.path === path)) throw new CliError(`release is missing required file ${path}`);
    }
  } catch (error) {
    if (error instanceof CliError && error.message.startsWith("factory release is corrupt:")) throw error;
    throw new CliError(
      `factory release is corrupt: ${absolute}: ${errorMessage(error)}; remove it explicitly before rebuilding`,
    );
  }
  return metadata;
}

function writePreparedRelease(
  staging: string,
  source: ReleaseSource,
): FactoryRelease {
  const files = recordsForDirectory(staging);
  for (const path of REQUIRED_RELEASE_FILES) {
    if (!files.some((file) => file.path === path)) throw new CliError(`prepared release is missing ${path}`);
  }
  const bundleDigest = digestRecords(files);
  const releaseId = releaseIdFor(source, bundleDigest);
  const metadata: FactoryRelease = {
    schemaVersion: FACTORY_RELEASE_SCHEMA_VERSION,
    releaseId,
    cliVersion: CLI_VERSION,
    templateVersion: IOS_TEMPLATE_VERSION,
    manifestSchemaVersion: MANIFEST_SCHEMA_VERSION,
    platform: "ios",
    source,
    bundleDigest,
    files,
  };
  writeFileSync(join(staging, "release.json"), `${JSON.stringify(metadata, null, 2)}\n`, { mode: 0o644 });
  return metadata;
}

export function canonicalReleasesDirectory(
  releasesDirectory: string,
): string {
  const requestedReleases = resolve(releasesDirectory);
  const requestedCache = dirname(requestedReleases);
  const requestedFactoryHome = dirname(requestedCache);
  mkdirSync(requestedFactoryHome, { recursive: true, mode: 0o700 });
  const factoryDetails = lstatSync(requestedFactoryHome);
  if (factoryDetails.isSymbolicLink() || !factoryDetails.isDirectory()) {
    throw new CliError("factory home must be a real directory");
  }
  if (!existsSync(requestedCache)) {
    mkdirSync(requestedCache, { mode: 0o700 });
  }
  const cacheDetails = lstatSync(requestedCache);
  if (cacheDetails.isSymbolicLink() || !cacheDetails.isDirectory()) {
    throw new CliError("factory cache must be a real directory");
  }
  if (!existsSync(requestedReleases)) {
    mkdirSync(requestedReleases, { mode: 0o700 });
  }
  const releasesDetails = lstatSync(requestedReleases);
  if (
    releasesDetails.isSymbolicLink() ||
    !releasesDetails.isDirectory()
  ) {
    throw new CliError("factory release cache must be a real directory");
  }
  const canonicalFactoryHome = realpathSync(requestedFactoryHome);
  const canonicalCache = realpathSync(requestedCache);
  const canonicalReleases = realpathSync(requestedReleases);
  const inside = (root: string, candidate: string): boolean => {
    const fromRoot = relative(root, candidate);
    return fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`);
  };
  if (
    canonicalCache === canonicalFactoryHome ||
    canonicalReleases === canonicalCache ||
    !inside(canonicalFactoryHome, canonicalCache) ||
    !inside(canonicalCache, canonicalReleases)
  ) {
    throw new CliError("factory release cache leaves its managed boundary");
  }
  return canonicalReleases;
}

export async function prepareFactoryRelease(
  sourceRoot: string,
  releasesDirectory: string,
): Promise<PreparedRelease> {
  const requestedSourceRoot = resolve(sourceRoot);
  const sourceRootDetails = lstatSync(requestedSourceRoot);
  if (
    sourceRootDetails.isSymbolicLink() ||
    !sourceRootDetails.isDirectory()
  ) {
    throw new CliError("factory source root must be a real directory");
  }
  const canonicalSourceRoot = realpathSync(requestedSourceRoot);
  const releasesRoot = canonicalReleasesDirectory(releasesDirectory);
  const staging = join(releasesRoot, `.build-${process.pid}-${randomUUID()}`);
  mkdirSync(staging, { mode: 0o700 });
  try {
    const before = await sourceProvenance(canonicalSourceRoot);
    const entries = await sourceEntries(canonicalSourceRoot);
    for (const entry of entries) {
      copyRegularFile(entry.source, join(staging, entry.destination), entry.executable);
    }
    const after = await sourceProvenance(canonicalSourceRoot);
    const changedDuringSnapshot = entries.some((entry) =>
      sha256File(entry.source) !== sha256File(join(staging, entry.destination))
    );
    const source: ReleaseSource = {
      kind: before.kind === "git" && after.kind === "git" && before.commit === after.commit ? "git" : "content",
      commit: before.kind === "git" && after.kind === "git" && before.commit === after.commit ? before.commit : null,
      dirty: before.dirty || after.dirty || before.commit !== after.commit || changedDuringSnapshot,
    };
    if (source.kind === "content") source.dirty = false;
    const metadata = writePreparedRelease(staging, source);
    const destination = join(releasesRoot, metadata.releaseId);

    if (existsSync(destination)) {
      const cached = verifyReleaseDirectory(destination, metadata.releaseId);
      if (cached.bundleDigest !== metadata.bundleDigest) {
        throw new CliError(`factory release id collision at ${destination}`);
      }
      removeTreeEvenIfReadOnly(staging);
      writeActiveReleasePointer(releasesRoot, cached.releaseId);
      return { directory: destination, metadata: cached, reused: true };
    }

    makeTreeReadOnly(staging);
    try {
      renameSync(staging, destination);
      writeActiveReleasePointer(releasesRoot, metadata.releaseId);
      return { directory: destination, metadata, reused: false };
    } catch (error) {
      if (!existsSync(destination)) throw error;
      const cached = verifyReleaseDirectory(destination, metadata.releaseId);
      if (cached.bundleDigest !== metadata.bundleDigest) {
        throw new CliError(`concurrent factory release differs at ${destination}`);
      }
      removeTreeEvenIfReadOnly(staging);
      writeActiveReleasePointer(releasesRoot, cached.releaseId);
      return { directory: destination, metadata: cached, reused: true };
    }
  } catch (error) {
    removeTreeEvenIfReadOnly(staging);
    throw new CliError(`could not prepare immutable factory release: ${errorMessage(error)}`);
  }
}

function activeReleasePointerPath(releasesDirectory: string): string {
  return join(dirname(releasesDirectory), "active-release.json");
}

function writeActiveReleasePointer(releasesDirectory: string, releaseId: string): void {
  const path = activeReleasePointerPath(releasesDirectory);
  const temporary = `${path}.writing-${process.pid}-${randomUUID()}`;
  const value: ActiveReleasePointer = { schemaVersion: 1, releaseId };
  try {
    writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
    renameSync(temporary, path);
  } finally {
    if (existsSync(temporary)) unlinkSync(temporary);
  }
}

export function useActiveCachedRelease(releasesDirectory: string): PreparedRelease {
  const releasesRoot = canonicalReleasesDirectory(releasesDirectory);
  const pointerPath = activeReleasePointerPath(releasesRoot);
  let releaseId: string | undefined;
  if (lstatSync(pointerPath, { throwIfNoEntry: false }) !== undefined) {
    try {
      const value = readBoundedJson<Partial<ActiveReleasePointer>>(
        pointerPath,
        4_096,
        "active release pointer",
      );
      if (value.schemaVersion !== 1 || typeof value.releaseId !== "string") {
        throw new Error("unsupported pointer shape");
      }
      releaseId = value.releaseId;
    } catch (error) {
      throw new CliError(`active release pointer is corrupt: ${pointerPath}: ${errorMessage(error)}`);
    }
  } else {
    const cached = listCachedReleaseDirectories(releasesRoot);
    if (cached.length === 1) releaseId = basename(cached[0]!);
    else if (cached.length === 0) {
      throw new CliError("no cached factory release is available");
    } else {
      throw new CliError(
        `factory source is unavailable and ${cached.length} cached releases exist without an active-release pointer`,
      );
    }
  }
  const directory = releasePathWithinCache(releasesRoot, releaseId);
  const metadata = verifyReleaseDirectory(directory, releaseId);
  return { directory, metadata, reused: true };
}

export function listCachedReleaseDirectories(releasesDirectory: string): string[] {
  const releasesRoot = canonicalReleasesDirectory(releasesDirectory);
  return readdirSync(releasesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith("."))
    .map((entry) => join(releasesRoot, entry.name))
    .sort();
}

export function releasePathWithinCache(releasesDirectory: string, releaseId: string): string {
  const releasesRoot = canonicalReleasesDirectory(releasesDirectory);
  const candidate = resolve(releasesRoot, releaseId);
  const fromRoot = relative(releasesRoot, candidate);
  if (fromRoot.startsWith(`..${sep}`) || fromRoot === "..") throw new CliError("invalid release id");
  return candidate;
}
