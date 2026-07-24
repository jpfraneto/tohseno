import { describe, expect, test } from "bun:test";
import Ajv2020, { type AnySchema } from "ajv/dist/2020";
import { mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
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
      "token",
    ]);
  });

  test("the executable JSON Schema matches new cross-field validator rules", async () => {
    const schema = await readJson(
      "packages/manifest/continuity.manifest.schema.json",
    );
    const validateSchema = new Ajv2020({ allErrors: true, strict: false }).compile(
      schema as AnySchema,
    );

    const base = await template();
    expect(validateSchema(base)).toBe(true);
    expect(validateManifest(base).valid).toBe(true);

    const undisclosedNetworkExtension = await template();
    undisclosedNetworkExtension.runtime.modules.extensions = {
      "realtime-host": { enabled: true, requiresNetwork: true },
    };
    undisclosedNetworkExtension.runtime.privacy.externalDisclosure = [];
    expect(validateSchema(undisclosedNetworkExtension)).toBe(false);
    expect(validateManifest(undisclosedNetworkExtension).valid).toBe(false);

    const serverWithoutTarget = await template();
    serverWithoutTarget.operations.requiresServer = "credential-minting-only";
    serverWithoutTarget.operations.deploymentTargets = ["native-ios"];
    expect(validateSchema(serverWithoutTarget)).toBe(false);
    expect(validateManifest(serverWithoutTarget).valid).toBe(false);

    const wrongDevelopmentSlot = await template() as unknown as {
      operations: { developmentSecrets: Array<{ slot: string; purpose: string }> };
    };
    wrongDevelopmentSlot.operations.developmentSecrets = [
      { slot: "openai-api-key", purpose: "prototype provider access" },
    ];
    expect(validateSchema(wrongDevelopmentSlot)).toBe(false);
    expect(validateManifest(wrongDevelopmentSlot).valid).toBe(false);
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
    expect(manifest.runtime.properties.permissionRequestPolicy).toBe(
      "first-core-action",
    );
    expect(manifest.runtime.modules.paywall.enabled).toBe(false);
    expect(manifest.runtime.modules.shareCard.enabled).toBe(true);
    expect(manifest.runtime.modules.notifications.enabled).toBe(false);
    expect(manifest.runtime.modules.sessionLink).toEqual({
      enabled: false,
      status: "reserved",
    });
    expect(manifest.runtime.modules.tokenMint).toEqual({
      enabled: false,
      status: "reserved",
    });
    expect(manifest.runtime.privacy.externalDisclosure).toEqual([
      "identity seed phrase, end-to-end encrypted in iCloud Keychain (automatic backup; stays local when iCloud Keychain is off)",
      "operational health request to the owner-operated app API (never writing content)",
    ]);
    expect(manifest.runtime.recovery.identity).toBe("automatic-encrypted-backup");
    expect(manifest.runtime.recovery.content).toBe("manual-export");
    expect(manifest.operations.requiresServer).toBe(true);
    expect(manifest.operations.deploymentTargets).toEqual(["native-ios", "server"]);
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

  test("application names cannot inject Xcode configuration syntax", async () => {
    for (const name of [
      "$(DEV_SECRET)",
      "Name = $(OTHER)",
      "Name \\\nOTHER = value",
      "Name // comment",
      "Name /* comment",
    ]) {
      const manifest = await template();
      manifest.application.name = name;
      const result = validateManifest(manifest);
      expect(result.valid, name).toBe(false);
      expect(
        result.errors.some((issue) => issue.path === "$.application.name"),
        name,
      ).toBe(true);
    }
  });

  test("public-by-default is a builder decision, not a refusal", async () => {
    const manifest = await template();
    manifest.runtime.privacy.publicByDefault = true;
    expect(validateManifest(manifest).valid).toBe(true);
  });

  test("a network-dependent core action must declare its offline surface and disclosure", async () => {
    const manifest = await template();
    const properties = manifest.runtime.properties as unknown as Record<string, unknown>;

    properties.offlineCoreAction = "network-required";
    let codes = validateManifest(manifest).errors.map((issue) => issue.code);
    expect(codes).toContain("type.string");

    properties.offlineSurface =
      "Past shows stay readable; starting a show needs network.";
    manifest.runtime.privacy.externalDisclosure = [];
    codes = validateManifest(manifest).errors.map((issue) => issue.code);
    expect(codes).toContain("offline.network-disclosure");

    manifest.runtime.privacy.externalDisclosure = [
      "mic audio streams to a realtime AI provider during a show only",
    ];
    expect(validateManifest(manifest).valid).toBe(true);
  });

  test("degraded core actions use the bounded 0.4.0 value", async () => {
    const manifest = await template();
    manifest.runtime.properties = {
      ...manifest.runtime.properties,
      offlineCoreAction: "degraded",
      offlineSurface: "Past records remain readable; creating a new record needs network.",
    };
    expect(validateManifest(manifest).valid).toBe(true);
  });

  test("a fully offline core action must not carry an offline surface", async () => {
    const manifest = await template();
    (manifest.runtime.properties as unknown as Record<string, unknown>).offlineSurface =
      "everything";
    const codes = validateManifest(manifest).errors.map((issue) => issue.code);
    expect(codes).toContain("offline.unused-surface");
  });

  test("OS permission requests stay at first core action, never launch", async () => {
    const manifest = (await template()) as unknown as {
      runtime: { properties: { permissionRequestPolicy: string } };
    };
    manifest.runtime.properties.permissionRequestPolicy = "at-launch";
    expect(validateManifest(manifest).errors.map((issue) => issue.code)).toContain(
      "permission-request-policy",
    );
  });

  test("old manifest shapes fail with an explicit 0.4.0 migration message", async () => {
    const manifest = (await template()) as unknown as {
      schemaVersion: string;
      runtime: { properties: Record<string, unknown> };
      operations: Record<string, unknown>;
    };
    manifest.schemaVersion = "0.3.0";
    manifest.runtime.properties.offlineCoreAction = true;
    delete manifest.runtime.properties.permissionRequestPolicy;
    manifest.operations.requiresServer = true;
    manifest.operations.serverRole = "token-mint-only";

    const result = validateManifest(manifest);
    const version = result.errors.find((issue) => issue.code === "schema-version");
    expect(version?.message).toContain("migrate 0.3.0");
    expect(version?.message).toContain("credential-minting-only");
    expect(result.errors.find((issue) => issue.path.endsWith("offlineCoreAction"))?.message)
      .toContain("full, degraded, network-required");
    expect(result.errors.map((issue) => issue.code)).toContain(
      "operations.server-role-removed",
    );
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

  test("tokenMint stays reserved: enabling it is a schema error", async () => {
    const invalid = (await template()) as unknown as {
      runtime: { modules: { tokenMint: { enabled: boolean } } };
    };
    invalid.runtime.modules.tokenMint.enabled = true;
    const result = validateManifest(invalid);
    expect(result.valid).toBe(false);
    expect(result.errors.map((issue) => issue.code)).toContain(
      "modules.token-mint.reserved",
    );
  });

  test("a token launch record is an optional bounded fact, never a credential", async () => {
    const manifest = await template();
    expect(manifest.token).toBeUndefined();

    manifest.token = {
      provider: "bankr",
      chain: "base",
      name: "Continuity",
      symbol: "CONT",
      feeRecipient: "owner.eth",
      address: "0x1111111111111111111111111111111111111111",
      txHash: `0x${"2".repeat(64)}`,
      launchedAt: "2026-07-23T00:00:00Z",
    };
    expect(validateManifest(manifest).valid).toBe(true);

    const schema = await readJson(
      "packages/manifest/continuity.manifest.schema.json",
    );
    const validateSchema = new Ajv2020({ allErrors: true, strict: false }).compile(
      schema as AnySchema,
    );
    expect(validateSchema(manifest)).toBe(true);

    const badChain = structuredClone(manifest) as unknown as { token: { chain: string } };
    badChain.token.chain = "ethereum";
    expect(validateManifest(badChain).valid).toBe(false);
    expect(validateSchema(badChain)).toBe(false);

    const longSymbol = structuredClone(manifest);
    longSymbol.token!.symbol = "TOOLONGSYMBOL";
    expect(validateManifest(longSymbol).valid).toBe(false);

    const badAddress = structuredClone(manifest);
    badAddress.token!.address = "0x123";
    expect(validateManifest(badAddress).valid).toBe(false);

    const badDate = structuredClone(manifest);
    badDate.token!.launchedAt = "yesterday";
    expect(validateManifest(badDate).valid).toBe(false);

    const extraKey = structuredClone(manifest) as unknown as {
      token: Record<string, unknown>;
    };
    extraKey.token.apiKey = "bk_never";
    const extraResult = validateManifest(extraKey);
    expect(extraResult.valid).toBe(false);
    expect(extraResult.errors.map((issue) => issue.path)).toContain("$.token.apiKey");
  });

  test("server requirements distinguish a credential-only mint from broader servers", async () => {
    const manifest = await template();
    manifest.operations.requiresServer = "credential-minting-only";
    expect(validateManifest(manifest).valid).toBe(true);

    manifest.operations.requiresServer = true;
    expect(validateManifest(manifest).valid).toBe(true);

    manifest.operations.deploymentTargets = ["native-ios"];
    expect(validateManifest(manifest).errors.map((issue) => issue.code)).toContain(
      "operations.server-target",
    );
  });

  test("development secrets are declared slots with a prototype-only warning", async () => {
    const manifest = await template();
    manifest.operations.developmentSecrets = [
      { slot: "dev-secret", purpose: "prototype-only realtime voice; gitignored Local.xcconfig" },
    ];
    const result = validateManifest(manifest);
    expect(result.valid).toBe(true);
    expect(result.warnings.map((issue) => issue.code)).toContain(
      "operations.development-secrets",
    );

    (manifest.operations.developmentSecrets[0] as { slot: string }).slot = "openai-api-key";
    expect(validateManifest(manifest).errors.map((issue) => issue.code)).toContain(
      "development-secrets.slot",
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
    manifest.operations.requiresServer = false;
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

  test("encrypted synchronization requires a real server, not only a credential mint", async () => {
    const manifest = await template();
    manifest.runtime.synchronization = {
      mode: "opt-in-encrypted",
      conflictPolicy: "Append immutable events; surface identity collisions.",
    };
    manifest.runtime.privacy.localStorage = "application-encrypted";
    manifest.runtime.recovery.content = "opt-in-encrypted-backup";
    manifest.operations.requiresServer = "credential-minting-only";
    expect(validateManifest(manifest).errors.map((issue) => issue.code)).toContain(
      "operations.sync-server-required",
    );

    manifest.operations.requiresServer = true;
    expect(validateManifest(manifest).valid).toBe(true);
  });

  test("app-specific modules are declared extensions, not implementation notes", async () => {
    const manifest = await template();
    manifest.runtime.modules.extensions = {
      "realtime-host": {
        enabled: true,
        keySlot: "realtime-host-endpoint",
        requiresNetwork: true,
        notes: "Live AI game-show host over a realtime voice session.",
      },
    };
    expect(validateManifest(manifest).valid).toBe(true);

    manifest.runtime.privacy.externalDisclosure = [];
    expect(validateManifest(manifest).errors.map((issue) => issue.code)).toContain(
      "modules.extension-disclosure",
    );

    const badName = await template();
    badName.runtime.modules.extensions = {
      "Bad Name!": { enabled: false },
    };
    expect(validateManifest(badName).errors.map((issue) => issue.code)).toContain(
      "extension.name",
    );

    const badShape = await template();
    (badShape.runtime.modules as unknown as Record<string, unknown>).extensions = {
      "realtime-host": { enabled: true, apiKey: "sk-live" },
    };
    expect(validateManifest(badShape).errors.map((issue) => issue.code)).toContain(
      "additional-property",
    );
  });

  test("metered dependencies are bounded declarations that warn the owner", async () => {
    const manifest = await template();
    manifest.operations.meteredDependencies = [
      { provider: "openai.realtime", unit: "realtime session minute" },
    ];
    let result = validateManifest(manifest);
    expect(result.valid).toBe(true);
    expect(result.warnings.map((issue) => issue.code)).toContain(
      "operations.metered-dependencies",
    );

    (manifest.operations.meteredDependencies[0] as unknown as Record<string, unknown>).price =
      "secret pricing prose";
    result = validateManifest(manifest);
    expect(result.errors.map((issue) => issue.code)).toContain(
      "additional-property",
    );
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

  test("the root and package validate CLIs exit 0 only on valid manifests", async () => {
    const rootDir = fileURLToPath(root);
    const packageDir = fileURLToPath(new URL("../", import.meta.url));
    const runCli = async (cwd: string, ...cliArgs: string[]): Promise<number> => {
      const child = Bun.spawn(["bun", "run", "validate", ...cliArgs], {
        cwd,
        stdout: "ignore",
        stderr: "ignore",
      });
      return child.exited;
    };
    expect(await runCli(rootDir, "templates/continuity-app/continuity.manifest.json")).toBe(0);
    expect(await runCli(packageDir, "../../templates/continuity-app/continuity.manifest.json")).toBe(0);
    expect(await runCli(rootDir)).toBe(2);
    expect(await runCli(rootDir, "no-such-file.json")).toBe(2);
    const broken = `${rootDir}packages/manifest/tests/.broken-manifest.tmp.json`;
    await Bun.write(broken, JSON.stringify({ schemaVersion: "0.1.0" }));
    try {
      expect(await runCli(rootDir, broken)).toBe(1);
    } finally {
      await Bun.file(broken).delete();
    }

    const scratch = mkdtempSync(join(tmpdir(), "tohseno-manifest-gate-"));
    try {
      const target = join(scratch, "target.json");
      const link = join(scratch, "manifest.json");
      writeFileSync(target, "{}\n");
      symlinkSync(target, link);
      expect(await runCli(rootDir, link)).toBe(1);

      const oversized = join(scratch, "oversized.json");
      writeFileSync(oversized, " ".repeat(1_048_577));
      expect(await runCli(rootDir, oversized)).toBe(1);
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
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
