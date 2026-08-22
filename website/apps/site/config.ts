export const PRODUCT = Object.freeze({
  repositoryUrl: "https://github.com/jpfraneto/tohseno",
  installCommand: "curl -fsSL https://tohseno.com/oneshot.sh | bash",
  copy: {
    BRAND: "TOHSENO",
    COPY_LABEL: "COPY INSTALLER",
    COPIED_LABEL: "INSTALLER COPIED",
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
  billing: BillingConfig;
}

export interface BillingConfig {
  enabled: boolean;
  provider: "stripe" | "fake";
  root?: string;
  stripeSecretKey?: string;
  stripeWebhookSecret?: string;
  monthlyPriceId?: string;
  yearlyPriceId?: string;
  receiptSigningPrivateKey?: string;
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

  const billingEnabled = parseBoolean("BILLING_ENABLED", env.BILLING_ENABLED, false);
  const billingProvider = oneOf(
    "BILLING_PROVIDER",
    env.BILLING_PROVIDER,
    ["stripe", "fake"] as const,
    "stripe",
  );
  const billingRoot = env.BILLING_ROOT;
  if (billingEnabled) {
    if (!billingRoot?.startsWith("/")) {
      throw new Error("BILLING_ROOT must be an explicit absolute path when billing is enabled");
    }
    if (billingProvider === "fake" && nodeEnv !== "test") {
      throw new Error("the fake billing provider is available only in tests");
    }
    for (const [name, value] of [
      ["BILLING_RECEIPT_SIGNING_PKCS8_BASE64URL", env.BILLING_RECEIPT_SIGNING_PKCS8_BASE64URL],
      ["BILLING_MONTHLY_PRICE_ID", env.BILLING_MONTHLY_PRICE_ID],
      ["BILLING_YEARLY_PRICE_ID", env.BILLING_YEARLY_PRICE_ID],
    ] as const) {
      if (!value) throw new Error(`${name} is required when billing is enabled`);
    }
    if (billingProvider === "stripe") {
      if (!env.STRIPE_SECRET_KEY || !env.STRIPE_WEBHOOK_SECRET) {
        throw new Error("Stripe credentials are required when production billing is enabled");
      }
      if (nodeEnv === "production" && !env.STRIPE_SECRET_KEY.startsWith("sk_live_")) {
        throw new Error("production billing requires a live Stripe secret key");
      }
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
    billing: {
      enabled: billingEnabled,
      provider: billingProvider,
      root: billingRoot,
      stripeSecretKey: env.STRIPE_SECRET_KEY,
      stripeWebhookSecret: env.STRIPE_WEBHOOK_SECRET,
      monthlyPriceId: env.BILLING_MONTHLY_PRICE_ID,
      yearlyPriceId: env.BILLING_YEARLY_PRICE_ID,
      receiptSigningPrivateKey: env.BILLING_RECEIPT_SIGNING_PKCS8_BASE64URL,
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
    billingEnabled: config.billing.enabled,
    billingProvider: config.billing.enabled ? config.billing.provider : "disabled",
  };
}
