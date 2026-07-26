import { describe, expect, test } from "bun:test";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import {
  AppCatalogError,
  applyComposition,
  loadCatalog,
  resolveComposition,
  resolveInstalledComposition,
  runAcceptanceChecks,
  validateSkillDescriptor,
  verifyLock,
} from "../index.ts";

const ROOT = resolve(import.meta.dir, "../../..");

function withCatalog<T>(run: (catalogRoot: string, output: string) => T): T {
  const scratch = mkdtempSync(join(tmpdir(), "tohseno-skills-"));
  try {
    const catalog = join(scratch, "catalog");
    mkdirSync(join(catalog, "kernels"), { recursive: true });
    mkdirSync(join(catalog, "templates"), { recursive: true });
    mkdirSync(join(catalog, "skills"), { recursive: true });
    cpSync(
      join(ROOT, "templates", "ios-kernel"),
      join(catalog, "kernels", "ios-kernel"),
      { recursive: true },
    );
    for (const template of ["blank", "daily-game"]) {
      cpSync(
        join(ROOT, "templates", template),
        join(catalog, "templates", template),
        { recursive: true },
      );
    }
    for (const skill of [
      "daily-challenge",
      "local-progress",
      "rank-progression",
      "share-card",
    ]) {
      cpSync(
        join(ROOT, "skills", skill),
        join(catalog, "skills", skill),
        { recursive: true },
      );
    }
    return run(catalog, join(scratch, "output"));
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

describe("bundled app skill catalog", () => {
  test("loads and deterministically resolves the Daily Game composition", () => {
    withCatalog((root) => {
      const catalog = loadCatalog(root);
      const composition = resolveComposition(catalog, {
        schemaVersion: 1,
        template: "daily-game",
        skills: [],
      });
      expect(composition.skills.map((skill) => skill.descriptor.id)).toEqual([
        "local-progress",
        "rank-progression",
        "daily-challenge",
        "share-card",
      ]);
    });
  });

  test("rejects duplicate, template-overlapping, and noncanonical persisted skill lists", () => {
    withCatalog((root) => {
      const catalog = loadCatalog(root);
      expect(() => resolveComposition(catalog, {
        schemaVersion: 1,
        template: "blank",
        skills: ["share-card", "share-card"],
      })).toThrow("must not contain duplicates");
      expect(() => resolveComposition(catalog, {
        schemaVersion: 1,
        template: "daily-game",
        skills: ["share-card"],
      })).toThrow("already supplied by template");
      expect(() => resolveInstalledComposition(catalog, {
        schemaVersion: 1,
        template: "daily-game",
        skills: [
          "daily-challenge",
          "local-progress",
          "rank-progression",
          "share-card",
        ],
      })).toThrow("complete and canonical");

      const installed = resolveComposition(catalog, {
        schemaVersion: 1,
        template: "daily-game",
        skills: [],
      });
      expect(resolveInstalledComposition(catalog, {
        schemaVersion: 1,
        template: "daily-game",
        skills: installed.skills.map((skill) => skill.descriptor.id),
      }).skills.map((skill) => skill.descriptor.id)).toEqual(
        installed.skills.map((skill) => skill.descriptor.id),
      );
    });
  });

  test("applies real overlays, pins digests, and passes acceptance checks", () => {
    withCatalog((root, output) => {
      const catalog = loadCatalog(root);
      const composition = resolveComposition(catalog, {
        schemaVersion: 1,
        template: "daily-game",
        skills: [],
      });
      const applied = applyComposition({
        composition,
        target: output,
        factoryReleaseId: "content-test",
      });
      expect(readFileSync(
        join(output, "App", "Features", "AppRootView.swift"),
        "utf8",
      )).toContain("DailyChallengeView");
      expect(readFileSync(
        join(output, "App", "Skills", "LocalProgress", "LocalProgressStore.swift"),
        "utf8",
      )).toContain("UserDefaults");
      expect(applied.lock.skills).toHaveLength(4);
      expect(applied.lock.skills.every((skill) =>
        /^[a-f0-9]{64}$/u.test(skill.digest)
      )).toBe(true);
      expect(runAcceptanceChecks(output, composition).length).toBeGreaterThan(4);
      expect(verifyLock(catalog, applied.lock).skills.map((skill) =>
        skill.descriptor.id
      )).toEqual(composition.skills.map((skill) => skill.descriptor.id));
    });
  });

  test("rejects unknown skills, cycles, conflicts, traversal, collisions, and symlinks", () => {
    withCatalog((root, output) => {
      const catalog = loadCatalog(root);
      expect(() => resolveComposition(catalog, {
        schemaVersion: 1,
        template: "blank",
        skills: ["not-installed"],
      })).toThrow("unknown app skill");

      const localProgress = join(root, "skills", "local-progress", "skill.json");
      const daily = join(root, "skills", "daily-challenge", "skill.json");
      const localValue = JSON.parse(readFileSync(localProgress, "utf8"));
      localValue.requires = ["daily-challenge"];
      writeFileSync(localProgress, `${JSON.stringify(localValue, null, 2)}\n`);
      expect(() => resolveComposition(loadCatalog(root), {
        schemaVersion: 1,
        template: "daily-game",
        skills: [],
      })).toThrow("dependency cycle");

      localValue.requires = [];
      localValue.conflicts = ["daily-challenge"];
      writeFileSync(localProgress, `${JSON.stringify(localValue, null, 2)}\n`);
      expect(() => resolveComposition(loadCatalog(root), {
        schemaVersion: 1,
        template: "daily-game",
        skills: [],
      })).toThrow("conflicts");

      expect(() => validateSkillDescriptor({
        ...JSON.parse(readFileSync(daily, "utf8")),
        contributions: {
          ...JSON.parse(readFileSync(daily, "utf8")).contributions,
          overlay: "../outside",
        },
      })).toThrow("stay inside");

      localValue.conflicts = [];
      writeFileSync(localProgress, `${JSON.stringify(localValue, null, 2)}\n`);
      const collisionPath = join(
        root,
        "skills",
        "local-progress",
        "overlay",
        "App",
        "Features",
        "AppRootView.swift",
      );
      mkdirSync(dirname(collisionPath), { recursive: true });
      writeFileSync(collisionPath, "collision\n");
      const collisionCatalog = loadCatalog(root);
      const collisionComposition = resolveComposition(collisionCatalog, {
        schemaVersion: 1,
        template: "daily-game",
        skills: [],
      });
      expect(() => applyComposition({
        composition: collisionComposition,
        target: output,
        factoryReleaseId: "content-test",
      })).toThrow("may not overwrite");

      const victim = join(root, "victim.swift");
      writeFileSync(victim, "private\n");
      symlinkSync(
        victim,
        join(root, "skills", "share-card", "overlay", "linked.swift"),
      );
      expect(() => loadCatalog(root)).toThrow("symbolic link");
    });
  });

  test("reports typed errors suitable for CLI and Studio", () => {
    try {
      validateSkillDescriptor({ schemaVersion: 1 });
      throw new Error("expected validation to fail");
    } catch (error) {
      expect(error).toBeInstanceOf(AppCatalogError);
      expect((error as AppCatalogError).code).toBe("INVALID_DESCRIPTOR");
    }
  });
});
