export const APP_MANIFEST_SCHEMA_VERSION = "1.0.0" as const;

export interface AppManifest {
  schemaVersion: typeof APP_MANIFEST_SCHEMA_VERSION;
  kind: "app";
  application: { id: string; name: string };
  platform: "ios";
  composition: { kernel: string; template: string; skills: string[] };
  data: { local: string[]; remote: string[] };
  storage: Array<{ id: string; location: string; content: string }>;
  network: Array<{ id: string; purpose: string; data: string[] }>;
  identity: {
    strategy: "none" | "local-device" | "wallet" | "account";
    details?: string;
  };
  entitlements: string[];
  integrations: Array<{ id: string; purpose: string }>;
  operations: { project: string; scheme: string; product: string };
  privacy: {
    rawIntentionTracked: false;
    appContentLeavesDevice: boolean;
    declarations?: string[];
  };
  production: { ready: boolean; declarations: string[] };
  irreversibleOperations: Array<{
    id: string;
    approval: "explicit-owner";
  }>;
}

export interface AppManifestIssue {
  severity: "error" | "warning";
  code: string;
  path: string;
  message: string;
}

export interface AppManifestValidationResult {
  valid: boolean;
  errors: AppManifestIssue[];
  warnings: AppManifestIssue[];
}

const ID = /^[a-z0-9]+(?:-[a-z0-9]+)*$/u;
const BUNDLE_ID = /^[A-Za-z0-9]+(?:\.[A-Za-z0-9-]+)+$/u;
const PROJECT = /^[A-Za-z0-9._-]+\.xcodeproj$/u;
const NAME = /^[A-Za-z0-9._-]+$/u;

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function issue(
  issues: AppManifestIssue[],
  code: string,
  path: string,
  message: string,
): void {
  issues.push({ severity: "error", code, path, message });
}

function shape(
  value: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[],
  path: string,
  issues: AppManifestIssue[],
): void {
  const allowed = new Set([...required, ...optional]);
  for (const key of required) {
    if (!(key in value)) issue(issues, "required", `${path}.${key}`, "field is required");
  }
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) issue(issues, "unknown", `${path}.${key}`, "field is not supported");
  }
}

function strings(
  value: unknown,
  path: string,
  issues: AppManifestIssue[],
  identifiers = false,
): string[] {
  if (
    !Array.isArray(value) ||
    value.some((item) =>
      typeof item !== "string" ||
      (identifiers && !ID.test(item))
    )
  ) {
    issue(
      issues,
      "type",
      path,
      identifiers ? "must be an array of app identifiers" : "must be an array of strings",
    );
    return [];
  }
  if (new Set(value).size !== value.length) {
    issue(issues, "duplicate", path, "must not contain duplicate values");
  }
  return value as string[];
}

function identifier(value: unknown, path: string, issues: AppManifestIssue[]): void {
  if (typeof value !== "string" || !ID.test(value)) {
    issue(issues, "format", path, "must be a lowercase app identifier");
  }
}

function objectArray(
  value: unknown,
  path: string,
  issues: AppManifestIssue[],
  validate: (item: Record<string, unknown>, path: string) => void,
): void {
  if (!Array.isArray(value)) {
    issue(issues, "type", path, "must be an array");
    return;
  }
  value.forEach((item, index) => {
    const itemRecord = record(item);
    if (itemRecord === null) {
      issue(issues, "type", `${path}[${index}]`, "must be an object");
    } else {
      validate(itemRecord, `${path}[${index}]`);
    }
  });
}

function nonEmpty(
  value: unknown,
  path: string,
  issues: AppManifestIssue[],
): void {
  if (typeof value !== "string" || value.trim() === "") {
    issue(issues, "type", path, "must be a non-empty string");
  }
}

export function validateAppManifest(input: unknown): AppManifestValidationResult {
  const errors: AppManifestIssue[] = [];
  const warnings: AppManifestIssue[] = [];
  const root = record(input);
  if (root === null) {
    issue(errors, "type", "$", "manifest must be an object");
    return { valid: false, errors, warnings };
  }
  shape(root, [
    "schemaVersion",
    "kind",
    "application",
    "platform",
    "composition",
    "data",
    "storage",
    "network",
    "identity",
    "entitlements",
    "integrations",
    "operations",
    "privacy",
    "production",
    "irreversibleOperations",
  ], [], "$", errors);
  if (root.schemaVersion !== APP_MANIFEST_SCHEMA_VERSION) {
    issue(errors, "schema-version", "$.schemaVersion", `must be ${APP_MANIFEST_SCHEMA_VERSION}`);
  }
  if (root.kind !== "app") issue(errors, "kind", "$.kind", "must be app");
  if (root.platform !== "ios") issue(errors, "platform", "$.platform", "must be ios");

  const application = record(root.application);
  if (application === null) {
    issue(errors, "type", "$.application", "must be an object");
  } else {
    shape(application, ["id", "name"], [], "$.application", errors);
    if (typeof application.id !== "string" || !BUNDLE_ID.test(application.id)) {
      issue(errors, "bundle-id", "$.application.id", "must be a reverse-domain bundle identifier");
    }
    if (
      typeof application.name !== "string" ||
      application.name.trim() === "" ||
      application.name.length > 80
    ) {
      issue(errors, "name", "$.application.name", "must be a non-empty app name up to 80 characters");
    }
  }

  const composition = record(root.composition);
  if (composition === null) {
    issue(errors, "type", "$.composition", "must be an object");
  } else {
    shape(composition, ["kernel", "template", "skills"], [], "$.composition", errors);
    identifier(composition.kernel, "$.composition.kernel", errors);
    identifier(composition.template, "$.composition.template", errors);
    strings(composition.skills, "$.composition.skills", errors, true);
  }

  const data = record(root.data);
  if (data === null) {
    issue(errors, "type", "$.data", "must be an object");
  } else {
    shape(data, ["local", "remote"], [], "$.data", errors);
    strings(data.local, "$.data.local", errors);
    strings(data.remote, "$.data.remote", errors);
  }

  objectArray(root.storage, "$.storage", errors, (item, path) => {
    shape(item, ["id", "location", "content"], [], path, errors);
    identifier(item.id, `${path}.id`, errors);
    nonEmpty(item.location, `${path}.location`, errors);
    nonEmpty(item.content, `${path}.content`, errors);
  });
  objectArray(root.network, "$.network", errors, (item, path) => {
    shape(item, ["id", "purpose", "data"], [], path, errors);
    identifier(item.id, `${path}.id`, errors);
    nonEmpty(item.purpose, `${path}.purpose`, errors);
    strings(item.data, `${path}.data`, errors);
  });

  const identity = record(root.identity);
  if (identity === null) {
    issue(errors, "type", "$.identity", "must be an object");
  } else {
    shape(identity, ["strategy"], ["details"], "$.identity", errors);
    if (!["none", "local-device", "wallet", "account"].includes(String(identity.strategy))) {
      issue(errors, "enum", "$.identity.strategy", "uses an unsupported identity strategy");
    }
    if (identity.details !== undefined) nonEmpty(identity.details, "$.identity.details", errors);
  }
  strings(root.entitlements, "$.entitlements", errors);
  objectArray(root.integrations, "$.integrations", errors, (item, path) => {
    shape(item, ["id", "purpose"], [], path, errors);
    identifier(item.id, `${path}.id`, errors);
    nonEmpty(item.purpose, `${path}.purpose`, errors);
  });

  const operations = record(root.operations);
  if (operations === null) {
    issue(errors, "type", "$.operations", "must be an object");
  } else {
    shape(operations, ["project", "scheme", "product"], [], "$.operations", errors);
    if (typeof operations.project !== "string" || !PROJECT.test(operations.project)) {
      issue(errors, "format", "$.operations.project", "must name an Xcode project");
    }
    for (const key of ["scheme", "product"] as const) {
      if (typeof operations[key] !== "string" || !NAME.test(operations[key])) {
        issue(errors, "format", `$.operations.${key}`, "contains unsupported characters");
      }
    }
  }

  const privacy = record(root.privacy);
  if (privacy === null) {
    issue(errors, "type", "$.privacy", "must be an object");
  } else {
    shape(
      privacy,
      ["rawIntentionTracked", "appContentLeavesDevice"],
      ["declarations"],
      "$.privacy",
      errors,
    );
    if (privacy.rawIntentionTracked !== false) {
      issue(errors, "private-intention", "$.privacy.rawIntentionTracked", "raw intention must remain untracked");
    }
    if (typeof privacy.appContentLeavesDevice !== "boolean") {
      issue(errors, "type", "$.privacy.appContentLeavesDevice", "must be boolean");
    }
    if (privacy.declarations !== undefined) {
      strings(privacy.declarations, "$.privacy.declarations", errors);
    }
  }

  const production = record(root.production);
  if (production === null) {
    issue(errors, "type", "$.production", "must be an object");
  } else {
    shape(production, ["ready", "declarations"], [], "$.production", errors);
    if (typeof production.ready !== "boolean") {
      issue(errors, "type", "$.production.ready", "must be boolean");
    }
    strings(production.declarations, "$.production.declarations", errors);
  }

  objectArray(
    root.irreversibleOperations,
    "$.irreversibleOperations",
    errors,
    (item, path) => {
      shape(item, ["id", "approval"], [], path, errors);
      identifier(item.id, `${path}.id`, errors);
      if (item.approval !== "explicit-owner") {
        issue(errors, "approval", `${path}.approval`, "must require explicit-owner approval");
      }
    },
  );
  return { valid: errors.length === 0, errors, warnings };
}

export function parseAppManifest(json: string): AppManifest {
  const value = JSON.parse(json) as unknown;
  const result = validateAppManifest(value);
  if (!result.valid) {
    throw new Error(
      result.errors.map((item) => `${item.path}: ${item.message}`).join("\n"),
    );
  }
  return value as AppManifest;
}
