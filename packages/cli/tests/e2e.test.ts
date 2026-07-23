import { describe, expect, test } from "bun:test";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { validateManifest } from "../../manifest/validate.ts";
import { main } from "../src/cli.ts";
import { AGENT_INSTRUCTION } from "../src/constants.ts";
import { removeTreeEvenIfReadOnly } from "../src/files.ts";
import type { ShotMetadata } from "../src/shot.ts";
import {
  createMemoryIo,
  fakeAgentRecordPath,
  installCommitFailingGit,
  installFakeAgent,
  listTree,
  REPOSITORY_ROOT,
  runGit,
  runProcess,
  setFakeAgentExit,
  textFilesOutsideGit,
  withScratchEnvironment,
} from "./helpers.ts";

describe("shot creation end to end", () => {
  test("creates an ejectable validated repository in a path containing spaces", async () => {
    await withScratchEnvironment(async (scratch) => {
      installFakeAgent(scratch, "codex");
      const privateSentinel = "test-only-private-environment-value-8f31c2";
      scratch.environment.TOHSENO_TEST_PRIVATE_VALUE = privateSentinel;
      scratch.environment.GIT_AUTHOR_NAME = "Private Test Owner";
      scratch.environment.GIT_AUTHOR_EMAIL = "private-owner@example.invalid";
      scratch.environment.GIT_COMMITTER_NAME = "Private Test Owner";
      scratch.environment.GIT_COMMITTER_EMAIL = "private-owner@example.invalid";
      const io = createMemoryIo();

      const exitCode = await main([
        "create",
        "the-trenches",
        "--platform", "ios",
        "--agent", "codex",
        "--no-launch",
        "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });

      expect(exitCode).toBe(0);
      expect(io.stderr).toEqual([]);
      expect(io.stdout.join("\n")).toContain("Manifest valid.");
      expect(io.stdout.join("\n")).toContain("Baseline committed.");
      expect(io.stdout.join("\n")).toContain("Git author identity was not configured");

      const shot = join(scratch.shotsDirectory, "the-trenches");
      expect(existsSync(shot)).toBe(true);
      expect(existsSync(join(shot, "AGENTS.md"))).toBe(true);
      expect(existsSync(join(shot, ".tohseno", "verify.ts"))).toBe(true);
      expect(existsSync(join(shot, ".tohseno", "manifest", "validate.ts"))).toBe(true);

      const manifest = JSON.parse(readFileSync(join(shot, "continuity.manifest.json"), "utf8")) as {
        application: { id: string; name: string };
      };
      expect(manifest.application).toMatchObject({
        id: "com.tohseno.the-trenches",
        name: "The Trenches",
      });
      expect(validateManifest(manifest).valid).toBe(true);

      const metadata = JSON.parse(
        readFileSync(join(shot, ".tohseno", "shot.json"), "utf8"),
      ) as ShotMetadata;
      expect(metadata.slug).toBe("the-trenches");
      expect(metadata.platform).toBe("ios");
      expect(metadata.selectedAgent).toBe("codex");
      expect(metadata.adopted).toBe(false);
      expect(metadata.baselineAuthor).toBe("factory");
      expect(typeof metadata.factory.sourceDirty).toBe("boolean");
      expect(metadata.factory.releaseId.includes("-dirty-")).toBe(metadata.factory.sourceDirty);

      const top = await runGit(["rev-parse", "--show-toplevel"], shot, scratch.environment);
      expect(top.exitCode).toBe(0);
      expect(realpathSync(resolve(top.stdout.trim()))).toBe(realpathSync(shot));
      expect((await runGit(["rev-list", "--count", "HEAD"], shot, scratch.environment)).stdout.trim()).toBe("1");
      expect((await runGit(["status", "--porcelain"], shot, scratch.environment)).stdout).toBe("");
      expect((await runGit(["remote"], shot, scratch.environment)).stdout).toBe("");
      expect((await runGit(["log", "-1", "--format=%an <%ae>"], shot, scratch.environment)).stdout.trim()).toBe(
        "TOHSENO Factory <factory@tohseno.local>",
      );
      expect((await runGit(["config", "--local", "--get", "user.email"], shot, scratch.environment)).exitCode).not.toBe(0);

      for (const path of listTree(shot)) {
        expect(lstatSync(join(shot, path)).isSymbolicLink(), path).toBe(false);
      }
      for (const file of textFilesOutsideGit(shot)) {
        expect(file.source, file.path).not.toContain(privateSentinel);
        expect(file.source, file.path).not.toContain("private-owner@example.invalid");
        expect(file.source, file.path).not.toContain(REPOSITORY_ROOT);
      }
      expect(listTree(shot).some((path) => /MASTER_PROMPT\.md|Local\.xcconfig$/u.test(path))).toBe(false);

      const pinnedVerify = await runProcess(
        [process.execPath, "run", "verify"],
        shot,
        scratch.environment,
      );
      expect(pinnedVerify.exitCode).toBe(0);
      expect(pinnedVerify.stdout).toContain("manifest");

      const offlineIo = createMemoryIo();
      const offlineEnvironment = {
        ...scratch.environment,
        TOHSENO_SOURCE_ROOT: join(scratch.root, "factory source is offline"),
      };
      expect(await main([
        "create", "cached-offline", "--platform", "ios", "--no-launch", "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: offlineEnvironment,
        io: offlineIo,
      })).toBe(0);
      expect(offlineIo.stdout.join("\n")).toContain("(cached)");
      const offlineMetadata = JSON.parse(readFileSync(
        join(scratch.shotsDirectory, "cached-offline", ".tohseno", "shot.json"),
        "utf8",
      )) as ShotMetadata;
      expect(offlineMetadata.factory.releaseId).toBe(metadata.factory.releaseId);

      mkdirSync(join(scratch.shotsDirectory, "ordinary-directory"));
      const corrupt = join(scratch.shotsDirectory, "corrupt-metadata", ".tohseno");
      mkdirSync(corrupt, { recursive: true });
      writeFileSync(join(corrupt, "shot.json"), JSON.stringify({
        schemaVersion: 1,
        platform: "ios",
        slug: "corrupt-metadata",
      }));

      let commandIo = createMemoryIo();
      expect(await main(["list"], {
        cwd: scratch.root,
        environment: scratch.environment,
        io: commandIo,
      })).toBe(0);
      expect(commandIo.stdout.join("\n")).toContain("the-trenches");
      expect(commandIo.stdout.join("\n")).not.toContain("ordinary-directory");
      expect(commandIo.stdout.join("\n")).not.toContain("corrupt-metadata");

      commandIo = createMemoryIo();
      expect(await main(["open", "the-trenches"], {
        cwd: scratch.root,
        environment: scratch.environment,
        io: commandIo,
      })).toBe(0);
      expect(commandIo.stdout).toEqual([shot]);

      removeTreeEvenIfReadOnly(join(scratch.factoryHome, "cache"));
      commandIo = createMemoryIo();
      expect(await main(["verify", "the-trenches"], {
        cwd: scratch.root,
        environment: { ...scratch.environment, TOHSENO_SOURCE_ROOT: undefined },
        io: commandIo,
        sourceRoot: join(scratch.root, "source deliberately unavailable"),
      })).toBe(0);

      const originalManifest = readFileSync(join(shot, "continuity.manifest.json"), "utf8");
      const invalidManifest = JSON.parse(originalManifest) as { application: { name: string } };
      invalidManifest.application.name = "";
      writeFileSync(join(shot, "continuity.manifest.json"), `${JSON.stringify(invalidManifest, null, 2)}\n`);
      const invalidVerify = await runProcess(
        [process.execPath, ".tohseno/verify.ts"],
        shot,
        scratch.environment,
      );
      expect(invalidVerify.exitCode).not.toBe(0);
      expect(invalidVerify.stderr).toContain("continuity.manifest");
      writeFileSync(join(shot, "continuity.manifest.json"), originalManifest);
    });
  }, 30_000);

  test("refuses an existing destination before touching cache or its contents", async () => {
    await withScratchEnvironment(async (scratch) => {
      const destination = join(scratch.shotsDirectory, "already-there");
      mkdirSync(destination, { recursive: true });
      const sentinel = join(destination, "owner-file.txt");
      writeFileSync(sentinel, "preserve me\n");
      const io = createMemoryIo();
      const exitCode = await main([
        "create", "already-there", "--platform", "ios", "--no-launch", "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(1);
      expect(io.stderr.join("\n")).toContain("refusing to overwrite");
      expect(readFileSync(sentinel, "utf8")).toBe("preserve me\n");
      expect(existsSync(join(scratch.factoryHome, "cache"))).toBe(false);
    });
  });

  test("removes staging and final paths when the baseline commit fails", async () => {
    await withScratchEnvironment(async (scratch) => {
      installCommitFailingGit(scratch);
      const privateSentinel = "commit-failure-private-sentinel";
      scratch.environment.TOHSENO_TEST_PRIVATE_VALUE = privateSentinel;
      const io = createMemoryIo();
      const exitCode = await main([
        "create", "atomic-failure", "--platform", "ios", "--no-launch", "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(1);
      expect(io.stderr.join("\n")).toContain("shot creation failed before publication");
      expect(io.stderr.join("\n")).toContain("baseline commit failed");
      expect(io.stderr.join("\n")).not.toContain(privateSentinel);
      expect(existsSync(join(scratch.shotsDirectory, "atomic-failure"))).toBe(false);
      expect(readdirSync(scratch.shotsDirectory)).toEqual([]);
    });
  }, 30_000);

  test("prompts among multiple agents, auto-selects one, and preserves shots after launch failure", async () => {
    await withScratchEnvironment(async (scratch) => {
      const codexPath = installFakeAgent(scratch, "codex");
      installFakeAgent(scratch, "claude");
      const record = fakeAgentRecordPath(scratch);
      let io = createMemoryIo(true, ["1", "2"]);

      let exitCode = await main(["create", "agent-choice"], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(0);
      expect(io.questions).toHaveLength(2);
      expect(io.stdout.join("\n")).toContain("1. Codex");
      expect(io.stdout.join("\n")).toContain("2. Claude Code");
      const launch = readFileSync(record, "utf8").split("\n");
      expect(launch[0]).toBe(join(scratch.binDirectory, "claude"));
      expect(realpathSync(launch[1]!)).toBe(realpathSync(join(scratch.shotsDirectory, "agent-choice")));
      expect(launch[2]).toBe("1");
      expect(launch[3]).toBe(AGENT_INSTRUCTION);
      expect(launch[4]).toBe("");
      const selected = JSON.parse(
        readFileSync(join(scratch.shotsDirectory, "agent-choice", ".tohseno", "shot.json"), "utf8"),
      ) as ShotMetadata;
      expect(selected.selectedAgent).toBe("claude");

      unlinkSync(join(scratch.binDirectory, "claude"));
      setFakeAgentExit(scratch, 0);
      io = createMemoryIo(true, ["1"]);
      exitCode = await main(["create", "only-agent"], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(0);
      expect(io.questions).toHaveLength(1);
      expect(io.stdout.join("\n")).toContain("Using Codex, the only supported agent found.");

      setFakeAgentExit(scratch, 23);
      io = createMemoryIo();
      exitCode = await main([
        "create", "agent-exit", "--platform", "ios", "--agent", "codex", "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      });
      expect(exitCode).toBe(23);
      expect(io.stderr.join("\n")).toContain("Codex exited with status 23");
      expect(existsSync(join(scratch.shotsDirectory, "agent-exit", ".git"))).toBe(true);
      expect(readFileSync(record, "utf8").split("\n")[0]).toBe(codexPath);
    });
  }, 30_000);

  test("keeps legacy shots verifiable without silently adding the newer machine runtime", async () => {
    await withScratchEnvironment(async (scratch) => {
      const io = createMemoryIo();
      expect(await main([
        "create", "legacy-compatible", "--platform", "ios", "--no-launch", "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
        sourceRoot: REPOSITORY_ROOT,
      })).toBe(0);
      const shot = join(scratch.shotsDirectory, "legacy-compatible");
      const machinePath = join(shot, ".tohseno", "machine.ts");
      unlinkSync(machinePath);

      let machineIo = createMemoryIo();
      expect(await main(["machine", "verify", "--json"], {
        cwd: shot,
        environment: scratch.environment,
        io: machineIo,
      })).toBe(0);
      expect(JSON.parse(machineIo.stdout[0]!)).toMatchObject({
        ok: true,
        operation: "verify",
        result: { valid: true, compatibility: "legacy-shot" },
      });

      machineIo = createMemoryIo();
      expect(await main(["machine", "dev", "status", "--json"], {
        cwd: shot,
        environment: scratch.environment,
        io: machineIo,
      })).toBe(2);
      expect(JSON.parse(machineIo.stdout[0]!)).toMatchObject({
        ok: false,
        error: { code: "INVALID_CONFIGURATION" },
      });
      expect(machineIo.stdout[0]).toContain("legacy shot has no pinned machine runtime");
      expect(existsSync(machinePath)).toBe(false);
    });
  }, 20_000);
});
