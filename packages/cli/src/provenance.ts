import { createHash } from "node:crypto";
import {
  lstatSync,
  mkdirSync,
  writeFileSync,
} from "node:fs";
import { basename, extname, join } from "node:path";
import { isAgentId, type AgentId } from "./agents.ts";
import {
  CLI_VERSION,
  IOS_TEMPLATE_VERSION,
  MANIFEST_SCHEMA_VERSION,
} from "./constants.ts";
import type { CreationDoor } from "./progress.ts";
import type { FactoryRelease } from "./release.ts";
import { CliError } from "./errors.ts";
import { readBoundedRegularFile } from "./files.ts";

export const CREATION_PROVENANCE_SCHEMA_VERSION = 1 as const;
export const MAX_INTENTION_BYTES = 1_048_576;
export const MAX_REFERENCE_BYTES = 12 * 1_048_576;
export const MAX_REFERENCES = 8;

export interface MarkdownInput {
  path: string;
  originalName: string;
}

export interface ReferenceInput {
  path: string;
  originalName: string;
  mediaType?: string;
}

export interface CreationInput {
  text?: string;
  markdown?: MarkdownInput;
  references?: readonly ReferenceInput[];
}

export interface NormalizedIntentionComponent {
  kind: "textarea" | "markdown";
  originalName?: string;
  sha256: string;
  bytes: number;
  byteOffset: number;
  byteLength: number;
}

export interface NormalizedReference {
  sourcePath: string;
  originalName: string;
  mediaType: string;
  extension: string;
  bytes: Uint8Array;
  sha256: string;
}

export interface NormalizedCreationInput {
  intention: string | null;
  intentionSha256: string | null;
  intentionBytes: number;
  components: NormalizedIntentionComponent[];
  references: NormalizedReference[];
  inputDigest: string;
}

export interface CreationProvenance {
  schemaVersion: typeof CREATION_PROVENANCE_SCHEMA_VERSION;
  createdAt: string;
  door: CreationDoor;
  factory: {
    releaseId: string;
    cliVersion: string;
    templateVersion: string;
    manifestSchemaVersion: string;
    bundleDigest: string;
  };
  intention: null | {
    path: "intention.md";
    sha256: string;
    bytes: number;
    components: NormalizedIntentionComponent[];
  };
  references: Array<{
    path: string;
    originalName: string;
    mediaType: string;
    bytes: number;
    sha256: string;
  }>;
  inputDigest: string;
  options: {
    selectedAgent: AgentId | null;
    agentMode: "interactive" | "automated" | "none";
    verifyAfterAgent: boolean;
    runAfterCreate: boolean;
  };
  events: "events.jsonl";
}

function provenanceRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function provenanceKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
): boolean {
  const actual = Object.keys(value).sort();
  const canonical = [...expected].sort();
  return actual.length === canonical.length &&
    actual.every((key, index) => key === canonical[index]);
}

function canonicalTimestamp(value: unknown): value is string {
  return typeof value === "string" &&
    Number.isFinite(Date.parse(value)) &&
    new Date(value).toISOString() === value;
}

function canonicalSha256(value: unknown): value is string {
  return typeof value === "string" && /^[a-f0-9]{64}$/u.test(value);
}

function invalidProvenance(): never {
  throw new CliError(
    "private creation provenance is not canonical; pre-release compatibility is unsupported; create a fresh Shot with `tohseno`",
    2,
  );
}

export function validateCreationProvenance(
  value: unknown,
): CreationProvenance {
  const root = provenanceRecord(value);
  if (
    root === null ||
    !provenanceKeys(root, [
      "schemaVersion",
      "createdAt",
      "door",
      "factory",
      "intention",
      "references",
      "inputDigest",
      "options",
      "events",
    ]) ||
    root.schemaVersion !== CREATION_PROVENANCE_SCHEMA_VERSION ||
    !canonicalTimestamp(root.createdAt) ||
    (root.door !== "cli" && root.door !== "studio") ||
    !canonicalSha256(root.inputDigest) ||
    root.events !== "events.jsonl" ||
    !Array.isArray(root.references)
  ) {
    return invalidProvenance();
  }

  const factory = provenanceRecord(root.factory);
  if (
    factory === null ||
    !provenanceKeys(factory, [
      "releaseId",
      "cliVersion",
      "templateVersion",
      "manifestSchemaVersion",
      "bundleDigest",
    ]) ||
    typeof factory.releaseId !== "string" ||
    !/^(?:git-[0-9a-f]{40}(?:-dirty)?-[0-9a-f]{16}|content-[0-9a-f]{32})$/u
      .test(factory.releaseId) ||
    factory.cliVersion !== CLI_VERSION ||
    factory.templateVersion !== IOS_TEMPLATE_VERSION ||
    factory.manifestSchemaVersion !== MANIFEST_SCHEMA_VERSION ||
    !canonicalSha256(factory.bundleDigest)
  ) {
    return invalidProvenance();
  }

  const options = provenanceRecord(root.options);
  if (
    options === null ||
    !provenanceKeys(options, [
      "selectedAgent",
      "agentMode",
      "verifyAfterAgent",
      "runAfterCreate",
    ]) ||
    (
      options.selectedAgent !== null &&
      (
        typeof options.selectedAgent !== "string" ||
        !isAgentId(options.selectedAgent)
      )
    ) ||
    (
      options.agentMode !== "interactive" &&
      options.agentMode !== "automated" &&
      options.agentMode !== "none"
    ) ||
    typeof options.verifyAfterAgent !== "boolean" ||
    typeof options.runAfterCreate !== "boolean"
  ) {
    return invalidProvenance();
  }

  let intentionSha256: string | null = null;
  if (root.intention !== null) {
    const intention = provenanceRecord(root.intention);
    if (
      intention === null ||
      !provenanceKeys(intention, [
        "path",
        "sha256",
        "bytes",
        "components",
      ]) ||
      intention.path !== "intention.md" ||
      !canonicalSha256(intention.sha256) ||
      !Number.isSafeInteger(intention.bytes) ||
      (intention.bytes as number) < 1 ||
      !Array.isArray(intention.components) ||
      intention.components.length < 1
    ) {
      return invalidProvenance();
    }
    for (const value of intention.components) {
      const component = provenanceRecord(value);
      const hasOriginalName = component !== null &&
        Object.hasOwn(component, "originalName");
      if (
        component === null ||
        !provenanceKeys(
          component,
          hasOriginalName
            ? [
              "kind",
              "originalName",
              "sha256",
              "bytes",
              "byteOffset",
              "byteLength",
            ]
            : ["kind", "sha256", "bytes", "byteOffset", "byteLength"],
        ) ||
        (component.kind !== "textarea" && component.kind !== "markdown") ||
        (
          hasOriginalName &&
          (
            component.kind !== "markdown" ||
            typeof component.originalName !== "string" ||
            component.originalName.length < 1 ||
            component.originalName.length > 255 ||
            component.originalName !==
              basename(component.originalName.replaceAll("\\", "/"))
                .normalize("NFC") ||
            extname(component.originalName).toLowerCase() !== ".md" ||
            /[\u0000-\u001f\u007f-\u009f]/u.test(component.originalName)
          )
        ) ||
        !canonicalSha256(component.sha256) ||
        !Number.isSafeInteger(component.bytes) ||
        !Number.isSafeInteger(component.byteOffset) ||
        !Number.isSafeInteger(component.byteLength) ||
        (component.bytes as number) < 1 ||
        component.bytes !== component.byteLength ||
        (component.byteOffset as number) < 0 ||
        (component.byteLength as number) < 1 ||
        (component.byteOffset as number) +
            (component.byteLength as number) >
          (intention.bytes as number)
      ) {
        return invalidProvenance();
      }
    }
    intentionSha256 = intention.sha256;
  }

  if (root.references.length > MAX_REFERENCES) {
    return invalidProvenance();
  }
  const referenceHashes: string[] = [];
  for (const value of root.references) {
    const reference = provenanceRecord(value);
    if (
      reference === null ||
      !provenanceKeys(reference, [
        "path",
        "originalName",
        "mediaType",
        "bytes",
        "sha256",
      ]) ||
      typeof reference.path !== "string" ||
      !/^references\/reference-[0-9]{3}\.(?:png|jpg|webp|gif|heic|avif)$/u
        .test(reference.path) ||
      typeof reference.originalName !== "string" ||
      reference.originalName.length < 1 ||
      reference.originalName.length > 255 ||
      reference.originalName !==
        basename(reference.originalName.replaceAll("\\", "/")).normalize("NFC") ||
      /[\u0000-\u001f\u007f-\u009f]/u.test(reference.originalName) ||
      typeof reference.mediaType !== "string" ||
      ![
        "image/png",
        "image/jpeg",
        "image/webp",
        "image/gif",
        "image/heic",
        "image/avif",
      ].includes(reference.mediaType) ||
      !Number.isSafeInteger(reference.bytes) ||
      (reference.bytes as number) < 1 ||
      !canonicalSha256(reference.sha256)
    ) {
      return invalidProvenance();
    }
    referenceHashes.push(reference.sha256);
  }
  if (
    root.inputDigest !== sha256(JSON.stringify({
      intentionSha256,
      references: referenceHashes,
    }))
  ) {
    return invalidProvenance();
  }
  return root as unknown as CreationProvenance;
}

function sha256(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

function normalizedText(value: string): string {
  const withoutBom = value.startsWith("\uFEFF") ? value.slice(1) : value;
  const normalizedLines = withoutBom.replace(/\r\n?/gu, "\n").trim();
  return normalizedLines === "" ? "" : `${normalizedLines}\n`;
}

function readRegularFile(path: string, maximumBytes: number, label: string): Uint8Array {
  let details;
  try {
    details = lstatSync(path);
  } catch {
    throw new CliError(`${label} does not exist or cannot be read`);
  }
  if (details.isSymbolicLink() || !details.isFile()) {
    throw new CliError(`${label} must be a regular file, not a symbolic link`);
  }
  if (details.size > maximumBytes) {
    throw new CliError(`${label} exceeds the ${Math.floor(maximumBytes / 1_048_576)} MiB limit`, 2);
  }
  return readBoundedRegularFile(path, maximumBytes, label);
}

function decodeMarkdown(input: MarkdownInput): string {
  const originalName = safeOriginalName(
    input.originalName,
    "intention Markdown filename",
  );
  if (extname(originalName).toLowerCase() !== ".md") {
    throw new CliError("intention file must have a .md extension", 2);
  }
  const bytes = readRegularFile(input.path, MAX_INTENTION_BYTES, "intention Markdown");
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new CliError("intention Markdown must be valid UTF-8", 2);
  }
}

function safeOriginalName(value: string, label: string): string {
  const name = basename(value.replaceAll("\\", "/")).normalize("NFC");
  if (
    name === "" ||
    name === "." ||
    name === ".." ||
    Buffer.byteLength(name) > 255 ||
    /[\u0000-\u001f\u007f-\u009f]/u.test(name)
  ) {
    throw new CliError(`${label} is invalid`, 2);
  }
  return name;
}

interface ImageType {
  mediaType: string;
  extension: string;
}

export function detectImageType(bytes: Uint8Array): ImageType | null {
  if (
    bytes.length >= 8 &&
    bytes[0] === 0x89 &&
    bytes[1] === 0x50 &&
    bytes[2] === 0x4e &&
    bytes[3] === 0x47 &&
    bytes[4] === 0x0d &&
    bytes[5] === 0x0a &&
    bytes[6] === 0x1a &&
    bytes[7] === 0x0a
  ) {
    return { mediaType: "image/png", extension: ".png" };
  }
  if (
    bytes.length >= 3 &&
    bytes[0] === 0xff &&
    bytes[1] === 0xd8 &&
    bytes[2] === 0xff
  ) {
    return { mediaType: "image/jpeg", extension: ".jpg" };
  }
  const ascii = (start: number, end: number): string =>
    new TextDecoder("ascii").decode(bytes.slice(start, end));
  if (
    bytes.length >= 12 &&
    ascii(0, 4) === "RIFF" &&
    ascii(8, 12) === "WEBP"
  ) {
    return { mediaType: "image/webp", extension: ".webp" };
  }
  if (bytes.length >= 6 && (ascii(0, 6) === "GIF87a" || ascii(0, 6) === "GIF89a")) {
    return { mediaType: "image/gif", extension: ".gif" };
  }
  if (bytes.length >= 12 && ascii(4, 8) === "ftyp") {
    const brand = ascii(8, 12).toLowerCase();
    if (["heic", "heix", "hevc", "hevx", "mif1", "msf1"].includes(brand)) {
      return { mediaType: "image/heic", extension: ".heic" };
    }
    if (brand === "avif" || brand === "avis") {
      return { mediaType: "image/avif", extension: ".avif" };
    }
  }
  return null;
}

function normalizeReferences(
  inputs: readonly ReferenceInput[],
): NormalizedReference[] {
  if (inputs.length > MAX_REFERENCES) {
    throw new CliError(`at most ${MAX_REFERENCES} reference images may be attached`, 2);
  }
  return inputs.map((input, index) => {
    const originalName = safeOriginalName(
      input.originalName,
      `reference image ${index + 1} filename`,
    );
    const bytes = readRegularFile(
      input.path,
      MAX_REFERENCE_BYTES,
      `reference image ${index + 1}`,
    );
    const detected = detectImageType(bytes);
    if (detected === null) {
      throw new CliError(
        `reference image ${index + 1} must be PNG, JPEG, WebP, GIF, HEIC, or AVIF`,
        2,
      );
    }
    if (
      input.mediaType !== undefined &&
      input.mediaType !== "" &&
      input.mediaType !== "application/octet-stream" &&
      input.mediaType.toLowerCase() !== detected.mediaType
    ) {
      throw new CliError(
        `reference image ${index + 1} content does not match its declared media type`,
        2,
      );
    }
    return {
      sourcePath: input.path,
      originalName,
      mediaType: detected.mediaType,
      extension: detected.extension,
      bytes,
      sha256: sha256(bytes),
    };
  });
}

function addComponent(
  target: { value: string },
  components: NormalizedIntentionComponent[],
  kind: NormalizedIntentionComponent["kind"],
  content: string,
  originalName?: string,
): void {
  const offset = Buffer.byteLength(target.value);
  target.value += content;
  const length = Buffer.byteLength(content);
  components.push({
    kind,
    ...(originalName === undefined
      ? {}
      : {
        originalName: safeOriginalName(
          originalName,
          "intention Markdown filename",
        ),
      }),
    sha256: sha256(content),
    bytes: length,
    byteOffset: offset,
    byteLength: length,
  });
}

export function normalizeCreationInput(input: CreationInput = {}): NormalizedCreationInput {
  const typed = normalizedText(input.text ?? "");
  const markdown = input.markdown === undefined
    ? ""
    : normalizedText(decodeMarkdown(input.markdown));
  const components: NormalizedIntentionComponent[] = [];
  const combined = { value: "" };
  if (typed !== "" && markdown !== "") {
    combined.value += "# Typed intention\n\n";
    addComponent(combined, components, "textarea", typed);
    combined.value += "\n# Attached Markdown\n\n";
    addComponent(
      combined,
      components,
      "markdown",
      markdown,
      input.markdown?.originalName,
    );
  } else if (typed !== "") {
    addComponent(combined, components, "textarea", typed);
  } else if (markdown !== "") {
    addComponent(
      combined,
      components,
      "markdown",
      markdown,
      input.markdown?.originalName,
    );
  }
  if (Buffer.byteLength(combined.value) > MAX_INTENTION_BYTES) {
    throw new CliError("normalized intention exceeds the 1 MiB limit", 2);
  }
  const references = normalizeReferences(input.references ?? []);
  const intentionSha256 = combined.value === "" ? null : sha256(combined.value);
  const digestInput = JSON.stringify({
    intentionSha256,
    references: references.map((reference) => reference.sha256),
  });
  return {
    intention: combined.value === "" ? null : combined.value,
    intentionSha256,
    intentionBytes: Buffer.byteLength(combined.value),
    components,
    references,
    inputDigest: sha256(digestInput),
  };
}

export function writeCreationProvenance(options: {
  shotRoot: string;
  createdAt: Date;
  door: CreationDoor;
  release: FactoryRelease;
  input: NormalizedCreationInput;
  selectedAgent: AgentId | null;
  agentMode: CreationProvenance["options"]["agentMode"];
  verifyAfterAgent: boolean;
  runAfterCreate: boolean;
}): CreationProvenance {
  const root = join(options.shotRoot, ".tohseno", "provenance");
  const referencesDirectory = join(root, "references");
  mkdirSync(referencesDirectory, { recursive: true, mode: 0o700 });
  if (options.input.intention !== null) {
    writeFileSync(join(root, "intention.md"), options.input.intention, {
      mode: 0o600,
    });
  }
  const referenceRecords = options.input.references.map((reference, index) => {
    const internalName = `reference-${String(index + 1).padStart(3, "0")}${reference.extension}`;
    writeFileSync(join(referencesDirectory, internalName), reference.bytes, {
      mode: 0o600,
    });
    return {
      path: `references/${internalName}`,
      originalName: reference.originalName,
      mediaType: reference.mediaType,
      bytes: reference.bytes.byteLength,
      sha256: reference.sha256,
    };
  });
  const provenance: CreationProvenance = {
    schemaVersion: CREATION_PROVENANCE_SCHEMA_VERSION,
    createdAt: options.createdAt.toISOString(),
    door: options.door,
    factory: {
      releaseId: options.release.releaseId,
      cliVersion: options.release.cliVersion,
      templateVersion: options.release.templateVersion,
      manifestSchemaVersion: options.release.manifestSchemaVersion,
      bundleDigest: options.release.bundleDigest,
    },
    intention: options.input.intentionSha256 === null
      ? null
      : {
        path: "intention.md",
        sha256: options.input.intentionSha256,
        bytes: options.input.intentionBytes,
        components: options.input.components,
      },
    references: referenceRecords,
    inputDigest: options.input.inputDigest,
    options: {
      selectedAgent: options.selectedAgent,
      agentMode: options.agentMode,
      verifyAfterAgent: options.verifyAfterAgent,
      runAfterCreate: options.runAfterCreate,
    },
    events: "events.jsonl",
  };
  writeFileSync(
    join(root, "provenance.json"),
    `${JSON.stringify(provenance, null, 2)}\n`,
    { mode: 0o600 },
  );
  return provenance;
}
