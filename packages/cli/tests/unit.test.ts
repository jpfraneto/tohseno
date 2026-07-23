import { describe, expect, test } from "bun:test";
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  detectInstalledAgents,
  requireInstalledAgent,
} from "../src/agents.ts";
import { main } from "../src/cli.ts";
import { resolveConfig } from "../src/config.ts";
import {
  bundleIdForSlug,
  displayNameForSlug,
  validateShotSlug,
} from "../src/slug.ts";
import {
  createMemoryIo,
  REPOSITORY_ROOT,
  withScratchEnvironment,
  writeExecutable,
} from "./helpers.ts";

describe("shot slugs", () => {
  test("accepts canonical filesystem-safe slugs and derives identity", () => {
    for (const slug of ["a", "the-trenches", "app2", "2fast-4you", "a".repeat(63)]) {
      expect(validateShotSlug(slug)).toBe(slug);
    }
    expect(displayNameForSlug("the-trenches")).toBe("The Trenches");
    expect(bundleIdForSlug("the-trenches")).toBe("com.tohseno.the-trenches");
  });

  test("rejects traversal, ambiguous separators, Unicode, and overlong names", () => {
    for (const slug of [
      "",
      ".",
      "..",
      "../escape",
      "has/slash",
      "UPPER",
      "under_score",
      "leading-",
      "-trailing",
      "double--hyphen",
      "with space",
      "café",
      "a".repeat(64),
    ]) {
      expect(() => validateShotSlug(slug), slug).toThrow();
    }
  });
});

describe("configuration resolution", () => {
  test("defaults entirely inside the injected home", async () => {
    await withScratchEnvironment((scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: { HOME: scratch.home },
      });
      expect(config.factoryHome).toBe(join(scratch.home, ".tohseno"));
      expect(config.cacheDirectory).toBe(join(scratch.home, ".tohseno", "cache", "releases"));
      expect(config.shotsDirectory).toBe(join(scratch.home, "tohseno", "shots"));
      expect(config.configExists).toBe(false);
    });
  });

  test("resolves config-relative and tilde paths with documented precedence", async () => {
    await withScratchEnvironment((scratch) => {
      mkdirSync(scratch.factoryHome, { recursive: true });
      writeFileSync(join(scratch.factoryHome, "config.json"), JSON.stringify({
        schemaVersion: 1,
        shotsDirectory: "configured shots",
      }));

      const fromConfig = resolveConfig({
        cwd: scratch.root,
        environment: {
          HOME: scratch.home,
          TOHSENO_HOME: scratch.factoryHome,
        },
      });
      expect(fromConfig.shotsDirectory).toBe(join(scratch.factoryHome, "configured shots"));

      const fromEnvironment = resolveConfig({
        cwd: scratch.root,
        environment: {
          HOME: scratch.home,
          TOHSENO_HOME: scratch.factoryHome,
          TOHSENO_SHOTS_DIR: "~/environment shots",
        },
      });
      expect(fromEnvironment.shotsDirectory).toBe(join(scratch.home, "environment shots"));

      const fromFlag = resolveConfig({
        cwd: scratch.root,
        environment: {
          HOME: scratch.home,
          TOHSENO_HOME: scratch.factoryHome,
          TOHSENO_SHOTS_DIR: "ignored",
        },
        shotsDirectoryOverride: "flag shots with spaces",
      });
      expect(fromFlag.shotsDirectory).toBe(join(scratch.root, "flag shots with spaces"));
    });
  });

  test("rejects malformed, unsupported, and user-home shorthand paths", async () => {
    await withScratchEnvironment((scratch) => {
      mkdirSync(scratch.factoryHome, { recursive: true });
      const configPath = join(scratch.factoryHome, "config.json");
      writeFileSync(configPath, "{not json");
      expect(() => resolveConfig({
        cwd: scratch.root,
        environment: { HOME: scratch.home, TOHSENO_HOME: scratch.factoryHome },
      })).toThrow("cannot read");

      writeFileSync(configPath, JSON.stringify({ schemaVersion: 2 }));
      expect(() => resolveConfig({
        cwd: scratch.root,
        environment: { HOME: scratch.home, TOHSENO_HOME: scratch.factoryHome },
      })).toThrow("schemaVersion must be 1");

      writeFileSync(configPath, JSON.stringify({ schemaVersion: 1, unknown: true }));
      expect(() => resolveConfig({
        cwd: scratch.root,
        environment: { HOME: scratch.home, TOHSENO_HOME: scratch.factoryHome },
      })).toThrow("unsupported field");

      writeFileSync(configPath, JSON.stringify({ schemaVersion: 1 }));
      expect(() => resolveConfig({
        cwd: scratch.root,
        environment: { HOME: scratch.home, TOHSENO_HOME: scratch.factoryHome },
        shotsDirectoryOverride: "~someone/shots",
      })).toThrow("unsupported home path");
    });
  });
});

describe("coding-agent detection and selection", () => {
  test("detects executable supported agents in stable product order", async () => {
    await withScratchEnvironment((scratch) => {
      const secondBin = join(scratch.root, "second fake bin");
      writeExecutable(scratch.binDirectory, "claude", "#!/bin/sh\nexit 0");
      writeExecutable(secondBin, "codex", "#!/bin/sh\nexit 0");
      writeExecutable(secondBin, "unsupported-agent", "#!/bin/sh\nexit 0");
      writeFileSync(join(secondBin, "non-executable"), "not executable\n");
      chmodSync(join(secondBin, "non-executable"), 0o644);

      const found = detectInstalledAgents(
        [scratch.binDirectory, secondBin].join(":"),
        scratch.root,
      );
      expect(found.map((agent) => agent.id)).toEqual(["codex", "claude"]);
      expect(found[0]!.executable).toBe(join(secondBin, "codex"));
      expect(found[1]!.executable).toBe(join(scratch.binDirectory, "claude"));
    });
  });

  test("does not mistake an executable directory for an installed agent", async () => {
    await withScratchEnvironment((scratch) => {
      mkdirSync(join(scratch.binDirectory, "codex"), { mode: 0o755 });
      expect(detectInstalledAgents(scratch.binDirectory, scratch.root)).toEqual([]);
    });
  });

  test("rejects unsupported and unavailable explicit agents", () => {
    expect(() => requireInstalledAgent("gemini", [])).toThrow("unsupported coding agent");
    expect(() => requireInstalledAgent("codex", [])).toThrow("not installed");
  });

  test("non-interactive create requires explicit selections unless launch is skipped", async () => {
    await withScratchEnvironment(async (scratch) => {
      let io = createMemoryIo();
      let exitCode = await main(
        ["create", "quiet-shot", "--no-interactive", "--no-launch"],
        {
          cwd: scratch.root,
          environment: scratch.environment,
          io,
          sourceRoot: REPOSITORY_ROOT,
        },
      );
      expect(exitCode).toBe(2);
      expect(io.stderr.join("\n")).toContain("requires --platform ios");

      writeExecutable(scratch.binDirectory, "codex", "#!/bin/sh\nexit 0");
      writeExecutable(scratch.binDirectory, "claude", "#!/bin/sh\nexit 0");
      io = createMemoryIo();
      exitCode = await main(
        ["create", "quiet-shot", "--platform", "ios", "--no-interactive"],
        {
          cwd: scratch.root,
          environment: scratch.environment,
          io,
          sourceRoot: REPOSITORY_ROOT,
        },
      );
      expect(exitCode).toBe(2);
      expect(io.stderr.join("\n")).toContain("requires --agent codex or --agent claude");

      io = createMemoryIo();
      exitCode = await main(
        ["create", "quiet-shot", "--platform", "android", "--agent", "codex", "--no-interactive"],
        {
          cwd: scratch.root,
          environment: scratch.environment,
          io,
          sourceRoot: REPOSITORY_ROOT,
        },
      );
      expect(exitCode).toBe(2);
      expect(io.stderr.join("\n")).toContain("this factory release implements ios only");
    });
  });

  test("reports missing slug, missing agents, and missing Git before generation", async () => {
    const missingSlugIo = createMemoryIo();
    expect(await main(["create"], { io: missingSlugIo })).toBe(2);
    expect(missingSlugIo.stderr.join("\n")).toContain("shot slug is required");

    await withScratchEnvironment(async (scratch) => {
      let io = createMemoryIo(true);
      let exitCode = await main([
        "create", "needs-agent", "--platform", "ios",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(3);
      expect(io.stderr.join("\n")).toContain("no supported coding agent found");

      io = createMemoryIo();
      exitCode = await main([
        "create", "needs-git", "--platform", "ios", "--no-launch", "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: { ...scratch.environment, PATH: scratch.binDirectory },
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(3);
      expect(io.stderr.join("\n")).toContain("Git is required");
    });
  });

  test("help advertises only the implemented platform", async () => {
    const io = createMemoryIo();
    expect(await main(["--help"], { io })).toBe(0);
    const help = io.stdout.join("\n");
    expect(help).toContain("Take another one.");
    expect(help).toContain("--platform ios");
    expect(help).toContain("iOS is the only implemented platform");
    expect(help).not.toContain("--platform android");
    expect(help).not.toContain("--platform web");
  });
});
