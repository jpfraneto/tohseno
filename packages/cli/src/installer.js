import { spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import {
  chmod, cp, lstat, mkdir, mkdtemp, readFile, rename, rm, symlink, writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  MANIFEST_ORIGINS, MANIFEST_URL, MAX_MANIFEST_BYTES, NPM_CLI_VERSION,
} from "./constants.js";
import { extractVerifiedArchive, verifyReleaseTree } from "./archive.js";
import { downloadArtifact, fetchBounded } from "./download.js";
import { nodeArchitecture, validateManifest } from "./manifest.js";
import { installRoot } from "./native.js";

async function realDirectory(directory, mode = 0o700) {
  try {
    const metadata = await lstat(directory);
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) throw new Error("installer-owned path is unsafe");
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    await mkdir(directory, { mode });
  }
}

async function replaceableFile(pathname, allowSymlink = false) {
  try {
    const metadata = await lstat(pathname);
    if ((allowSymlink && metadata.isSymbolicLink()) || (metadata.isFile() && !metadata.isSymbolicLink())) return;
    throw new Error("installer-owned replacement path is unsafe");
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
}

async function safeHome() {
  const home = os.homedir();
  if (!path.isAbsolute(home)) throw new Error("the Mac home directory is not absolute");
  let cursor = path.parse(home).root;
  for (const component of home.slice(cursor.length).split(path.sep).filter(Boolean)) {
    cursor = path.join(cursor, component);
    const metadata = await lstat(cursor);
    if (metadata.isSymbolicLink()) throw new Error("the Mac home directory contains a symlink");
  }
  return home;
}

function verifyAppleSignature(binary, signing) {
  if (signing.kind !== "apple-developer-id") return;
  const verified = spawnSync("/usr/bin/codesign", ["--verify", "--deep", "--strict", "-R", signing.designated_requirement, binary], { encoding: "utf8" });
  if (verified.status !== 0) throw new Error("native Apple signature verification failed");
  const details = spawnSync("/usr/bin/codesign", ["-dv", "--verbose=4", binary], { encoding: "utf8" });
  if (details.status !== 0 || !details.stderr.includes(`TeamIdentifier=${signing.team_id}`)) {
    throw new Error("native Apple signing Team ID differs from the release manifest");
  }
}

const LAUNCHER = `#!/bin/sh
set -eu
launcher_directory="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)"
launcher_root="$(dirname -- "$launcher_directory")"
release_root="$(CDPATH= cd -- "$launcher_root/current" && pwd -P)" || {
  printf '%s\n' "tohseno: release pointer is missing or broken" >&2
  exit 1
}
case "$release_root/" in
  "$launcher_root"/releases/*/) ;;
  *) printf '%s\n' "tohseno: release pointer escapes its installation root" >&2; exit 1 ;;
esac
native="$release_root/bin/tohseno"
if [ -L "$native" ] || [ ! -f "$native" ] || [ ! -x "$native" ]; then
  printf '%s\n' "tohseno: installed executable is missing or unsafe" >&2
  exit 1
fi
exec "$native" "$@"
`;

export async function installAuthorizedNative() {
  await safeHome();
  const manifestBytes = await fetchBounded(MANIFEST_URL, MAX_MANIFEST_BYTES, MANIFEST_ORIGINS);
  let decoded;
  try { decoded = new TextDecoder("utf-8", { fatal: true }).decode(manifestBytes); }
  catch { throw new Error("release manifest is not UTF-8"); }
  if (decoded.charCodeAt(0) === 0xfeff) throw new Error("release manifest has a noncanonical byte-order mark");
  let parsed;
  try { parsed = JSON.parse(decoded); } catch { throw new Error("release manifest is not valid JSON"); }
  const manifest = validateManifest(parsed, NPM_CLI_VERSION, nodeArchitecture());

  const temporary = await mkdtemp(path.join(os.tmpdir(), "tohseno-native-"));
  try {
    const archive = path.join(temporary, "release.tar.gz");
    await downloadArtifact(manifest.artifact.url, archive, manifest.artifact.byte_size, manifest.artifact.sha256);
    const extracted = await extractVerifiedArchive(archive, path.join(temporary, "extracted"));
    await verifyReleaseTree(extracted, manifest.native_release_version, manifest.artifact.target);
    verifyAppleSignature(path.join(extracted, "bin", "tohseno"), manifest.artifact.signing);

    const root = installRoot();
    await realDirectory(root);
    const releases = path.join(root, "releases");
    const binaries = path.join(root, "bin");
    await realDirectory(releases);
    await realDirectory(binaries, 0o755);
    const releaseName = `${manifest.native_release_version}-${manifest.artifact.target}-${manifest.artifact.sha256.slice(0, 12)}`;
    const finalRelease = path.join(releases, releaseName);
    try {
      const existing = await lstat(finalRelease);
      if (!existing.isDirectory() || existing.isSymbolicLink()) throw new Error("installed native release path is unsafe");
      await verifyReleaseTree(finalRelease, manifest.native_release_version, manifest.artifact.target);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      const stage = await mkdtemp(path.join(releases, ".release-stage-"));
      try {
        await cp(extracted, stage, { recursive: true, errorOnExist: true, force: false, preserveTimestamps: true, verbatimSymlinks: true });
        await verifyReleaseTree(stage, manifest.native_release_version, manifest.artifact.target);
        verifyAppleSignature(path.join(stage, "bin", "tohseno"), manifest.artifact.signing);
        await rename(stage, finalRelease);
      } catch (stageError) {
        await rm(stage, { recursive: true, force: true });
        throw stageError;
      }
    }

    const nonce = randomBytes(12).toString("hex");
    await replaceableFile(path.join(root, "current"), true);
    await replaceableFile(path.join(binaries, "tohseno"));
    const currentStage = path.join(root, `.current-${nonce}`);
    await symlink(path.join("releases", releaseName), currentStage);
    await rename(currentStage, path.join(root, "current"));
    const launcherStage = path.join(binaries, `.tohseno-${nonce}`);
    await writeFile(launcherStage, LAUNCHER, { flag: "wx", mode: 0o755 });
    await chmod(launcherStage, 0o755);
    await rename(launcherStage, path.join(binaries, "tohseno"));
    await writeFile(path.join(root, ".installer-managed"), "tohseno-stable-install-v2\n", { mode: 0o600 });
    return { version: manifest.native_release_version, launcher: path.join(binaries, "tohseno") };
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}
