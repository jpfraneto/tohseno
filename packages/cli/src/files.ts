import {
  chmodSync,
  copyFileSync,
  lstatSync,
  mkdirSync,
  readlinkSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { CliError } from "./errors.ts";

const FORBIDDEN_FILE = /(?:^|\/)(?:MASTER_PROMPT\.md|Local\.xcconfig|app\.config\.json|\.env(?:\..*)?)$|\.(?:p8|p12|pem|pfx|mobileprovision)$/iu;

export interface TreeFile {
  absolutePath: string;
  relativePath: string;
  executable: boolean;
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
        const mode = lstatSync(absolutePath).mode;
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
      chmodSync(directory, 0o755);
      for (const entry of readdirSync(directory, { withFileTypes: true })) {
        const path = join(directory, entry.name);
        if (entry.isDirectory()) makeWritable(path);
        else if (!entry.isSymbolicLink()) chmodSync(path, 0o644);
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
