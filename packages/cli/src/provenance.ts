import { createHash } from "node:crypto";
import {
  lstatSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { basename, extname, join } from "node:path";
import type { AgentId } from "./agents.ts";
import type { CreationDoor } from "./progress.ts";
import type { FactoryRelease } from "./release.ts";
import { CliError } from "./errors.ts";

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
  return readFileSync(path);
}

function decodeMarkdown(input: MarkdownInput): string {
  if (extname(input.originalName).toLowerCase() !== ".md") {
    throw new CliError("intention file must have a .md extension", 2);
  }
  const bytes = readRegularFile(input.path, MAX_INTENTION_BYTES, "intention Markdown");
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new CliError("intention Markdown must be valid UTF-8", 2);
  }
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
      originalName: basename(input.originalName),
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
    ...(originalName === undefined ? {} : { originalName: basename(originalName) }),
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
