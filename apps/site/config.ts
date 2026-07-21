export const PRODUCT = Object.freeze({
  repositoryUrl: "https://github.com/jpfraneto/tohseno",
  skillPath: "skills/continuity-app/SKILL.md",
  manifestVersion: "0.2.0",
  oneshotCommand: "curl -fsSL https://tohseno.com/oneshot.sh | bash",
  copy: {
    BRAND: "TOHSENO",
    HEADER_NOTE: "one command · your agent · your app",
    HERO_META_DESCRIPTION: "Open rails for continuity apps — auth-less software built around one meaningful action a person returns to over time. Private by default, ejectable from birth.",
    HERO_HEADLINE: "Open rails for continuity apps.",
    HERO_LEDE: "Auth-less software built around one meaningful action a person returns to over time. TOHSENO does not generate your app — your coding agent does, inside rails that keep it private by default and ejectable from birth.",
    HERO_COPY_LABEL: "COPY",
    HERO_COPIED_LABEL: "COPIED",
    HERO_STEP_1: "run the one-liner",
    HERO_STEP_2: "hand off: “read AGENTS.md and begin”",
    HERO_STEP_3: "build inside the rails",
    HERO_REQUIREMENTS: "Needs git and a coding agent. The script is a small, pinned, inspectable bootstrap: no secrets, no telemetry, no accounts, and it deploys nothing. Start blank or from a shipped working example — anky or daily-observation.",
    HERO_LINKS_ARIA_LABEL: "More about TOHSENO",
    HERO_REPO_LINK: "Source on GitHub",
    HERO_DOCS_LINK: "Docs",
    PRIVACY_LINK: "Privacy",
    FOOTER_OPERATOR: "Currently operated by Anky, Inc.",
    FOOTER_DOCTRINE: "No accounts. No feeds. No manipulative streaks. Yours from birth.",
  },
});

export type NodeEnvironment = "development" | "test" | "production";

export interface AppConfig {
  nodeEnv: NodeEnvironment;
  port: number;
  baseUrl: string;
  trustProxy: boolean;
}

type Environment = Record<string, string | undefined>;

function oneOf<T extends string>(name: string, value: string | undefined, values: readonly T[], fallback: T): T {
  const candidate = value ?? fallback;
  if (!values.includes(candidate as T)) {
    throw new Error(`${name} must be one of: ${values.join(", ")}`);
  }
  return candidate as T;
}

export function loadConfig(env: Environment = process.env): AppConfig {
  const nodeEnv = oneOf("NODE_ENV", env.NODE_ENV, ["development", "test", "production"] as const, "development");

  const portText = env.PORT ?? "3000";
  if (!/^\d{1,5}$/.test(portText)) throw new Error("PORT must be a whole number between 1 and 65535");
  const port = Number(portText);
  if (!Number.isInteger(port) || port < 1 || port > 65_535) throw new Error("PORT must be between 1 and 65535");

  const baseUrl = env.BASE_URL ?? `http://localhost:${port}`;
  let parsedBase: URL;
  try {
    parsedBase = new URL(baseUrl);
  } catch {
    throw new Error("BASE_URL must be an absolute http(s) URL");
  }
  if (!(["http:", "https:"] as const).includes(parsedBase.protocol as "http:" | "https:")) {
    throw new Error("BASE_URL must use http or https");
  }
  if (
    parsedBase.username || parsedBase.password || parsedBase.pathname !== "/" ||
    parsedBase.search || parsedBase.hash
  ) {
    throw new Error("BASE_URL must be a bare origin without credentials, path, query, or fragment");
  }
  if (nodeEnv === "production" && parsedBase.protocol !== "https:") {
    throw new Error("BASE_URL must use https when NODE_ENV=production");
  }
  if (env.TRUST_PROXY !== undefined && env.TRUST_PROXY !== "true" && env.TRUST_PROXY !== "false") {
    throw new Error("TRUST_PROXY must be true or false");
  }

  return {
    nodeEnv,
    port,
    baseUrl: parsedBase.origin,
    trustProxy: env.TRUST_PROXY === "true",
  };
}

export function safeStartupSummary(config: AppConfig): Record<string, string | number | boolean> {
  return {
    service: "tohseno",
    environment: config.nodeEnv,
    port: config.port,
    baseUrl: config.baseUrl,
    trustProxy: config.trustProxy,
  };
}
