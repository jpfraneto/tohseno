import { describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, realpathSync, unlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { main } from "../src/cli.ts";
import { AGENT_INSTRUCTION } from "../src/constants.ts";
import type { ShotMetadata } from "../src/shot.ts";
import {
  createMemoryIo,
  fakeAgentRecordPath,
  installFakeAgent,
  REPOSITORY_ROOT,
  withScratchEnvironment,
} from "./helpers.ts";

describe("agent-first launcher", () => {
  test("no arguments requires a terminal instead of printing a command manual", async () => {
    const io = createMemoryIo();
    expect(await main([], { io })).toBe(2);
    expect(io.stdout).toEqual([]);
    expect(io.stderr.join("\n")).toContain("interactive terminal");
  });

  test("creates from a human name, shows only iOS, launches one agent, and withholds provider secrets", async () => {
    await withScratchEnvironment(async (scratch) => {
      installFakeAgent(scratch, "codex");
      scratch.environment.OPENAI_API_KEY = "test-provider-secret-never-forward";
      scratch.environment.DEV_SECRET = "another-secret-never-forward";
      const io = createMemoryIo(true, ["1", "My First Shot", "1"]);
      expect(await main([], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      })).toBe(0);
      expect(io.stdout.join("\n")).toContain("What would you like to do?");
      expect(io.stdout.join("\n")).toContain("1. Take your first shot");
      expect(io.stdout.join("\n")).toContain("2. Continue a shot");
      expect(io.stdout.join("\n")).toContain("1. iOS");
      expect(io.stdout.join("\n")).not.toMatch(/Android|Web/u);
      const shot = join(scratch.shotsDirectory, "my-first-shot");
      expect(existsSync(join(shot, "CLAUDE.md"))).toBe(true);
      expect(existsSync(join(shot, ".tohseno", "OPERATIONS.md"))).toBe(true);
      const launch = readFileSync(fakeAgentRecordPath(scratch), "utf8").split("\n");
      expect(realpathSync(launch[1]!)).toBe(realpathSync(shot));
      expect(launch[3]).toBe(AGENT_INSTRUCTION);
      expect(launch[5]).toBe("");
    });
  }, 20_000);

  test("asks among multiple agents and honors a configured default on blank input", async () => {
    await withScratchEnvironment(async (scratch) => {
      installFakeAgent(scratch, "codex");
      installFakeAgent(scratch, "claude");
      mkdirSync(scratch.factoryHome, { recursive: true });
      writeFileSync(join(scratch.factoryHome, "config.json"), JSON.stringify({
        schemaVersion: 1,
        shotsDirectory: scratch.shotsDirectory,
        defaultAgent: "claude",
      }));
      const io = createMemoryIo(true, ["1", "default-agent", "1", ""]);
      expect(await main([], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      })).toBe(0);
      expect(io.stdout.join("\n")).toContain("Claude Code (configured default)");
      const metadata = JSON.parse(readFileSync(
        join(scratch.shotsDirectory, "default-agent", ".tohseno", "shot.json"),
        "utf8",
      )) as ShotMetadata;
      expect(metadata.selectedAgent).toBe("claude");
      expect(readFileSync(fakeAgentRecordPath(scratch), "utf8").split("\n")[0]).toBe(
        join(scratch.binDirectory, "claude"),
      );
    });
  }, 20_000);

  test("continues an existing shot interactively and through the unambiguous slug shortcut", async () => {
    await withScratchEnvironment(async (scratch) => {
      installFakeAgent(scratch, "codex");
      let io = createMemoryIo();
      expect(await main([
        "create", "the-trenches", "--platform", "ios", "--agent", "codex", "--no-launch", "--no-interactive",
      ], { cwd: scratch.root, environment: scratch.environment, io, sourceRoot: REPOSITORY_ROOT })).toBe(0);

      io = createMemoryIo(true, ["2", "1"]);
      expect(await main([], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      })).toBe(0);
      expect(io.stdout.join("\n")).toContain("Shots here: 1");
      expect(io.stdout.join("\n")).toContain("1. Take another shot");
      expect(io.stdout.join("\n")).toContain("The Trenches — iOS · clean · development stopped");
      expect(realpathSync(readFileSync(fakeAgentRecordPath(scratch), "utf8").split("\n")[1]!)).toBe(
        realpathSync(join(scratch.shotsDirectory, "the-trenches")),
      );

      io = createMemoryIo();
      expect(await main(["the-trenches", "--no-interactive"], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
      })).toBe(0);
      expect(io.stdout.join("\n")).toContain("Continuing the-trenches");
    });
  }, 20_000);

  test("reports no-agent and no-shot failure modes without creating partial repositories", async () => {
    await withScratchEnvironment(async (scratch) => {
      let io = createMemoryIo(true, ["1", "needs-agent", "1"]);
      expect(await main([], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      })).toBe(3);
      expect(io.stderr.join("\n")).toContain("no supported coding agent");
      expect(existsSync(join(scratch.shotsDirectory, "needs-agent"))).toBe(false);

      io = createMemoryIo(true, ["2"]);
      expect(await main([], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      })).toBe(2);
      expect(io.stderr.join("\n")).toContain("no shots exist");
    });
  });

  test("reports a missing configured or prior agent and changes adapters only with an interactive choice", async () => {
    await withScratchEnvironment(async (scratch) => {
      installFakeAgent(scratch, "codex");
      mkdirSync(scratch.factoryHome, { recursive: true });
      const config = join(scratch.factoryHome, "config.json");
      writeFileSync(config, JSON.stringify({
        schemaVersion: 1,
        shotsDirectory: scratch.shotsDirectory,
        defaultAgent: "claude",
      }));

      let io = createMemoryIo(true, ["1", "fallback-default", "1"]);
      expect(await main([], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      })).toBe(0);
      expect(io.stderr.join("\n")).toContain("Configured default claude is not installed");
      expect(JSON.parse(readFileSync(
        join(scratch.shotsDirectory, "fallback-default", ".tohseno", "shot.json"),
        "utf8",
      )).selectedAgent).toBe("codex");

      const claude = installFakeAgent(scratch, "claude");
      io = createMemoryIo();
      expect(await main([
        "create", "prior-claude", "--platform", "ios", "--agent", "claude", "--no-launch", "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      })).toBe(0);
      unlinkSync(claude);
      writeFileSync(config, JSON.stringify({
        schemaVersion: 1,
        shotsDirectory: scratch.shotsDirectory,
      }));

      io = createMemoryIo();
      expect(await main(["prior-claude", "--no-interactive"], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
      })).toBe(3);
      expect(io.stderr.join("\n")).toContain("preferred agent claude is not installed");

      io = createMemoryIo(true, ["2", "2"]);
      expect(await main([], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
      })).toBe(0);
      expect(io.stderr.join("\n")).toContain("Previously selected claude is not installed");
      expect(readFileSync(fakeAgentRecordPath(scratch), "utf8").split("\n")[0]).toBe(
        join(scratch.binDirectory, "codex"),
      );
    });
  }, 20_000);
});
