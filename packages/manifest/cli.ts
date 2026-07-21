/**
 * Command-line gate for continuity manifests: `bun run validate <path>`.
 *
 * Exit codes: 0 valid, 1 invalid or unparseable, 2 usage/missing file.
 * Running the library file directly validates nothing; this entry point
 * exists so an agent's "validate it" step cannot silently false-green.
 */
import { CONTINUITY_MANIFEST_SCHEMA_VERSION } from "./types";
import { formatManifestIssues, validateManifest } from "./validate";

const path = Bun.argv[2];
if (path === undefined || path === "--help" || path === "-h") {
  console.error("usage: bun run validate <continuity.manifest.json>");
  process.exit(2);
}

const file = Bun.file(path);
if (!(await file.exists())) {
  console.error(`✗ no such file: ${path}`);
  process.exit(2);
}

let value: unknown;
try {
  value = JSON.parse(await file.text()) as unknown;
} catch (error) {
  const detail = error instanceof Error ? error.message : "unknown JSON parse error";
  console.error(`✗ ${path} is not valid JSON: ${detail}`);
  process.exit(1);
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
