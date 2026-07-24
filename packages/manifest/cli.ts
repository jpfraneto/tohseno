/**
 * Command-line gate for continuity manifests: `bun run validate <path>`.
 *
 * Exit codes: 0 valid, 1 invalid or unparseable, 2 usage/missing file.
 * Running the library file directly validates nothing; this entry point
 * exists so an agent's "validate it" step cannot silently false-green.
 */
import {
  closeSync,
  constants,
  fstatSync,
  lstatSync,
  openSync,
  readSync,
} from "node:fs";
import { CONTINUITY_MANIFEST_SCHEMA_VERSION } from "./types";
import { formatManifestIssues, validateManifest } from "./validate";

const MAX_MANIFEST_BYTES = 1_048_576;
const path = Bun.argv[2];
if (path === undefined || path === "--help" || path === "-h") {
  console.error("usage: bun run validate <continuity.manifest.json>");
  process.exit(2);
}

let details: ReturnType<typeof lstatSync>;
try {
  details = lstatSync(path);
} catch (error) {
  if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
    console.error(
      `✗ cannot safely read ${path}: expected a regular manifest no larger than ${MAX_MANIFEST_BYTES} bytes`,
    );
    process.exit(1);
  }
  console.error(`✗ no such file: ${path}`);
  process.exit(2);
}
if (
  details.isSymbolicLink() ||
  !details.isFile() ||
  details.nlink !== 1 ||
  details.size > MAX_MANIFEST_BYTES
) {
  console.error(
    `✗ cannot safely read ${path}: expected a regular manifest no larger than ${MAX_MANIFEST_BYTES} bytes`,
  );
  process.exit(1);
}

let value: unknown;
let descriptor: number | undefined;
try {
  descriptor = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  const opened = fstatSync(descriptor);
  if (
    !opened.isFile() ||
    opened.nlink !== 1 ||
    opened.dev !== details.dev ||
    opened.ino !== details.ino ||
    opened.size > MAX_MANIFEST_BYTES
  ) {
    throw new Error("manifest identity changed while opening");
  }
  const chunks: Buffer[] = [];
  const buffer = Buffer.allocUnsafe(65_536);
  let total = 0;
  while (true) {
    const length = readSync(descriptor, buffer, 0, buffer.length, null);
    if (length === 0) break;
    total += length;
    if (total > MAX_MANIFEST_BYTES) {
      throw new Error("manifest grew past its size limit");
    }
    chunks.push(Buffer.from(buffer.subarray(0, length)));
  }
  const bytes = Buffer.concat(chunks, total);
  const source = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  value = JSON.parse(source) as unknown;
} catch (error) {
  const detail = error instanceof Error ? error.message : "unknown JSON parse error";
  console.error(`✗ ${path} is not valid JSON: ${detail}`);
  process.exit(1);
} finally {
  if (descriptor !== undefined) closeSync(descriptor);
}

const result = validateManifest(value);
if (result.warnings.length > 0) {
  console.error(formatManifestIssues(result.warnings));
}
if (!result.valid) {
  console.error(formatManifestIssues(result.errors));
  console.error(
    `✗ ${path} · continuity.manifest ${CONTINUITY_MANIFEST_SCHEMA_VERSION} · ${result.errors.length} error${result.errors.length === 1 ? "" : "s"}`,
  );
  console.error("  fix the paths above, then rerun: bun run validate " + path);
  process.exit(1);
}
console.log(
  `✓ ${path} · continuity.manifest ${CONTINUITY_MANIFEST_SCHEMA_VERSION} · valid${result.warnings.length > 0 ? ` · ${result.warnings.length} warning${result.warnings.length === 1 ? "" : "s"}` : ""}`,
);
