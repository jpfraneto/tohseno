import { afterEach, describe, expect, test } from "bun:test";
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  validateCanonicalLocalShotState,
  validateCanonicalShotMetadata,
} from "../factory/runtime/shared.ts";
import { readLocalShotProtocolState } from "../src/protocol-state.ts";
import { readShotMetadata } from "../src/shot.ts";

const roots: string[] = [];
const SHOT_ID = `shot_${"A".repeat(32)}` as `shot_${string}`;

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

function metadataFixture() {
  return {
    schemaVersion: 1,
    slug: "canonical-shot",
    platform: "ios",
    createdAt: "2026-07-25T12:00:00.000Z",
    sequence: 1,
    selectedAgent: null,
    creation: {
      door: "cli",
      inputDigest: "1".repeat(64),
      hasIntention: true,
      referenceCount: 0,
      provenancePath: ".tohseno/provenance/provenance.json",
      options: {
        selectedAgent: null,
        agentMode: "none",
        verifyAfterAgent: true,
        runAfterCreate: false,
      },
    },
    factory: {
      releaseId: `content-${"2".repeat(32)}`,
      cliVersion: "0.5.0",
      templateVersion: "ios-kernel-v1",
      manifestSchemaVersion: "1.0.0",
      sourceCommit: null,
      sourceDirty: false,
      bundleDigest: "3".repeat(64),
    },
    app: {
      name: "Canonical Shot",
      bundleId: "com.tohseno.canonical-shot",
    },
    composition: {
      kernel: {
        id: "ios-kernel",
        version: "1.0.0",
        digest: "4".repeat(64),
      },
      template: {
        id: "blank",
        version: "1.0.0",
        digest: "5".repeat(64),
      },
      skills: [],
    },
    sanitizedPlanDigest: "6".repeat(64),
    protocol: {
      version: 1,
      shotId: SHOT_ID,
      statePath: ".tohseno/protocol-state.json",
    },
  };
}

function appManifest(kernel = "ios-kernel") {
  return {
    schemaVersion: "1.0.0",
    kind: "app",
    application: {
      id: "com.tohseno.canonical-shot",
      name: "Canonical Shot",
    },
    platform: "ios",
    composition: { kernel, template: "blank", skills: [] },
    data: { local: [], remote: [] },
    storage: [],
    network: [],
    identity: { strategy: "none" },
    entitlements: [],
    integrations: [],
    operations: {
      project: "Shot.xcodeproj",
      scheme: "Shot",
      product: "Shot",
    },
    privacy: {
      rawIntentionTracked: false,
      appContentLeavesDevice: false,
    },
    production: { ready: false, declarations: [] },
    irreversibleOperations: [],
  };
}

function reorderedMetadata() {
  const source = metadataFixture();
  return {
    protocol: source.protocol,
    sanitizedPlanDigest: source.sanitizedPlanDigest,
    composition: {
      skills: source.composition.skills,
      template: {
        digest: source.composition.template.digest,
        id: source.composition.template.id,
        version: source.composition.template.version,
      },
      kernel: {
        digest: source.composition.kernel.digest,
        id: source.composition.kernel.id,
        version: source.composition.kernel.version,
      },
    },
    app: source.app,
    factory: {
      bundleDigest: source.factory.bundleDigest,
      sourceDirty: source.factory.sourceDirty,
      sourceCommit: source.factory.sourceCommit,
      manifestSchemaVersion: source.factory.manifestSchemaVersion,
      templateVersion: source.factory.templateVersion,
      cliVersion: source.factory.cliVersion,
      releaseId: source.factory.releaseId,
    },
    creation: source.creation,
    selectedAgent: source.selectedAgent,
    sequence: source.sequence,
    createdAt: source.createdAt,
    platform: source.platform,
    slug: source.slug,
    schemaVersion: source.schemaVersion,
  };
}

function writeJson(path: string, value: unknown): void {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function writeFixture(
  metadata: unknown,
  manifest = appManifest(),
): string {
  const root = mkdtempSync(join(tmpdir(), "tohseno-metadata-"));
  roots.push(root);
  mkdirSync(join(root, ".tohseno"));
  writeJson(join(root, ".tohseno", "shot.json"), metadata);
  writeJson(join(root, "app.manifest.json"), manifest);
  writeJson(join(root, ".tohseno", "protocol-state.json"), {
    evolution: 0,
    lifecycle: "EVOLVING",
    shotId: SHOT_ID,
    protocolVersion: 1,
  });
  return root;
}

describe("canonical Shot metadata", () => {
  test("uses one order-insensitive object-key policy in global and pinned validators", () => {
    const metadata = reorderedMetadata();
    const root = writeFixture(metadata);

    expect(readShotMetadata(root)?.protocol.shotId).toBe(SHOT_ID);
    expect(validateCanonicalShotMetadata(metadata).protocol.shotId).toBe(
      SHOT_ID,
    );
    expect(readLocalShotProtocolState(root)?.evolution).toBe(0);
    expect(validateCanonicalLocalShotState({
      evolution: 0,
      lifecycle: "EVOLVING",
      shotId: SHOT_ID,
      protocolVersion: 1,
    }, SHOT_ID).evolution).toBe(0);
  });

  test("rejects inconsistent release provenance and noncanonical kernels everywhere", () => {
    const source = metadataFixture();
    const inconsistentSource = {
      ...source,
      factory: {
        ...source.factory,
        sourceCommit: "7".repeat(40),
      },
    };
    const sourceRoot = writeFixture(inconsistentSource);
    expect(() => readShotMetadata(sourceRoot)).toThrow(
      "pre-release compatibility is unsupported",
    );
    expect(() => validateCanonicalShotMetadata(inconsistentSource)).toThrow(
      "Shot factory source provenance is inconsistent",
    );

    const wrongKernel = {
      ...source,
      composition: {
        ...source.composition,
        kernel: {
          ...source.composition.kernel,
          id: "old-kernel",
        },
      },
    };
    const kernelRoot = writeFixture(wrongKernel, appManifest("old-kernel"));
    expect(() => readShotMetadata(kernelRoot)).toThrow(
      "pre-release compatibility is unsupported",
    );
    expect(() => validateCanonicalShotMetadata(wrongKernel)).toThrow(
      "Shot composition is not canonical",
    );
  });

  test("rejects malformed matching local identities in the pinned validator", () => {
    expect(() => validateCanonicalLocalShotState({
      protocolVersion: 1,
      shotId: "not-a-shot",
      lifecycle: "EVOLVING",
      evolution: 0,
    }, "not-a-shot")).toThrow(
      "local Shot protocol state is not canonical",
    );
  });
});
