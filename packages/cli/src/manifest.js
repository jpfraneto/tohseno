import {
  ARTIFACT_ORIGINS,
  MAX_ARTIFACT_BYTES,
  RELEASE_LAYOUT,
} from "./constants.js";
import { compareVersions, parseVersion } from "./semver.js";

const HEX_256 = /^[0-9a-f]{64}$/;
const ARCHITECTURES = new Map([
  ["arm64", "aarch64-apple-darwin"],
  ["x64", "x86_64-apple-darwin"],
]);
const EXACT_KEYS = [
  "schema",
  "native_release_version",
  "minimum_npm_cli_version",
  "layout_version",
  "artifacts",
];

function exactKeys(value, expected, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  const observed = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (observed.length !== wanted.length || observed.some((key, index) => key !== wanted[index])) {
    throw new Error(`${label} contains missing or unexpected fields`);
  }
}

export function validatedHttpsURL(value, origins = ARTIFACT_ORIGINS) {
  let url;
  try { url = new URL(value); } catch { throw new Error("release URL is malformed"); }
  if (url.protocol !== "https:" || url.username || url.password || url.port || !origins.has(url.origin)) {
    throw new Error("release URL is not on the exact HTTPS allowlist");
  }
  if (url.hash) throw new Error("release URL must not contain a fragment");
  return url;
}

export function validateManifest(value, npmVersion, architecture) {
  exactKeys(value, EXACT_KEYS, "release manifest");
  if (value.schema !== "tohseno.native-release-manifest/1") {
    throw new Error("release manifest schema is unsupported");
  }
  parseVersion(value.native_release_version);
  parseVersion(value.minimum_npm_cli_version);
  if (compareVersions(npmVersion, value.minimum_npm_cli_version) < 0) {
    throw new Error("this npm TOHSENO is too old for the authorized native release");
  }
  if (value.layout_version !== RELEASE_LAYOUT) {
    throw new Error("native release layout is unsupported");
  }
  if (!Array.isArray(value.artifacts) || value.artifacts.length !== ARCHITECTURES.size) {
    throw new Error("release manifest must contain exactly one artifact per supported architecture");
  }
  const seen = new Set();
  const artifacts = value.artifacts.map((artifact) => {
    exactKeys(artifact, ["architecture", "target", "url", "byte_size", "sha256", "signing"], "artifact");
    if (!ARCHITECTURES.has(artifact.architecture) || seen.has(artifact.architecture)) {
      throw new Error("release manifest contains a duplicate or unsupported architecture");
    }
    seen.add(artifact.architecture);
    if (artifact.target !== ARCHITECTURES.get(artifact.architecture)) {
      throw new Error("release target does not match its architecture");
    }
    if (!Number.isSafeInteger(artifact.byte_size) || artifact.byte_size <= 0 || artifact.byte_size > MAX_ARTIFACT_BYTES) {
      throw new Error("release artifact byte size is invalid");
    }
    if (typeof artifact.sha256 !== "string" || !HEX_256.test(artifact.sha256)) {
      throw new Error("release artifact SHA-256 is missing or noncanonical");
    }
    const url = validatedHttpsURL(artifact.url);
    exactKeys(artifact.signing, ["kind", "team_id", "designated_requirement"], "artifact signing policy");
    if (!["release-package", "apple-developer-id"].includes(artifact.signing.kind)) {
      throw new Error("release signing policy is unsupported");
    }
    if (artifact.signing.kind === "apple-developer-id") {
      if (!/^[A-Z0-9]{10}$/.test(artifact.signing.team_id)
        || typeof artifact.signing.designated_requirement !== "string"
        || artifact.signing.designated_requirement.length < 10
        || artifact.signing.designated_requirement.length > 512) {
        throw new Error("Apple signing identity policy is invalid");
      }
    } else if (artifact.signing.team_id !== null || artifact.signing.designated_requirement !== null) {
      throw new Error("unsigned release-package policy must not invent an Apple identity");
    }
    return { ...artifact, url };
  });
  const artifact = artifacts.find((candidate) => candidate.architecture === architecture);
  if (!artifact) throw new Error("this Mac architecture is not supported by the release");
  return { ...value, artifacts, artifact };
}

export function nodeArchitecture(value = process.arch) {
  if (!ARCHITECTURES.has(value)) throw new Error("TOHSENO supports Apple silicon and Intel Macs only");
  return value;
}
