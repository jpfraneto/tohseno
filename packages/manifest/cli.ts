/**
 * Command-line gate for the canonical app manifest: `bun run validate <path>`.
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
import {
  APP_MANIFEST_SCHEMA_VERSION,
  validateAppManifest,
} from "./app";

const MAX_MANIFEST_BYTES = 1_048_576;
const path = Bun.argv[2];
if (path === undefined || path === "--help" || path === "-h") {
  console.error("usage: bun run validate <app.manifest.json>");
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
  console.error(
    "  Pre-release compatibility is unsupported. Create a fresh Shot with TOHSENO 0.5.",
  );
  process.exit(1);
} finally {
  if (descriptor !== undefined) closeSync(descriptor);
}

const root =
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
const label = `app.manifest ${APP_MANIFEST_SCHEMA_VERSION}`;
if (
  root.kind !== "app" ||
  root.schemaVersion !== APP_MANIFEST_SCHEMA_VERSION
) {
  console.error(`✗ ${path} is not a canonical ${label}`);
  console.error(
    "  Pre-release compatibility is unsupported. Create a fresh Shot with TOHSENO 0.5.",
  );
  process.exit(1);
}
const result = validateAppManifest(value);
const formatIssues = (
  issues: ReadonlyArray<{
    severity: string;
    path: string;
    code: string;
    message: string;
  }>,
): string =>
  issues
    .map(
      (issue) =>
        `${issue.severity.toUpperCase()} ${issue.path} [${issue.code}]: ${issue.message}`,
    )
    .join("\n");
if (result.warnings.length > 0) {
  console.error(formatIssues(result.warnings));
}
if (!result.valid) {
  console.error(formatIssues(result.errors));
  console.error(
    `✗ ${path} · ${label} · ${result.errors.length} error${result.errors.length === 1 ? "" : "s"}`,
  );
  console.error("  fix the paths above, then rerun: bun run validate " + path);
  process.exit(1);
}
console.log(
  `✓ ${path} · ${label} · valid${result.warnings.length > 0 ? ` · ${result.warnings.length} warning${result.warnings.length === 1 ? "" : "s"}` : ""}`,
);
