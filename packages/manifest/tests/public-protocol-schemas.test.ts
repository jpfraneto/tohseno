import { describe, expect, test } from "bun:test";
import Ajv2020, { type AnySchema } from "ajv/dist/2020";
import {
  validateIdentityReference,
  validateVerificationMethod,
} from "../../identity/src/index.ts";
import { validateSignatureEnvelope } from "../../signer/src/index.ts";
import { validateSignedPublicShotRecord } from "../../protocol/src/index.ts";
import { validatePublicShotProjection } from "../../registry/src/index.ts";

const root = new URL("../../../", import.meta.url);

async function readJson(path: string): Promise<unknown> {
  return Bun.file(new URL(path, root)).json() as Promise<unknown>;
}

function record(value: unknown): Record<string, unknown> {
  expect(value).toBeObject();
  return value as Record<string, unknown>;
}

function resolveJsonPointer(
  document: unknown,
  reference: string,
): unknown {
  expect(reference.startsWith("#/")).toBe(true);
  let current = document;
  for (const encoded of reference.slice(2).split("/")) {
    const key = encoded.replaceAll("~1", "/").replaceAll("~0", "~");
    expect(current).toBeObject();
    expect(Object.hasOwn(current as object, key)).toBe(true);
    current = (current as Record<string, unknown>)[key];
  }
  return current;
}

function localReferences(value: unknown): string[] {
  if (Array.isArray(value)) return value.flatMap(localReferences);
  if (typeof value !== "object" || value === null) return [];
  const candidate = value as Record<string, unknown>;
  return [
    ...(typeof candidate.$ref === "string" &&
        candidate.$ref.startsWith("#/")
      ? [candidate.$ref]
      : []),
    ...Object.values(candidate).flatMap(localReferences),
  ];
}

describe("public protocol contract artifacts", () => {
  test("schemas resolve and agree with runtime validators and fixtures", async () => {
    const schemaPaths = [
      "packages/identity/schemas/identity-reference.schema.json",
      "packages/identity/schemas/verification-method.schema.json",
      "packages/signer/schemas/signature-envelope.schema.json",
      "packages/protocol/schemas/signed-public-shot-record.schema.json",
      "packages/registry/schemas/public-shot-projection.schema.json",
    ] as const;
    const schemas = await Promise.all(schemaPaths.map(readJson));
    const ajv = new Ajv2020({ allErrors: true, strict: false });
    ajv.addFormat("date-time", {
      type: "string",
      validate(value: string): boolean {
        return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u
          .test(value) &&
          Number.isFinite(Date.parse(value)) &&
          new Date(value).toISOString() === value;
      },
    });
    ajv.addFormat("uri", {
      type: "string",
      validate(value: string): boolean {
        try {
          return new URL(value).toString() === value;
        } catch {
          return false;
        }
      },
    });
    for (const schema of schemas) ajv.addSchema(schema as AnySchema);

    const identity = await readJson(
      "packages/identity/fixtures/builder-identity.json",
    );
    const signature = await readJson(
      "packages/signer/fixtures/signature-envelope.json",
    );
    const signedRecord = await readJson(
      "packages/protocol/fixtures/signed-shot-created.json",
    );
    const projection = await readJson(
      "packages/registry/fixtures/evolving-projection.json",
    );
    const signed = record(signedRecord);

    expect(
      ajv.getSchema("urn:tohseno:schema:identity-reference:v1")!(identity),
    ).toBe(true);
    expect(
      ajv.getSchema("urn:tohseno:schema:verification-method:v1")!(
        signed.authority,
      ),
    ).toBe(true);
    expect(
      ajv.getSchema("urn:tohseno:schema:signature-envelope:v1")!(signature),
    ).toBe(true);
    expect(
      ajv.getSchema("urn:tohseno:schema:signed-public-shot-record:v1")!(
        signedRecord,
      ),
    ).toBe(true);
    expect(
      ajv.getSchema("urn:tohseno:schema:public-shot-projection:v1")!(projection),
    ).toBe(true);

    expect(() => validateIdentityReference(identity)).not.toThrow();
    expect(() => validateVerificationMethod(signed.authority)).not.toThrow();
    expect(() => validateSignatureEnvelope(signature)).not.toThrow();
    expect(() => validateSignedPublicShotRecord(signedRecord)).not.toThrow();
    expect(() => validatePublicShotProjection(projection)).not.toThrow();

    const privateField = {
      ...signed,
      privatePrompt: "not a public protocol field",
    };
    expect(
      ajv.getSchema("urn:tohseno:schema:signed-public-shot-record:v1")!(
        privateField,
      ),
    ).toBe(false);
    expect(() => validateSignedPublicShotRecord(privateField)).toThrow(
      "is not allowed",
    );
  });

  test("every local OpenAPI reference resolves", async () => {
    const openapi = await readJson("apps/reference-node/openapi.json");
    const references = localReferences(openapi);
    expect(references.length).toBeGreaterThan(0);
    for (const reference of references) {
      expect(resolveJsonPointer(openapi, reference)).toBeDefined();
    }
  });
});
