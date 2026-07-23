import { randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { basename, extname, join, relative, resolve, sep } from "node:path";
import {
  detectImageType,
  MAX_INTENTION_BYTES,
  MAX_REFERENCE_BYTES,
  MAX_REFERENCES,
  normalizeCreationInput,
  type CreationInput,
  type ReferenceInput,
} from "../provenance.ts";
import { StudioHttpError } from "./security.ts";

const MAX_NAME_BYTES = 256;
const MULTIPART_OVERHEAD_BYTES = 1_048_576;
export const MAX_STUDIO_UPLOAD_BYTES =
  MAX_INTENTION_BYTES * 2 +
  MAX_REFERENCE_BYTES * MAX_REFERENCES +
  MULTIPART_OVERHEAD_BYTES;

const MARKDOWN_MEDIA_TYPES = new Set([
  "",
  "application/octet-stream",
  "text/markdown",
  "text/plain",
  "text/x-markdown",
]);

const IMAGE_EXTENSION_TYPES = new Map<string, ReadonlySet<string>>([
  [".png", new Set(["image/png"])],
  [".jpg", new Set(["image/jpeg"])],
  [".jpeg", new Set(["image/jpeg"])],
  [".webp", new Set(["image/webp"])],
  [".gif", new Set(["image/gif"])],
  [".heic", new Set(["image/heic"])],
  [".heif", new Set(["image/heic"])],
  [".avif", new Set(["image/avif"])],
]);

export interface StagedStudioInput {
  name: string | undefined;
  input: CreationInput;
  directory: string;
  cleanup(): void;
}

function inside(root: string, candidate: string): boolean {
  const fromRoot = relative(resolve(root), resolve(candidate));
  return fromRoot === "" ||
    (fromRoot !== ".." && !fromRoot.startsWith(`..${sep}`));
}

function privateUploadRoot(factoryHome: string): string {
  const requestedFactoryHome = resolve(factoryHome);
  mkdirSync(requestedFactoryHome, { recursive: true, mode: 0o700 });
  const factoryDetails = lstatSync(requestedFactoryHome);
  if (factoryDetails.isSymbolicLink() || !factoryDetails.isDirectory()) {
    throw new StudioHttpError(
      500,
      "unsafe-staging",
      "Studio's private upload directory is unavailable.",
    );
  }
  const canonicalFactoryHome = realpathSync(requestedFactoryHome);
  const checkedChild = (parent: string, name: string): string => {
    const candidate = join(parent, name);
    if (!existsSync(candidate)) mkdirSync(candidate, { mode: 0o700 });
    const details = lstatSync(candidate);
    if (details.isSymbolicLink() || !details.isDirectory()) {
      throw new StudioHttpError(
        500,
        "unsafe-staging",
        "Studio's private upload directory is unavailable.",
      );
    }
    const canonical = realpathSync(candidate);
    if (!inside(canonicalFactoryHome, canonical) || canonical === canonicalFactoryHome) {
      throw new StudioHttpError(
        500,
        "unsafe-staging",
        "Studio's private upload directory is unavailable.",
      );
    }
    return canonical;
  };
  const studio = checkedChild(canonicalFactoryHome, "studio");
  return checkedChild(studio, "uploads");
}

function safeOriginalName(value: string): string {
  const normalized = value.replaceAll("\\", "/");
  const name = basename(normalized).normalize("NFC");
  if (
    name === "" ||
    name === "." ||
    name === ".." ||
    name.includes("\0") ||
    Buffer.byteLength(name) > 255
  ) {
    throw new StudioHttpError(
      400,
      "invalid-filename",
      "An uploaded file has an invalid filename.",
    );
  }
  return name;
}

function requireFile(value: FormDataEntryValue, label: string): File {
  if (!(value instanceof File)) {
    throw new StudioHttpError(400, "invalid-upload", `${label} must be a file.`);
  }
  return value;
}

function isUploadFile(value: unknown): value is File {
  return typeof File !== "undefined" && value instanceof File;
}

async function fileBytes(
  file: File,
  maximumBytes: number,
  label: string,
): Promise<Uint8Array> {
  if (file.size > maximumBytes) {
    throw new StudioHttpError(
      413,
      "upload-too-large",
      `${label} exceeds the local upload limit.`,
    );
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (bytes.byteLength !== file.size || bytes.byteLength > maximumBytes) {
    throw new StudioHttpError(
      413,
      "upload-too-large",
      `${label} exceeds the local upload limit.`,
    );
  }
  return bytes;
}

function writeInternalFile(
  directory: string,
  prefix: string,
  extension: string,
  bytes: Uint8Array,
): string {
  const path = join(directory, `${prefix}-${randomUUID()}${extension}`);
  writeFileSync(path, bytes, { flag: "wx", mode: 0o600 });
  return path;
}

function textField(
  entries: readonly FormDataEntryValue[],
  field: string,
): string {
  if (entries.length > 1 || entries.some((entry) => typeof entry !== "string")) {
    throw new StudioHttpError(
      400,
      "invalid-form",
      `Studio expected one text value for ${field}.`,
    );
  }
  return typeof entries[0] === "string" ? entries[0] : "";
}

function cleanShotName(value: string): string | undefined {
  const name = value.trim().normalize("NFC");
  if (name === "") return undefined;
  if (
    Buffer.byteLength(name) > MAX_NAME_BYTES ||
    /[\u0000-\u001f\u007f]/u.test(name)
  ) {
    throw new StudioHttpError(
      400,
      "invalid-name",
      "The optional shot name is too long or contains control characters.",
    );
  }
  return name;
}

function cleanupFunction(root: string, directory: string): () => void {
  let cleaned = false;
  return () => {
    if (cleaned) return;
    cleaned = true;
    if (
      !inside(root, directory) ||
      directory === root ||
      !basename(directory).startsWith("upload-")
    ) {
      return;
    }
    if (existsSync(directory)) {
      const details = lstatSync(directory);
      if (!details.isSymbolicLink() && details.isDirectory()) {
        rmSync(directory, { recursive: true, force: true });
      }
    }
  };
}

export async function stageStudioCreationRequest(options: {
  request: Request;
  factoryHome: string;
}): Promise<StagedStudioInput> {
  const contentType = options.request.headers.get("content-type") ?? "";
  if (!contentType.toLowerCase().startsWith("multipart/form-data;")) {
    throw new StudioHttpError(
      415,
      "multipart-required",
      "Studio creation requires multipart form data.",
    );
  }
  const contentLength = options.request.headers.get("content-length");
  if (contentLength === null) {
    throw new StudioHttpError(
      411,
      "content-length-required",
      "Studio requires a known upload size.",
    );
  }
  const parsed = Number(contentLength);
  if (!Number.isSafeInteger(parsed) || parsed < 0) {
    throw new StudioHttpError(
      400,
      "invalid-content-length",
      "Studio received an invalid upload size.",
    );
  }
  if (parsed > MAX_STUDIO_UPLOAD_BYTES) {
    throw new StudioHttpError(
      413,
      "request-too-large",
      "The Studio creation request is too large.",
    );
  }

  let form: FormData;
  try {
    form = await options.request.formData();
  } catch {
    throw new StudioHttpError(
      400,
      "invalid-multipart",
      "Studio could not read the multipart upload.",
    );
  }

  const fields = new Map<string, FormDataEntryValue[]>();
  for (const [key, value] of form.entries()) {
    if (!["name", "intention", "markdown", "reference"].includes(key)) {
      throw new StudioHttpError(
        400,
        "unknown-field",
        `Studio does not accept the multipart field ${JSON.stringify(key)}.`,
      );
    }
    const entry = value as unknown as FormDataEntryValue;
    if (
      (key === "markdown" || key === "reference") &&
      isUploadFile(entry) &&
      entry.name === "" &&
      entry.size === 0
    ) {
      continue;
    }
    const values = fields.get(key) ?? [];
    values.push(entry);
    fields.set(key, values);
  }

  const name = cleanShotName(textField(fields.get("name") ?? [], "name"));
  const intention = textField(
    fields.get("intention") ?? [],
    "intention",
  );
  if (Buffer.byteLength(intention) > MAX_INTENTION_BYTES) {
    throw new StudioHttpError(
      413,
      "intention-too-large",
      "The typed intention exceeds the 1 MiB limit.",
    );
  }

  const markdownValues = fields.get("markdown") ?? [];
  if (markdownValues.length > 1) {
    throw new StudioHttpError(
      400,
      "too-many-markdown-files",
      "Attach at most one Markdown intention.",
    );
  }
  const referenceValues = fields.get("reference") ?? [];
  if (referenceValues.length > MAX_REFERENCES) {
    throw new StudioHttpError(
      400,
      "too-many-references",
      `Attach at most ${MAX_REFERENCES} reference images.`,
    );
  }
  if (intention.trim() === "" && markdownValues.length === 0) {
    throw new StudioHttpError(
      400,
      "intention-required",
      "Write an intention or attach one Markdown file.",
    );
  }

  const root = privateUploadRoot(options.factoryHome);
  const directory = mkdtempSync(join(root, "upload-"));
  const cleanup = cleanupFunction(root, directory);
  try {
    let markdown: CreationInput["markdown"];
    if (markdownValues.length === 1) {
      const file = requireFile(markdownValues[0]!, "The Markdown intention");
      const originalName = safeOriginalName(file.name);
      if (extname(originalName).toLowerCase() !== ".md") {
        throw new StudioHttpError(
          415,
          "invalid-markdown-extension",
          "The intention file must use the .md extension.",
        );
      }
      const mediaType = file.type.toLowerCase();
      if (!MARKDOWN_MEDIA_TYPES.has(mediaType)) {
        throw new StudioHttpError(
          415,
          "invalid-markdown-type",
          "The intention upload must be Markdown or plain text.",
        );
      }
      const bytes = await fileBytes(file, MAX_INTENTION_BYTES, "The Markdown intention");
      try {
        new TextDecoder("utf-8", { fatal: true }).decode(bytes);
      } catch {
        throw new StudioHttpError(
          415,
          "invalid-markdown-encoding",
          "The intention Markdown must be valid UTF-8.",
        );
      }
      markdown = {
        path: writeInternalFile(directory, "intention", ".md", bytes),
        originalName,
      };
    }

    const references: ReferenceInput[] = [];
    for (const [index, value] of referenceValues.entries()) {
      const file = requireFile(value, `Reference image ${index + 1}`);
      const originalName = safeOriginalName(file.name);
      const expectedTypes = IMAGE_EXTENSION_TYPES.get(
        extname(originalName).toLowerCase(),
      );
      if (expectedTypes === undefined) {
        throw new StudioHttpError(
          415,
          "invalid-reference-extension",
          `Reference image ${index + 1} has an unsupported extension.`,
        );
      }
      const bytes = await fileBytes(
        file,
        MAX_REFERENCE_BYTES,
        `Reference image ${index + 1}`,
      );
      const detected = detectImageType(bytes);
      if (detected === null) {
        throw new StudioHttpError(
          415,
          "invalid-reference",
          `Reference image ${index + 1} is not a supported image.`,
        );
      }
      if (!expectedTypes.has(detected.mediaType)) {
        throw new StudioHttpError(
          415,
          "reference-extension-mismatch",
          `Reference image ${index + 1} does not match its filename extension.`,
        );
      }
      const declaredType = file.type.toLowerCase();
      if (
        declaredType !== "" &&
        declaredType !== "application/octet-stream" &&
        declaredType !== detected.mediaType &&
        !(declaredType === "image/heif" && detected.mediaType === "image/heic")
      ) {
        throw new StudioHttpError(
          415,
          "reference-type-mismatch",
          `Reference image ${index + 1} does not match its declared type.`,
        );
      }
      references.push({
        path: writeInternalFile(
          directory,
          `reference-${String(index + 1).padStart(3, "0")}`,
          detected.extension,
          bytes,
        ),
        originalName,
        mediaType: detected.mediaType,
      });
    }

    const input: CreationInput = {
      ...(intention === "" ? {} : { text: intention }),
      ...(markdown === undefined ? {} : { markdown }),
      ...(references.length === 0 ? {} : { references }),
    };
    // Run the shared normalizer before starting a long-lived job. The factory
    // repeats this check when it takes ownership of the staged input.
    const normalized = normalizeCreationInput(input);
    if (normalized.intention === null) {
      throw new StudioHttpError(
        400,
        "intention-required",
        "The intention cannot be empty.",
      );
    }
    return {
      name,
      input,
      directory,
      cleanup,
    };
  } catch (error) {
    cleanup();
    throw error;
  }
}
