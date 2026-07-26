import {
  existsSync,
  mkdirSync,
  readFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  deriveDeterministicTestIdentity,
} from "../../../packages/identity/src/index.ts";
import {
  LocalEd25519Signer,
  localEd25519VerifierSet,
} from "../../../packages/signer/src/index.ts";
import {
  deriveDeterministicTestShotId,
  hashSignedPublicShotRecord,
  PUBLIC_SHOT_PROTOCOL_VERSION,
  type AppcoinLinkedRecord,
  type EvolutionRecordedRecord,
  type LifecycleTransitionedRecord,
  type PublicationEvidence,
  type PublicShotRecord,
  type ShotCreatedRecord,
  type SignedPublicShotRecord,
  signPublicShotRecord,
  validatePublicShotRecord,
  verifySignedPublicShotRecord,
} from "../../../packages/protocol/src/index.ts";
import {
  projectPublicShotProjection,
} from "../../../packages/registry/src/index.ts";
import {
  createNodeRecordsPayload,
  createNodeSubmissionPayload,
  NODE_PAYLOAD_SCHEMA_VERSION,
} from "../../../packages/node-client/src/payloads.ts";
import {
  REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
} from "../src/database.ts";

const repositoryRoot = fileURLToPath(
  new URL("../../../", import.meta.url),
);
const fixtureTimestamp = (sequence: number): string =>
  new Date(Date.UTC(2026, 6, 25, 0, sequence, 0)).toISOString();

interface Artifact {
  path: string;
  value: unknown;
}

function readJson(relativePath: string): unknown {
  return JSON.parse(
    readFileSync(join(repositoryRoot, relativePath), "utf8"),
  ) as unknown;
}

function object(value: unknown, label: string): Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value)
  ) {
    throw new Error(`${label} must be a JSON object`);
  }
  return value as Record<string, unknown>;
}

const externalSchemaReferences: Readonly<Record<string, string>> = {
  "urn:tohseno:schema:identity-reference:v1":
    "#/components/schemas/IdentityReference",
  "urn:tohseno:schema:verification-method:v1":
    "#/components/schemas/VerificationMethod",
  "urn:tohseno:schema:signature-envelope:v1":
    "#/components/schemas/SignatureEnvelope",
};

function embedSchema(
  value: unknown,
  component: string,
): unknown {
  if (Array.isArray(value)) {
    return value.map((item) => embedSchema(item, component));
  }
  if (typeof value !== "object" || value === null) return value;
  const result: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) {
    if (key === "$schema" || key === "$id") continue;
    if (key === "$ref" && typeof item === "string") {
      if (item.startsWith("#/")) {
        result[key] =
          `#/components/schemas/${component}${item.slice(1)}`;
      } else {
        result[key] = externalSchemaReferences[item] ?? item;
      }
    } else {
      result[key] = embedSchema(item, component);
    }
  }
  return result;
}

function errorResponse(description: string): Record<string, unknown> {
  return {
    description,
    content: {
      "application/json": {
        schema: { $ref: "#/components/schemas/NodeErrorPayload" },
      },
    },
  };
}

function openApiDocument(): Record<string, unknown> {
  const identity = embedSchema(
    readJson("packages/identity/schemas/identity-reference.schema.json"),
    "IdentityReference",
  );
  const verification = embedSchema(
    readJson("packages/identity/schemas/verification-method.schema.json"),
    "VerificationMethod",
  );
  const signature = embedSchema(
    readJson("packages/signer/schemas/signature-envelope.schema.json"),
    "SignatureEnvelope",
  );
  const signedRecord = embedSchema(
    readJson(
      "packages/protocol/schemas/signed-public-shot-record.schema.json",
    ),
    "SignedPublicShotRecord",
  );
  const projection = embedSchema(
    readJson(
      "packages/registry/schemas/public-shot-projection.schema.json",
    ),
    "PublicShotProjection",
  );

  return {
    openapi: "3.1.0",
    info: {
      title: "TOHSENO optional reference node",
      version: "0.5.0",
      description:
        "A replaceable, non-authoritative index for exact signed public Shot records and deterministic public projections.",
    },
    paths: {
      "/healthz": {
        get: {
          operationId: "health",
          responses: {
            "200": {
              description: "The process and SQLite adapter are ready.",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/Health" },
                },
              },
            },
          },
        },
      },
      "/openapi.json": {
        get: {
          operationId: "openApi",
          responses: {
            "200": {
              description: "This OpenAPI document.",
              content: {
                "application/json": {
                  schema: { type: "object" },
                },
              },
            },
          },
        },
      },
      "/v1/records": {
        post: {
          operationId: "submitRecord",
          description:
            "Verify and append one exact signed public Shot record. Exact duplicates are idempotent.",
          requestBody: {
            required: true,
            content: {
              "application/json": {
                schema: {
                  $ref: "#/components/schemas/SignedPublicShotRecord",
                },
              },
            },
          },
          responses: {
            "200": {
              description: "The exact record already existed.",
              content: {
                "application/json": {
                  schema: {
                    $ref: "#/components/schemas/NodeSubmissionPayload",
                  },
                },
              },
            },
            "201": {
              description: "The verified record was appended.",
              content: {
                "application/json": {
                  schema: {
                    $ref: "#/components/schemas/NodeSubmissionPayload",
                  },
                },
              },
            },
            "400": {
              $ref: "#/components/responses/InvalidRecord",
            },
            "409": { $ref: "#/components/responses/Conflict" },
            "413": { $ref: "#/components/responses/TooLarge" },
            "415": {
              $ref: "#/components/responses/UnsupportedMediaType",
            },
            "507": {
              $ref: "#/components/responses/CapacityExceeded",
            },
          },
        },
      },
      "/v1/shots/{shotId}": {
        get: {
          operationId: "getProjection",
          description:
            "Return the exact deterministic public Shot projection.",
          parameters: [
            { $ref: "#/components/parameters/ShotId" },
          ],
          responses: {
            "200": {
              description: "The public Shot projection.",
              content: {
                "application/json": {
                  schema: {
                    $ref: "#/components/schemas/PublicShotProjection",
                  },
                },
              },
            },
            "404": { $ref: "#/components/responses/NotFound" },
          },
        },
      },
      "/v1/shots/{shotId}/records": {
        get: {
          operationId: "getRecords",
          description:
            "Export one exact signed record chain in sequence order.",
          parameters: [
            { $ref: "#/components/parameters/ShotId" },
          ],
          responses: {
            "200": {
              description: "The accepted signed public records.",
              content: {
                "application/json": {
                  schema: {
                    $ref: "#/components/schemas/NodeRecordsPayload",
                  },
                },
              },
            },
            "404": { $ref: "#/components/responses/NotFound" },
          },
        },
      },
    },
    components: {
      parameters: {
        ShotId: {
          name: "shotId",
          in: "path",
          required: true,
          schema: {
            type: "string",
            pattern: "^shot_[A-Za-z0-9_-]{32}$",
          },
        },
      },
      schemas: {
        IdentityReference: identity,
        VerificationMethod: verification,
        SignatureEnvelope: signature,
        SignedPublicShotRecord: signedRecord,
        PublicShotProjection: projection,
        NodeSubmissionPayload: {
          type: "object",
          additionalProperties: false,
          required: [
            "schemaVersion",
            "status",
            "recordHash",
            "projection",
          ],
          properties: {
            schemaVersion: { const: NODE_PAYLOAD_SCHEMA_VERSION },
            status: { enum: ["appended", "existing"] },
            recordHash: {
              type: "string",
              pattern: "^sha256:[a-f0-9]{64}$",
            },
            projection: {
              $ref: "#/components/schemas/PublicShotProjection",
            },
          },
        },
        NodeRecordsPayload: {
          type: "object",
          additionalProperties: false,
          required: ["schemaVersion", "records"],
          properties: {
            schemaVersion: { const: NODE_PAYLOAD_SCHEMA_VERSION },
            records: {
              type: "array",
              minItems: 1,
              items: {
                $ref: "#/components/schemas/SignedPublicShotRecord",
              },
            },
          },
        },
        NodeErrorPayload: {
          type: "object",
          additionalProperties: false,
          required: ["error"],
          properties: {
            error: {
              type: "string",
              pattern: "^[a-z][a-z0-9-]{0,63}$",
            },
          },
        },
        Health: {
          type: "object",
          additionalProperties: false,
          required: [
            "status",
            "service",
            "databaseSchemaVersion",
          ],
          properties: {
            status: { const: "ok" },
            service: { const: "tohseno-reference-node" },
            databaseSchemaVersion: {
              const: REFERENCE_NODE_DATABASE_SCHEMA_VERSION,
            },
          },
        },
      },
      responses: {
        InvalidRecord: errorResponse(
          "The signed public record is invalid.",
        ),
        Conflict: errorResponse(
          "The record conflicts with the accepted Shot history.",
        ),
        TooLarge: errorResponse(
          "The request exceeds the node record limit.",
        ),
        UnsupportedMediaType: errorResponse(
          "The request is not application/json.",
        ),
        CapacityExceeded: errorResponse(
          "This replaceable node's bounded history capacity is exhausted.",
        ),
        NotFound: errorResponse("No public Shot was found."),
      },
    },
  };
}

async function signed(
  record: PublicShotRecord,
  signer: LocalEd25519Signer,
): Promise<SignedPublicShotRecord> {
  return signPublicShotRecord(validatePublicShotRecord(record), signer);
}

async function canonicalFixtureArtifacts(): Promise<Artifact[]> {
  const identity = deriveDeterministicTestIdentity(
    "BUILDER",
    "tohseno-0.5-public-fixture",
    "canonical-artifacts",
  );
  const signer = LocalEd25519Signer.deterministicForTests(
    identity,
    "tohseno-0.5-public-fixture-signing-seed",
    "canonical-artifacts",
  );
  const shotId = deriveDeterministicTestShotId(
    "tohseno-0.5-public-fixture",
    "canonical-artifacts",
  );
  const publication: PublicationEvidence = {
    source: {
      url: "https://source.example/shots/canonical",
      revision: "0123456789abcdef",
    },
    download: {
      url: "https://download.example/shots/canonical.zip",
      artifactDigest:
        `sha256:${"a".repeat(64)}`,
      manifestDigest:
        `sha256:${"b".repeat(64)}`,
    },
  };

  const genesisValue: ShotCreatedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "SHOT_CREATED",
    shotId,
    sequence: 0,
    previousRecordHash: null,
    recordedAt: fixtureTimestamp(0),
    authority: signer.verificationMethod,
    body: {
      name: "Fixture Shot",
      summary: "A deliberately public canonical protocol fixture.",
      platform: "IOS",
      builder: identity,
      lifecycle: "EVOLVING",
      evolution: 0,
    },
  };
  const genesis = await signed(genesisValue, signer);

  const evolutionValue: EvolutionRecordedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "EVOLUTION_RECORDED",
    shotId,
    sequence: 1,
    previousRecordHash: hashSignedPublicShotRecord(genesis),
    recordedAt: fixtureTimestamp(1),
    authority: signer.verificationMethod,
    body: {
      evolution: 1,
      title: "Canonical evolution",
      summary: "The same Shot after one public Evolution.",
    },
  };
  const evolution = await signed(evolutionValue, signer);

  const publicationValue: LifecycleTransitionedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "LIFECYCLE_TRANSITIONED",
    shotId,
    sequence: 2,
    previousRecordHash: hashSignedPublicShotRecord(evolution),
    recordedAt: fixtureTimestamp(2),
    authority: signer.verificationMethod,
    body: {
      from: "EVOLVING",
      to: "PUBLISHED",
      evidence: publication,
    },
  };
  const published = await signed(publicationValue, signer);

  const appStoreValue: LifecycleTransitionedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "LIFECYCLE_TRANSITIONED",
    shotId,
    sequence: 3,
    previousRecordHash: hashSignedPublicShotRecord(published),
    recordedAt: fixtureTimestamp(3),
    authority: signer.verificationMethod,
    body: {
      from: "PUBLISHED",
      to: "APP_STORE",
      evidence: {
        listingId: "1234567890",
        listingUrl:
          "https://apps.apple.com/us/app/fixture-shot/id1234567890",
      },
    },
  };
  const inStore = await signed(appStoreValue, signer);

  const appcoinValue: AppcoinLinkedRecord = {
    protocolVersion: PUBLIC_SHOT_PROTOCOL_VERSION,
    kind: "APPCOIN_LINKED",
    shotId,
    sequence: 4,
    previousRecordHash: hashSignedPublicShotRecord(inStore),
    recordedAt: fixtureTimestamp(4),
    authority: signer.verificationMethod,
    body: {
      link: {
        deployment: {
          namespace: "deployment-system",
          id: "fixture-deployment",
        },
        network: {
          namespace: "network-family",
          id: "fixture-network",
        },
        asset: {
          namespace: "asset-standard",
          id: "fixture-asset",
        },
        evidence: {
          namespace: "public-receipt",
          id: "fixture-receipt",
          url: "https://evidence.example/fixture-receipt",
        },
      },
    },
  };
  const appcoin = await signed(appcoinValue, signer);
  const records = [genesis, evolution, published, inStore, appcoin];
  const verifier = localEd25519VerifierSet();
  const verified = await Promise.all(
    records.map((record) =>
      verifySignedPublicShotRecord(record, verifier)
    ),
  );
  const evolvingProjection = projectPublicShotProjection([verified[0]!]);
  const appStoreProjection = projectPublicShotProjection(verified);
  const submission = createNodeSubmissionPayload({
    status: "appended",
    recordHash: hashSignedPublicShotRecord(appcoin),
    projection: appStoreProjection,
  });

  return [
    {
      path: "packages/identity/fixtures/builder-identity.json",
      value: identity,
    },
    {
      path:
        "packages/identity/fixtures/builder-verification-method.json",
      value: signer.verificationMethod,
    },
    {
      path: "packages/signer/fixtures/signature-envelope.json",
      value: genesis.signature,
    },
    {
      path: "packages/protocol/fixtures/unsigned-shot-created.json",
      value: validatePublicShotRecord(genesisValue),
    },
    {
      path: "packages/protocol/fixtures/signed-shot-created.json",
      value: genesis,
    },
    {
      path: "packages/registry/fixtures/evolving-projection.json",
      value: evolvingProjection,
    },
    {
      path: "packages/registry/fixtures/app-store-projection.json",
      value: appStoreProjection,
    },
    {
      path: "apps/reference-node/fixtures/submission.json",
      value: submission,
    },
    {
      path: "apps/reference-node/fixtures/records-response.json",
      value: createNodeRecordsPayload(records),
    },
    {
      path: "apps/reference-node/openapi.json",
      value: openApiDocument(),
    },
  ];
}

function serialized(value: unknown): string {
  return `${JSON.stringify(object(value, "artifact"), null, 2)}\n`;
}

export async function regenerateCanonicalArtifacts(
  check = false,
): Promise<void> {
  const artifacts = await canonicalFixtureArtifacts();
  const stale: string[] = [];
  for (const artifact of artifacts) {
    const path = join(repositoryRoot, artifact.path);
    const source = serialized(artifact.value);
    if (check) {
      if (!existsSync(path) || readFileSync(path, "utf8") !== source) {
        stale.push(artifact.path);
      }
      continue;
    }
    mkdirSync(dirname(path), { recursive: true });
    await Bun.write(path, source);
  }
  if (stale.length > 0) {
    throw new Error(
      `canonical artifacts are stale: ${stale.join(", ")}`,
    );
  }
}

if (import.meta.main) {
  const args = Bun.argv.slice(2);
  if (args.some((argument) => argument !== "--check")) {
    throw new Error(
      "usage: bun scripts/regenerate-artifacts.ts [--check]",
    );
  }
  await regenerateCanonicalArtifacts(args.includes("--check"));
}
