import { spawnSync } from "node:child_process";
import { lstat, readFile, realpath } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

export function installRoot() { return path.join(os.homedir(), ".tohseno"); }
export function nativeLauncher() { return path.join(installRoot(), "bin", "tohseno"); }

export async function installedNative(expectedVersion = null) {
  return installedNativeAt(nativeLauncher(), installRoot(), expectedVersion);
}

export async function installedNativeAt(launcher, root, expectedVersion = null, spawn = spawnSync) {
  try {
    const metadata = await lstat(launcher);
    if (!metadata.isFile() || metadata.isSymbolicLink()) return null;
    const resolved = await realpath(launcher);
    if (!resolved.startsWith(`${root}${path.sep}`)) return null;
    const result = spawn(launcher, ["--version"], { encoding: "utf8", timeout: 10_000 });
    if (result.status !== 0) return null;
    const match = /^tohseno (\d+\.\d+\.\d+)\n?$/.exec(result.stdout);
    if (!match || (expectedVersion && match[1] !== expectedVersion)) return null;
    return { launcher, version: match[1] };
  } catch { return null; }
}

export function delegate(args) {
  const launcher = nativeLauncher();
  const result = spawnSync(launcher, args, { stdio: "inherit" });
  if (result.error) throw new Error("the installed native TOHSENO could not start");
  return result.status ?? 1;
}
