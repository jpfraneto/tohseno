export type NodeEnvironment = "development" | "test" | "production";
export type PushMode = "noop" | "fake" | "apns";
export type ApnsEnvironment = "sandbox" | "production";

export interface CompanionRelayConfig {
  nodeEnv: NodeEnvironment;
  host: string;
  port: number;
  baseUrl: string;
  trustProxy: boolean;
  healthcheckHost?: string;
  enabled: boolean;
  activationReady: boolean;
  root?: string;
  limits: {
    pairingSessions: number;
    mailboxes: number;
    mailboxEnvelopes: number;
    envelopes: number;
    bytes: number;
    /** Maximum decoded ciphertext bytes, including the AEAD tag. */
    envelopeBytes: number;
    /** Maximum encoded outer-envelope HTTP body bytes. */
    envelopeBodyBytes: number;
    pairingResponseBytes: number;
    retentionMs: number;
    clockSkewMs: number;
    pairingLifetimeMs: number;
    revocationRetentionMs: number;
    globalRequestsPerMinute: number;
    sourceRequestsPerMinute: number;
    catchUpLimit: number;
  };
  push: {
    mode: PushMode;
    environment: ApnsEnvironment;
    teamId?: string;
    keyId?: string;
    topic?: string;
    privateKeyPath?: string;
  };
}

type Environment = Record<string, string | undefined>;

export function loadCompanionRelayConfig(
  env: Environment = process.env,
): CompanionRelayConfig {
  const nodeEnv = oneOf(
    "NODE_ENV",
    env.NODE_ENV,
    ["development", "test", "production"] as const,
    "development",
  );
  const port = positiveInteger("PORT", env.PORT, 3100, 65_535);
  const host = env.HOST ?? "127.0.0.1";
  if (!isHost(host)) throw new Error("HOST must be a hostname or IP address without a scheme or path");

  const baseUrlText = env.BASE_URL ?? `http://${host}:${port}`;
  let baseUrl: URL;
  try {
    baseUrl = new URL(baseUrlText);
  } catch {
    throw new Error("BASE_URL must be an absolute http(s) URL");
  }
  if (!['http:', 'https:'].includes(baseUrl.protocol)) {
    throw new Error("BASE_URL must use http or https");
  }
  if (
    baseUrl.username ||
    baseUrl.password ||
    baseUrl.pathname !== "/" ||
    baseUrl.search ||
    baseUrl.hash
  ) {
    throw new Error("BASE_URL must be a bare origin without credentials, path, query, or fragment");
  }
  if (nodeEnv === "production" && baseUrl.protocol !== "https:") {
    throw new Error("BASE_URL must use https when NODE_ENV=production");
  }

  const trustProxy = boolean("TRUST_PROXY", env.TRUST_PROXY, false);
  const healthcheckHost = env.COMPANION_RELAY_HEALTHCHECK_HOST;
  if (healthcheckHost && !isHost(healthcheckHost)) {
    throw new Error(
      "COMPANION_RELAY_HEALTHCHECK_HOST must be a hostname or IP address without a scheme, port, or path",
    );
  }
  const enabled = boolean("COMPANION_RELAY_ENABLED", env.COMPANION_RELAY_ENABLED, false);
  const activationReady = boolean(
    "COMPANION_RELAY_ACTIVATION_READY",
    env.COMPANION_RELAY_ACTIVATION_READY,
    false,
  );
  const root = env.COMPANION_RELAY_ROOT;
  if (enabled && (!root || !root.startsWith("/"))) {
    throw new Error("COMPANION_RELAY_ROOT must be an explicit absolute path when the relay is enabled");
  }
  if (enabled && nodeEnv === "production" && !activationReady) {
    throw new Error("COMPANION_RELAY_ACTIVATION_READY must be true before production activation");
  }

  const pushMode = oneOf(
    "COMPANION_RELAY_PUSH_MODE",
    env.COMPANION_RELAY_PUSH_MODE,
    ["noop", "fake", "apns"] as const,
    "noop",
  );
  if (nodeEnv === "production" && pushMode === "fake") {
    throw new Error("COMPANION_RELAY_PUSH_MODE=fake is forbidden in production");
  }
  if (!enabled && pushMode !== "noop") {
    throw new Error("push delivery cannot be enabled while the companion relay is disabled");
  }
  const apnsEnvironment = oneOf(
    "COMPANION_RELAY_APNS_ENVIRONMENT",
    env.COMPANION_RELAY_APNS_ENVIRONMENT,
    ["sandbox", "production"] as const,
    nodeEnv === "production" ? "production" : "sandbox",
  );
  if (pushMode === "apns") {
    requiredMatch("COMPANION_RELAY_APNS_TEAM_ID", env.COMPANION_RELAY_APNS_TEAM_ID, /^[A-Z0-9]{10}$/);
    requiredMatch("COMPANION_RELAY_APNS_KEY_ID", env.COMPANION_RELAY_APNS_KEY_ID, /^[A-Z0-9]{10}$/);
    requiredMatch(
      "COMPANION_RELAY_APNS_TOPIC",
      env.COMPANION_RELAY_APNS_TOPIC,
      /^[A-Za-z0-9](?:[A-Za-z0-9.-]{0,253}[A-Za-z0-9])?$/,
    );
    if (!env.COMPANION_RELAY_APNS_PRIVATE_KEY_PATH?.startsWith("/")) {
      throw new Error("COMPANION_RELAY_APNS_PRIVATE_KEY_PATH must be an explicit absolute path");
    }
  }

  const envelopeBytes = positiveInteger(
    "COMPANION_RELAY_MAX_ENVELOPE_BYTES",
    env.COMPANION_RELAY_MAX_ENVELOPE_BYTES,
    16 * 1024 * 1024 + 16,
    16 * 1024 * 1024 + 16,
  );
  const envelopeBodyBytes = Math.ceil(envelopeBytes * 4 / 3) + 8 * 1024;

  return {
    nodeEnv,
    host,
    port,
    baseUrl: baseUrl.origin,
    trustProxy,
    healthcheckHost,
    enabled,
    activationReady,
    root,
    limits: {
      pairingSessions: positiveInteger("COMPANION_RELAY_MAX_PAIRING_SESSIONS", env.COMPANION_RELAY_MAX_PAIRING_SESSIONS, 10_000, 1_000_000),
      mailboxes: positiveInteger("COMPANION_RELAY_MAX_MAILBOXES", env.COMPANION_RELAY_MAX_MAILBOXES, 10_000, 1_000_000),
      mailboxEnvelopes: positiveInteger("COMPANION_RELAY_MAX_MAILBOX_ENVELOPES", env.COMPANION_RELAY_MAX_MAILBOX_ENVELOPES, 10_000, 50_000),
      envelopes: positiveInteger("COMPANION_RELAY_MAX_ENVELOPES", env.COMPANION_RELAY_MAX_ENVELOPES, 100_000, 10_000_000),
      bytes: positiveInteger("COMPANION_RELAY_MAX_BYTES", env.COMPANION_RELAY_MAX_BYTES, 10 * 1024 * 1024 * 1024, 1024 * 1024 * 1024 * 1024),
      envelopeBytes,
      envelopeBodyBytes,
      pairingResponseBytes: positiveInteger("COMPANION_RELAY_MAX_PAIRING_RESPONSE_BYTES", env.COMPANION_RELAY_MAX_PAIRING_RESPONSE_BYTES, 64 * 1024, 1024 * 1024),
      retentionMs: positiveInteger("COMPANION_RELAY_RETENTION_SECONDS", env.COMPANION_RELAY_RETENTION_SECONDS, 7 * 24 * 60 * 60, 30 * 24 * 60 * 60) * 1000,
      clockSkewMs: positiveInteger("COMPANION_RELAY_CLOCK_SKEW_SECONDS", env.COMPANION_RELAY_CLOCK_SKEW_SECONDS, 300, 900) * 1000,
      pairingLifetimeMs: positiveInteger("COMPANION_RELAY_PAIRING_LIFETIME_SECONDS", env.COMPANION_RELAY_PAIRING_LIFETIME_SECONDS, 120, 180) * 1000,
      revocationRetentionMs: positiveInteger("COMPANION_RELAY_REVOCATION_RETENTION_SECONDS", env.COMPANION_RELAY_REVOCATION_RETENTION_SECONDS, 30 * 24 * 60 * 60, 365 * 24 * 60 * 60) * 1000,
      globalRequestsPerMinute: positiveInteger("COMPANION_RELAY_GLOBAL_RATE", env.COMPANION_RELAY_GLOBAL_RATE, 1_200, 1_000_000),
      sourceRequestsPerMinute: positiveInteger("COMPANION_RELAY_SOURCE_RATE", env.COMPANION_RELAY_SOURCE_RATE, 120, 1_000_000),
      catchUpLimit: positiveInteger("COMPANION_RELAY_CATCH_UP_LIMIT", env.COMPANION_RELAY_CATCH_UP_LIMIT, 100, 256),
    },
    push: {
      mode: pushMode,
      environment: apnsEnvironment,
      teamId: env.COMPANION_RELAY_APNS_TEAM_ID,
      keyId: env.COMPANION_RELAY_APNS_KEY_ID,
      topic: env.COMPANION_RELAY_APNS_TOPIC,
      privateKeyPath: env.COMPANION_RELAY_APNS_PRIVATE_KEY_PATH,
    },
  };
}

export function safeCompanionStartupSummary(
  config: CompanionRelayConfig,
): Record<string, string | number | boolean> {
  return {
    service: "tohseno-companion-relay",
    version: "0.9.9",
    environment: config.nodeEnv,
    host: config.host,
    port: config.port,
    baseUrl: config.baseUrl,
    trustProxy: config.trustProxy,
    enabled: config.enabled,
    activationReady: config.activationReady,
    pushMode: config.push.mode,
  };
}

function boolean(name: string, value: string | undefined, fallback: boolean): boolean {
  if (value === undefined) return fallback;
  if (value !== "true" && value !== "false") throw new Error(`${name} must be true or false`);
  return value === "true";
}

function positiveInteger(
  name: string,
  value: string | undefined,
  fallback: number,
  maximum = Number.MAX_SAFE_INTEGER,
): number {
  if (value === undefined) return fallback;
  if (!/^\d+$/.test(value)) throw new Error(`${name} must be a positive whole number`);
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 1 || parsed > maximum) {
    throw new Error(`${name} must be between 1 and ${maximum}`);
  }
  return parsed;
}

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

function requiredMatch(name: string, value: string | undefined, pattern: RegExp): void {
  if (!value || !pattern.test(value)) throw new Error(`${name} is missing or malformed`);
}

function isHost(value: string): boolean {
  return value.length > 0 && value.length <= 253 && !/[\s/:?#@]/.test(value);
}
