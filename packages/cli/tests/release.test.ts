import { describe, expect, test } from "bun:test";
import {
  appendFileSync,
  chmodSync,
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
} from "node:fs";
import { join } from "node:path";
import {
  listCachedReleaseDirectories,
  prepareFactoryRelease,
  useActiveCachedRelease,
  verifyReleaseDirectory,
} from "../src/release.ts";
import { REPOSITORY_ROOT, withScratchEnvironment } from "./helpers.ts";

describe("immutable factory releases", () => {
  test("creates, verifies, and exactly reuses a deterministic release", async () => {
    await withScratchEnvironment(async (scratch) => {
      const releases = join(scratch.factoryHome, "cache", "releases");
      const first = await prepareFactoryRelease(REPOSITORY_ROOT, releases);
      expect(first.reused).toBe(false);
      expect(first.directory).toBe(join(releases, first.metadata.releaseId));
      expect(first.metadata.bundleDigest).toMatch(/^[0-9a-f]{64}$/);
      expect(first.metadata.files.length).toBeGreaterThan(20);
      expect(first.metadata.files.some((file) => file.path === "factory/cli/src/release.ts")).toBe(true);
      expect(verifyReleaseDirectory(first.directory)).toEqual(first.metadata);
      expect(lstatSync(first.directory).mode & 0o222).toBe(0);
      expect(lstatSync(join(first.directory, "release.json")).mode & 0o222).toBe(0);

      const second = await prepareFactoryRelease(REPOSITORY_ROOT, releases);
      expect(second.reused).toBe(true);
      expect(second.directory).toBe(first.directory);
      expect(second.metadata).toEqual(first.metadata);
      expect(listCachedReleaseDirectories(releases)).toEqual([first.directory]);
      expect(useActiveCachedRelease(releases).directory).toBe(first.directory);

      const pointer = JSON.parse(
        readFileSync(join(scratch.factoryHome, "cache", "active-release.json"), "utf8"),
      ) as { schemaVersion: number; releaseId: string };
      expect(pointer).toEqual({ schemaVersion: 1, releaseId: first.metadata.releaseId });
      expect(readdirSync(releases).some((name) => name.startsWith(".build-"))).toBe(false);
    });
  }, 20_000);

  test("allows concurrent builders to converge on one verified release", async () => {
    await withScratchEnvironment(async (scratch) => {
      const releases = join(scratch.factoryHome, "cache", "releases");
      const [left, right] = await Promise.all([
        prepareFactoryRelease(REPOSITORY_ROOT, releases),
        prepareFactoryRelease(REPOSITORY_ROOT, releases),
      ]);
      expect(left.metadata.releaseId).toBe(right.metadata.releaseId);
      expect(left.metadata.bundleDigest).toBe(right.metadata.bundleDigest);
      expect([left.reused, right.reused].sort()).toEqual([false, true]);
      expect(listCachedReleaseDirectories(releases)).toHaveLength(1);
      expect(readdirSync(releases).some((name) => name.startsWith(".build-"))).toBe(false);
      expect(verifyReleaseDirectory(left.directory).bundleDigest).toBe(left.metadata.bundleDigest);
    });
  }, 20_000);

  test("detects corruption and never heals or mutates it silently", async () => {
    await withScratchEnvironment(async (scratch) => {
      const releases = join(scratch.factoryHome, "cache", "releases");
      const prepared = await prepareFactoryRelease(REPOSITORY_ROOT, releases);
      const victim = join(prepared.directory, "manifest", "validate.ts");
      chmodSync(victim, 0o644);
      appendFileSync(victim, "\n// corrupted by release integrity test\n");

      expect(() => verifyReleaseDirectory(prepared.directory)).toThrow("factory release is corrupt");
      await expect(prepareFactoryRelease(REPOSITORY_ROOT, releases)).rejects.toThrow(
        "factory release is corrupt",
      );
      expect(readFileSync(victim, "utf8")).toContain("corrupted by release integrity test");
      expect(existsSync(prepared.directory)).toBe(true);
      expect(readdirSync(releases).some((name) => name.startsWith(".build-"))).toBe(false);
    });
  }, 20_000);
});
