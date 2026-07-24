import {
  chmodSync,
  closeSync,
  constants,
  copyFileSync,
  fchmodSync,
  fstatSync,
  lstatSync,
  mkdirSync,
  openSync,
  readSync,
  readlinkSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { CliError } from "./errors.ts";

const FORBIDDEN_FILE = /(?:^|\/)(?:MASTER_PROMPT\.md|Local\.xcconfig|app\.config\.json|\.env(?:\..*)?)$|\.(?:p8|p12|pem|pfx|mobileprovision)$/iu;
export const MAX_LOCAL_JSON_BYTES = 1_048_576;

export interface TreeFile {
  absolutePath: string;
  relativePath: string;
  executable: boolean;
}

export function readBoundedRegularFile(
  path: string,
  maximumBytes: number,
  label = path,
): Buffer {
  let descriptor: number | undefined;
  try {
    const before = lstatSync(path);
    if (
      before.isSymbolicLink() ||
      !before.isFile() ||
      before.nlink !== 1 ||
      before.size > maximumBytes
    ) {
      throw new Error("unsafe or oversized file");
    }
    descriptor = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW);
    const opened = fstatSync(descriptor);
    if (
      !opened.isFile() ||
      opened.nlink !== 1 ||
      opened.dev !== before.dev ||
      opened.ino !== before.ino ||
      opened.size > maximumBytes
    ) {
      throw new Error("file identity changed while opening");
    }
    const chunks: Buffer[] = [];
    const buffer = Buffer.allocUnsafe(65_536);
    let total = 0;
    while (true) {
      const length = readSync(descriptor, buffer, 0, buffer.length, null);
      if (length === 0) break;
      total += length;
      if (total > maximumBytes) throw new Error("file grew past its limit");
      chunks.push(Buffer.from(buffer.subarray(0, length)));
    }
    return Buffer.concat(chunks, total);
  } catch (error) {
    if (error instanceof CliError) throw error;
    throw new CliError(
      `${label} must be a regular file with one link and no more than ${maximumBytes} bytes`,
      2,
    );
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
}

export function readBoundedUtf8(
  path: string,
  maximumBytes = MAX_LOCAL_JSON_BYTES,
  label = path,
): string {
  const bytes = readBoundedRegularFile(path, maximumBytes, label);
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new CliError(`${label} must contain valid UTF-8`, 2);
  }
}

export function readBoundedJson<T>(
  path: string,
  maximumBytes = MAX_LOCAL_JSON_BYTES,
  label = path,
): T {
  try {
    return JSON.parse(readBoundedUtf8(path, maximumBytes, label)) as T;
  } catch (error) {
    if (error instanceof CliError) throw error;
    throw new CliError(`${label} must contain valid JSON`, 2);
  }
}

export function assertSafeBundlePath(path: string): void {
  const normalized = path.split(sep).join("/");
  if (FORBIDDEN_FILE.test(normalized)) {
    throw new CliError(`refusing to include private or credential-bearing file ${normalized}`);
  }
  if (normalized.split("/").some((part) => part === ".git" || part === "node_modules")) {
    throw new CliError(`refusing to include factory-only directory in release: ${normalized}`);
  }
}

export function listRegularFiles(root: string): TreeFile[] {
  const absoluteRoot = resolve(root);
  const rootDetails = lstatSync(absoluteRoot);
  if (rootDetails.isSymbolicLink() || !rootDetails.isDirectory()) {
    throw new CliError(
      `factory release root must be a real directory: ${absoluteRoot}`,
    );
  }
  const files: TreeFile[] = [];

  function visit(directory: string): void {
    const entries = readdirSync(directory, { withFileTypes: true })
      .sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const absolutePath = join(directory, entry.name);
      const relativePath = relative(absoluteRoot, absolutePath).split(sep).join("/");
      if (entry.isSymbolicLink()) {
        throw new CliError(`symbolic links are not allowed in factory releases: ${relativePath}`);
      }
      if (entry.isDirectory()) {
        visit(absolutePath);
      } else if (entry.isFile()) {
        assertSafeBundlePath(relativePath);
        const details = lstatSync(absolutePath);
        if (!details.isFile() || details.nlink !== 1) {
          throw new CliError(
            `hardlinked files are not allowed in factory releases: ${relativePath}`,
          );
        }
        const mode = details.mode;
        files.push({ absolutePath, relativePath, executable: (mode & 0o111) !== 0 });
      } else {
        throw new CliError(`unsupported filesystem entry in factory release: ${relativePath}`);
      }
    }
  }

  visit(absoluteRoot);
  return files;
}

export function copyRegularFile(source: string, destination: string, executable: boolean): void {
  mkdirSync(dirname(destination), { recursive: true });
  copyFileSync(source, destination);
  chmodSync(destination, executable ? 0o755 : 0o644);
}

export function copyTree(source: string, destination: string): void {
  mkdirSync(destination, { recursive: true });
  for (const file of listRegularFiles(source)) {
    copyRegularFile(file.absolutePath, join(destination, file.relativePath), file.executable);
  }
}

export function makeTreeReadOnly(root: string): void {
  const directories: string[] = [];
  function visit(directory: string): void {
    directories.push(directory);
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile()) {
        const executable = (lstatSync(path).mode & 0o111) !== 0;
        chmodSync(path, executable ? 0o555 : 0o444);
      }
    }
  }
  visit(root);
  for (const directory of directories.reverse()) chmodSync(directory, 0o555);
}

export function removeTreeEvenIfReadOnly(root: string): void {
  try {
    function makeWritable(directory: string): void {
      const before = lstatSync(directory);
      if (before.isSymbolicLink() || !before.isDirectory()) return;

      let descriptor: number | undefined;
      try {
        descriptor = openSync(
          directory,
          constants.O_RDONLY | constants.O_DIRECTORY | constants.O_NOFOLLOW,
        );
        const opened = fstatSync(descriptor);
        if (
          !opened.isDirectory() ||
          opened.dev !== before.dev ||
          opened.ino !== before.ino
        ) {
          return;
        }
        fchmodSync(descriptor, 0o755);
      } finally {
        if (descriptor !== undefined) closeSync(descriptor);
      }

      for (const entry of readdirSync(directory, { withFileTypes: true })) {
        const path = join(directory, entry.name);
        if (entry.isDirectory()) makeWritable(path);
      }
    }
    makeWritable(root);
  } catch {
    // The tree may be partial or already absent; rm below remains authoritative.
  }
  rmSync(root, { recursive: true, force: true });
}

export function assertNoExternalSymlinks(root: string): void {
  const absoluteRoot = resolve(root);
  function insideRoot(path: string): boolean {
    const pathFromRoot = relative(absoluteRoot, path);
    return pathFromRoot === "" || (!pathFromRoot.startsWith(`..${sep}`) && pathFromRoot !== "..");
  }
  function visit(directory: string): void {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (entry.name === ".git") continue;
      const path = join(directory, entry.name);
      if (entry.isSymbolicLink()) {
        const resolved = resolve(dirname(path), readlinkSync(path));
        if (!insideRoot(resolved)) {
          throw new CliError(`shot contains a symbolic link outside its repository: ${relative(absoluteRoot, path)}`);
        }
      } else if (entry.isDirectory()) {
        visit(path);
      }
    }
  }
  visit(absoluteRoot);
}
