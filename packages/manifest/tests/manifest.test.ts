import { describe, expect, test } from "bun:test";
import {
  assertValidManifest,
  parseManifest,
  validateManifest,
} from "../validate";
import type { ContinuityManifest } from "../types";

const root = new URL("../../../", import.meta.url);

async function readJson(relativePath: string): Promise<unknown> {
  return Bun.file(new URL(relativePath, root)).json() as Promise<unknown>;
}

async function example(name: string): Promise<ContinuityManifest> {
  const value = await readJson(`examples/${name}/continuity.manifest.json`);
  assertValidManifest(value);
  return value;
}

describe("continuity manifest", () => {
  test("the JSON Schema is valid JSON and keeps three concern boundaries", async () => {
    const schema = await readJson(
      "packages/manifest/continuity.manifest.schema.json",
    );
    expect(schema).toBeObject();
    const object = schema as Record<string, unknown>;
    expect(object.$schema).toBe(
      "https://json-schema.org/draft/2020-12/schema",
    );
    const properties = object.properties as Record<string, unknown>;
    expect(Object.keys(properties)).toEqual([
      "schemaVersion",
      "application",
      "runtime",
      "guidance",
      "operations",
    ]);
  });

  test.each(["anky", "daily-observation"])(
    "%s example validates",
    async (name) => {
      const manifest = await readJson(
        `examples/${name}/continuity.manifest.json`,
      );
      const result = validateManifest(manifest);
      expect(result.errors).toEqual([]);
      expect(result.valid).toBe(true);
    },
  );

  test("the continuity-app template validates", async () => {
    const manifest = await readJson(
      "templates/continuity-app/continuity.manifest.json",
    );
    expect(validateManifest(manifest).valid).toBe(true);
  });

  test("the second example is materially different from Anky", async () => {
    const anky = await example("anky");
    const observation = await example("daily-observation");

    expect(anky.runtime.action.input.kind).toBe("continuous-writing");
    expect(observation.runtime.action.input.kind).toBe(
      "photo-and-observation",
    );
    expect(anky.runtime.action.completion.kind).toBe("active-duration");
    expect(observation.runtime.action.completion.kind).toBe("explicit");
    expect(anky.runtime.reflection?.mode).toBe("remote-service");
    expect(observation.runtime.reflection?.mode).toBe("local-deterministic");
    expect(anky.runtime.privacy.localStorage).toBe("platform-private");
    expect(observation.runtime.privacy.localStorage).toBe(
      "application-encrypted",
    );
    expect(anky.runtime.payments).toBeDefined();
    expect(observation.runtime.payments).toBeUndefined();
    expect(anky.operations.requiresServer).toBe(true);
    expect(observation.operations.requiresServer).toBe(false);
  });

  test("invalid manifests are rejected with useful paths", async () => {
    const manifest = await example("daily-observation");
    const invalid: unknown = {
      ...structuredClone(manifest),
      runtime: {
        ...structuredClone(manifest.runtime),
        privacy: {
          ...structuredClone(manifest.runtime.privacy),
          publicByDefault: true,
        },
      },
    };
    const result = validateManifest(invalid);
    expect(result.valid).toBe(false);
    expect(result.errors.some((issue) => issue.path === "$.runtime.privacy.publicByDefault")).toBe(true);
    expect(() => assertValidManifest(invalid)).toThrow(TypeError);
  });

  test("runtime invariants cannot be weakened in guidance", async () => {
    const manifest = await example("daily-observation");
    const invalid = structuredClone(manifest) as unknown as {
      runtime: { properties: { offlineCoreAction: boolean } };
    };
    invalid.runtime.properties.offlineCoreAction = false;
    const result = validateManifest(invalid);
    expect(result.valid).toBe(false);
    expect(result.errors.map((issue) => issue.code)).toContain(
      "runtime-invariant",
    );
  });

  test("payment cannot gate protected continuity capabilities", async () => {
    const manifest = await example("anky");
    const invalid = structuredClone(manifest);
    invalid.runtime.payments?.mayGate.push("core-action");
    const result = validateManifest(invalid);
    expect(result.errors.map((issue) => issue.code)).toContain(
      "payments.gate-conflict",
    );
  });

  test("remote reflection requires declared disclosure and a server", async () => {
    const manifest = await example("anky");
    const invalid = structuredClone(manifest);
    invalid.runtime.privacy.externalDisclosure = [];
    invalid.operations.requiresServer = false;
    const codes = validateManifest(invalid).errors.map((issue) => issue.code);
    expect(codes).toContain("privacy.reflection-disclosure");
    expect(codes).toContain("operations.server-required");
  });

  test("encrypted synchronization requires encryption and recovery", async () => {
    const manifest = await example("daily-observation");
    const invalid = structuredClone(manifest) as ContinuityManifest;
    invalid.runtime.privacy.localStorage = "platform-private";
    invalid.runtime.recovery.content = "manual-export";
    invalid.runtime.synchronization = {
      mode: "opt-in-encrypted",
      conflictPolicy: "Append immutable events; surface identity collisions.",
    };
    const codes = validateManifest(invalid).errors.map((issue) => issue.code);
    expect(codes).toContain("synchronization.encryption");
    expect(codes).toContain("synchronization.recovery");
  });

  test("unknown fields are rejected instead of becoming silent custom work", async () => {
    const manifest = await example("daily-observation");
    const invalid: unknown = {
      ...structuredClone(manifest),
      dashboard: { widgets: ["everything"] },
    };
    const result = validateManifest(invalid);
    expect(result.errors.map((issue) => issue.code)).toContain(
      "additional-property",
    );
  });

  test("control-plane intake cannot declare private continuity content", async () => {
    const manifest = structuredClone(await example("daily-observation"));
    manifest.runtime.privacy.controlPlaneReceives = ["raw-private-user-content"] as never;
    const result = validateManifest(manifest);
    expect(result.valid).toBe(false);
    expect(result.errors.map((issue) => issue.code)).toContain("privacy.control-plane-metadata-only");
  });

  test("ejection requires at least one owner-exportable canonical artifact", async () => {
    const manifest = structuredClone(await example("daily-observation"));
    for (const artifact of manifest.runtime.continuity.artifacts) artifact.export = "none";
    const result = validateManifest(manifest);
    expect(result.valid).toBe(false);
    expect(result.errors.map((issue) => issue.code)).toContain("artifact.owner-export-required");
  });

  test("warnings remain visible without claiming schema failure", async () => {
    const manifest = await example("anky");
    const result = validateManifest(manifest);
    expect(result.valid).toBe(true);
    expect(result.warnings.map((issue) => issue.code)).toContain(
      "privacy.platform-private",
    );
    expect(result.warnings.map((issue) => issue.code)).toContain(
      "identity.early",
    );
  });

  test("parseManifest rejects malformed JSON and returns validated values", async () => {
    const manifestText = await Bun.file(
      new URL("examples/daily-observation/continuity.manifest.json", root),
    ).text();
    expect(parseManifest(manifestText).application.name).toBe(
      "Daily Observation",
    );
    expect(() => parseManifest("{not-json")).toThrow(
      "Invalid continuity manifest JSON",
    );
  });
});
