import { createHash, randomUUID } from "node:crypto";
import {
  closeSync,
  constants,
  fchmodSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { gzipSync } from "node:zlib";
import { basename, dirname, join, resolve, sep } from "node:path";
import {
  CLI_VERSION,
  MAX_FACTORY_RELEASE_BYTES,
  MAX_FACTORY_RELEASE_FILE_BYTES,
  MAX_FACTORY_RELEASE_FILES,
} from "../src/constants.ts";
import {
  listRegularFiles,
  readBoundedRegularFile,
  readBoundedUtf8,
} from "../src/files.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const ARCHIVE_ROOT = `tohseno-cli-${CLI_VERSION}`;
const INPUTS = [
  "LICENSE",
  "skills/continuity-app",
  "packages/manifest/cli.ts",
  "packages/manifest/continuity.manifest.schema.json",
  "packages/manifest/types.ts",
  "packages/manifest/validate.ts",
  "packages/cli/src",
  "packages/cli/factory",
  "packages/cli/package.json",
  "packages/cli/THIRD_PARTY_NOTICES.md",
  "templates/continuity-app",
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
  source?: string;
  content?: Buffer;
  mode: number;
}

const CHECKSUM_MANIFEST = ".tohseno-install-checksums-v1";
const EXECUTABLE_MANIFEST = ".tohseno-install-executables-v1";
const ROOT_MANIFEST = ".tohseno-install-root-v1";

export function assertThirdPartyPackageIdentity(options: {
  directory: string;
  packageName: string;
  version: string;
  treeSha256: string;
}): void {
  const directoryDetails = lstatSync(options.directory);
  if (
    directoryDetails.isSymbolicLink() ||
    !directoryDetails.isDirectory()
  ) {
    throw new Error(
      `managed release dependency is not a real directory: ${options.directory}`,
    );
  }
  const packageJson = join(options.directory, "package.json");
  const packageDetails = lstatSync(packageJson);
  if (packageDetails.isSymbolicLink() || !packageDetails.isFile()) {
    throw new Error(
      `managed release dependency has no regular package.json: ${options.directory}`,
    );
  }
  let value: unknown;
  try {
    value = JSON.parse(
      readBoundedUtf8(
        packageJson,
        65_536,
        "managed dependency package manifest",
      ),
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
  const actualTreeSha256 = thirdPartyTreeSha256(options.directory);
  if (actualTreeSha256 !== options.treeSha256) {
    throw new Error(
      `managed release dependency tree mismatch for ${options.packageName}@${options.version}: expected ${options.treeSha256}, found ${actualTreeSha256}`,
    );
  }
}

export function thirdPartyTreeSha256(directory: string): string {
  const digest = createHash("sha256");
  for (const file of listRegularFiles(directory)) {
    const content = readBoundedRegularFile(
      file.absolutePath,
      MAX_FACTORY_RELEASE_FILE_BYTES,
      "managed release dependency file",
    );
    const fileSha256 = createHash("sha256")
      .update(content)
      .digest("hex");
    digest.update(
      `${file.relativePath.split(sep).join("/")}\0${file.executable ? "755" : "644"}\0${content.length}\0${fileSha256}\n`,
    );
  }
  return digest.digest("hex");
}

function sourceFiles(): ArchiveFile[] {
  const files: ArchiveFile[] = [];
  for (const input of INPUTS) {
    const source = join(ROOT, input);
    const details = lstatSync(source);
    if (details.isFile()) {
      if (details.nlink !== 1) {
        throw new Error(
          `CLI release input must be a single-link regular file: ${input}`,
        );
      }
      files.push({
        path: `${ARCHIVE_ROOT}/factory-source/${input}`,
        source,
        mode: (details.mode & 0o111) === 0 ? 0o644 : 0o755,
      });
      continue;
    }
    for (const file of listRegularFiles(source)) {
      const path = `${input}/${file.relativePath}`.split(sep).join("/");
      files.push({
        path: `${ARCHIVE_ROOT}/factory-source/${path}`,
        source: file.absolutePath,
        mode: file.executable ? 0o755 : 0o644,
      });
    }
  }
  for (const input of THIRD_PARTY_INPUTS) {
    const source = join(ROOT, input.source);
    assertThirdPartyPackageIdentity({
      directory: source,
      packageName: input.packageName,
      version: input.version,
      treeSha256: input.treeSha256,
    });
    for (const file of listRegularFiles(source)) {
      const path = `${input.destination}/${file.relativePath}`.split(sep).join("/");
      files.push({
        path: `${ARCHIVE_ROOT}/factory-source/${path}`,
        source: file.absolutePath,
        mode: file.executable ? 0o755 : 0o644,
      });
    }
  }
  return files.sort((left, right) => left.path.localeCompare(right.path));
}

function archiveFileContent(file: ArchiveFile): Buffer {
  if (file.content !== undefined) return file.content;
  if (file.source !== undefined) {
    return readBoundedRegularFile(
      file.source,
      MAX_FACTORY_RELEASE_FILE_BYTES,
      "CLI release source file",
    );
  }
  throw new Error(`archive file has no content source: ${file.path}`);
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
  const integrity = withInstallIntegrity(sourceFiles());
  const files = integrity.files;
  const archive = archiveBytes(files);
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
