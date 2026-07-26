import { describe, expect, test } from "bun:test";
import Ajv2020, { type AnySchema } from "ajv/dist/2020";
import { validateAppManifest } from "../app.ts";

const ROOT = new URL("../../../", import.meta.url);

async function readJson(relativePath: string): Promise<unknown> {
  return Bun.file(new URL(relativePath, ROOT)).json() as Promise<unknown>;
}

describe("app manifest JSON Schema alignment", () => {
  test("identity details must be non-empty in schema and runtime", async () => {
    const schema = await readJson(
      "packages/manifest/app.manifest.schema.json",
    );
    const validateSchema = new Ajv2020({
      allErrors: true,
      strict: false,
    }).compile(schema as AnySchema);
    const manifest = await readJson(
      "templates/ios-kernel/overlay/app.manifest.json",
    ) as { identity: { details?: string } };
    manifest.identity.details = "";

    expect(validateSchema(manifest)).toBe(false);
    expect(validateAppManifest(manifest).valid).toBe(false);
  });
});
