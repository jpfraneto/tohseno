import { describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { main } from "../src/cli.ts";
import type { ShotMetadata } from "../src/shot.ts";
import {
  createMemoryIo,
  initializeCompatibleProject,
  REPOSITORY_ROOT,
  runGit,
  withScratchEnvironment,
} from "./helpers.ts";

describe("safe project adoption", () => {
  test("requires explicit confirmation and cancellation changes nothing", async () => {
    await withScratchEnvironment(async (scratch) => {
      const project = await initializeCompatibleProject(scratch);
      const packageBefore = readFileSync(join(project, "package.json"), "utf8");
      const statusBefore = await runGit(["status", "--porcelain"], project, scratch.environment);
      expect(statusBefore.stdout).toBe("");

      let io = createMemoryIo();
      let exitCode = await main([
        "adopt", project, "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(2);
      expect(io.stderr.join("\n")).toContain("requires explicit confirmation");
      expect(existsSync(join(project, ".tohseno"))).toBe(false);
      expect(existsSync(join(scratch.factoryHome, "cache"))).toBe(false);

      io = createMemoryIo(true, ["no"]);
      exitCode = await main(["adopt", project], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(0);
      expect(io.stdout.join("\n")).toContain("Adoption cancelled");
      expect(existsSync(join(project, ".tohseno"))).toBe(false);
      expect(readFileSync(join(project, "package.json"), "utf8")).toBe(packageBefore);
      expect((await runGit(["status", "--porcelain"], project, scratch.environment)).stdout).toBe("");
    });
  });

  test("adopts in place by adding only pinned metadata and validation tools", async () => {
    await withScratchEnvironment(async (scratch) => {
      const project = await initializeCompatibleProject(scratch, "compatible project with spaces");
      const packageBefore = readFileSync(join(project, "package.json"), "utf8");
      const commitBefore = (await runGit(["rev-parse", "HEAD"], project, scratch.environment)).stdout.trim();
      const io = createMemoryIo();
      const exitCode = await main([
        "adopt", project, "--yes", "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(0);
      expect(io.stderr).toEqual([]);
      expect(io.stdout.join("\n")).toContain("Adopted");
      expect(existsSync(join(project, ".tohseno", "verify.ts"))).toBe(true);
      expect(existsSync(join(project, ".tohseno", "manifest", "validate.ts"))).toBe(true);
      expect(existsSync(join(project, "AGENTS.md"))).toBe(false);
      expect(readFileSync(join(project, "package.json"), "utf8")).toBe(packageBefore);

      const metadata = JSON.parse(
        readFileSync(join(project, ".tohseno", "shot.json"), "utf8"),
      ) as ShotMetadata;
      expect(metadata.adopted).toBe(true);
      expect(metadata.selectedAgent).toBeNull();
      expect(metadata.baselineAuthor).toBe("existing-history");
      expect(metadata.slug).toBe("compatible-project-with-spaces");
      expect((await runGit(["rev-parse", "HEAD"], project, scratch.environment)).stdout.trim()).toBe(commitBefore);
      expect((await runGit(["status", "--porcelain"], project, scratch.environment)).stdout).toBe("?? .tohseno/\n");

      const verifyIo = createMemoryIo();
      expect(await main(["verify", project], {
        cwd: scratch.root,
        environment: scratch.environment,
        io: verifyIo,
        sourceRoot: join(scratch.root, "unavailable source"),
      })).toBe(0);

      const secondIo = createMemoryIo();
      expect(await main(["adopt", project, "--no-interactive"], {
        cwd: scratch.root,
        environment: scratch.environment,
        io: secondIo,
        sourceRoot: REPOSITORY_ROOT,
      })).toBe(0);
      expect(secondIo.stdout).toEqual([`Already a recognized shot: ${project}`]);
      expect((await runGit(["status", "--porcelain"], project, scratch.environment)).stdout).toBe("?? .tohseno/\n");
    });
  }, 30_000);

  test("rejects incompatible projects without adding shot metadata", async () => {
    await withScratchEnvironment(async (scratch) => {
      const project = join(scratch.root, "not-an-ios-app");
      mkdirSync(project);
      writeFileSync(join(project, "owner-file.txt"), "unchanged\n");
      const io = createMemoryIo();
      const exitCode = await main([
        "adopt", project, "--yes", "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(1);
      expect(io.stderr.join("\n")).toContain("project is not a compatible iOS base");
      expect(readFileSync(join(project, "owner-file.txt"), "utf8")).toBe("unchanged\n");
      expect(existsSync(join(project, ".tohseno"))).toBe(false);
      expect(readdirSync(project)).toEqual(["owner-file.txt"]);
    });
  }, 30_000);

  test("rolls back metadata if pinned verification detects a tracked private file", async () => {
    await withScratchEnvironment(async (scratch) => {
      const project = await initializeCompatibleProject(scratch, "unsafe-existing-app");
      writeFileSync(join(project, ".env"), "TEST_ONLY_SECRET=never-copy-or-log\n");
      expect((await runGit(["add", "-f", ".env"], project, scratch.environment)).exitCode).toBe(0);
      expect((await runGit([
        "-c", "commit.gpgSign=false",
        "-c", "user.name=CLI Test",
        "-c", "user.email=cli-test@tohseno.local",
        "commit", "--quiet", "--no-verify", "-m", "unsafe test fixture",
      ], project, scratch.environment)).exitCode).toBe(0);

      const io = createMemoryIo();
      const exitCode = await main([
        "adopt", project, "--yes", "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(1);
      expect(io.stderr.join("\n")).toContain("adoption failed without changing the app");
      expect(io.stderr.join("\n")).not.toContain("never-copy-or-log");
      expect(existsSync(join(project, ".tohseno"))).toBe(false);
      expect(readdirSync(project).some((name) => name.startsWith(".tohseno-adopting-"))).toBe(false);
      expect(readFileSync(join(project, ".env"), "utf8")).toBe("TEST_ONLY_SECRET=never-copy-or-log\n");
    });
  }, 30_000);
});
