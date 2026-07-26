import { describe, expect, test } from "bun:test";
import Ajv2020, { type AnySchema } from "ajv/dist/2020";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  APP_MANIFEST_SCHEMA_VERSION,
  parseAppManifest,
  validateAppManifest,
  type AppManifest,
} from "../app";

const root = new URL("../../../", import.meta.url);
const freshShotError =
  "Pre-release compatibility is unsupported. Create a fresh Shot with TOHSENO 0.5.";

async function readJson(relativePath: string): Promise<unknown> {
  return Bun.file(new URL(relativePath, root)).json() as Promise<unknown>;
}

async function canonicalManifest(): Promise<AppManifest> {
  const value = await readJson(
    "templates/ios-kernel/overlay/app.manifest.json",
  );
  const result = validateAppManifest(value);
  expect(result.errors).toEqual([]);
  return structuredClone(value) as AppManifest;
}

function runCli(
  cwd: string,
  ...args: string[]
): { exitCode: number; stdout: string; stderr: string } {
  const child = Bun.spawnSync(["bun", "run", "validate", ...args], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  const decode = (bytes: Uint8Array): string => new TextDecoder().decode(bytes);
  return {
    exitCode: child.exitCode,
    stdout: decode(child.stdout),
    stderr: decode(child.stderr),
  };
}

describe("canonical app manifest", () => {
  test("the JSON Schema and runtime validator accept every bundled manifest", async () => {
    const schema = await readJson(
      "packages/manifest/app.manifest.schema.json",
    );
    const object = schema as Record<string, unknown>;
    expect(object.$schema).toBe(
      "https://json-schema.org/draft/2020-12/schema",
    );
    expect(object.title).toBe("TOHSENO app manifest");

    const validateSchema = new Ajv2020({
      allErrors: true,
      strict: false,
    }).compile(schema as AnySchema);
    for (const path of [
      "templates/ios-kernel/overlay/app.manifest.json",
      "templates/daily-game/overlay/app.manifest.json",
    ]) {
      const value = await readJson(path);
      expect(validateSchema(value), path).toBe(true);
      expect(validateAppManifest(value).valid, path).toBe(true);
    }
  });

  test("uses one exact schema version and closed root shape", async () => {
    const wrongVersion = await canonicalManifest() as unknown as {
      schemaVersion: string;
    };
    wrongVersion.schemaVersion = "999";
    expect(validateAppManifest(wrongVersion).errors.map((issue) => issue.code))
      .toContain("schema-version");

    const unknown = {
      ...(await canonicalManifest()),
      unexpectedRuntime: {},
    };
    const result = validateAppManifest(unknown);
    expect(result.valid).toBe(false);
    expect(result.errors).toContainEqual({
      severity: "error",
      code: "unknown",
      path: "$.unexpectedRuntime",
      message: "field is not supported",
    });
  });

  test("does not make continuity capabilities universal", async () => {
    const value = await canonicalManifest();
    expect(JSON.stringify(value)).not.toMatch(
      /continuity|writing|bip39|sqlite|backend/iu,
    );
  });

  test("requires raw intention to remain untracked", async () => {
    const value = await canonicalManifest() as unknown as {
      privacy: { rawIntentionTracked: boolean };
    };
    value.privacy.rawIntentionTracked = true;
    const result = validateAppManifest(value);
    expect(result.valid).toBe(false);
    expect(result.errors.map((issue) => issue.code)).toContain(
      "private-intention",
    );
  });

  test("rejects duplicate values and duplicate object identities", async () => {
    const duplicateValue = await canonicalManifest();
    duplicateValue.data.local = ["progress", "progress"];
    expect(validateAppManifest(duplicateValue).errors.map((issue) => issue.code))
      .toContain("duplicate");

    const duplicateIdentity = await canonicalManifest();
    duplicateIdentity.storage = [
      { id: "progress", location: "device", content: "score" },
      { id: "progress", location: "device", content: "rank" },
    ];
    expect(validateAppManifest(duplicateIdentity).errors).toContainEqual({
      severity: "error",
      code: "duplicate",
      path: "$.storage[1].id",
      message: "duplicates $.storage[0].id",
    });
  });

  test("parseAppManifest returns only validated canonical values", async () => {
    const source = await Bun.file(
      new URL("templates/ios-kernel/overlay/app.manifest.json", root),
    ).text();
    expect(parseAppManifest(source).schemaVersion).toBe(
      APP_MANIFEST_SCHEMA_VERSION,
    );
    expect(() => parseAppManifest("{not-json")).toThrow();
    expect(() =>
      parseAppManifest(JSON.stringify({
        schemaVersion: "0.4.0",
        kind: "continuity",
      }))
    ).toThrow();
  });
});

describe("app manifest CLI", () => {
  test("the root and package gates accept only canonical manifests", () => {
    const rootDir = fileURLToPath(root);
    const packageDir = fileURLToPath(new URL("../", import.meta.url));
    for (const [cwd, path] of [
      [rootDir, "templates/ios-kernel/overlay/app.manifest.json"],
      [packageDir, "../../templates/daily-game/overlay/app.manifest.json"],
    ] as const) {
      const result = runCli(cwd, path);
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain(
        `app.manifest ${APP_MANIFEST_SCHEMA_VERSION} · valid`,
      );
      expect(result.stderr).not.toContain("✗");
      expect(result.stderr).not.toContain(freshShotError);
    }
  });

  test("rejects obsolete and unrecognized formats without changing them", () => {
    const rootDir = fileURLToPath(root);
    const scratch = mkdtempSync(join(tmpdir(), "tohseno-manifest-old-"));
    try {
      const cases = [
        {
          name: "continuity.manifest.json",
          value: {
            schemaVersion: "0.4.0",
            application: { id: "com.example.old", name: "Old" },
            runtime: {},
          },
        },
        {
          name: "unrecognized.json",
          value: { schemaVersion: "999", kind: "project" },
        },
      ];
      for (const item of cases) {
        const path = join(scratch, item.name);
        const source = `${JSON.stringify(item.value, null, 2)}\n`;
        writeFileSync(path, source);
        const result = runCli(rootDir, path);
        expect(result.exitCode).toBe(1);
        expect(result.stderr).toContain(
          "is not a canonical app.manifest 1.0.0",
        );
        expect(result.stderr).toContain(freshShotError);
        expect(readFileSync(path, "utf8")).toBe(source);
      }
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  });

  test("rejects malformed JSON with the same fresh-Shot direction", () => {
    const rootDir = fileURLToPath(root);
    const scratch = mkdtempSync(join(tmpdir(), "tohseno-manifest-json-"));
    try {
      const path = join(scratch, "app.manifest.json");
      writeFileSync(path, "{not-json\n");
      const result = runCli(rootDir, path);
      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("is not valid JSON");
      expect(result.stderr).toContain(freshShotError);
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  });

  test("keeps usage and safe-file failures distinct from format failures", () => {
    const rootDir = fileURLToPath(root);
    expect(runCli(rootDir).exitCode).toBe(2);
    expect(runCli(rootDir, "no-such-file.json").exitCode).toBe(2);

    const scratch = mkdtempSync(join(tmpdir(), "tohseno-manifest-gate-"));
    try {
      const target = join(scratch, "target.json");
      const link = join(scratch, "app.manifest.json");
      writeFileSync(target, "{}\n");
      symlinkSync(target, link);
      expect(runCli(rootDir, link).exitCode).toBe(1);

      const oversized = join(scratch, "oversized.json");
      writeFileSync(oversized, " ".repeat(1_048_577));
      expect(runCli(rootDir, oversized).exitCode).toBe(1);
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  });
});
