import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { main } from "../src/cli.ts";
import { resolveConfig } from "../src/config.ts";
import { createShot } from "../src/creation.ts";
import { removeTreeEvenIfReadOnly } from "../src/files.ts";
import { readProgressJournal } from "../src/progress.ts";
import {
  normalizeCreationInput,
  type CreationProvenance,
} from "../src/provenance.ts";
import type { FactoryRelease } from "../src/release.ts";
import {
  publishStagedShot,
  type ShotMetadata,
} from "../src/shot.ts";
import { allocateShotSequence } from "../src/workspace.ts";
import {
  createMemoryIo,
  REPOSITORY_ROOT,
  withScratchEnvironment,
  writeExecutable,
} from "./helpers.ts";

const ONE_PIXEL_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Z1iQAAAAASUVORK5CYII=",
  "base64",
);

describe("shared shot factory", () => {
  test("combines typed and Markdown intention deterministically and hashes references", async () => {
    await withScratchEnvironment((scratch) => {
      const markdown = join(scratch.root, "owner intention.md");
      const reference = join(scratch.root, "sketch.png");
      writeFileSync(markdown, "\uFEFFSecond section\r\nwith CRLF\r\n");
      writeFileSync(reference, ONE_PIXEL_PNG);

      const normalized = normalizeCreationInput({
        text: "  First section\r\n",
        markdown: { path: markdown, originalName: "owner intention.md" },
        references: [{
          path: reference,
          originalName: "../sketch.png",
          mediaType: "image/png",
        }],
      });

      expect(normalized.intention).toBe(
        "# Typed intention\n\nFirst section\n\n# Attached Markdown\n\nSecond section\nwith CRLF\n",
      );
      expect(normalized.components.map((component) => component.kind)).toEqual([
        "textarea",
        "markdown",
      ]);
      expect(normalized.references).toHaveLength(1);
      expect(normalized.references[0]).toMatchObject({
        originalName: "sketch.png",
        mediaType: "image/png",
        extension: ".png",
      });
      expect(normalized.inputDigest).toMatch(/^[a-f0-9]{64}$/);
    });
  });

  test("CLI and Studio doors create the same shot format and portable private provenance", async () => {
    await withScratchEnvironment(async (scratch) => {
      const markdown = join(scratch.root, "intention.md");
      const reference = join(scratch.root, "reference image.png");
      const privatePhrase = "A quiet local app for one deliberate paragraph.";
      writeFileSync(markdown, `${privatePhrase}\r\n`);
      writeFileSync(reference, ONE_PIXEL_PNG);
      const cliIo = createMemoryIo();

      expect(await main([
        "create",
        "--file", markdown,
        "--reference", reference,
        "--platform", "ios",
        "--no-launch",
        "--no-interactive",
      ], {
        cwd: scratch.root,
        environment: scratch.environment,
        io: cliIo,
        sourceRoot: REPOSITORY_ROOT,
      })).toBe(0);
      expect(cliIo.stdout.join("\n")).not.toContain(privatePhrase);
      const cliRoot = join(scratch.shotsDirectory, "shot-001");
      expect(existsSync(cliRoot)).toBe(true);

      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      const studioResult = await createShot({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        slug: "studio-door",
        door: "studio",
        input: {
          markdown: { path: markdown, originalName: "intention.md" },
          references: [{
            path: reference,
            originalName: "reference image.png",
            mediaType: "image/png",
          }],
        },
        agent: null,
        noLaunch: true,
      });

      const readMetadata = (root: string): ShotMetadata => JSON.parse(
        readFileSync(join(root, ".tohseno", "shot.json"), "utf8"),
      ) as ShotMetadata;
      const readProvenance = (root: string): CreationProvenance => JSON.parse(
        readFileSync(
          join(root, ".tohseno", "provenance", "provenance.json"),
          "utf8",
        ),
      ) as CreationProvenance;
      const cliMetadata = readMetadata(cliRoot);
      const studioMetadata = readMetadata(studioResult.path);
      const cliProvenance = readProvenance(cliRoot);
      const studioProvenance = readProvenance(studioResult.path);

      expect(cliMetadata.creation?.door).toBe("cli");
      expect(studioMetadata.creation?.door).toBe("studio");
      expect(cliMetadata.creation?.inputDigest).toBe(studioMetadata.creation?.inputDigest);
      expect(cliMetadata.creation?.referenceCount).toBe(1);
      expect(cliMetadata.factory.bundleDigest).toBe(studioMetadata.factory.bundleDigest);
      expect(cliProvenance.inputDigest).toBe(studioProvenance.inputDigest);
      expect(cliProvenance.references[0]?.originalName).toBe("reference image.png");
      expect(
        readFileSync(
          join(cliRoot, ".tohseno", "provenance", "intention.md"),
          "utf8",
        ),
      ).toBe(`${privatePhrase}\n`);
      expect(existsSync(
        join(cliRoot, ".tohseno", "provenance", "references", "reference-001.png"),
      )).toBe(true);
      expect(readProgressJournal(
        join(
          scratch.shotsDirectory,
          ".tohseno",
          "events",
          `${studioResult.jobId}.jsonl`,
        ),
      ).map((event) => event.type)).toContain("completed");
    });
  }, 30_000);

  test("allocates unique monotonically increasing shot numbers concurrently", async () => {
    await withScratchEnvironment(async (scratch) => {
      const allocated = await Promise.all(
        Array.from({ length: 12 }, async () =>
          await allocateShotSequence(scratch.shotsDirectory)),
      );
      expect(new Set(allocated).size).toBe(12);
      expect([...allocated].sort((left, right) => left - right)).toEqual(
        Array.from({ length: 12 }, (_, index) => index + 1),
      );
    });
  });

  test("a resumed stale allocator cannot duplicate a sequence or unlink its replacement lock", async () => {
    await withScratchEnvironment(async (scratch) => {
      let firstClaimedResolve!: () => void;
      const firstClaimed = new Promise<void>((resolve) => {
        firstClaimedResolve = resolve;
      });
      let resumeFirstResolve!: () => void;
      const resumeFirst = new Promise<void>((resolve) => {
        resumeFirstResolve = resolve;
      });
      const first = allocateShotSequence(scratch.shotsDirectory, {
        afterSequenceClaimed: async (sequence) => {
          if (sequence !== 1) return;
          firstClaimedResolve();
          await resumeFirst;
        },
      });
      await firstClaimed;

      const lockPath = join(
        scratch.shotsDirectory,
        ".tohseno",
        "allocation.lock",
      );
      let replacementClaimedResolve!: () => void;
      const replacementClaimed = new Promise<void>((resolve) => {
        replacementClaimedResolve = resolve;
      });
      let resumeReplacementResolve!: () => void;
      const resumeReplacement = new Promise<void>((resolve) => {
        resumeReplacementResolve = resolve;
      });
      const replacement = allocateShotSequence(scratch.shotsDirectory, {
        isProcessAlive: () => false,
        afterSequenceClaimed: async (sequence) => {
          expect(sequence).toBe(2);
          replacementClaimedResolve();
          await resumeReplacement;
        },
      });
      await replacementClaimed;
      const replacementLock = readFileSync(lockPath, "utf8");

      resumeFirstResolve();
      await new Promise<void>((resolve) => setTimeout(resolve, 50));
      expect(readFileSync(lockPath, "utf8")).toBe(replacementLock);

      resumeReplacementResolve();
      expect(await replacement).toBe(2);
      expect(await first).toBe(3);
      expect(await allocateShotSequence(scratch.shotsDirectory)).toBe(4);
    });
  });

  test("atomic publication never replaces an empty destination on macOS", async () => {
    await withScratchEnvironment((scratch) => {
      const staging = join(scratch.shotsDirectory, ".staged-shot");
      const destination = join(scratch.shotsDirectory, "owner-directory");
      mkdirSync(scratch.shotsDirectory, { recursive: true });
      mkdirSync(staging);
      mkdirSync(destination);
      writeFileSync(join(staging, "factory.txt"), "factory\n");

      expect(() => publishStagedShot(staging, destination)).toThrow(
        "refusing to overwrite",
      );
      expect(readdirSync(destination)).toEqual([]);
      expect(readFileSync(join(staging, "factory.txt"), "utf8")).toBe(
        "factory\n",
      );
    });
  });

  test("launches automated Codex with global policy flags before exec", async () => {
    await withScratchEnvironment(async (scratch) => {
      const record = join(scratch.home, "automated-codex-arguments");
      const executable = writeExecutable(scratch.binDirectory, "codex", [
        "#!/bin/sh",
        `printf '%s\\n' "$@" > ${JSON.stringify(record)}`,
      ].join("\n"));
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });

      await createShot({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        slug: "automated-codex",
        door: "studio",
        input: { text: "Create a local writing surface." },
        agent: {
          id: "codex",
          label: "Codex",
          binary: "codex",
          executable,
          launchArguments: [],
        },
        noLaunch: false,
        runAfterCreate: false,
      });

      expect(readFileSync(record, "utf8").split("\n").slice(0, 7)).toEqual([
        "--sandbox",
        "workspace-write",
        "--ask-for-approval",
        "never",
        "exec",
        "--color",
        "never",
      ]);
    });
  }, 30_000);

  test("refuses to execute a verifier changed by the coding agent", async () => {
    await withScratchEnvironment(async (scratch) => {
      const marker = join(scratch.root, "untrusted-verifier-executed");
      const executable = writeExecutable(scratch.binDirectory, "codex", [
        `#!${process.execPath}`,
        'const { writeFileSync } = require("node:fs");',
        'const { join } = require("node:path");',
        `writeFileSync(join(process.cwd(), ".tohseno", "verify.ts"), ${JSON.stringify(
          `import { writeFileSync } from "node:fs";\nwriteFileSync(${JSON.stringify(marker)}, "unsafe");\n`,
        )});`,
      ].join("\n"));
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });

      await expect(createShot({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        slug: "tampered-by-agent",
        door: "studio",
        input: { text: "Create a private local writing surface." },
        agent: {
          id: "codex",
          label: "Codex",
          binary: "codex",
          executable,
          launchArguments: [],
        },
        noLaunch: false,
        runAfterCreate: false,
      })).rejects.toThrow(
        "differs from its immutable factory release",
      );
      expect(existsSync(marker)).toBe(false);
      expect(existsSync(
        join(scratch.shotsDirectory, "tampered-by-agent", ".tohseno", "verify.ts"),
      )).toBe(true);
    });
  }, 30_000);

  test("rejects coding-agent changes to immutable creation provenance", async () => {
    await withScratchEnvironment(async (scratch) => {
      const executable = writeExecutable(scratch.binDirectory, "codex", [
        `#!${process.execPath}`,
        'const { chmodSync, readFileSync, writeFileSync } = require("node:fs");',
        'const { join } = require("node:path");',
        'const path = join(process.cwd(), ".tohseno", "shot.json");',
        'const value = JSON.parse(readFileSync(path, "utf8"));',
        'value.creation.inputDigest = "0".repeat(64);',
        'writeFileSync(path, `${JSON.stringify(value, null, 2)}\\n`);',
        'const provenance = join(process.cwd(), ".tohseno", "provenance");',
        'writeFileSync(join(provenance, "intention.md"), "corrupted\\n");',
        'writeFileSync(join(provenance, "agent-added.txt"), "corrupted\\n");',
        'chmodSync(provenance, 0o755);',
        'chmodSync(join(provenance, "references"), 0o755);',
        "process.exitCode = 19;",
      ].join("\n"));
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });

      await expect(createShot({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        slug: "immutable-provenance",
        door: "studio",
        input: { text: "Create a private local writing surface." },
        agent: {
          id: "codex",
          label: "Codex",
          binary: "codex",
          executable,
          launchArguments: [],
        },
        noLaunch: false,
        runAfterCreate: false,
      })).rejects.toThrow(
        "changed immutable .tohseno/shot.json creation provenance",
      );
      const root = join(scratch.shotsDirectory, "immutable-provenance");
      const metadata = JSON.parse(
        readFileSync(join(root, ".tohseno", "shot.json"), "utf8"),
      ) as ShotMetadata;
      expect(metadata.creation?.inputDigest).not.toBe("0".repeat(64));
      expect(
        readFileSync(
          join(root, ".tohseno", "provenance", "intention.md"),
          "utf8",
        ),
      ).toBe("Create a private local writing surface.\n");
      expect(existsSync(
        join(root, ".tohseno", "provenance", "agent-added.txt"),
      )).toBe(false);
      expect(
        statSync(join(root, ".tohseno", "provenance")).mode & 0o777,
      ).toBe(0o700);
      expect(
        statSync(
          join(root, ".tohseno", "provenance", "references"),
        ).mode & 0o777,
      ).toBe(0o700);
    });
  }, 30_000);

  test("restores private provenance directory permissions changed by the coding agent", async () => {
    await withScratchEnvironment(async (scratch) => {
      const executable = writeExecutable(scratch.binDirectory, "codex", [
        `#!${process.execPath}`,
        'const { chmodSync } = require("node:fs");',
        'const { join } = require("node:path");',
        'const provenance = join(process.cwd(), ".tohseno", "provenance");',
        "chmodSync(provenance, 0o755);",
        'chmodSync(join(provenance, "references"), 0o755);',
      ].join("\n"));
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });

      await expect(createShot({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        slug: "private-provenance-modes",
        door: "studio",
        input: { text: "Create a private local writing surface." },
        agent: {
          id: "codex",
          label: "Codex",
          binary: "codex",
          executable,
          launchArguments: [],
        },
        noLaunch: false,
        runAfterCreate: false,
      })).rejects.toThrow(
        "changed immutable private creation provenance",
      );

      const provenance = join(
        scratch.shotsDirectory,
        "private-provenance-modes",
        ".tohseno",
        "provenance",
      );
      expect(statSync(provenance).mode & 0o777).toBe(0o700);
      expect(statSync(join(provenance, "references")).mode & 0o777).toBe(
        0o700,
      );
    });
  }, 30_000);

  test("CLI verify rejects tampered pinned machinery before launch", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      const result = await createShot({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        slug: "tampered-before-verify",
        door: "cli",
        agent: null,
        noLaunch: true,
      });
      const marker = join(scratch.root, "cli-machine-executed");
      writeFileSync(
        join(result.path, ".tohseno", "machine.ts"),
        `import { writeFileSync } from "node:fs";\nwriteFileSync(${JSON.stringify(marker)}, "unsafe");\n`,
      );
      const io = createMemoryIo();

      expect(await main(["verify", result.path], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
      })).toBe(2);
      expect(io.stderr.join("\n")).toContain(
        "differs from its immutable factory release",
      );
      expect(existsSync(marker)).toBe(false);
    });
  }, 30_000);

  test("CLI verify never trusts a self-authenticated shot verifier", async () => {
    await withScratchEnvironment(async (scratch) => {
      const config = resolveConfig({
        cwd: scratch.root,
        environment: scratch.environment,
      });
      const result = await createShot({
        config,
        cwd: scratch.root,
        environment: scratch.environment,
        sourceRoot: REPOSITORY_ROOT,
        slug: "forged-release-record",
        door: "cli",
        agent: null,
        noLaunch: true,
      });
      const marker = join(scratch.root, "forged-verifier-executed");
      const local = join(result.path, ".tohseno");
      const verifier = join(local, "verify.ts");
      writeFileSync(
        verifier,
        `import { writeFileSync } from "node:fs";\nwriteFileSync(${JSON.stringify(marker)}, "unsafe");\n`,
      );

      const releasePath = join(local, "factory-release.json");
      const release = JSON.parse(
        readFileSync(releasePath, "utf8"),
      ) as FactoryRelease;
      const verifierRecord = release.files.find(
        (record) => record.path === "shot/verify.ts",
      );
      if (verifierRecord === undefined) throw new Error("missing verifier record");
      const verifierDetails = statSync(verifier);
      verifierRecord.sha256 = createHash("sha256")
        .update(readFileSync(verifier))
        .digest("hex");
      verifierRecord.size = verifierDetails.size;
      verifierRecord.executable = (verifierDetails.mode & 0o111) !== 0;

      const digest = createHash("sha256");
      for (const record of release.files) {
        digest.update(record.path);
        digest.update("\0");
        digest.update(record.sha256);
        digest.update("\0");
        digest.update(String(record.size));
        digest.update("\0");
        digest.update(record.executable ? "x" : "-");
        digest.update("\0");
      }
      release.bundleDigest = digest.digest("hex");
      release.releaseId =
        release.source.kind === "git" && release.source.commit !== null
          ? `git-${release.source.commit}${
            release.source.dirty ? "-dirty" : ""
          }-${release.bundleDigest.slice(0, 16)}`
          : `content-${release.bundleDigest.slice(0, 32)}`;
      writeFileSync(releasePath, `${JSON.stringify(release, null, 2)}\n`);

      const shotPath = join(local, "shot.json");
      const shot = JSON.parse(
        readFileSync(shotPath, "utf8"),
      ) as ShotMetadata;
      shot.factory.releaseId = release.releaseId;
      shot.factory.bundleDigest = release.bundleDigest;
      writeFileSync(shotPath, `${JSON.stringify(shot, null, 2)}\n`);

      removeTreeEvenIfReadOnly(dirname(config.cacheDirectory));
      const poisoned = join(
        dirname(config.cacheDirectory),
        "trusted-tools",
        release.releaseId,
        ".tohseno",
      );
      mkdirSync(poisoned, { recursive: true });
      for (const path of [
        "factory-release.json",
        "machine.ts",
        "manifest",
        "runtime",
        "verify.ts",
      ]) {
        cpSync(join(local, path), join(poisoned, path), { recursive: true });
      }
      const io = createMemoryIo();
      expect(await main(["verify", result.path], {
        cwd: scratch.root,
        environment: scratch.environment,
        io,
      })).toBe(2);
      expect(io.stderr.join("\n")).toContain(
        "does not match the installed CLI",
      );
      expect(existsSync(marker)).toBe(false);
    });
  }, 30_000);
});
