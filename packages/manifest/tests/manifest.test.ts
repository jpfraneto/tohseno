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

async function template(): Promise<ContinuityManifest> {
  const value = await readJson("templates/continuity-app/continuity.manifest.json");
  assertValidManifest(value);
  return structuredClone(value);
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

  test("the base-app template manifest validates without warnings", async () => {
    const manifest = await readJson(
      "templates/continuity-app/continuity.manifest.json",
    );
    const result = validateManifest(manifest);
    expect(result.errors).toEqual([]);
    expect(result.valid).toBe(true);
  });

  test("the base app declares the full spine", async () => {
    const manifest = await template();
    expect(manifest.runtime.identity.mode).toBe("seed-phrase");
    expect(manifest.runtime.identity.wordlist).toBe("bip39-english");
    expect(manifest.runtime.identity.creation).toBe("first-launch");
    expect(manifest.runtime.modules.paywall.enabled).toBe(false);
    expect(manifest.runtime.modules.shareCard.enabled).toBe(true);
    expect(manifest.runtime.modules.notifications.enabled).toBe(false);
    expect(manifest.runtime.modules.sessionLink).toEqual({
      enabled: false,
      status: "reserved",
    });
    expect(manifest.runtime.privacy.externalDisclosure).toEqual([
      "identity seed phrase, end-to-end encrypted in iCloud Keychain (automatic backup; stays local when iCloud Keychain is off)",
    ]);
    expect(manifest.runtime.recovery.identity).toBe("automatic-encrypted-backup");
    expect(manifest.runtime.recovery.content).toBe("manual-export");
    expect(manifest.operations.requiresServer).toBe(false);
  });

  test("invalid manifests are rejected with useful paths", async () => {
    const invalid = (await template()) as unknown as {
      runtime: { privacy: { publicByDefault: unknown } };
    };
    invalid.runtime.privacy.publicByDefault = "yes";
    const result = validateManifest(invalid);
    expect(result.valid).toBe(false);
    expect(result.errors.some((issue) => issue.path === "$.runtime.privacy.publicByDefault")).toBe(true);
    expect(() => assertValidManifest(invalid)).toThrow(TypeError);
  });

  test("public-by-default is a builder decision, not a refusal", async () => {
    const manifest = await template();
    manifest.runtime.privacy.publicByDefault = true;
    expect(validateManifest(manifest).valid).toBe(true);
  });

  test("reliability invariants cannot be weakened", async () => {
    const invalid = (await template()) as unknown as {
      runtime: { properties: { crashSafePersistence: boolean } };
    };
    invalid.runtime.properties.crashSafePersistence = false;
    const result = validateManifest(invalid);
    expect(result.valid).toBe(false);
    expect(result.errors.map((issue) => issue.code)).toContain(
      "runtime-invariant",
    );
  });

  test("seed-phrase identity requires the bip39 wordlist and no suite", async () => {
    const missingWordlist = (await template()) as unknown as {
      runtime: { identity: Record<string, unknown> };
    };
    delete missingWordlist.runtime.identity.wordlist;
    expect(
      validateManifest(missingWordlist).errors.map((issue) => issue.code),
    ).toContain("identity.wordlist");

    const withSuite = await template();
    (withSuite.runtime.identity as unknown as Record<string, unknown>).suite = "some.suite.v1";
    expect(
      validateManifest(withSuite).errors.map((issue) => issue.code),
    ).toContain("identity.unused-suite");
  });

  test("sessionLink stays reserved: enabling it is a schema error", async () => {
    const invalid = (await template()) as unknown as {
      runtime: { modules: { sessionLink: { enabled: boolean } } };
    };
    invalid.runtime.modules.sessionLink.enabled = true;
    const result = validateManifest(invalid);
    expect(result.valid).toBe(false);
    expect(result.errors.map((issue) => issue.code)).toContain(
      "modules.session-link.reserved",
    );
  });

  test("the paywall module is a flag plus a key slot, never a key", async () => {
    const manifest = await template();
    manifest.runtime.modules.paywall.enabled = true;
    expect(validateManifest(manifest).valid).toBe(true);

    const invalidProvider = (await template()) as unknown as {
      runtime: { modules: { paywall: { provider: string } } };
    };
    invalidProvider.runtime.modules.paywall.provider = "stripe";
    expect(
      validateManifest(invalidProvider).errors.map((issue) => issue.code),
    ).toContain("modules.paywall.provider");
  });

  test("remote reflection requires declared disclosure and a server", async () => {
    const manifest = await template();
    manifest.runtime.privacy.externalDisclosure = [];
    manifest.runtime.reflection = {
      mode: "remote-service",
      trigger: "after-event-opt-in",
      eventEligibility: "completed-only",
      consent: "per-event",
      inputDisclosure: "private-artifact",
      policyId: "reflection.v1",
      fallback: "continue-without-reflection",
    };
    const codes = validateManifest(manifest).errors.map((issue) => issue.code);
    expect(codes).toContain("privacy.reflection-disclosure");
    expect(codes).toContain("operations.server-required");
  });

  test("encrypted synchronization requires encryption and recovery", async () => {
    const manifest = await template();
    manifest.runtime.synchronization = {
      mode: "opt-in-encrypted",
      conflictPolicy: "Append immutable events; surface identity collisions.",
    };
    const codes = validateManifest(manifest).errors.map((issue) => issue.code);
    expect(codes).toContain("synchronization.encryption");
    expect(codes).toContain("synchronization.recovery");
  });

  test("unknown fields are rejected instead of becoming silent custom work", async () => {
    const invalid: unknown = {
      ...(await template()),
      dashboard: { widgets: ["everything"] },
    };
    const result = validateManifest(invalid);
    expect(result.errors.map((issue) => issue.code)).toContain(
      "additional-property",
    );
  });

  test("ejection requires at least one owner-exportable canonical artifact", async () => {
    const manifest = await template();
    for (const artifact of manifest.runtime.continuity.artifacts) artifact.export = "none";
    const result = validateManifest(manifest);
    expect(result.valid).toBe(false);
    expect(result.errors.map((issue) => issue.code)).toContain(
      "artifact.owner-export-required",
    );
  });

  test("parseManifest rejects malformed JSON and returns validated values", async () => {
    const manifestText = await Bun.file(
      new URL("templates/continuity-app/continuity.manifest.json", root),
    ).text();
    expect(parseManifest(manifestText).application.name).toBe("Writing");
    expect(() => parseManifest("{not-json")).toThrow(
      "Invalid continuity manifest JSON",
    );
  });
});
