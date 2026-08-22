import { createHash } from "node:crypto";
import { access, lstat, mkdir, open, readFile, readdir, stat } from "node:fs/promises";
import { constants } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";

const SAFE_PATH = /^[A-Za-z0-9._/-]+$/;

export function validateArchivePaths(listing) {
  const names = listing.split("\n").filter(Boolean);
  if (!names.length || names.length > 100_000) throw new Error("native release archive has an invalid entry count");
  for (const name of names) {
    const normalized = name.replace(/^\.\//, "").replace(/\/$/, "");
    if (!normalized || normalized.startsWith("/") || !SAFE_PATH.test(normalized)
      || normalized.split("/").some((part) => !part || part === "." || part === "..")) {
      throw new Error("native release archive contains an unsafe path");
    }
  }
  return names;
}

export async function extractVerifiedArchive(archive, destination) {
  const table = spawnSync("/usr/bin/tar", ["-tzf", archive], { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
  if (table.status !== 0) throw new Error("native release archive could not be listed");
  validateArchivePaths(table.stdout);
  const verbose = spawnSync("/usr/bin/tar", ["-tvzf", archive], { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 });
  if (verbose.status !== 0 || verbose.stdout.split("\n").filter(Boolean).some((line) => !["-", "d"].includes(line[0]))) {
    throw new Error("native release archive contains links or unsupported entries");
  }
  await mkdir(destination, { mode: 0o700 });
  const extracted = spawnSync("/usr/bin/tar", ["-xzf", archive, "-C", destination, "--no-same-owner"], { encoding: "utf8" });
  if (extracted.status !== 0) throw new Error("native release archive extraction failed");
  const roots = (await readdir(destination)).filter((name) => !name.startsWith("."));
  if (roots.length !== 1) throw new Error("native release archive must contain one root directory");
  const root = path.join(destination, roots[0]);
  if (!(await lstat(root)).isDirectory()) throw new Error("native release root is not a directory");
  return root;
}

async function walk(root, current = root, output = []) {
  for (const entry of await readdir(current, { withFileTypes: true })) {
    const absolute = path.join(current, entry.name);
    const relative = path.relative(root, absolute).split(path.sep).join("/");
    const metadata = await lstat(absolute);
    if (metadata.isSymbolicLink()) throw new Error("native release contains a symlink");
    if (metadata.isDirectory()) await walk(root, absolute, output);
    else if (metadata.isFile()) output.push(relative);
    else throw new Error("native release contains an unsupported file");
  }
  return output;
}

export async function verifyReleaseTree(root, version, target) {
  const release = JSON.parse(await readFile(path.join(root, "RELEASE.json"), "utf8"));
  if (release.schema !== "tohseno.release/1" || release.version !== version || release.target !== target
    || release.channel !== "stable" || release.prerelease !== false || release.dirty !== false) {
    throw new Error("native RELEASE.json does not match the authorized release");
  }
  const manifestBytes = await readFile(path.join(root, "CHECKSUMS.sha256"));
  const manifest = manifestBytes.toString("ascii");
  if (!manifest.endsWith("\n")) throw new Error("native checksum manifest is not canonical");
  const expected = new Map();
  for (const line of manifest.trimEnd().split("\n")) {
    const match = /^([0-9a-f]{64})  ([A-Za-z0-9._/-]+)$/.exec(line);
    if (!match || match[2] === "CHECKSUMS.sha256" || expected.has(match[2])
      || match[2].split("/").some((part) => !part || part === "." || part === "..")) {
      throw new Error("native checksum manifest is invalid");
    }
    expected.set(match[2], match[1]);
  }
  const actual = (await walk(root)).filter((name) => name !== "CHECKSUMS.sha256").sort();
  if (actual.length !== expected.size || actual.some((name, index) => name !== [...expected.keys()].sort()[index])) {
    throw new Error("native checksum manifest does not cover the exact release tree");
  }
  for (const [name, digest] of expected) {
    const bytes = await readFile(path.join(root, name));
    if (createHash("sha256").update(bytes).digest("hex") !== digest) throw new Error("native release tree checksum differs");
  }
  const launcher = path.join(root, "bin", "tohseno");
  const launcherMetadata = await stat(launcher);
  if (!launcherMetadata.isFile() || (launcherMetadata.mode & 0o022) !== 0) {
    throw new Error("native release launcher is missing or writable by another user");
  }
  await access(launcher, constants.X_OK);
  const descriptor = await open(launcher, "r");
  await descriptor.close();
  return release;
}
