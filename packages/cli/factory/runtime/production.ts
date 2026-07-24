import { lstatSync } from "node:fs";
import { join } from "node:path";
import {
  MachineError,
  readBoundedUtf8,
  readJson,
  requireRegularFile,
} from "./shared.ts";

interface ProductionConfiguration {
  schemaVersion: 1;
  persistence: {
    engine: "sqlite";
    configured: boolean;
    pathEnvironment: string;
    semantics: "single-instance";
  };
  backups: {
    configured: boolean;
    strategy: string | null;
  };
  requiredSecrets: Array<{
    slot: string;
    reference: string | null;
    resolved: boolean;
  }>;
  capabilities: {
    inspect: "implemented";
    deploy: "implemented" | "prepared" | "proposed";
    monitor: "implemented" | "prepared" | "proposed";
    recover: "implemented" | "prepared" | "proposed";
  };
}

export interface EndpointInspection {
  configured: boolean;
  value: string | null;
  stableHttps: boolean;
  localhost: boolean;
  quickTunnel: boolean;
  valid: boolean;
  issues: string[];
}

function decodedXcconfigValue(value: string): string {
  return value.trim().replaceAll(":/$()/", "://");
}

export function configuredProductionEndpoint(root: string): string | null {
  const path = join(root, "Config", "Production.xcconfig");
  if (lstatSync(path, { throwIfNoEntry: false }) === undefined) return null;
  const source = readBoundedUtf8(
    path,
    65_536,
    "production endpoint configuration",
  );
  const assignments = [...source.matchAll(/^\s*PRODUCTION_API_BASE_URL\s*=\s*(.*?)\s*$/gmu)];
  if (assignments.length !== 1) {
    throw new MachineError(
      "INVALID_CONFIGURATION",
      "Config/Production.xcconfig must contain exactly one PRODUCTION_API_BASE_URL assignment",
    );
  }
  const value = decodedXcconfigValue(assignments[0]![1] ?? "");
  return value === "" ? null : value;
}

export function inspectEndpoint(value: string | null): EndpointInspection {
  const issues: string[] = [];
  if (!value) {
    return {
      configured: false,
      value: null,
      stableHttps: false,
      localhost: false,
      quickTunnel: false,
      valid: false,
      issues: ["production API endpoint is not configured"],
    };
  }
  let parsed: URL | null = null;
  try {
    parsed = new URL(value);
  } catch {
    issues.push("production API endpoint is not an absolute URL");
  }
  const host = parsed?.hostname.toLowerCase() ?? "";
  const comparisonHost = host
    .replace(/^\[|\]$/gu, "")
    .replace(/\.$/u, "");
  const localhost = comparisonHost === "localhost" ||
    comparisonHost.endsWith(".localhost") ||
    comparisonHost === "::1" ||
    comparisonHost === "0:0:0:0:0:0:0:1" ||
    comparisonHost === "::" ||
    comparisonHost.startsWith("127.") ||
    comparisonHost.startsWith("0.") ||
    comparisonHost === "::ffff:7f00:1" ||
    comparisonHost.startsWith("::ffff:127.");
  const quickTunnel = comparisonHost === "trycloudflare.com" ||
    comparisonHost.endsWith(".trycloudflare.com");
  const stableHostname = /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$/u
    .test(host) &&
    host.length <= 253 &&
    /[a-z]/u.test(host.split(".").at(-1) ?? "");
  if (parsed) {
    if (parsed.protocol !== "https:") issues.push("production API endpoint must use HTTPS");
    if (parsed.username || parsed.password || parsed.pathname !== "/" || parsed.search || parsed.hash) {
      issues.push("production API endpoint must be a bare origin without credentials, path, query, or fragment");
    }
    if (localhost) issues.push("localhost and loopback endpoints are development-only");
    if (quickTunnel) issues.push("Cloudflare Quick Tunnels are development-only and cannot be production endpoints");
    if (!stableHostname) {
      issues.push("production API endpoint must use a stable fully qualified DNS hostname");
    }
  }
  const stableHttps = parsed !== null &&
    parsed.protocol === "https:" &&
    !localhost &&
    !quickTunnel &&
    stableHostname;
  return {
    configured: true,
    value,
    stableHttps,
    localhost,
    quickTunnel,
    valid: issues.length === 0,
    issues,
  };
}

function readProductionConfiguration(root: string): ProductionConfiguration {
  const path = join(root, "operations", "production.json");
  requireRegularFile(path, "production operations configuration");
  const value = readJson<Partial<ProductionConfiguration>>(path);
  if (
    value.schemaVersion !== 1 ||
    value.persistence?.engine !== "sqlite" ||
    typeof value.persistence.configured !== "boolean" ||
    value.persistence.pathEnvironment !== "TOHSENO_DATABASE_PATH" ||
    value.persistence.semantics !== "single-instance" ||
    typeof value.backups?.configured !== "boolean" ||
    !Array.isArray(value.requiredSecrets) ||
    value.capabilities?.inspect !== "implemented" ||
    !["implemented", "prepared", "proposed"].includes(String(value.capabilities.deploy)) ||
    !["implemented", "prepared", "proposed"].includes(String(value.capabilities.monitor)) ||
    !["implemented", "prepared", "proposed"].includes(String(value.capabilities.recover))
  ) {
    throw new MachineError("INVALID_CONFIGURATION", `${path} has an unsupported or incomplete shape`);
  }
  if (
    (value.backups.configured && (typeof value.backups.strategy !== "string" || value.backups.strategy.trim() === "")) ||
    (!value.backups.configured && value.backups.strategy !== null)
  ) {
    throw new MachineError("INVALID_CONFIGURATION", `${path} has an invalid backup strategy declaration`);
  }
  const slots = new Set<string>();
  for (const secret of value.requiredSecrets) {
    if (
      typeof secret !== "object" ||
      secret === null ||
      typeof secret.slot !== "string" ||
      secret.slot.trim() === "" ||
      (secret.reference !== null && typeof secret.reference !== "string") ||
      typeof secret.resolved !== "boolean"
    ) {
      throw new MachineError("INVALID_CONFIGURATION", `${path} contains an invalid required-secret reference`);
    }
    if (slots.has(secret.slot)) {
      throw new MachineError("INVALID_CONFIGURATION", `${path} contains duplicate required-secret slot ${secret.slot}`);
    }
    slots.add(secret.slot);
    if (
      secret.reference !== null &&
      !/^(?:env|file|keychain|provider):[A-Za-z0-9_./-]+$/u.test(secret.reference)
    ) {
      throw new MachineError(
        "INVALID_CONFIGURATION",
        `${path} secret references must use env:, file:, keychain:, or provider: identifiers, never secret values`,
      );
    }
    if (secret.resolved && secret.reference === null) {
      throw new MachineError("INVALID_CONFIGURATION", `${path} cannot mark a missing secret reference resolved`);
    }
  }
  return value as ProductionConfiguration;
}

function manifestRequiresServer(root: string): boolean | "credential-minting-only" {
  const path = join(root, "continuity.manifest.json");
  requireRegularFile(path, "continuity manifest");
  const manifest = readJson<{ operations?: { requiresServer?: unknown } }>(path);
  const requirement = manifest.operations?.requiresServer;
  if (requirement === true || requirement === "credential-minting-only") return requirement;
  if (requirement === false) return false;
  throw new MachineError("INVALID_CONFIGURATION", "manifest operations.requiresServer is invalid");
}

export function inspectProduction(root: string): {
  productionReady: boolean;
  endpoint: EndpointInspection;
  persistence: ProductionConfiguration["persistence"];
  backups: ProductionConfiguration["backups"];
  secrets: {
    required: number;
    unresolved: string[];
  };
  serverRequired: boolean | "credential-minting-only";
  blockers: string[];
  capabilities: {
    implemented: string[];
    prepared: string[];
    proposed: string[];
  };
} {
  const configuration = readProductionConfiguration(root);
  const serverRequired = manifestRequiresServer(root);
  const endpoint = inspectEndpoint(configuredProductionEndpoint(root));
  const unresolved = configuration.requiredSecrets
    .filter((secret) => !secret.resolved || !secret.reference)
    .map((secret) => secret.slot);
  const blockers: string[] = [];
  if (serverRequired !== false) {
    blockers.push(...endpoint.issues);
    if (!configuration.persistence.configured) {
      blockers.push("production SQLite persistence path is not configured");
    }
    if (!configuration.backups.configured) {
      blockers.push("production backups are not configured");
    }
  }
  if (unresolved.length > 0) blockers.push(`required secret references are unresolved: ${unresolved.join(", ")}`);
  if (serverRequired !== false && configuration.capabilities.deploy !== "implemented") {
    blockers.push("production deployment is not implemented by this factory release");
  }

  const implemented = ["production.inspect"];
  const prepared: string[] = [];
  const proposed: string[] = [];
  for (const [capability, status] of Object.entries(configuration.capabilities)) {
    if (capability === "inspect") continue;
    if (status === "implemented") implemented.push(`production.${capability}`);
    else if (status === "prepared") prepared.push(`production.${capability}`);
    else proposed.push(`production.${capability}`);
  }
  return {
    productionReady: blockers.length === 0,
    endpoint,
    persistence: configuration.persistence,
    backups: configuration.backups,
    secrets: { required: configuration.requiredSecrets.length, unresolved },
    serverRequired,
    blockers,
    capabilities: { implemented, prepared, proposed },
  };
}
