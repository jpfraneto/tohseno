export const PRODUCT = Object.freeze({
  repositoryUrl: "https://github.com/jpfraneto/tohseno",
  installCommand: "curl -fsSL https://tohseno.com/oneshot.sh | bash",
  copy: {
    BRAND: "TOHSENO",
    HEADER_NOTE: "ONE SHOT",
    HERO_META_DESCRIPTION:
      "The open source app blueprint system for builders with infinite ideas. Prototype iOS apps you can install, use, and judge.",
    HERO_HEADLINE: "Give every idea a shot.",
    HERO_LEDE:
      "Get rid of your recurring thoughts by turning them into an app you can install, use, and judge.",
    HERO_COPY_LABEL: "COPY INSTALLER",
    HERO_COPIED_LABEL: "INSTALLER COPIED",
    HERO_STEP_1: "install TOHSENO",
    HERO_STEP_2: "run tohseno",
    HERO_STEP_3: "tell your coding agent what to make",
    HERO_REQUIREMENTS:
      "macOS, Git, and Codex or Claude Code for the full iOS path. TOHSENO manages its own Bun runtime. iOS is the only implemented app platform.",
    HERO_LINKS_ARIA_LABEL: "More about TOHSENO",
    HERO_REPO_LINK: "Source on GitHub",
    HERO_DOCS_LINK: "Docs",
    PRIVACY_LINK: "Privacy",
    FOOTER_OPERATOR: "Currently operated by Anky, Inc.",
    FOOTER_DOCTRINE: "Private by default. Account-free. Take another one.",
  },
});

export type NodeEnvironment = "development" | "test" | "production";

export interface AppConfig {
  nodeEnv: NodeEnvironment;
  port: number;
  baseUrl: string;
  trustProxy: boolean;
  relay: RelayConfig;
}

export interface RelayConfig {
  enabled: boolean;
  claimInstallerReady: boolean;
  root?: string;
  maxRecords: number;
  maxBytes: number;
  globalRequestsPerMinute: number;
  sourceRequestsPerMinute: number;
}

type Environment = Record<string, string | undefined>;

function oneOf<T extends string>(
  name: string,
  value: string | undefined,
  values: readonly T[],
  fallback: T,
): T {
  const candidate = value ?? fallback;
  if (!values.includes(candidate as T)) {
    throw new Error(`${name} must be one of: ${values.join(", ")}`);
  }
  return candidate as T;
}

export function loadConfig(env: Environment = process.env): AppConfig {
  const nodeEnv = oneOf(
    "NODE_ENV",
    env.NODE_ENV,
    ["development", "test", "production"] as const,
    "development",
  );

  const portText = env.PORT ?? "3000";
  if (!/^\d{1,5}$/.test(portText))
    throw new Error("PORT must be a whole number between 1 and 65535");
  const port = Number(portText);
  if (!Number.isInteger(port) || port < 1 || port > 65_535)
    throw new Error("PORT must be between 1 and 65535");

  const baseUrl = env.BASE_URL ?? `http://localhost:${port}`;
  let parsedBase: URL;
  try {
    parsedBase = new URL(baseUrl);
  } catch {
    throw new Error("BASE_URL must be an absolute http(s) URL");
  }
  if (
    !(["http:", "https:"] as const).includes(
      parsedBase.protocol as "http:" | "https:",
    )
  ) {
    throw new Error("BASE_URL must use http or https");
  }
  if (
    parsedBase.username ||
    parsedBase.password ||
    parsedBase.pathname !== "/" ||
    parsedBase.search ||
    parsedBase.hash
  ) {
    throw new Error(
      "BASE_URL must be a bare origin without credentials, path, query, or fragment",
    );
  }
  if (nodeEnv === "production" && parsedBase.protocol !== "https:") {
    throw new Error("BASE_URL must use https when NODE_ENV=production");
  }
  if (
    env.TRUST_PROXY !== undefined &&
    env.TRUST_PROXY !== "true" &&
    env.TRUST_PROXY !== "false"
  ) {
    throw new Error("TRUST_PROXY must be true or false");
  }

  const relayEnabled = parseBoolean("INTENT_RELAY_ENABLED", env.INTENT_RELAY_ENABLED, false);
  const claimInstallerReady = parseBoolean(
    "CLAIM_INSTALLER_READY",
    env.CLAIM_INSTALLER_READY,
    false,
  );
  const relayRoot = env.INTENT_RELAY_ROOT;
  if (relayEnabled) {
    if (!relayRoot || !relayRoot.startsWith("/")) {
      throw new Error("INTENT_RELAY_ROOT must be an explicit absolute path when the relay is enabled");
    }
    if (nodeEnv === "production" && !claimInstallerReady) {
      throw new Error("CLAIM_INSTALLER_READY must be true before the production relay can be enabled");
    }
  }

  return {
    nodeEnv,
    port,
    baseUrl: parsedBase.origin,
    trustProxy: env.TRUST_PROXY === "true",
    relay: {
      enabled: relayEnabled,
      claimInstallerReady,
      root: relayRoot,
      maxRecords: parsePositiveInteger("INTENT_RELAY_MAX_RECORDS", env.INTENT_RELAY_MAX_RECORDS, 1_000),
      maxBytes: parsePositiveInteger("INTENT_RELAY_MAX_BYTES", env.INTENT_RELAY_MAX_BYTES, 10 * 1024 * 1024 * 1024),
      globalRequestsPerMinute: parsePositiveInteger("INTENT_RELAY_GLOBAL_RATE", env.INTENT_RELAY_GLOBAL_RATE, 1_200),
      sourceRequestsPerMinute: parsePositiveInteger("INTENT_RELAY_SOURCE_RATE", env.INTENT_RELAY_SOURCE_RATE, 120),
    },
  };
}

function parseBoolean(name: string, value: string | undefined, fallback: boolean): boolean {
  if (value === undefined) return fallback;
  if (value !== "true" && value !== "false") throw new Error(`${name} must be true or false`);
  return value === "true";
}

function parsePositiveInteger(name: string, value: string | undefined, fallback: number): number {
  if (value === undefined) return fallback;
  if (!/^\d+$/.test(value)) throw new Error(`${name} must be a positive whole number`);
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 1) throw new Error(`${name} must be a positive whole number`);
  return parsed;
}

export function safeStartupSummary(
  config: AppConfig,
): Record<string, string | number | boolean> {
  return {
    service: "tohseno",
    environment: config.nodeEnv,
    port: config.port,
    baseUrl: config.baseUrl,
    trustProxy: config.trustProxy,
    relayEnabled: config.relay.enabled,
    claimInstallerReady: config.relay.claimInstallerReady,
  };
}
