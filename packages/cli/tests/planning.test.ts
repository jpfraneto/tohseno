import { describe, expect, test } from "bun:test";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { loadCatalog } from "../../skills/index.ts";
import type { AppCatalog } from "../../skills/types.ts";
import {
  planIntention,
  validateShotPlan,
} from "../src/planning.ts";
import { REPOSITORY_ROOT } from "./helpers.ts";

async function withCatalog<T>(
  run: (catalog: AppCatalog) => T | Promise<T>,
): Promise<T> {
  const scratch = mkdtempSync(join(tmpdir(), "tohseno-planning-test-"));
  try {
    const catalogRoot = join(scratch, "catalog");
    mkdirSync(join(catalogRoot, "kernels"), { recursive: true });
    mkdirSync(join(catalogRoot, "templates"), { recursive: true });
    mkdirSync(join(catalogRoot, "skills"), { recursive: true });
    cpSync(
      join(REPOSITORY_ROOT, "templates", "ios-kernel"),
      join(catalogRoot, "kernels", "ios-kernel"),
      { recursive: true },
    );
    for (const id of ["blank", "daily-game"]) {
      cpSync(
        join(REPOSITORY_ROOT, "templates", id),
        join(catalogRoot, "templates", id),
        { recursive: true },
      );
    }
    for (const id of [
      "daily-challenge",
      "local-progress",
      "rank-progression",
      "share-card",
    ]) {
      cpSync(
        join(REPOSITORY_ROOT, "skills", id),
        join(catalogRoot, "skills", id),
        { recursive: true },
      );
    }
    return await run(loadCatalog(catalogRoot));
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

function dailyGamePlan(): unknown {
  return {
    schemaVersion: 1,
    app: {
      name: "Five Choices",
      slug: "five-choices",
      bundleId: "com.tohseno.five-choices",
    },
    summary: "A deterministic five-step daily decision game.",
    template: "daily-game",
    skills: [
      { id: "daily-challenge", reason: "Provides the daily run." },
      { id: "share-card", reason: "Shares an owner-initiated result." },
    ],
    data: { strategy: "local", reason: "Progress stays on this device." },
    identity: { strategy: "none", reason: "The experience needs no account." },
    definitionOfDone: [
      "A run presents five choices and reaches a result.",
      "Completed progress survives relaunch.",
    ],
    assumptions: ["The first release uses bundled deterministic content."],
    questions: [],
  };
}

describe("intention planning", () => {
  test("strictly validates a plan and resolves transitive skills deterministically", async () => {
    await withCatalog((catalog) => {
      const result = validateShotPlan(
        dailyGamePlan(),
        catalog,
        "Make me a tiny daily game about choosing between two options.",
      );
      expect(result.composition.skills.map((skill) => skill.descriptor.id)).toEqual([
        "local-progress",
        "rank-progression",
        "daily-challenge",
        "share-card",
      ]);
      expect(result.plan.skills.map((skill) => skill.id)).toEqual([
        "local-progress",
        "rank-progression",
        "daily-challenge",
        "share-card",
      ]);
    });
  });

  test("uses one selected provider in a private scratch directory and removes it", async () => {
    await withCatalog(async (catalog) => {
      let plannerDirectory = "";
      const result = await planIntention({
        intention: "Make me a tiny daily game about choosing between two options.",
        catalog,
        agent: {
          id: "codex",
          label: "Codex",
          binary: "codex",
          executable: "/usr/bin/false",
          launchArguments: [],
        },
        environment: { HOME: "/private/test-home" },
        invoker: async (invocation) => {
          plannerDirectory = invocation.cwd;
          expect(readFileSync(join(invocation.cwd, "intention.md"), "utf8"))
            .toContain("tiny daily game");
          expect(readFileSync(join(invocation.cwd, "catalog.json"), "utf8"))
            .toContain("daily-game");
          expect(invocation.agent.id).toBe("codex");
          return JSON.stringify(dailyGamePlan());
        },
      });
      expect(result.fallback).toBe(false);
      expect(existsSync(plannerDirectory)).toBe(false);
    });
  });

  test("falls back safely on unavailable or invalid planning without changing providers", async () => {
    await withCatalog(async (catalog) => {
      const noAgent = await planIntention({
        intention: "Private owner intent",
        catalog,
        agent: null,
        environment: {},
      });
      expect(noAgent.fallbackReason).toBe("no-agent");
      expect(noAgent.plan.template).toBe("blank");
      expect(noAgent.plan.skills).toEqual([]);

      const invalid = await planIntention({
        intention: "Private owner intent",
        catalog,
        agent: {
          id: "claude",
          label: "Claude Code",
          binary: "claude",
          executable: "/usr/bin/false",
          launchArguments: [],
        },
        environment: {},
        invoker: async () => "not json",
      });
      expect(invalid.fallbackReason).toBe("invalid-output");
      expect(invalid.plan.template).toBe("blank");
    });
  });

  test("rejects unknown fields and verbatim private intention in tracked plan text", async () => {
    await withCatalog((catalog) => {
      expect(() => validateShotPlan(
        { ...(dailyGamePlan() as object), surprise: true },
        catalog,
        "A sufficiently long private intention that must remain private.",
      )).toThrow("unsupported field");

      const copied = dailyGamePlan() as ReturnType<typeof dailyGamePlan> & {
        summary: string;
      };
      copied.summary =
        "A sufficiently long private intention that must remain private.";
      expect(() => validateShotPlan(
        copied,
        catalog,
        copied.summary,
      )).toThrow("copied private intention");
    });
  });
});
