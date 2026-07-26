import { describe, expect, test } from "bun:test";
import Ajv2020, {
  type AnySchema,
  type ValidateFunction,
} from "ajv/dist/2020";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  validateSkillDescriptor,
  validateTemplateDescriptor,
} from "../index.ts";

const ROOT = resolve(import.meta.dir, "../../..");

function readJson(path: string): unknown {
  return JSON.parse(readFileSync(resolve(ROOT, path), "utf8")) as unknown;
}

function compile(path: string): ValidateFunction {
  return new Ajv2020({ allErrors: true, strict: false }).compile(
    readJson(path) as AnySchema,
  );
}

function cloneRecord(value: unknown): Record<string, unknown> {
  return structuredClone(value) as Record<string, unknown>;
}

function expectRejectedBySchemaAndRuntime(
  validateSchema: ValidateFunction,
  validateRuntime: (value: unknown) => unknown,
  value: unknown,
): void {
  expect(validateSchema(value), JSON.stringify(validateSchema.errors)).toBe(
    false,
  );
  expect(() => validateRuntime(value)).toThrow();
}

function arraySchemasWithoutUniqueness(
  value: unknown,
  path = "$",
): string[] {
  if (Array.isArray(value)) {
    return value.flatMap((item, index) =>
      arraySchemasWithoutUniqueness(item, `${path}[${index}]`)
    );
  }
  if (typeof value !== "object" || value === null) {
    return [];
  }
  const record = value as Record<string, unknown>;
  const current = record.type === "array" && record.uniqueItems !== true
    ? [path]
    : [];
  return [
    ...current,
    ...Object.entries(record).flatMap(([key, item]) =>
      arraySchemasWithoutUniqueness(item, `${path}.${key}`)
    ),
  ];
}

describe("app descriptor JSON Schemas", () => {
  test("accept every bundled descriptor that the runtime accepts", () => {
    const validateSkillSchema = compile(
      "packages/skills/app-skill.schema.json",
    );
    const validateTemplateSchema = compile(
      "packages/skills/template.schema.json",
    );

    for (const path of [
      "skills/daily-challenge/skill.json",
      "skills/local-progress/skill.json",
      "skills/rank-progression/skill.json",
      "skills/share-card/skill.json",
    ]) {
      const descriptor = readJson(path);
      expect(validateSkillSchema(descriptor), path).toBe(true);
      expect(() => validateSkillDescriptor(descriptor), path).not.toThrow();
    }

    for (const path of [
      "templates/blank/template.json",
      "templates/daily-game/template.json",
    ]) {
      const descriptor = readJson(path);
      expect(validateTemplateSchema(descriptor), path).toBe(true);
      expect(() => validateTemplateDescriptor(descriptor), path).not.toThrow();
    }
  });

  test("require exactly the iOS platform and unique array values", () => {
    const skillSchema = readJson("packages/skills/app-skill.schema.json");
    const templateSchema = readJson("packages/skills/template.schema.json");
    expect(arraySchemasWithoutUniqueness(skillSchema)).toEqual([]);
    expect(arraySchemasWithoutUniqueness(templateSchema)).toEqual([]);

    const validateSkillSchema = new Ajv2020({
      allErrors: true,
      strict: false,
    }).compile(skillSchema as AnySchema);
    const skill = readJson("skills/daily-challenge/skill.json");
    for (const platforms of [[], ["ios", "ios"], ["android"]]) {
      expectRejectedBySchemaAndRuntime(
        validateSkillSchema,
        validateSkillDescriptor,
        { ...cloneRecord(skill), platforms },
      );
    }
    expectRejectedBySchemaAndRuntime(
      validateSkillSchema,
      validateSkillDescriptor,
      {
        ...cloneRecord(skill),
        requires: ["local-progress", "local-progress"],
      },
    );

    const validateTemplateSchema = new Ajv2020({
      allErrors: true,
      strict: false,
    }).compile(templateSchema as AnySchema);
    const template = readJson("templates/daily-game/template.json");
    for (const platforms of [[], ["ios", "ios"], ["android"]]) {
      expectRejectedBySchemaAndRuntime(
        validateTemplateSchema,
        validateTemplateDescriptor,
        { ...cloneRecord(template), platforms },
      );
    }
    expectRejectedBySchemaAndRuntime(
      validateTemplateSchema,
      validateTemplateDescriptor,
      {
        ...cloneRecord(template),
        skills: ["local-progress", "local-progress"],
      },
    );
  });

  test("apply the runtime identifier and safe relative-path constraints", () => {
    const validateSkillSchema = compile(
      "packages/skills/app-skill.schema.json",
    );
    const skill = readJson("skills/daily-challenge/skill.json");
    const invalidSkillId = { ...cloneRecord(skill), id: "Daily Challenge" };
    expectRejectedBySchemaAndRuntime(
      validateSkillSchema,
      validateSkillDescriptor,
      invalidSkillId,
    );

    for (const overlay of [
      "",
      " ",
      "/absolute",
      "\\absolute",
      ".",
      "..",
      "../outside",
      "nested/./overlay",
      "nested/../overlay",
      "nested//overlay",
      "nested\\\\overlay",
      "nested/overlay/",
    ]) {
      const descriptor = cloneRecord(skill);
      descriptor.contributions = {
        ...descriptor.contributions as Record<string, unknown>,
        overlay,
      };
      expectRejectedBySchemaAndRuntime(
        validateSkillSchema,
        validateSkillDescriptor,
        descriptor,
      );
    }

    const backslashPath = cloneRecord(skill);
    backslashPath.contributions = {
      ...backslashPath.contributions as Record<string, unknown>,
      overlay: "nested\\overlay",
    };
    expect(validateSkillSchema(backslashPath)).toBe(true);
    expect(
      validateSkillDescriptor(backslashPath).contributions.overlay,
    ).toBe("nested/overlay");

    const validateTemplateSchema = compile(
      "packages/skills/template.schema.json",
    );
    const template = readJson("templates/daily-game/template.json");
    expectRejectedBySchemaAndRuntime(
      validateTemplateSchema,
      validateTemplateDescriptor,
      { ...cloneRecord(template), kernel: "iOS Kernel" },
    );

    const unsafeReplacement = cloneRecord(template);
    unsafeReplacement.replaces = ["App/../outside.swift"];
    expectRejectedBySchemaAndRuntime(
      validateTemplateSchema,
      validateTemplateDescriptor,
      unsafeReplacement,
    );
  });
});
