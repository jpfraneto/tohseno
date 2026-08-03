import { LIMITS, PACKAGE_SCHEMA } from "./intent-limits.js";
import { portableSha256 } from "./sha256-portable.js";

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const MAGIC = encoder.encode("TOHSINT1");
const MIME_BY_EXTENSION = Object.freeze({
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  heic: "image/heic",
  webp: "image/webp",
});

export function utf8ByteLength(value) {
  return encoder.encode(value).byteLength;
}

export function safeFilename(name) {
  const bytes = encoder.encode(name);
  return name.length > 0
    && bytes.byteLength <= 255
    && /^[\x20-\x7e]+$/.test(name)
    && !name.startsWith(".")
    && !name.endsWith(".")
    && !name.endsWith(" ")
    && !/[\\/:]/.test(name);
}

export function mediaTypeForFilename(name) {
  const extension = name.includes(".") ? name.split(".").at(-1).toLowerCase() : "";
  return MIME_BY_EXTENSION[extension] || null;
}

export function validateImageBytes(name, mediaType, bytes) {
  if (!safeFilename(name)) throw new Error("Reference filenames must be safe portable names.");
  const expected = mediaTypeForFilename(name);
  if (!expected) throw new Error("Use PNG, JPEG, HEIC, or WebP reference images.");
  if (expected !== mediaType) throw new Error(`${name} has a filename and media type that disagree.`);
  if (bytes.byteLength > LIMITS.referenceBytes) {
    throw new Error(`${name} is over the 16 MiB web handoff limit. Add it later through local Studio, which supports the broader local path.`);
  }
  const starts = (...signature) => signature.every((value, index) => bytes[index] === value);
  const ascii = (start, length) => String.fromCharCode(...bytes.slice(start, start + length));
  const valid = expected === "image/png"
    ? starts(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a)
    : expected === "image/jpeg"
      ? starts(0xff, 0xd8, 0xff)
      : expected === "image/webp"
        ? bytes.byteLength >= 12 && ascii(0, 4) === "RIFF" && ascii(8, 4) === "WEBP"
        : bytes.byteLength >= 12 && ascii(4, 4) === "ftyp"
          && ["heic", "heix", "hevc", "hevx", "mif1", "msf1"].includes(ascii(8, 4));
  if (!valid) throw new Error(`${name} bytes do not match its supported image format.`);
}

export async function sha256Hex(bytes) {
  const digest = globalThis.crypto?.subtle
    ? new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))
    : portableSha256(bytes);
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

export async function prepareReferences(files) {
  if (files.length > LIMITS.references) throw new Error("An intention accepts at most eight reference images.");
  let total = 0;
  const digests = new Set();
  const names = new Set();
  const prepared = [];
  for (const file of files) {
    const bytes = new Uint8Array(await file.blob.arrayBuffer());
    const mediaType = mediaTypeForFilename(file.originalFilename);
    validateImageBytes(file.originalFilename, mediaType, bytes);
    total += bytes.byteLength;
    if (total > LIMITS.totalReferenceBytes) throw new Error("References exceed the 48 MiB web handoff limit. Add larger material later through local Studio.");
    const digest = await sha256Hex(bytes);
    if (digests.has(digest)) throw new Error(`${file.originalFilename} repeats reference content already attached.`);
    digests.add(digest);
    const nameKey = file.originalFilename.toLowerCase();
    if (names.has(nameKey)) throw new Error("Reference filenames must be unique on Apple filesystems.");
    names.add(nameKey);
    prepared.push({ ...file, bytes, mediaType, digest });
  }
  return prepared;
}

export async function buildIntentPackage({ createdAt, prompt, references }) {
  const promptBytes = encoder.encode(prompt);
  if (prompt.trim().length === 0) throw new Error("Write an intention before taking a Shot.");
  if (promptBytes.byteLength > LIMITS.promptBytes) throw new Error("The web handoff accepts up to 1 MiB of prompt text. Use local Studio for a larger intention.");
  const prepared = await prepareReferences(references);
  let payloadOffset = 0;
  const manifestReferences = [];
  for (const [ordinal, reference] of prepared.entries()) {
    manifestReferences.push({
      ordinal,
      display_filename: reference.originalFilename,
      media_type: reference.mediaType,
      byte_length: reference.bytes.byteLength,
      sha256: reference.digest,
      payload_offset: payloadOffset,
    });
    payloadOffset += reference.bytes.byteLength;
  }
  const manifest = {
    schema: PACKAGE_SCHEMA,
    created_at: createdAt,
    prompt: {
      encoding: "utf-8",
      byte_length: promptBytes.byteLength,
      sha256: await sha256Hex(promptBytes),
      text: prompt,
    },
    references: manifestReferences,
    limits: {
      version: 1,
      max_prompt_bytes: LIMITS.promptBytes,
      max_references: LIMITS.references,
      max_reference_bytes: LIMITS.referenceBytes,
      max_total_reference_bytes: LIMITS.totalReferenceBytes,
      max_package_bytes: LIMITS.packageBytes,
    },
  };
  const manifestBytes = encoder.encode(JSON.stringify(manifest));
  const packageLength = 12 + manifestBytes.byteLength + payloadOffset;
  if (packageLength > LIMITS.packageBytes) throw new Error("The private intent package exceeds 64 MiB. Use the broader local Studio path.");
  const output = new Uint8Array(packageLength);
  output.set(MAGIC, 0);
  new DataView(output.buffer).setUint32(8, manifestBytes.byteLength, false);
  output.set(manifestBytes, 12);
  let cursor = 12 + manifestBytes.byteLength;
  for (const reference of prepared) { output.set(reference.bytes, cursor); cursor += reference.bytes.byteLength; }
  return output;
}

export async function parseIntentPackage(bytes) {
  if (!(bytes instanceof Uint8Array) || bytes.byteLength < 12 || ascii(bytes.slice(0, 8)) !== "TOHSINT1") throw new Error("Unsupported intent package magic.");
  const manifestLength = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(8, false);
  const start = 12 + manifestLength;
  if (manifestLength === 0 || start > bytes.byteLength) throw new Error("Malformed intent package manifest length.");
  const manifest = JSON.parse(decoder.decode(bytes.slice(12, start)));
  return { manifest, payload: bytes.slice(start) };
}

function ascii(bytes) { return String.fromCharCode(...bytes); }
