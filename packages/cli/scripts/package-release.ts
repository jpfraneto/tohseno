import { createHash, randomUUID } from "node:crypto";
import {
  closeSync,
  constants,
  fchmodSync,
  fstatSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  readSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import type { Stats } from "node:fs";
import { gzipSync } from "node:zlib";
import { basename, dirname, join, resolve, sep } from "node:path";
import {
  CLI_VERSION,
  MAX_FACTORY_RELEASE_BYTES,
  MAX_FACTORY_RELEASE_FILE_BYTES,
  MAX_FACTORY_RELEASE_FILES,
} from "../src/constants.ts";
import {
  assertSafeBundlePath,
  listRegularFiles,
  readBoundedRegularFile,
} from "../src/files.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const ARCHIVE_ROOT = `tohseno-cli-${CLI_VERSION}`;
const INPUTS = [
  "LICENSE",
  "skills/daily-challenge",
  "skills/local-progress",
  "skills/rank-progression",
  "skills/share-card",
  "packages/skills",
  "packages/identity",
  "packages/signer",
  "packages/protocol",
  "packages/registry",
  "packages/node-client",
  "packages/manifest/app.ts",
  "packages/manifest/app.manifest.schema.json",
  "packages/manifest/cli.ts",
  "packages/cli/src",
  "packages/cli/factory",
  "packages/cli/package.json",
  "packages/cli/THIRD_PARTY_NOTICES.md",
  "templates/ios-kernel",
  "templates/blank",
  "templates/daily-game",
] as const;
const THIRD_PARTY_INPUTS = [
  {
    source: "node_modules/serve-sim",
    destination: "packages/cli/node_modules/serve-sim",
    packageName: "serve-sim",
    version: "0.1.45",
    treeSha256: "8520993c3e169a95fefda5273532ea7050dbdf76074d5a8569c151e017b8f433",
  },
  {
    source: "node_modules/ws",
    destination: "packages/cli/node_modules/ws",
    packageName: "ws",
    version: "8.21.1",
    treeSha256: "31b870ff37f767a5120693371e97ee1cd1dc86afc7b92f774d9fb8e391df2fbc",
  },
] as const;

interface ArchiveFile {
  path: string;
  content: Buffer;
  mode: number;
}

interface FirstPartySnapshotRecord {
  path: string;
  contentSha256: string;
  gitBlobSha1: string;
  mode: number;
  fileSystemIdentity: string;
}

interface FirstPartySnapshot {
  files: ArchiveFile[];
  records: FirstPartySnapshotRecord[];
  fingerprint: string;
}

interface StableGitReleaseSnapshot {
  files: ArchiveFile[];
  matchesHead: boolean;
  source: CliReleaseSourceProvenance;
}

interface GitHeadEntry {
  path: string;
  object: string;
  mode: number;
}

interface ThirdPartySnapshotFile {
  relativePath: string;
  content: Buffer;
  executable: boolean;
}

const SOURCE_INVENTORY =
  "git ls-files --cached --others --exclude-standard" as const;
const CHECKSUM_MANIFEST = ".tohseno-install-checksums-v1";
const EXECUTABLE_MANIFEST = ".tohseno-install-executables-v1";
const ROOT_MANIFEST = ".tohseno-install-root-v1";

export interface CliReleaseSourceProvenance {
  kind: "git";
  commit: string;
  dirty: boolean;
  inventory: typeof SOURCE_INVENTORY;
}

function runGit(root: string, arguments_: readonly string[], label: string): Buffer {
  const git = (() => {
    try {
      const candidate = Bun.which("git");
      if (candidate === null) throw new Error("Git is unavailable");
      const canonical = realpathSync(candidate);
      const details = lstatSync(canonical);
      if (!details.isFile() || (details.mode & 0o111) === 0) {
        throw new Error("Git is not executable");
      }
      return canonical;
    } catch {
      throw new Error(
        `could not ${label}; Git is required to build a CLI release`,
      );
    }
  })();
  const result = (() => {
    try {
      return Bun.spawnSync(
        [
          git,
          "--no-pager",
          "--literal-pathspecs",
          "-c",
          "core.hooksPath=/dev/null",
          "-c",
          "core.fsmonitor=false",
          "-c",
          "core.filemode=true",
          "-c",
          "core.attributesFile=/dev/null",
          "-c",
          "core.excludesFile=/dev/null",
          "-c",
          "core.untrackedCache=false",
          "-c",
          "submodule.recurse=false",
          "-C",
          root,
          ...arguments_,
        ],
        {
          env: {
            LANG: "C",
            LC_ALL: "C",
            GIT_CONFIG_NOSYSTEM: "1",
            GIT_CONFIG_GLOBAL: "/dev/null",
            GIT_NO_REPLACE_OBJECTS: "1",
            GIT_OPTIONAL_LOCKS: "0",
            GIT_TERMINAL_PROMPT: "0",
          },
          stdin: "ignore",
          stdout: "pipe",
          stderr: "pipe",
        },
      );
    } catch {
      throw new Error(
        `could not ${label}; Git is required to build a CLI release`,
      );
    }
  })();
  if (result.exitCode !== 0) {
    throw new Error(`could not ${label}; refusing to build without Git provenance`);
  }
  return Buffer.from(result.stdout);
}

function decodeGitUtf8(value: Uint8Array, label: string): string {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(value);
  } catch {
    throw new Error(`${label} contains a path that is not valid UTF-8`);
  }
}

function decodeGitNulRecords(value: Uint8Array, label: string): string[] {
  const output = decodeGitUtf8(value, label);
  if (output === "") return [];
  if (!output.endsWith("\0")) {
    throw new Error(`${label} is missing its path terminator`);
  }
  const records = output.slice(0, -1).split("\0");
  if (records.some((record) => record === "")) {
    throw new Error(`${label} contains an empty record`);
  }
  return records;
}

function assertExactGitRoot(root: string): string {
  const absolute = resolve(root);
  const details = lstatSync(absolute);
  if (details.isSymbolicLink() || !details.isDirectory()) {
    throw new Error(`CLI release source must be a real directory: ${absolute}`);
  }
  const canonical = realpathSync(absolute);
  const topLevel = decodeGitUtf8(
    runGit(
      absolute,
      ["rev-parse", "--show-toplevel"],
      "resolve the CLI release repository",
    ),
    "Git repository root",
  ).trimEnd();
  if (
    topLevel === "" ||
    /[\r\n]/u.test(topLevel) ||
    realpathSync(topLevel) !== canonical
  ) {
    throw new Error("CLI release source must be the exact Git worktree root");
  }
  return canonical;
}

function assertSafeReleaseInput(input: string): void {
  if (
    input === "" ||
    input.startsWith("/") ||
    input.includes("\\") ||
    /[\r\n]/u.test(input) ||
    input.split("/").some((part) =>
      part === "" || part === "." || part === ".."
    )
  ) {
    throw new Error(`CLI release input must be a safe repository path: ${input}`);
  }
}

export function gitReleaseInputPaths(
  root: string,
  inputs: readonly string[],
): string[] {
  const repositoryRoot = assertExactGitRoot(root);
  if (inputs.length === 0) {
    throw new Error("CLI release requires at least one first-party input");
  }
  for (const input of inputs) {
    assertSafeReleaseInput(input);
    const details = lstatSync(join(repositoryRoot, input));
    if (
      details.isSymbolicLink() ||
      (!details.isDirectory() && !details.isFile())
    ) {
      throw new Error(
        `CLI release input must be a real file or directory: ${input}`,
      );
    }
  }

  const paths = decodeGitNulRecords(
    runGit(
      repositoryRoot,
      [
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
        ...inputs,
      ],
      "inventory CLI release inputs",
    ),
    "Git release inventory",
  );
  const unique = [...new Set(paths)]
    .filter((path) =>
      lstatSync(join(repositoryRoot, path), { throwIfNoEntry: false }) !==
        undefined
    )
    .sort((left, right) => left.localeCompare(right));
  if (unique.length > MAX_FACTORY_RELEASE_FILES) {
    throw new Error("CLI release Git inventory contains too many files");
  }
  for (const path of unique) {
    if (
      path === "" ||
      path.startsWith("/") ||
      path.includes("\\") ||
      /[\r\n]/u.test(path) ||
      path.split("/").some((part) =>
        part === "" || part === "." || part === ".."
      )
    ) {
      throw new Error("Git release inventory contains an unsafe path");
    }
    if (
      !inputs.some((input) =>
        path === input || path.startsWith(`${input}/`)
      )
    ) {
      throw new Error("Git release inventory escaped its declared inputs");
    }
  }
  for (const input of inputs) {
    if (
      !unique.some((path) =>
        path === input || path.startsWith(`${input}/`)
      )
    ) {
      throw new Error(`CLI release input has no Git-visible files: ${input}`);
    }
  }
  return unique;
}

function hasConservativeGitIndexState(root: string): boolean {
  const tags = decodeGitNulRecords(
    runGit(
      root,
      ["ls-files", "-v", "-z"],
      "inspect CLI release index flags",
    ),
    "Git index flag inventory",
  );
  for (const record of tags) {
    if (record.length < 3 || record[1] !== " ") {
      throw new Error("Git index flag inventory has an unsupported record");
    }
    if (record[0] !== "H") {
      return true;
    }
  }

  const stages = decodeGitNulRecords(
    runGit(
      root,
      ["ls-files", "--stage", "-z"],
      "inspect CLI release index entries",
    ),
    "Git index entry inventory",
  );
  for (const record of stages) {
    const tab = record.indexOf("\t");
    if (
      tab === -1 ||
      !/^[0-9]{6} [0-9a-f]{40} [0-3]$/u.test(record.slice(0, tab))
    ) {
      throw new Error("Git index entry inventory has an unsupported record");
    }
    if (record.startsWith("160000 ")) {
      return true;
    }
  }
  return false;
}

export function cliReleaseSourceProvenance(
  root: string,
): CliReleaseSourceProvenance {
  const repositoryRoot = assertExactGitRoot(root);
  const commit = decodeGitUtf8(
    runGit(
      repositoryRoot,
      ["rev-parse", "--verify", "HEAD^{commit}"],
      "resolve CLI release source commit",
    ),
    "Git source commit",
  ).trim();
  if (!/^[0-9a-f]{40}$/u.test(commit)) {
    throw new Error("CLI release source commit has an unsupported identity");
  }
  const status = runGit(
    repositoryRoot,
    [
      "status",
      "--porcelain=v1",
      "--untracked-files=all",
      "--ignore-submodules=all",
    ],
    "inspect CLI release source status",
  );
  return {
    kind: "git",
    commit,
    dirty: status.length > 0 || hasConservativeGitIndexState(repositoryRoot),
    inventory: SOURCE_INVENTORY,
  };
}

function firstPartyPathIdentity(
  root: string,
  path: string,
): { identity: string; fileIdentity: string; mode: number } {
  let current = root;
  const rootDetails = lstatSync(root);
  if (rootDetails.isSymbolicLink() || !rootDetails.isDirectory()) {
    throw new Error("CLI release repository root changed during snapshot");
  }
  const identities = [
    `d:${rootDetails.dev}:${rootDetails.ino}:${rootDetails.mode}`,
  ];
  const parts = path.split("/");
  for (const [index, part] of parts.entries()) {
    current = join(current, part);
    const details = lstatSync(current);
    const isFile = index === parts.length - 1;
    if (
      details.isSymbolicLink() ||
      (isFile
        ? !details.isFile() || details.nlink !== 1
        : !details.isDirectory())
    ) {
      throw new Error(
        `CLI release input must be a single-link regular file beneath real directories: ${path}`,
      );
    }
    const fileIdentity = isFile ? statFileIdentity(details) : "";
    identities.push(
      isFile
        ? `f:${fileIdentity}`
        : `d:${details.dev}:${details.ino}:${details.mode}`,
    );
    if (isFile) {
      return {
        identity: identities.join("\0"),
        fileIdentity,
        mode: (details.mode & 0o111) === 0 ? 0o644 : 0o755,
      };
    }
  }
  throw new Error(`CLI release input is not a file: ${path}`);
}

function statFileIdentity(details: Stats): string {
  return `${details.dev}:${details.ino}:${details.mode}:${details.nlink}:${details.size}:${details.mtimeMs}:${details.ctimeMs}`;
}

function gitBlobSha1(content: Buffer): string {
  return createHash("sha1")
    .update(`blob ${content.length}\0`)
    .update(content)
    .digest("hex");
}

function snapshotFirstPartyFile(
  root: string,
  path: string,
): {
  content: Buffer;
  mode: number;
  fileSystemIdentity: string;
} {
  assertSafeBundlePath(path);
  const before = firstPartyPathIdentity(root, path);
  const source = join(root, path);
  let descriptor: number | undefined;
  let content: Buffer;
  try {
    descriptor = openSync(source, constants.O_RDONLY | constants.O_NOFOLLOW);
    const opened = fstatSync(descriptor);
    if (
      !opened.isFile() ||
      opened.nlink !== 1 ||
      opened.size > MAX_FACTORY_RELEASE_FILE_BYTES ||
      statFileIdentity(opened) !== before.fileIdentity
    ) {
      throw new Error("opened file identity differs");
    }
    const chunks: Buffer[] = [];
    const buffer = Buffer.allocUnsafe(65_536);
    let total = 0;
    while (true) {
      const length = readSync(descriptor, buffer, 0, buffer.length, null);
      if (length === 0) break;
      total += length;
      if (total > MAX_FACTORY_RELEASE_FILE_BYTES) {
        throw new Error("file grew past its limit");
      }
      chunks.push(Buffer.from(buffer.subarray(0, length)));
    }
    if (statFileIdentity(fstatSync(descriptor)) !== before.fileIdentity) {
      throw new Error("opened file changed while being read");
    }
    content = Buffer.concat(chunks, total);
  } catch {
    throw new Error(
      `CLI release input changed or left its repository while being read: ${path}`,
    );
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
  const after = firstPartyPathIdentity(root, path);
  if (
    before.identity !== after.identity ||
    before.mode !== after.mode
  ) {
    throw new Error(`CLI release input changed while being read: ${path}`);
  }
  return {
    content,
    mode: before.mode,
    fileSystemIdentity: before.identity,
  };
}

function firstPartySnapshot(
  root: string,
  inputs: readonly string[],
): FirstPartySnapshot {
  const repositoryRoot = assertExactGitRoot(root);
  const files: ArchiveFile[] = [];
  const records: FirstPartySnapshotRecord[] = [];
  let totalBytes = 0;
  for (const path of gitReleaseInputPaths(repositoryRoot, inputs)) {
    const snapshot = snapshotFirstPartyFile(repositoryRoot, path);
    totalBytes += snapshot.content.length;
    if (totalBytes > MAX_FACTORY_RELEASE_BYTES) {
      throw new Error("CLI release Git snapshot exceeds the total size limit");
    }
    const contentSha256 = createHash("sha256")
      .update(snapshot.content)
      .digest("hex");
    records.push({
      path,
      contentSha256,
      gitBlobSha1: gitBlobSha1(snapshot.content),
      mode: snapshot.mode,
      fileSystemIdentity: snapshot.fileSystemIdentity,
    });
    files.push({
      path: `${ARCHIVE_ROOT}/factory-source/${path}`,
      content: snapshot.content,
      mode: snapshot.mode,
    });
  }
  const digest = createHash("sha256");
  for (const record of records) {
    digest.update(
      `${record.path}\0${record.contentSha256}\0${record.mode}\0${record.fileSystemIdentity}\n`,
    );
  }
  return {
    files,
    records,
    fingerprint: digest.digest("hex"),
  };
}

function gitHeadEntries(
  root: string,
  commit: string,
  inputs: readonly string[],
): GitHeadEntry[] {
  const records = decodeGitNulRecords(
    runGit(
      root,
      [
        "ls-tree",
        "-r",
        "-z",
        "--full-tree",
        commit,
        "--",
        ...inputs,
      ],
      "inventory committed CLI release inputs",
    ),
    "Git committed release inventory",
  );
  return records.map((record) => {
    const tab = record.indexOf("\t");
    const metadata = tab === -1 ? "" : record.slice(0, tab);
    const path = tab === -1 ? "" : record.slice(tab + 1);
    const match =
      /^([0-9]{6}) (blob|commit) ([0-9a-f]{40})$/u.exec(metadata);
    if (
      match === null ||
      path === "" ||
      path.startsWith("/") ||
      path.includes("\\") ||
      /[\r\n]/u.test(path) ||
      path.split("/").some((part) =>
        part === "" || part === "." || part === ".."
      ) ||
      !inputs.some((input) =>
        path === input || path.startsWith(`${input}/`)
      )
    ) {
      throw new Error("Git committed release inventory is unsafe");
    }
    const mode = match[1] === "100644"
      ? 0o644
      : match[1] === "100755"
      ? 0o755
      : 0;
    return {
      path,
      object: match[3] ?? "",
      mode,
    };
  }).sort((left, right) => left.path.localeCompare(right.path));
}

function snapshotMatchesHead(
  root: string,
  commit: string,
  inputs: readonly string[],
  snapshot: FirstPartySnapshot,
): boolean {
  const head = gitHeadEntries(root, commit, inputs);
  if (head.length !== snapshot.records.length) return false;
  return snapshot.records.every((record, index) => {
    const expected = head[index];
    return expected !== undefined &&
      expected.path === record.path &&
      expected.object === record.gitBlobSha1 &&
      expected.mode === record.mode;
  });
}

export function snapshotGitReleaseInputs(
  root: string,
  inputs: readonly string[],
): StableGitReleaseSnapshot {
  const sourceBefore = cliReleaseSourceProvenance(root);
  const before = firstPartySnapshot(root, inputs);
  const exactHeadSnapshot = snapshotMatchesHead(
    root,
    sourceBefore.commit,
    inputs,
    before,
  );
  const after = firstPartySnapshot(root, inputs);
  if (before.fingerprint !== after.fingerprint) {
    throw new Error(
      "CLI release inputs changed while building the immutable snapshot",
    );
  }
  const sourceAfter = cliReleaseSourceProvenance(root);
  if (sourceBefore.commit !== sourceAfter.commit) {
    throw new Error("CLI release source commit changed while building the archive");
  }
  return {
    files: before.files,
    matchesHead: exactHeadSnapshot,
    source: {
      ...sourceBefore,
      dirty:
        sourceBefore.dirty ||
        sourceAfter.dirty ||
        !exactHeadSnapshot,
    },
  };
}

export function assertThirdPartyPackageIdentity(options: {
  directory: string;
  packageName: string;
  version: string;
  treeSha256: string;
}): void {
  verifiedThirdPartySnapshot(options);
}

function readThirdPartySnapshot(directory: string): ThirdPartySnapshotFile[] {
  const directoryDetails = lstatSync(directory);
  if (directoryDetails.isSymbolicLink() || !directoryDetails.isDirectory()) {
    throw new Error(
      `managed release dependency is not a real directory: ${directory}`,
    );
  }
  const sourceFiles = listRegularFiles(directory);
  if (sourceFiles.length > MAX_FACTORY_RELEASE_FILES) {
    throw new Error("managed release dependency contains too many files");
  }
  let totalBytes = 0;
  return sourceFiles.map((file) => {
    const content = readBoundedRegularFile(
      file.absolutePath,
      MAX_FACTORY_RELEASE_FILE_BYTES,
      "managed release dependency file",
    );
    totalBytes += content.length;
    if (totalBytes > MAX_FACTORY_RELEASE_BYTES) {
      throw new Error("managed release dependency exceeds the total size limit");
    }
    return {
      relativePath: file.relativePath.split(sep).join("/"),
      content,
      executable: file.executable,
    };
  });
}

function thirdPartySnapshotTreeSha256(
  files: readonly ThirdPartySnapshotFile[],
): string {
  const digest = createHash("sha256");
  for (const file of files) {
    const fileSha256 = createHash("sha256")
      .update(file.content)
      .digest("hex");
    digest.update(
      `${file.relativePath}\0${file.executable ? "755" : "644"}\0${file.content.length}\0${fileSha256}\n`,
    );
  }
  return digest.digest("hex");
}

function verifiedThirdPartySnapshot(options: {
  directory: string;
  packageName: string;
  version: string;
  treeSha256: string;
}): ThirdPartySnapshotFile[] {
  const files = readThirdPartySnapshot(options.directory);
  const packageJson = files.find((file) =>
    file.relativePath === "package.json"
  );
  if (packageJson === undefined) {
    throw new Error(
      `managed release dependency has no regular package.json: ${options.directory}`,
    );
  }
  let value: unknown;
  try {
    if (packageJson.content.length > 65_536) {
      throw new Error("package manifest is oversized");
    }
    value = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(packageJson.content),
    ) as unknown;
  } catch {
    throw new Error(
      `managed release dependency has unreadable package.json: ${options.directory}`,
    );
  }
  const record =
    typeof value === "object" && value !== null && !Array.isArray(value)
      ? value as Record<string, unknown>
      : {};
  if (
    record.name !== options.packageName ||
    record.version !== options.version
  ) {
    const foundName =
      typeof record.name === "string" ? record.name : "unknown";
    const foundVersion =
      typeof record.version === "string" ? record.version : "unknown";
    throw new Error(
      `managed release dependency identity mismatch: expected ${options.packageName}@${options.version}, found ${foundName}@${foundVersion}`,
    );
  }
  const actualTreeSha256 = thirdPartySnapshotTreeSha256(files);
  if (actualTreeSha256 !== options.treeSha256) {
    throw new Error(
      `managed release dependency tree mismatch for ${options.packageName}@${options.version}: expected ${options.treeSha256}, found ${actualTreeSha256}`,
    );
  }
  return files;
}

export function thirdPartyTreeSha256(directory: string): string {
  return thirdPartySnapshotTreeSha256(readThirdPartySnapshot(directory));
}

function sourceFiles(firstPartyFiles: readonly ArchiveFile[]): ArchiveFile[] {
  const files: ArchiveFile[] = [...firstPartyFiles];
  for (const input of THIRD_PARTY_INPUTS) {
    const source = join(ROOT, input.source);
    const snapshot = verifiedThirdPartySnapshot({
      directory: source,
      packageName: input.packageName,
      version: input.version,
      treeSha256: input.treeSha256,
    });
    for (const file of snapshot) {
      const path = `${input.destination}/${file.relativePath}`;
      files.push({
        path: `${ARCHIVE_ROOT}/factory-source/${path}`,
        content: file.content,
        mode: file.executable ? 0o755 : 0o644,
      });
    }
  }
  return files.sort((left, right) => left.path.localeCompare(right.path));
}

function archiveFileContent(file: ArchiveFile): Buffer {
  return file.content;
}

function withInstallIntegrity(files: readonly ArchiveFile[]): {
  files: ArchiveFile[];
  treeSha256: string;
} {
  if (files.length > MAX_FACTORY_RELEASE_FILES) {
    throw new Error("CLI release contains too many files");
  }
  let totalBytes = 0;
  const relativePath = (path: string): string => {
    const prefix = `${ARCHIVE_ROOT}/`;
    if (!path.startsWith(prefix)) {
      throw new Error(`archive path leaves its release root: ${path}`);
    }
    const value = path.slice(prefix.length);
    if (/[\r\n\\]/u.test(value) || value === "" || value.startsWith("/")) {
      throw new Error(`archive path cannot be represented safely: ${path}`);
    }
    return value;
  };
  const checksums = Buffer.from(
    files
      .map((file) => {
        const content = archiveFileContent(file);
        totalBytes += content.length;
        if (totalBytes > MAX_FACTORY_RELEASE_BYTES) {
          throw new Error("CLI release exceeds the total size limit");
        }
        const sha256 = createHash("sha256").update(content).digest("hex");
        return `${sha256}  ${relativePath(file.path)}\n`;
      })
      .join(""),
  );
  const executables = Buffer.from(
    files
      .filter((file) => (file.mode & 0o111) !== 0)
      .map((file) => `./${relativePath(file.path)}\n`)
      .sort()
      .join(""),
  );
  const checksumsSha256 = createHash("sha256")
    .update(checksums)
    .digest("hex");
  const executablesSha256 = createHash("sha256")
    .update(executables)
    .digest("hex");
  const root = Buffer.from(
    `${checksumsSha256}  ${CHECKSUM_MANIFEST}\n` +
      `${executablesSha256}  ${EXECUTABLE_MANIFEST}\n`,
  );
  const treeSha256 = createHash("sha256").update(root).digest("hex");
  const integrityFiles: ArchiveFile[] = [
    {
      path: `${ARCHIVE_ROOT}/${CHECKSUM_MANIFEST}`,
      content: checksums,
      mode: 0o644,
    },
    {
      path: `${ARCHIVE_ROOT}/${EXECUTABLE_MANIFEST}`,
      content: executables,
      mode: 0o644,
    },
    {
      path: `${ARCHIVE_ROOT}/${ROOT_MANIFEST}`,
      content: root,
      mode: 0o644,
    },
  ];
  return {
    files: [...files, ...integrityFiles].sort((left, right) =>
      left.path.localeCompare(right.path)
    ),
    treeSha256,
  };
}

function writeString(target: Buffer, value: string, offset: number, length: number): void {
  const bytes = Buffer.from(value, "utf8");
  if (bytes.length > length) throw new Error(`tar field is too long: ${value}`);
  bytes.copy(target, offset);
}

function writeOctal(target: Buffer, value: number, offset: number, length: number): void {
  const encoded = value.toString(8).padStart(length - 1, "0") + "\0";
  writeString(target, encoded, offset, length);
}

function tarName(path: string): { name: string; prefix: string } {
  if (Buffer.byteLength(path) <= 100) return { name: path, prefix: "" };
  for (let index = path.lastIndexOf("/"); index > 0; index = path.lastIndexOf("/", index - 1)) {
    const prefix = path.slice(0, index);
    const name = path.slice(index + 1);
    if (Buffer.byteLength(name) <= 100 && Buffer.byteLength(prefix) <= 155) return { name, prefix };
  }
  throw new Error(`path cannot be represented in a ustar archive: ${path}`);
}

function header(path: string, mode: number, size: number, type: "0" | "5"): Buffer {
  const block = Buffer.alloc(512, 0);
  const names = tarName(path);
  writeString(block, names.name, 0, 100);
  writeOctal(block, mode, 100, 8);
  writeOctal(block, 0, 108, 8);
  writeOctal(block, 0, 116, 8);
  writeOctal(block, size, 124, 12);
  writeOctal(block, 0, 136, 12);
  block.fill(0x20, 148, 156);
  writeString(block, type, 156, 1);
  writeString(block, "ustar\0", 257, 6);
  writeString(block, "00", 263, 2);
  writeString(block, "root", 265, 32);
  writeString(block, "root", 297, 32);
  writeString(block, names.prefix, 345, 155);
  const checksum = block.reduce((sum, byte) => sum + byte, 0);
  writeString(block, checksum.toString(8).padStart(6, "0") + "\0 ", 148, 8);
  return block;
}

function archiveBytes(files: readonly ArchiveFile[]): Buffer {
  const directories = new Set<string>();
  for (const file of files) {
    let path = dirname(file.path).split(sep).join("/");
    while (path !== "." && path !== "/") {
      directories.add(`${path}/`);
      const parent = dirname(path).split(sep).join("/");
      if (parent === path) break;
      path = parent;
    }
  }
  const blocks: Buffer[] = [];
  for (const directory of [...directories].sort()) blocks.push(header(directory, 0o755, 0, "5"));
  for (const file of files) {
    const content = archiveFileContent(file);
    blocks.push(header(file.path, file.mode, content.length, "0"));
    blocks.push(content);
    const padding = (512 - (content.length % 512)) % 512;
    if (padding > 0) blocks.push(Buffer.alloc(padding));
  }
  blocks.push(Buffer.alloc(1_024));
  return gzipSync(Buffer.concat(blocks), { level: 9 });
}

function option(arguments_: readonly string[], name: string, fallback: string): string {
  const index = arguments_.indexOf(name);
  if (index === -1) return fallback;
  const value = arguments_[index + 1];
  if (!value || value.startsWith("--")) throw new Error(`${name} requires a path`);
  return resolve(value);
}

function stageOutputFile(path: string, content: string | Buffer): string {
  const temporary =
    `${path}.writing-${process.pid}-${randomUUID()}`;
  let descriptor: number | undefined;
  try {
    descriptor = openSync(
      temporary,
      constants.O_WRONLY |
        constants.O_CREAT |
        constants.O_EXCL |
        constants.O_NOFOLLOW,
      0o644,
    );
    writeFileSync(descriptor, content);
    fchmodSync(descriptor, 0o644);
    fsyncSync(descriptor);
    closeSync(descriptor);
    descriptor = undefined;
    return temporary;
  } catch (error) {
    if (descriptor !== undefined) closeSync(descriptor);
    rmSync(temporary, { force: true });
    throw error;
  }
}

export function buildCliRelease(options: { output: string; manifest: string }): {
  output: string;
  manifest: string;
  sha256: string;
  treeSha256: string;
  size: number;
  files: number;
} {
  const gitSnapshot = snapshotGitReleaseInputs(ROOT, INPUTS);
  const integrity = withInstallIntegrity(sourceFiles(gitSnapshot.files));
  const files = integrity.files;
  const archive = archiveBytes(files);
  const source = gitSnapshot.source;
  const sha256 = createHash("sha256").update(archive).digest("hex");
  mkdirSync(dirname(options.output), { recursive: true });
  mkdirSync(dirname(options.manifest), { recursive: true });
  let outputTemporary: string | undefined;
  let manifestTemporary: string | undefined;
  try {
    outputTemporary = stageOutputFile(options.output, archive);
    manifestTemporary = stageOutputFile(
      options.manifest,
      `${JSON.stringify({
        schemaVersion: 1,
        cliVersion: CLI_VERSION,
        artifact: basename(options.output),
        sha256,
        treeSha256: integrity.treeSha256,
        size: archive.length,
        files: files.length,
        source,
      }, null, 2)}\n`,
    );
    renameSync(outputTemporary, options.output);
    outputTemporary = undefined;
    renameSync(manifestTemporary, options.manifest);
    manifestTemporary = undefined;
  } finally {
    if (outputTemporary !== undefined) {
      rmSync(outputTemporary, { force: true });
    }
    if (manifestTemporary !== undefined) {
      rmSync(manifestTemporary, { force: true });
    }
  }
  return {
    output: options.output,
    manifest: options.manifest,
    sha256,
    treeSha256: integrity.treeSha256,
    size: archive.length,
    files: files.length,
  };
}

if (import.meta.main) {
  const output = option(Bun.argv.slice(2), "--output", join(ROOT, "dist", `${ARCHIVE_ROOT}.tar.gz`));
  const manifest = option(Bun.argv.slice(2), "--manifest", join(ROOT, "dist", `${ARCHIVE_ROOT}.json`));
  console.log(JSON.stringify(buildCliRelease({ output, manifest })));
}
