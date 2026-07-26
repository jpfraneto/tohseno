import { lstatSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const MAX_OPENAPI_BYTES = 256 * 1024;
const OPENAPI_PATH = fileURLToPath(
  new URL("../openapi.json", import.meta.url),
);

function loadOpenApiDocument(): unknown {
  const details = lstatSync(OPENAPI_PATH);
  if (
    details.isSymbolicLink() ||
    !details.isFile() ||
    details.nlink !== 1 ||
    details.size > MAX_OPENAPI_BYTES
  ) {
    throw new Error("reference node OpenAPI document is unavailable");
  }
  const source = new TextDecoder("utf-8", { fatal: true }).decode(
    readFileSync(OPENAPI_PATH),
  );
  const parsed = JSON.parse(source) as unknown;
  if (
    typeof parsed !== "object" ||
    parsed === null ||
    Array.isArray(parsed)
  ) {
    throw new Error("reference node OpenAPI document is invalid");
  }
  return parsed;
}

export const REFERENCE_NODE_OPENAPI = Object.freeze(loadOpenApiDocument());
