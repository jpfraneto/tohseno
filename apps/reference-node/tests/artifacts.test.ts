import { describe, expect, test } from "bun:test";
import {
  validateIdentityReference,
  validateVerificationMethod,
} from "../../../packages/identity/src/index.ts";
import {
  validateSignatureEnvelope,
  localEd25519VerifierSet,
} from "../../../packages/signer/src/index.ts";
import {
  validatePublicShotRecord,
  verifySignedPublicShotRecord,
} from "../../../packages/protocol/src/index.ts";
import {
  validatePublicShotProjection,
} from "../../../packages/registry/src/index.ts";
import {
  validateNodeRecordsPayload,
  validateNodeSubmissionPayload,
} from "../../../packages/node-client/src/payloads.ts";
import {
  regenerateCanonicalArtifacts,
} from "../scripts/regenerate-artifacts.ts";

const root = new URL("../../../", import.meta.url);

async function readJson(path: string): Promise<unknown> {
  return Bun.file(new URL(path, root)).json() as Promise<unknown>;
}

function object(value: unknown): Record<string, unknown> {
  expect(value).toBeObject();
  return value as Record<string, unknown>;
}

describe("canonical generated protocol and node artifacts", () => {
  test("are byte-stable outputs of the current implementation", async () => {
    await regenerateCanonicalArtifacts(true);
    await regenerateCanonicalArtifacts(true);
  });

  test("all fixtures pass their canonical runtime validators", async () => {
    const identity = await readJson(
      "packages/identity/fixtures/builder-identity.json",
    );
    const method = await readJson(
      "packages/identity/fixtures/builder-verification-method.json",
    );
    const signature = await readJson(
      "packages/signer/fixtures/signature-envelope.json",
    );
    const unsigned = await readJson(
      "packages/protocol/fixtures/unsigned-shot-created.json",
    );
    const signed = await readJson(
      "packages/protocol/fixtures/signed-shot-created.json",
    );
    const evolving = await readJson(
      "packages/registry/fixtures/evolving-projection.json",
    );
    const inStore = await readJson(
      "packages/registry/fixtures/app-store-projection.json",
    );
    const submission = await readJson(
      "apps/reference-node/fixtures/submission.json",
    );
    const recordSet = await readJson(
      "apps/reference-node/fixtures/records-response.json",
    );

    expect(identity).toEqual(validateIdentityReference(identity));
    expect(method).toEqual(validateVerificationMethod(method));
    expect(signature).toEqual(validateSignatureEnvelope(signature));
    expect(unsigned).toEqual(validatePublicShotRecord(unsigned));
    expect(signed).toEqual(
      await verifySignedPublicShotRecord(
        signed,
        localEd25519VerifierSet(),
      ),
    );
    expect(evolving).toEqual(validatePublicShotProjection(evolving));
    expect(inStore).toEqual(validatePublicShotProjection(inStore));
    expect(submission).toEqual(validateNodeSubmissionPayload(submission));
    const records = validateNodeRecordsPayload(recordSet).records;
    await Promise.all(
      records.map((record) =>
        verifySignedPublicShotRecord(record, localEd25519VerifierSet())
      ),
    );
    const evolution = records.find((record) =>
      record.kind === "EVOLUTION_RECORDED"
    );
    expect(evolution?.body).not.toHaveProperty("publication");
    const publication = records.find((record) =>
      record.kind === "LIFECYCLE_TRANSITIONED" &&
      record.body.from === "EVOLVING"
    );
    expect(publication?.body).toHaveProperty("evidence.source");
  });

  test("OpenAPI uses exact embedded record and projection schemas", async () => {
    const openapi = object(
      await readJson("apps/reference-node/openapi.json"),
    );
    const paths = object(openapi.paths);
    const submit = object(object(object(paths["/v1/records"]).post)
      .requestBody);
    const submitContent = object(object(submit.content)["application/json"]);
    expect(object(submitContent.schema).$ref).toBe(
      "#/components/schemas/SignedPublicShotRecord",
    );

    const projectionGet = object(object(paths["/v1/shots/{shotId}"]).get);
    const projectionResponses = object(projectionGet.responses);
    const projectionContent = object(
      object(object(projectionResponses["200"]).content)["application/json"],
    );
    expect(object(projectionContent.schema).$ref).toBe(
      "#/components/schemas/PublicShotProjection",
    );

    const recordsGet = object(
      object(paths["/v1/shots/{shotId}/records"]).get,
    );
    const recordsResponses = object(recordsGet.responses);
    const recordsContent = object(
      object(object(recordsResponses["200"]).content)["application/json"],
    );
    expect(object(recordsContent.schema).$ref).toBe(
      "#/components/schemas/NodeRecordsPayload",
    );

    const components = object(openapi.components);
    const schemas = object(components.schemas);
    expect(object(schemas.SignedPublicShotRecord).additionalProperties)
      .toBe(false);
    expect(object(schemas.PublicShotProjection).additionalProperties)
      .toBe(false);
  });
});
