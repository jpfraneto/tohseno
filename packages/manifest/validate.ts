import {
  CONTINUITY_MANIFEST_SCHEMA_VERSION,
  type ContinuityManifest,
  type ManifestIssueSeverity,
  type ManifestValidationIssue,
  type ManifestValidationResult,
} from "./types";

type UnknownRecord = Record<string, unknown>;

const CONTRACT_ID = /^[a-z0-9][a-z0-9._-]*$/;
const APPLICATION_ID = /^[a-z0-9]+(?:[.-][a-z0-9]+)+$/;
const MEDIA_TYPE = /^[a-z0-9.+-]+\/[a-z0-9.+-]+$/;
const REQUIRED_APPROVALS = [
  "paid-infrastructure",
  "dns-change",
  "store-submission",
  "production-credential-rotation",
] as const;

function hasOwn(value: UnknownRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function pathFor(parent: string, key: string | number): string {
  return typeof key === "number" ? `${parent}[${key}]` : `${parent}.${key}`;
}

function addIssue(
  issues: ManifestValidationIssue[],
  path: string,
  code: string,
  message: string,
  severity: ManifestIssueSeverity = "error",
): void {
  issues.push({ path, code, message, severity });
}

function objectValue(
  value: unknown,
  path: string,
  issues: ManifestValidationIssue[],
): UnknownRecord | undefined {
  if (!isRecord(value)) {
    addIssue(issues, path, "type.object", "must be an object");
    return undefined;
  }
  return value;
}

function checkShape(
  value: UnknownRecord,
  path: string,
  required: readonly string[],
  allowed: readonly string[],
  issues: ManifestValidationIssue[],
): void {
  for (const key of required) {
    if (!hasOwn(value, key)) {
      addIssue(
        issues,
        pathFor(path, key),
        "required",
        "is required",
      );
    }
  }
  const allowedKeys = new Set(allowed);
  for (const key of Object.keys(value)) {
    if (!allowedKeys.has(key)) {
      addIssue(
        issues,
        pathFor(path, key),
        "additional-property",
        `is not part of continuity.manifest ${CONTINUITY_MANIFEST_SCHEMA_VERSION}`,
      );
    }
  }
}

interface StringOptions {
  min?: number;
  max?: number;
  pattern?: RegExp | undefined;
}

function stringValue(
  value: unknown,
  path: string,
  issues: ManifestValidationIssue[],
  options: StringOptions = {},
): string | undefined {
  if (typeof value !== "string") {
    addIssue(issues, path, "type.string", "must be a string");
    return undefined;
  }
  const min = options.min ?? 1;
  if (value.length < min || !/\S/u.test(value)) {
    addIssue(issues, path, "string.too-short", `must contain at least ${min} useful character${min === 1 ? "" : "s"}`);
  }
  if (options.max !== undefined && value.length > options.max) {
    addIssue(issues, path, "string.too-long", `must contain at most ${options.max} characters`);
  }
  if (options.pattern !== undefined && !options.pattern.test(value)) {
    addIssue(issues, path, "string.pattern", "has an invalid format");
  }
  return value;
}

function integerValue(
  value: unknown,
  path: string,
  issues: ManifestValidationIssue[],
  minimum: number,
  maximum: number,
): number | undefined {
  if (!Number.isInteger(value)) {
    addIssue(issues, path, "type.integer", "must be an integer");
    return undefined;
  }
  const integer = value as number;
  if (integer < minimum || integer > maximum) {
    addIssue(
      issues,
      path,
      "number.range",
      `must be between ${minimum} and ${maximum}`,
    );
  }
  return integer;
}

function enumValue<T extends string>(
  value: unknown,
  path: string,
  allowed: readonly T[],
  issues: ManifestValidationIssue[],
): T | undefined {
  if (typeof value !== "string" || !allowed.includes(value as T)) {
    addIssue(
      issues,
      path,
      "enum",
      `must be one of: ${allowed.join(", ")}`,
    );
    return undefined;
  }
  return value as T;
}

interface ArrayOptions {
  min?: number;
  max?: number;
  allowEmpty?: boolean;
  pattern?: RegExp;
}

function stringArrayValue(
  value: unknown,
  path: string,
  issues: ManifestValidationIssue[],
  options: ArrayOptions = {},
): string[] | undefined {
  if (!Array.isArray(value)) {
    addIssue(issues, path, "type.array", "must be an array");
    return undefined;
  }
  const minimum = options.allowEmpty ? 0 : (options.min ?? 1);
  if (value.length < minimum) {
    addIssue(issues, path, "array.too-short", `must contain at least ${minimum} item${minimum === 1 ? "" : "s"}`);
  }
  if (options.max !== undefined && value.length > options.max) {
    addIssue(issues, path, "array.too-long", `must contain at most ${options.max} items`);
  }
  const strings: string[] = [];
  value.forEach((entry, index) => {
    const itemPath = pathFor(path, index);
    const item = stringValue(entry, itemPath, issues, {
      max: 1000,
      pattern: options.pattern,
    });
    if (item !== undefined) strings.push(item);
  });
  if (new Set(strings).size !== strings.length) {
    addIssue(issues, path, "array.unique", "must not contain duplicate items");
  }
  return strings;
}

function validateApplication(
  value: unknown,
  issues: ManifestValidationIssue[],
): void {
  const path = "$.application";
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  checkShape(
    object,
    path,
    ["id", "name", "coreAction"],
    ["id", "name", "coreAction"],
    issues,
  );
  stringValue(object.id, `${path}.id`, issues, {
    min: 3,
    max: 120,
    pattern: APPLICATION_ID,
  });
  stringValue(object.name, `${path}.name`, issues, { max: 80 });
  const coreAction = stringValue(object.coreAction, `${path}.coreAction`, issues, {
    min: 12,
    max: 240,
  });
  if (coreAction !== undefined && /[\r\n]/u.test(coreAction)) {
    addIssue(
      issues,
      `${path}.coreAction`,
      "core-action.one-line",
      "must be one sentence on one line",
    );
  }
}

function validateCompletionCondition(
  value: unknown,
  path: string,
  issues: ManifestValidationIssue[],
): void {
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  const kind = enumValue(
    object.kind,
    `${path}.kind`,
    ["active-duration", "count", "explicit"] as const,
    issues,
  );
  if (kind === "active-duration") {
    checkShape(object, path, ["kind", "thresholdMs"], ["kind", "thresholdMs"], issues);
    integerValue(object.thresholdMs, `${path}.thresholdMs`, issues, 1, 86_400_000);
  } else if (kind === "count") {
    checkShape(object, path, ["kind", "threshold", "unit"], ["kind", "threshold", "unit"], issues);
    integerValue(object.threshold, `${path}.threshold`, issues, 1, 1_000_000);
    stringValue(object.unit, `${path}.unit`, issues, {
      min: 3,
      max: 120,
      pattern: CONTRACT_ID,
    });
  } else if (kind === "explicit") {
    checkShape(object, path, ["kind", "predicate"], ["kind", "predicate"], issues);
    stringValue(object.predicate, `${path}.predicate`, issues, { max: 1000 });
  }
}

function validateInterruptionCondition(
  value: unknown,
  path: string,
  issues: ManifestValidationIssue[],
): void {
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  const kind = enumValue(
    object.kind,
    `${path}.kind`,
    ["inactivity", "lifecycle-exit", "explicit"] as const,
    issues,
  );
  if (kind === "inactivity") {
    checkShape(object, path, ["kind", "afterMs"], ["kind", "afterMs"], issues);
    integerValue(object.afterMs, `${path}.afterMs`, issues, 1, 86_400_000);
  } else if (kind === "lifecycle-exit") {
    checkShape(object, path, ["kind"], ["kind"], issues);
  } else if (kind === "explicit") {
    checkShape(object, path, ["kind", "predicate"], ["kind", "predicate"], issues);
    stringValue(object.predicate, `${path}.predicate`, issues, { max: 1000 });
  }
}

function validateAction(
  value: unknown,
  issues: ManifestValidationIssue[],
): void {
  const path = "$.runtime.action";
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  checkShape(
    object,
    path,
    ["input", "completion", "interruption", "checkpoint"],
    ["input", "completion", "interruption", "checkpoint"],
    issues,
  );

  const inputPath = `${path}.input`;
  const input = objectValue(object.input, inputPath, issues);
  if (input !== undefined) {
    checkShape(input, inputPath, ["kind", "mediaTypes"], ["kind", "mediaTypes", "constraints"], issues);
    stringValue(input.kind, `${inputPath}.kind`, issues, {
      min: 3,
      max: 120,
      pattern: CONTRACT_ID,
    });
    stringArrayValue(input.mediaTypes, `${inputPath}.mediaTypes`, issues, {
      pattern: MEDIA_TYPE,
    });
    if (hasOwn(input, "constraints")) {
      stringArrayValue(input.constraints, `${inputPath}.constraints`, issues);
    }
  }

  validateCompletionCondition(object.completion, `${path}.completion`, issues);

  const interruptionPath = `${path}.interruption`;
  const interruption = objectValue(object.interruption, interruptionPath, issues);
  if (interruption !== undefined) {
    checkShape(
      interruption,
      interruptionPath,
      ["condition", "partialAction", "resumption"],
      ["condition", "partialAction", "resumption"],
      issues,
    );
    validateInterruptionCondition(
      interruption.condition,
      `${interruptionPath}.condition`,
      issues,
    );
    enumValue(
      interruption.partialAction,
      `${interruptionPath}.partialAction`,
      ["preserve", "discard-with-explicit-consent"] as const,
      issues,
    );
    enumValue(
      interruption.resumption,
      `${interruptionPath}.resumption`,
      ["resume", "start-new"] as const,
      issues,
    );
  }

  if (object.checkpoint === "on-progress") {
    // Valid literal.
  } else {
    const checkpointPath = `${path}.checkpoint`;
    const checkpoint = objectValue(object.checkpoint, checkpointPath, issues);
    if (checkpoint !== undefined) {
      checkShape(checkpoint, checkpointPath, ["intervalMs"], ["intervalMs"], issues);
      integerValue(checkpoint.intervalMs, `${checkpointPath}.intervalMs`, issues, 100, 3_600_000);
    }
  }
}

interface RuntimePropertiesResult {
  offlineCoreAction?: string | undefined;
}

function validateRuntimeProperties(
  value: unknown,
  issues: ManifestValidationIssue[],
): RuntimePropertiesResult {
  const path = "$.runtime.properties";
  const required = [
    "offlineCoreAction",
    "permissionRequestPolicy",
    "noAccountBeforeValue",
    "localFirstRecord",
    "crashSafePersistence",
    "stableEventIdentity",
  ] as const;
  const object = objectValue(value, path, issues);
  if (object === undefined) return {};
  checkShape(object, path, required, [...required, "offlineSurface"], issues);
  const offlineCoreAction = enumValue(
    object.offlineCoreAction,
    `${path}.offlineCoreAction`,
    ["full", "degraded", "network-required"] as const,
    issues,
  );
  if (offlineCoreAction === "full") {
    if (hasOwn(object, "offlineSurface")) {
      addIssue(
        issues,
        `${path}.offlineSurface`,
        "offline.unused-surface",
        "must be omitted when offlineCoreAction is full",
      );
    }
  } else if (offlineCoreAction !== undefined) {
    stringValue(object.offlineSurface, `${path}.offlineSurface`, issues, { max: 1000 });
  }
  if (object.permissionRequestPolicy !== "first-core-action") {
    addIssue(
      issues,
      `${path}.permissionRequestPolicy`,
      "permission-request-policy",
      "must be first-core-action: request OS permissions at the first core action that needs them, never at launch",
    );
  }
  for (const key of [
    "noAccountBeforeValue",
    "localFirstRecord",
    "crashSafePersistence",
    "stableEventIdentity",
  ] as const) {
    if (object[key] !== true) {
      addIssue(
        issues,
        pathFor(path, key),
        "runtime-invariant",
        `must be true in manifest version ${CONTINUITY_MANIFEST_SCHEMA_VERSION}`,
      );
    }
  }
  return { offlineCoreAction };
}

function validateFeedback(
  value: unknown,
  issues: ManifestValidationIssue[],
): void {
  const path = "$.runtime.feedback";
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  checkShape(
    object,
    path,
    ["onCompletion", "onInterruption"],
    ["onProgress", "onCompletion", "onInterruption"],
    issues,
  );
  if (hasOwn(object, "onProgress")) {
    stringValue(object.onProgress, `${path}.onProgress`, issues, { max: 1000 });
  }
  stringValue(object.onCompletion, `${path}.onCompletion`, issues, { max: 1000 });
  stringValue(object.onInterruption, `${path}.onInterruption`, issues, { max: 1000 });
}

function validateContinuity(
  value: unknown,
  issues: ManifestValidationIssue[],
): void {
  const path = "$.runtime.continuity";
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  checkShape(object, path, ["accumulated", "artifacts"], ["accumulated", "artifacts"], issues);

  const accumulatedPath = `${path}.accumulated`;
  const accumulated = objectValue(object.accumulated, accumulatedPath, issues);
  if (accumulated !== undefined) {
    checkShape(accumulated, accumulatedPath, ["description", "projections"], ["description", "projections"], issues);
    stringValue(accumulated.description, `${accumulatedPath}.description`, issues, { max: 1000 });
    stringArrayValue(accumulated.projections, `${accumulatedPath}.projections`, issues);
  }

  const artifactsPath = `${path}.artifacts`;
  if (!Array.isArray(object.artifacts)) {
    addIssue(issues, artifactsPath, "type.array", "must be an array");
    return;
  }
  if (object.artifacts.length < 1 || object.artifacts.length > 8) {
    addIssue(issues, artifactsPath, "array.range", "must contain between 1 and 8 artifacts");
  }
  const ids: string[] = [];
  let ownerExportCount = 0;
  object.artifacts.forEach((entry, index) => {
    const artifactPath = pathFor(artifactsPath, index);
    const artifact = objectValue(entry, artifactPath, issues);
    if (artifact === undefined) return;
    checkShape(
      artifact,
      artifactPath,
      ["id", "mediaType", "codec", "export"],
      ["id", "mediaType", "codec", "export"],
      issues,
    );
    const id = stringValue(artifact.id, `${artifactPath}.id`, issues, {
      min: 3,
      max: 120,
      pattern: CONTRACT_ID,
    });
    if (id !== undefined) ids.push(id);
    stringValue(artifact.mediaType, `${artifactPath}.mediaType`, issues, {
      pattern: MEDIA_TYPE,
    });
    stringValue(artifact.codec, `${artifactPath}.codec`, issues, {
      min: 3,
      max: 120,
      pattern: CONTRACT_ID,
    });
    const exportMode = enumValue(
      artifact.export,
      `${artifactPath}.export`,
      ["owner-selected", "private-by-default", "none"] as const,
      issues,
    );
    if (exportMode === "owner-selected") ownerExportCount += 1;
  });
  if (new Set(ids).size !== ids.length) {
    addIssue(issues, artifactsPath, "artifact.id.unique", "artifact IDs must be unique");
  }
  if (ownerExportCount === 0) {
    addIssue(
      issues,
      artifactsPath,
      "artifact.owner-export-required",
      "at least one canonical artifact must support owner-selected export for ejection",
    );
  }
}

function validateReflection(
  value: unknown,
  issues: ManifestValidationIssue[],
): void {
  const path = "$.runtime.reflection";
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  const keys = [
    "mode",
    "trigger",
    "eventEligibility",
    "consent",
    "inputDisclosure",
    "policyId",
    "fallback",
  ] as const;
  checkShape(object, path, keys, keys, issues);
  const mode = enumValue(
    object.mode,
    `${path}.mode`,
    ["local-deterministic", "local-ai", "remote-service"] as const,
    issues,
  );
  const trigger = enumValue(
    object.trigger,
    `${path}.trigger`,
    ["after-event-opt-in", "after-event-automatic"] as const,
    issues,
  );
  const consent = enumValue(
    object.consent,
    `${path}.consent`,
    ["per-event", "standing-explicit"] as const,
    issues,
  );
  enumValue(
    object.eventEligibility,
    `${path}.eventEligibility`,
    ["completed-only", "completed-or-interrupted"] as const,
    issues,
  );
  const inputDisclosure = enumValue(
    object.inputDisclosure,
    `${path}.inputDisclosure`,
    ["none", "derived-features", "private-artifact"] as const,
    issues,
  );
  stringValue(object.policyId, `${path}.policyId`, issues, {
    min: 3,
    max: 120,
    pattern: CONTRACT_ID,
  });
  if (object.fallback !== "continue-without-reflection") {
    addIssue(
      issues,
      `${path}.fallback`,
      "reflection.fallback",
      "must be continue-without-reflection so continuity survives without AI or a provider",
    );
  }
  if (trigger === "after-event-automatic" && consent === "per-event") {
    addIssue(
      issues,
      `${path}.consent`,
      "reflection.consent-conflict",
      "automatic reflection cannot claim per-event consent",
    );
  }
  if (mode === "remote-service" && inputDisclosure === "none") {
    addIssue(
      issues,
      `${path}.inputDisclosure`,
      "reflection.remote-input",
      "a remote reflection must explicitly declare derived-features or private-artifact disclosure",
    );
  }
  if (trigger === "after-event-automatic") {
    addIssue(
      issues,
      `${path}.trigger`,
      "reflection.automatic",
      "automatic reflection needs explicit product and consent review",
      "warning",
    );
  }
}

interface PrivacyResult {
  localStorage?: string | undefined;
  externalDisclosure?: string[] | undefined;
}

function validatePrivacy(
  value: unknown,
  issues: ManifestValidationIssue[],
): PrivacyResult {
  const path = "$.runtime.privacy";
  const object = objectValue(value, path, issues);
  if (object === undefined) return {};
  const keys = [
    "localStorage",
    "publicByDefault",
    "externalDisclosure",
    "telemetry",
  ] as const;
  checkShape(object, path, keys, keys, issues);
  const localStorage = enumValue(
    object.localStorage,
    `${path}.localStorage`,
    ["platform-private", "application-encrypted"] as const,
    issues,
  );
  if (typeof object.publicByDefault !== "boolean") {
    addIssue(
      issues,
      `${path}.publicByDefault`,
      "type.boolean",
      "must be a boolean",
    );
  }
  const externalDisclosure = stringArrayValue(
    object.externalDisclosure,
    `${path}.externalDisclosure`,
    issues,
    { allowEmpty: true },
  );
  const telemetry = enumValue(
    object.telemetry,
    `${path}.telemetry`,
    ["none", "operational-metadata-only", "minimal-pseudonymous"] as const,
    issues,
  );
  if (localStorage === "platform-private") {
    addIssue(
      issues,
      `${path}.localStorage`,
      "privacy.platform-private",
      "platform-private is not application-level encryption; document the threat model",
      "warning",
    );
  }
  if (telemetry === "minimal-pseudonymous") {
    addIssue(
      issues,
      `${path}.telemetry`,
      "privacy.telemetry",
      "pseudonymous telemetry needs a field-level data inventory",
      "warning",
    );
  }
  return { localStorage, externalDisclosure };
}

interface IdentityResult {
  mode?: string | undefined;
}

function validateIdentity(
  value: unknown,
  issues: ManifestValidationIssue[],
): IdentityResult {
  const path = "$.runtime.identity";
  const object = objectValue(value, path, issues);
  if (object === undefined) return {};
  checkShape(
    object,
    path,
    ["mode", "creation", "crossAppLinking"],
    ["mode", "creation", "wordlist", "suite", "crossAppLinking"],
    issues,
  );
  const mode = enumValue(
    object.mode,
    `${path}.mode`,
    ["none", "seed-phrase", "local-contextual"] as const,
    issues,
  );
  enumValue(
    object.creation,
    `${path}.creation`,
    ["first-action", "first-committed-event", "first-launch"] as const,
    issues,
  );
  enumValue(
    object.crossAppLinking,
    `${path}.crossAppLinking`,
    ["never", "explicit-consent-only"] as const,
    issues,
  );
  if (mode === "seed-phrase") {
    if (object.wordlist !== "bip39-english") {
      addIssue(
        issues,
        `${path}.wordlist`,
        "identity.wordlist",
        "must be bip39-english for seed-phrase identity",
      );
    }
    if (hasOwn(object, "suite")) {
      addIssue(
        issues,
        `${path}.suite`,
        "identity.unused-suite",
        "must be omitted when identity mode is seed-phrase",
      );
    }
  } else if (mode === "local-contextual") {
    stringValue(object.suite, `${path}.suite`, issues, {
      min: 3,
      max: 120,
      pattern: CONTRACT_ID,
    });
    if (hasOwn(object, "wordlist")) {
      addIssue(
        issues,
        `${path}.wordlist`,
        "identity.unused-wordlist",
        "must be omitted when identity mode is local-contextual",
      );
    }
  } else if (mode === "none") {
    if (hasOwn(object, "suite")) {
      addIssue(
        issues,
        `${path}.suite`,
        "identity.unused-suite",
        "must be omitted when identity mode is none",
      );
    }
    if (hasOwn(object, "wordlist")) {
      addIssue(
        issues,
        `${path}.wordlist`,
        "identity.unused-wordlist",
        "must be omitted when identity mode is none",
      );
    }
  }
  return { mode };
}

interface RecoveryResult {
  content?: string | undefined;
  identity?: string | undefined;
}

function validateRecovery(
  value: unknown,
  issues: ManifestValidationIssue[],
): RecoveryResult {
  const path = "$.runtime.recovery";
  const object = objectValue(value, path, issues);
  if (object === undefined) return {};
  checkShape(object, path, ["offer", "identity", "content"], ["offer", "identity", "content"], issues);
  enumValue(
    object.offer,
    `${path}.offer`,
    ["after-first-value", "settings-only", "never"] as const,
    issues,
  );
  const identity = enumValue(
    object.identity,
    `${path}.identity`,
    ["none", "manual", "automatic-encrypted-backup", "opt-in-encrypted-backup"] as const,
    issues,
  );
  const content = enumValue(
    object.content,
    `${path}.content`,
    ["none", "manual-export", "opt-in-encrypted-backup"] as const,
    issues,
  );
  return { content, identity };
}

function validateSynchronization(
  value: unknown,
  issues: ManifestValidationIssue[],
): void {
  const path = "$.runtime.synchronization";
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  checkShape(object, path, ["mode", "conflictPolicy"], ["mode", "conflictPolicy"], issues);
  if (object.mode !== "opt-in-encrypted") {
    addIssue(issues, `${path}.mode`, "synchronization.mode", "must be opt-in-encrypted");
  }
  stringValue(object.conflictPolicy, `${path}.conflictPolicy`, issues, { max: 1000 });
}

interface ModulesResult {
  enabledNetworkExtensions: string[];
}

function validateModules(
  value: unknown,
  issues: ManifestValidationIssue[],
): ModulesResult {
  const enabledNetworkExtensions: string[] = [];
  const path = "$.runtime.modules";
  const object = objectValue(value, path, issues);
  if (object === undefined) return { enabledNetworkExtensions };
  const keys = ["paywall", "shareCard", "notifications", "sessionLink", "tokenMint"] as const;
  checkShape(object, path, keys, [...keys, "extensions"], issues);

  const paywallPath = `${path}.paywall`;
  const paywall = objectValue(object.paywall, paywallPath, issues);
  if (paywall !== undefined) {
    checkShape(paywall, paywallPath, ["enabled", "provider"], ["enabled", "provider", "publicKeySlot"], issues);
    if (typeof paywall.enabled !== "boolean") {
      addIssue(issues, `${paywallPath}.enabled`, "type.boolean", "must be a boolean");
    }
    if (paywall.provider !== "revenuecat") {
      addIssue(issues, `${paywallPath}.provider`, "modules.paywall.provider", "must be revenuecat");
    }
    if (hasOwn(paywall, "publicKeySlot")) {
      stringValue(paywall.publicKeySlot, `${paywallPath}.publicKeySlot`, issues, {
        min: 3,
        max: 120,
        pattern: CONTRACT_ID,
      });
    }
  }

  for (const moduleKey of ["shareCard", "notifications"] as const) {
    const modulePath = pathFor(path, moduleKey);
    const module = objectValue(object[moduleKey], modulePath, issues);
    if (module === undefined) continue;
    checkShape(module, modulePath, ["enabled"], ["enabled"], issues);
    if (typeof module.enabled !== "boolean") {
      addIssue(issues, `${modulePath}.enabled`, "type.boolean", "must be a boolean");
    }
  }

  for (const [moduleKey, code] of [
    ["sessionLink", "session-link"],
    ["tokenMint", "token-mint"],
  ] as const) {
    const reservedPath = pathFor(path, moduleKey);
    const reserved = objectValue(object[moduleKey], reservedPath, issues);
    if (reserved === undefined) continue;
    checkShape(reserved, reservedPath, ["enabled", "status"], ["enabled", "status"], issues);
    if (reserved.enabled !== false) {
      addIssue(
        issues,
        `${reservedPath}.enabled`,
        `modules.${code}.reserved`,
        `must be false; ${moduleKey} is a reserved primitive and enabling it is unsupported in ${CONTINUITY_MANIFEST_SCHEMA_VERSION}`,
      );
    }
    if (reserved.status !== "reserved") {
      addIssue(issues, `${reservedPath}.status`, `modules.${code}.status`, "must be reserved");
    }
  }

  if (hasOwn(object, "extensions")) {
    const extensionsPath = `${path}.extensions`;
    const extensions = objectValue(object.extensions, extensionsPath, issues);
    if (extensions !== undefined) {
      const names = Object.keys(extensions);
      if (names.length < 1 || names.length > 16) {
        addIssue(issues, extensionsPath, "object.range", "must declare between 1 and 16 extensions");
      }
      for (const name of names) {
        const extensionPath = pathFor(extensionsPath, name);
        if (name.length < 3 || name.length > 120 || !CONTRACT_ID.test(name)) {
          addIssue(issues, extensionPath, "extension.name", "extension names are contract ids");
        }
        const extension = objectValue(extensions[name], extensionPath, issues);
        if (extension === undefined) continue;
        checkShape(
          extension,
          extensionPath,
          ["enabled"],
          ["enabled", "keySlot", "requiresNetwork", "notes"],
          issues,
        );
        if (typeof extension.enabled !== "boolean") {
          addIssue(issues, `${extensionPath}.enabled`, "type.boolean", "must be a boolean");
        }
        if (hasOwn(extension, "keySlot")) {
          stringValue(extension.keySlot, `${extensionPath}.keySlot`, issues, {
            min: 3,
            max: 120,
            pattern: CONTRACT_ID,
          });
        }
        if (hasOwn(extension, "requiresNetwork") && typeof extension.requiresNetwork !== "boolean") {
          addIssue(issues, `${extensionPath}.requiresNetwork`, "type.boolean", "must be a boolean");
        }
        if (hasOwn(extension, "notes")) {
          stringValue(extension.notes, `${extensionPath}.notes`, issues, { max: 1000 });
        }
        if (extension.enabled === true && extension.requiresNetwork === true) {
          enabledNetworkExtensions.push(name);
        }
      }
    }
  }
  return { enabledNetworkExtensions };
}

function validateRuntime(
  value: unknown,
  issues: ManifestValidationIssue[],
): void {
  const path = "$.runtime";
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  const required = [
    "properties",
    "action",
    "feedback",
    "continuity",
    "privacy",
    "identity",
    "recovery",
    "modules",
  ] as const;
  const allowed = [
    ...required,
    "reflection",
    "synchronization",
  ] as const;
  checkShape(object, path, required, allowed, issues);
  const properties = validateRuntimeProperties(object.properties, issues);
  validateAction(object.action, issues);
  validateFeedback(object.feedback, issues);
  validateContinuity(object.continuity, issues);
  if (hasOwn(object, "reflection")) validateReflection(object.reflection, issues);
  const privacy = validatePrivacy(object.privacy, issues);
  const identity = validateIdentity(object.identity, issues);
  const recovery = validateRecovery(object.recovery, issues);
  const modules = validateModules(object.modules, issues);
  if (hasOwn(object, "synchronization")) {
    validateSynchronization(object.synchronization, issues);
    if (privacy.localStorage !== "application-encrypted") {
      addIssue(
        issues,
        "$.runtime.privacy.localStorage",
        "synchronization.encryption",
        "encrypted synchronization requires application-encrypted local storage",
      );
    }
    if (recovery.content !== "opt-in-encrypted-backup") {
      addIssue(
        issues,
        "$.runtime.recovery.content",
        "synchronization.recovery",
        "encrypted synchronization requires an explicit encrypted content recovery plan",
      );
    }
  }

  if (
    properties.offlineCoreAction !== undefined &&
    properties.offlineCoreAction !== "full" &&
    (privacy.externalDisclosure?.length ?? 0) === 0
  ) {
    addIssue(
      issues,
      "$.runtime.privacy.externalDisclosure",
      "offline.network-disclosure",
      "a network-dependent core action requires an explicit external disclosure inventory",
    );
  }

  if (
    modules.enabledNetworkExtensions.length > 0 &&
    (privacy.externalDisclosure?.length ?? 0) === 0
  ) {
    addIssue(
      issues,
      "$.runtime.privacy.externalDisclosure",
      "modules.extension-disclosure",
      `enabled network extensions (${modules.enabledNetworkExtensions.join(", ")}) require an explicit external disclosure inventory`,
    );
  }

  const reflection = isRecord(object.reflection) ? object.reflection : undefined;
  if (
    reflection?.mode === "remote-service" &&
    (privacy.externalDisclosure?.length ?? 0) === 0
  ) {
    addIssue(
      issues,
      "$.runtime.privacy.externalDisclosure",
      "privacy.reflection-disclosure",
      "remote reflection requires an explicit external disclosure inventory",
    );
  }
  if (identity.mode === "none" && recovery.identity !== "none") {
    addIssue(
      issues,
      "$.runtime.recovery.identity",
      "recovery.identity-without-identity",
      "must be none when practice identity mode is none",
    );
  }
}

function validateGuidance(
  value: unknown,
  issues: ManifestValidationIssue[],
): void {
  const path = "$.guidance";
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  checkShape(
    object,
    path,
    ["visualDirection", "tonalDirection"],
    ["visualDirection", "tonalDirection", "implementationNotes"],
    issues,
  );
  stringArrayValue(object.visualDirection, `${path}.visualDirection`, issues);
  stringArrayValue(object.tonalDirection, `${path}.tonalDirection`, issues);
  if (hasOwn(object, "implementationNotes")) {
    stringArrayValue(object.implementationNotes, `${path}.implementationNotes`, issues);
  }
}

interface OperationsResult {
  requiresServer?: boolean | "credential-minting-only" | undefined;
  deploymentTargets?: string[] | undefined;
}

function validateOperations(
  value: unknown,
  issues: ManifestValidationIssue[],
): OperationsResult {
  const path = "$.operations";
  const object = objectValue(value, path, issues);
  if (object === undefined) return {};
  checkShape(
    object,
    path,
    [
      "deploymentTargets",
      "requiresServer",
      "ejectionRequired",
      "approvalRequiredFor",
    ],
    [
      "deploymentTargets",
      "requiresServer",
      "serverRole",
      "developmentSecrets",
      "meteredDependencies",
      "ejectionRequired",
      "approvalRequiredFor",
      "operatorNotes",
    ],
    issues,
  );
  const deploymentTargets = stringArrayValue(
    object.deploymentTargets,
    `${path}.deploymentTargets`,
    issues,
  );
  for (const target of deploymentTargets ?? []) {
    if (!["native-ios", "native-android", "web", "server"].includes(target)) {
      addIssue(
        issues,
        `${path}.deploymentTargets`,
        "operations.target",
        `${target} is not a supported deployment target`,
      );
    }
  }
  let requiresServer: boolean | "credential-minting-only" | undefined;
  if (
    object.requiresServer !== false &&
    object.requiresServer !== "credential-minting-only" &&
    object.requiresServer !== true
  ) {
    addIssue(
      issues,
      `${path}.requiresServer`,
      "operations.requires-server",
      "must be false, credential-minting-only, or true",
    );
  } else {
    requiresServer = object.requiresServer;
  }
  if (hasOwn(object, "serverRole")) {
    addIssue(
      issues,
      `${path}.serverRole`,
      "operations.server-role-removed",
      "was removed in 0.4.0; set requiresServer to credential-minting-only for a token mint, true for any broader server, or false for none",
    );
  }
  if (hasOwn(object, "developmentSecrets")) {
    const secretsPath = `${path}.developmentSecrets`;
    if (!Array.isArray(object.developmentSecrets)) {
      addIssue(issues, secretsPath, "type.array", "must be an array");
    } else {
      if (object.developmentSecrets.length !== 1) {
        addIssue(issues, secretsPath, "array.range", "must contain exactly the canonical dev-secret entry");
      }
      object.developmentSecrets.forEach((entry, index) => {
        const entryPath = pathFor(secretsPath, index);
        const secret = objectValue(entry, entryPath, issues);
        if (secret === undefined) return;
        checkShape(secret, entryPath, ["slot", "purpose"], ["slot", "purpose"], issues);
        if (secret.slot !== "dev-secret") {
          addIssue(
            issues,
            `${entryPath}.slot`,
            "development-secrets.slot",
            "must be dev-secret, the manifest id mapped to DEV_SECRET in gitignored Local.xcconfig",
          );
        }
        stringValue(secret.purpose, `${entryPath}.purpose`, issues, { max: 1000 });
      });
      addIssue(
        issues,
        secretsPath,
        "operations.development-secrets",
        "development secrets are prototype-only; replace them with a token-minting server before store submission",
        "warning",
      );
    }
  }
  if (hasOwn(object, "meteredDependencies")) {
    const dependenciesPath = `${path}.meteredDependencies`;
    if (!Array.isArray(object.meteredDependencies)) {
      addIssue(issues, dependenciesPath, "type.array", "must be an array");
    } else {
      if (object.meteredDependencies.length < 1 || object.meteredDependencies.length > 16) {
        addIssue(issues, dependenciesPath, "array.range", "must contain between 1 and 16 entries");
      }
      const meters: string[] = [];
      object.meteredDependencies.forEach((entry, index) => {
        const entryPath = pathFor(dependenciesPath, index);
        const dependency = objectValue(entry, entryPath, issues);
        if (dependency === undefined) return;
        checkShape(dependency, entryPath, ["provider", "unit"], ["provider", "unit"], issues);
        const provider = stringValue(dependency.provider, `${entryPath}.provider`, issues, {
          min: 3,
          max: 120,
          pattern: CONTRACT_ID,
        });
        const unit = stringValue(dependency.unit, `${entryPath}.unit`, issues, { max: 1000 });
        if (provider !== undefined && unit !== undefined) meters.push(`${provider}\u0000${unit}`);
      });
      if (new Set(meters).size !== meters.length) {
        addIssue(
          issues,
          dependenciesPath,
          "metered-dependencies.unique",
          "provider and unit pairs must be unique",
        );
      }
      addIssue(
        issues,
        dependenciesPath,
        "operations.metered-dependencies",
        "app usage consumes owner-funded third-party credits; review the declared meters before distribution",
        "warning",
      );
    }
  }
  if (object.ejectionRequired !== true) {
    addIssue(
      issues,
      `${path}.ejectionRequired`,
      "operations.ejection",
      "must be true; every generated app is ejectable from birth",
    );
  }
  const approvals = stringArrayValue(
    object.approvalRequiredFor,
    `${path}.approvalRequiredFor`,
    issues,
    { min: 4, max: 4 },
  );
  const approvalSet = new Set(approvals ?? []);
  for (const approval of REQUIRED_APPROVALS) {
    if (!approvalSet.has(approval)) {
      addIssue(
        issues,
        `${path}.approvalRequiredFor`,
        "operations.approval-boundary",
        `must include ${approval}`,
      );
    }
  }
  for (const approval of approvals ?? []) {
    if (!(REQUIRED_APPROVALS as readonly string[]).includes(approval)) {
      addIssue(
        issues,
        `${path}.approvalRequiredFor`,
        "operations.approval-value",
        `${approval} is not a version ${CONTINUITY_MANIFEST_SCHEMA_VERSION} approval boundary`,
      );
    }
  }
  if (hasOwn(object, "operatorNotes")) {
    stringArrayValue(object.operatorNotes, `${path}.operatorNotes`, issues);
  }
  return { requiresServer, deploymentTargets };
}

const TOKEN_ADDRESS = /^0x[a-fA-F0-9]{40}$/;
const TOKEN_TX_HASH = /^0x[a-fA-F0-9]{64}$/;

function validateToken(
  value: unknown,
  issues: ManifestValidationIssue[],
): void {
  const path = "$.token";
  const object = objectValue(value, path, issues);
  if (object === undefined) return;
  checkShape(
    object,
    path,
    ["provider", "chain", "name", "symbol", "launchedAt"],
    ["provider", "chain", "name", "symbol", "feeRecipient", "address", "txHash", "launchedAt"],
    issues,
  );
  enumValue(object.provider, `${path}.provider`, ["bankr"], issues);
  enumValue(object.chain, `${path}.chain`, ["base", "robinhood"], issues);
  stringValue(object.name, `${path}.name`, issues, { max: 1000 });
  stringValue(object.symbol, `${path}.symbol`, issues, { max: 10 });
  if (hasOwn(object, "feeRecipient")) {
    stringValue(object.feeRecipient, `${path}.feeRecipient`, issues, { max: 1000 });
  }
  if (hasOwn(object, "address")) {
    stringValue(object.address, `${path}.address`, issues, { pattern: TOKEN_ADDRESS });
  }
  if (hasOwn(object, "txHash")) {
    stringValue(object.txHash, `${path}.txHash`, issues, { pattern: TOKEN_TX_HASH });
  }
  const launchedAt = stringValue(object.launchedAt, `${path}.launchedAt`, issues);
  if (launchedAt !== undefined && Number.isNaN(Date.parse(launchedAt))) {
    addIssue(issues, `${path}.launchedAt`, "string.date-time", "must be an ISO-8601 date-time");
  }
}

export function validateManifest(input: unknown): ManifestValidationResult {
  const issues: ManifestValidationIssue[] = [];
  const root = objectValue(input, "$", issues);
  if (root !== undefined) {
    checkShape(
      root,
      "$",
      ["schemaVersion", "application", "runtime", "guidance", "operations"],
      ["schemaVersion", "application", "runtime", "guidance", "operations", "token"],
      issues,
    );
    if (root.schemaVersion !== CONTINUITY_MANIFEST_SCHEMA_VERSION) {
      const migration = root.schemaVersion === "0.3.0"
        ? "; migrate 0.3.0 by adding runtime.properties.permissionRequestPolicy = first-core-action, changing degraded-readonly to degraded, moving token-mint-only into operations.requiresServer as credential-minting-only, removing operations.serverRole, and mapping any single development secret to slot dev-secret"
        : root.schemaVersion === "0.2.0"
          ? "; migrate the boolean offlineCoreAction to full, degraded, or network-required, add the required 0.4.0 module and permission declarations, and use the bounded operations.requiresServer value"
          : "";
      addIssue(
        issues,
        "$.schemaVersion",
        "schema-version",
        `must equal ${CONTINUITY_MANIFEST_SCHEMA_VERSION}${migration}`,
      );
    }
    validateApplication(root.application, issues);
    validateRuntime(root.runtime, issues);
    validateGuidance(root.guidance, issues);
    const operations = validateOperations(root.operations, issues);
    if (hasOwn(root, "token")) {
      validateToken(root.token, issues);
    }

    const runtime = isRecord(root.runtime) ? root.runtime : undefined;
    const reflection = runtime !== undefined && isRecord(runtime.reflection)
      ? runtime.reflection
      : undefined;
    const synchronizationRequired = runtime !== undefined &&
      hasOwn(runtime, "synchronization");
    if (
      reflection?.mode === "remote-service" &&
      operations.requiresServer === false
    ) {
      addIssue(
        issues,
        "$.operations.requiresServer",
        "operations.server-required",
        "must be credential-minting-only or true when reflection uses a remote service",
      );
    }
    if (
      synchronizationRequired &&
      operations.requiresServer !== true
    ) {
      addIssue(
        issues,
        "$.operations.requiresServer",
        "operations.sync-server-required",
        "must be true when encrypted synchronization is enabled; credential-minting-only cannot provide synchronization",
      );
    }
    if (
      operations.requiresServer !== undefined &&
      operations.requiresServer !== false &&
      !(operations.deploymentTargets ?? []).includes("server")
    ) {
      addIssue(
        issues,
        "$.operations.deploymentTargets",
        "operations.server-target",
        "must include server when requiresServer is credential-minting-only or true",
      );
    }
  }

  const errors = issues.filter((issue) => issue.severity === "error");
  const warnings = issues.filter((issue) => issue.severity === "warning");
  return {
    valid: errors.length === 0,
    issues,
    errors,
    warnings,
  };
}

export function formatManifestIssues(
  issues: readonly ManifestValidationIssue[],
): string {
  return issues
    .map(
      (issue) =>
        `${issue.severity.toUpperCase()} ${issue.path} [${issue.code}]: ${issue.message}`,
    )
    .join("\n");
}

export function assertValidManifest(
  input: unknown,
): asserts input is ContinuityManifest {
  const result = validateManifest(input);
  if (!result.valid) {
    throw new TypeError(`Invalid continuity manifest:\n${formatManifestIssues(result.errors)}`);
  }
}

export function parseManifest(json: string): ContinuityManifest {
  let value: unknown;
  try {
    value = JSON.parse(json) as unknown;
  } catch (error) {
    const detail = error instanceof Error ? error.message : "unknown JSON parse error";
    throw new TypeError(`Invalid continuity manifest JSON: ${detail}`);
  }
  assertValidManifest(value);
  return value;
}
