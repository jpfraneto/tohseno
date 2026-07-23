import { createHash } from "node:crypto";
import {
  lstatSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { gzipSync } from "node:zlib";
import { basename, dirname, join, resolve, sep } from "node:path";
import { CLI_VERSION } from "../src/constants.ts";
import { listRegularFiles } from "../src/files.ts";

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
  "templates/continuity-app",
] as const;

interface ArchiveFile {
  path: string;
  source: string;
  mode: number;
}

function sourceFiles(): ArchiveFile[] {
  const files: ArchiveFile[] = [];
  for (const input of INPUTS) {
    const source = join(ROOT, input);
    const details = lstatSync(source);
    if (details.isFile()) {
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
  return files.sort((left, right) => left.path.localeCompare(right.path));
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
    const content = readFileSync(file.source);
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

export function buildCliRelease(options: { output: string; manifest: string }): {
  output: string;
  manifest: string;
  sha256: string;
  size: number;
  files: number;
} {
  const files = sourceFiles();
  const archive = archiveBytes(files);
  const sha256 = createHash("sha256").update(archive).digest("hex");
  mkdirSync(dirname(options.output), { recursive: true });
  mkdirSync(dirname(options.manifest), { recursive: true });
  const outputTemporary = `${options.output}.writing-${process.pid}`;
  const manifestTemporary = `${options.manifest}.writing-${process.pid}`;
  try {
    writeFileSync(outputTemporary, archive, { mode: 0o644 });
    writeFileSync(manifestTemporary, `${JSON.stringify({
      schemaVersion: 1,
      cliVersion: CLI_VERSION,
      artifact: basename(options.output),
      sha256,
      size: archive.length,
      files: files.length,
    }, null, 2)}\n`, { mode: 0o644 });
    renameSync(outputTemporary, options.output);
    renameSync(manifestTemporary, options.manifest);
  } finally {
    rmSync(outputTemporary, { force: true });
    rmSync(manifestTemporary, { force: true });
  }
  return { output: options.output, manifest: options.manifest, sha256, size: archive.length, files: files.length };
}

if (import.meta.main) {
  const output = option(Bun.argv.slice(2), "--output", join(ROOT, "dist", `${ARCHIVE_ROOT}.tar.gz`));
  const manifest = option(Bun.argv.slice(2), "--manifest", join(ROOT, "dist", `${ARCHIVE_ROOT}.json`));
  console.log(JSON.stringify(buildCliRelease({ output, manifest })));
}
