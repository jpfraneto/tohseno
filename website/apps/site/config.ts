export const PRODUCT = Object.freeze({
  repositoryUrl: "https://github.com/jpfraneto/tohseno",
  installCommand: "curl -fsSL https://tohseno.com/oneshot.sh | bash",
  copy: {
    BRAND: "Tohseno",
    COPY_LABEL: "COPY INSTALLER",
    COPIED_LABEL: "INSTALLER COPIED",
    FOOTER_OPERATOR: "Currently operated by Anky, Inc.",
    FOOTER_DOCTRINE: "Private by default. Account-free. Take another one.",
  },
});

export const RELEASED_CLAIMS_ACTIVATION = Object.freeze({
  contractAddress: "0x5012703d48d99224ac0035d58bc373de9e8b1934",
  signingDigest: "0xec418380f588b9a6f72fc251b7a0ae7bee8a19a1d843017e4733ebd2d094966d",
  deploymentBlock: 50_973_950n,
});

export type NodeEnvironment = "development" | "test" | "production";

export interface AppConfig {
  nodeEnv: NodeEnvironment;
  port: number;
  baseUrl: string;
  trustProxy: boolean;
  relay: RelayConfig;
  billing: BillingConfig;
  managed: ManagedConfig;
  distribution: DistributionConfig;
  registry: RegistryConfig;
  claims: ClaimsConfig;
}

export interface RegistryConfig {
  enabled: boolean;
  root?: string;
  rpcUrl?: string;
  chainId: 4663;
  factoryAddress: `0x${string}`;
  registryAddress: `0x${string}`;
  activationSigningDigest: `0x${string}`;
  globalRequestsPerMinute: number;
  sourceRequestsPerMinute: number;
  maxStagingRecords: number;
  maxStagingBytes: number;
  relayerEnabled: boolean;
  relayerPrivateKey?: `0x${string}`;
}

export interface ClaimsConfig {
  configured: boolean;
  contractAddress?: `0x${string}`;
  activationSigningDigest?: `0x${string}`;
  activationEvidencePath?: string;
  authorityPolicyPath?: string;
  deploymentBlock?: bigint;
  indexerEnabled: boolean;
  relayerEnabled: boolean;
  relayerPrivateKey?: `0x${string}`;
}

export interface DistributionConfig {
  macosEnabled: boolean;
  macosChannel: "release-candidate" | "stable";
  macosUrl?: string;
  macosSha256?: string;
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

export interface ManagedConfig {
  enabled: boolean;
  provider: "bankr" | "fake";
  root?: string;
  stripeSecretKey?: string;
  stripeWebhookSecret?: string;
  priceIds: Readonly<Record<"usd_10" | "usd_25" | "usd_50", string | undefined>>;
  checkoutSuccessUrl?: string;
  checkoutCancelUrl?: string;
  bankrBaseUrl: string;
  bankrApiKey?: string;
  modelAllowlist: readonly string[];
  rateLimitPerMinute: number;
  operatorTokenSha256?: string;
  launchFeeFundingConfirmed: boolean;
  launchFeeFundingReference?: string;
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

  const managedEnabled = parseBoolean("MANAGED_COMPUTE_ENABLED", env.MANAGED_COMPUTE_ENABLED, false);
  const managedProvider = oneOf(
    "MANAGED_COMPUTE_PROVIDER",
    env.MANAGED_COMPUTE_PROVIDER,
    ["bankr", "fake"] as const,
    "bankr",
  );
  const managedRoot = env.MANAGED_COMPUTE_ROOT;
  const modelAllowlist = (env.BANKR_MODEL_ALLOWLIST ?? "")
    .split(",").map((value) => value.trim()).filter(Boolean);
  const launchFeeFundingConfirmed = parseBoolean(
    "BANKR_LAUNCH_FEE_FUNDING_CONFIRMED",
    env.BANKR_LAUNCH_FEE_FUNDING_CONFIRMED,
    false,
  );
  if (managedEnabled) {
    if (!managedRoot?.startsWith("/")) throw new Error("MANAGED_COMPUTE_ROOT must be an explicit absolute path");
    if (managedProvider === "fake" && nodeEnv !== "test") throw new Error("the fake managed provider is available only in tests");
    for (const [name, value] of [
      ["STRIPE_SECRET_KEY", env.STRIPE_SECRET_KEY],
      ["STRIPE_WEBHOOK_SECRET", env.STRIPE_WEBHOOK_SECRET],
      ["STRIPE_BALANCE_PRICE_10", env.STRIPE_BALANCE_PRICE_10],
      ["STRIPE_BALANCE_PRICE_25", env.STRIPE_BALANCE_PRICE_25],
      ["STRIPE_BALANCE_PRICE_50", env.STRIPE_BALANCE_PRICE_50],
      ["MANAGED_CHECKOUT_SUCCESS_URL", env.MANAGED_CHECKOUT_SUCCESS_URL],
      ["MANAGED_CHECKOUT_CANCEL_URL", env.MANAGED_CHECKOUT_CANCEL_URL],
      ["TOHSENO_OPERATOR_TOKEN_SHA256", env.TOHSENO_OPERATOR_TOKEN_SHA256],
    ] as const) {
      if (!value) throw new Error(`${name} is required when managed compute is enabled`);
    }
    if (!/^[a-f0-9]{64}$/.test(env.TOHSENO_OPERATOR_TOKEN_SHA256 ?? "")) {
      throw new Error("TOHSENO_OPERATOR_TOKEN_SHA256 must be a lowercase SHA-256 digest");
    }
    if (managedProvider === "bankr" && !env.BANKR_API_KEY?.startsWith("bk_")) {
      throw new Error("BANKR_API_KEY is required and must be a Bankr LLM Gateway key");
    }
    if (managedProvider === "bankr") {
      const bankr = new URL(env.BANKR_BASE_URL ?? "https://llm.bankr.bot");
      if (bankr.protocol !== "https:" || bankr.hostname !== "llm.bankr.bot"
          || bankr.username || bankr.password || bankr.pathname !== "/"
          || bankr.search || bankr.hash) {
        throw new Error("BANKR_BASE_URL must be the bare official https://llm.bankr.bot origin");
      }
    }
    if (nodeEnv === "production" && !env.STRIPE_SECRET_KEY?.startsWith("sk_live_")) {
      throw new Error("production managed compute requires a live Stripe secret key");
    }
    if (!modelAllowlist.length || modelAllowlist.some((id) => !/^[A-Za-z0-9._:[\]-]{1,128}$/.test(id))) {
      throw new Error("BANKR_MODEL_ALLOWLIST must contain bounded model identifiers");
    }
    for (const name of ["MANAGED_CHECKOUT_SUCCESS_URL", "MANAGED_CHECKOUT_CANCEL_URL"] as const) {
      const url = new URL(env[name] ?? "");
      if (url.protocol !== "https:" || url.username || url.password || url.hash) {
        throw new Error(`${name} must be a trusted HTTPS URL`);
      }
    }
    if (launchFeeFundingConfirmed && !env.BANKR_LAUNCH_FEE_FUNDING_REFERENCE) {
      throw new Error("confirmed launch-fee funding requires BANKR_LAUNCH_FEE_FUNDING_REFERENCE");
    }
  }

  const macosEnabled = parseBoolean("MACOS_DOWNLOAD_ENABLED", env.MACOS_DOWNLOAD_ENABLED, false);
  const macosChannel = oneOf(
    "MACOS_DOWNLOAD_CHANNEL",
    env.MACOS_DOWNLOAD_CHANNEL,
    ["release-candidate", "stable"] as const,
    "stable",
  );
  if (macosEnabled) {
    let url: URL;
    try { url = new URL(env.MACOS_DOWNLOAD_URL ?? ""); }
    catch { throw new Error("MACOS_DOWNLOAD_URL must be an absolute HTTPS URL"); }
    if (url.protocol !== "https:" || url.username || url.password || url.hash) {
      throw new Error("MACOS_DOWNLOAD_URL must be an absolute HTTPS URL without credentials or a fragment");
    }
    if (!/^[a-f0-9]{64}$/.test(env.MACOS_DOWNLOAD_SHA256 ?? "")) {
      throw new Error("MACOS_DOWNLOAD_SHA256 must be the lowercase digest of the notarized DMG");
    }
  }

  const registryEnabled = parseBoolean("REGISTRY_ENABLED", env.REGISTRY_ENABLED, false);
  const registryRoot = env.REGISTRY_ROOT;
  const registryRpcUrl = env.ROBINHOOD_RPC_URL;
  const registryRelayerEnabled = parseBoolean(
    "REGISTRY_RELAYER_ENABLED",
    env.REGISTRY_RELAYER_ENABLED,
    false,
  );
  if (registryEnabled) {
    if (!registryRoot?.startsWith("/")) {
      throw new Error("REGISTRY_ROOT must be an explicit absolute path when the Registry is enabled");
    }
    let rpc: URL;
    try { rpc = new URL(registryRpcUrl ?? ""); }
    catch { throw new Error("ROBINHOOD_RPC_URL must be an absolute HTTPS URL"); }
    if (rpc.protocol !== "https:" || rpc.username || rpc.password || rpc.hash) {
      throw new Error("ROBINHOOD_RPC_URL must be an HTTPS URL without credentials or a fragment");
    }
  }
  if (registryRelayerEnabled) {
    if (!registryEnabled) throw new Error("REGISTRY_ENABLED must be true before its relayer can be enabled");
    if (!/^0x[0-9a-f]{64}$/.test(env.REGISTRY_RELAYER_PRIVATE_KEY ?? "")) {
      throw new Error("REGISTRY_RELAYER_PRIVATE_KEY must be one dedicated lowercase private key");
    }
  }

  const claimsAddress = env.CLAIMS_CONTRACT_ADDRESS;
  const claimsActivationDigest = env.CLAIMS_ACTIVATION_SIGNING_DIGEST;
  const claimsActivationEvidencePath = env.CLAIMS_ACTIVATION_EVIDENCE_PATH;
  const claimsAuthorityPolicyPath = env.CLAIMS_AUTHORITY_POLICY_PATH;
  const claimsConfigured = [claimsAddress, claimsActivationDigest, claimsActivationEvidencePath,
    claimsAuthorityPolicyPath, env.CLAIMS_DEPLOYMENT_BLOCK].some((value) => value !== undefined);
  const claimsIndexerEnabled = parseBoolean(
    "CLAIMS_INDEXER_ENABLED", env.CLAIMS_INDEXER_ENABLED, false,
  );
  const claimsRelayerEnabled = parseBoolean(
    "CLAIMS_RELAYER_ENABLED", env.CLAIMS_RELAYER_ENABLED, false,
  );
  if (claimsConfigured) {
    if (!/^0x[0-9a-f]{40}$/.test(claimsAddress ?? "")) {
      throw new Error("CLAIMS_CONTRACT_ADDRESS must be one exact lowercase address");
    }
    if (!/^0x[0-9a-f]{64}$/.test(claimsActivationDigest ?? "")) {
      throw new Error("CLAIMS_ACTIVATION_SIGNING_DIGEST must be one exact lowercase digest");
    }
    if (!/^\d+$/.test(env.CLAIMS_DEPLOYMENT_BLOCK ?? "")) {
      throw new Error("CLAIMS_DEPLOYMENT_BLOCK must be the canonical deployment block");
    }
    if (!claimsActivationEvidencePath?.startsWith("/") || !claimsAuthorityPolicyPath?.startsWith("/")) {
      throw new Error("Claims activation and authority policy paths must be explicit absolute paths");
    }
    if (nodeEnv === "production"
        && (claimsAddress !== RELEASED_CLAIMS_ACTIVATION.contractAddress
          || claimsActivationDigest !== RELEASED_CLAIMS_ACTIVATION.signingDigest
          || BigInt(env.CLAIMS_DEPLOYMENT_BLOCK!) !== RELEASED_CLAIMS_ACTIVATION.deploymentBlock)) {
      throw new Error("production Claims coordinates differ from the released signed activation");
    }
  }
  if (claimsIndexerEnabled && (!registryEnabled || !claimsConfigured)) {
    throw new Error("Claims indexing requires the Registry and complete Claims activation coordinates");
  }
  if (claimsRelayerEnabled) {
    if (!claimsIndexerEnabled) throw new Error("Claims relay requires canonical Claims indexing first");
    if (!/^0x[0-9a-f]{64}$/.test(env.CLAIMS_RELAYER_PRIVATE_KEY ?? "")) {
      throw new Error("CLAIMS_RELAYER_PRIVATE_KEY must be one dedicated lowercase private key");
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
    managed: {
      enabled: managedEnabled,
      provider: managedProvider,
      root: managedRoot,
      stripeSecretKey: env.STRIPE_SECRET_KEY,
      stripeWebhookSecret: env.STRIPE_WEBHOOK_SECRET,
      priceIds: {
        usd_10: env.STRIPE_BALANCE_PRICE_10,
        usd_25: env.STRIPE_BALANCE_PRICE_25,
        usd_50: env.STRIPE_BALANCE_PRICE_50,
      },
      checkoutSuccessUrl: env.MANAGED_CHECKOUT_SUCCESS_URL,
      checkoutCancelUrl: env.MANAGED_CHECKOUT_CANCEL_URL,
      bankrBaseUrl: env.BANKR_BASE_URL ?? "https://llm.bankr.bot",
      bankrApiKey: env.BANKR_API_KEY,
      modelAllowlist,
      rateLimitPerMinute: parsePositiveInteger("MANAGED_RATE_LIMIT_PER_MINUTE", env.MANAGED_RATE_LIMIT_PER_MINUTE, 30),
      operatorTokenSha256: env.TOHSENO_OPERATOR_TOKEN_SHA256,
      launchFeeFundingConfirmed,
      launchFeeFundingReference: env.BANKR_LAUNCH_FEE_FUNDING_REFERENCE,
    },
    distribution: {
      macosEnabled,
      macosChannel,
      macosUrl: env.MACOS_DOWNLOAD_URL,
      macosSha256: env.MACOS_DOWNLOAD_SHA256,
    },
    registry: {
      enabled: registryEnabled,
      root: registryRoot,
      rpcUrl: registryRpcUrl,
      chainId: 4663,
      factoryAddress: "0xb1bd208cd2af98e701f43d06aaa889d3a594df65",
      registryAddress: "0x3fe6508ba2660bc575080024f402c192a2e035a0",
      activationSigningDigest: "0x2b640260595def403343810d0dc4ee231e1faff427581be4f7b40cff4c189d28",
      globalRequestsPerMinute: parsePositiveInteger("REGISTRY_GLOBAL_RATE", env.REGISTRY_GLOBAL_RATE, 1_200),
      sourceRequestsPerMinute: parsePositiveInteger("REGISTRY_SOURCE_RATE", env.REGISTRY_SOURCE_RATE, 120),
      maxStagingRecords: parsePositiveInteger("REGISTRY_MAX_STAGING_RECORDS", env.REGISTRY_MAX_STAGING_RECORDS, 1_000),
      maxStagingBytes: parsePositiveInteger("REGISTRY_MAX_STAGING_BYTES", env.REGISTRY_MAX_STAGING_BYTES, 10 * 1024 * 1024 * 1024),
      relayerEnabled: registryRelayerEnabled,
      relayerPrivateKey: env.REGISTRY_RELAYER_PRIVATE_KEY as `0x${string}` | undefined,
    },
    claims: {
      configured: claimsConfigured,
      contractAddress: claimsAddress as `0x${string}` | undefined,
      activationSigningDigest: claimsActivationDigest as `0x${string}` | undefined,
      activationEvidencePath: claimsActivationEvidencePath,
      authorityPolicyPath: claimsAuthorityPolicyPath,
      deploymentBlock: claimsConfigured ? BigInt(env.CLAIMS_DEPLOYMENT_BLOCK!) : undefined,
      indexerEnabled: claimsIndexerEnabled,
      relayerEnabled: claimsRelayerEnabled,
      relayerPrivateKey: env.CLAIMS_RELAYER_PRIVATE_KEY as `0x${string}` | undefined,
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
    managedComputeEnabled: config.managed.enabled,
    managedComputeProvider: config.managed.enabled ? config.managed.provider : "disabled",
    macosDownloadEnabled: config.distribution.macosEnabled,
    macosDownloadChannel: config.distribution.macosChannel,
    registryEnabled: config.registry.enabled,
    registryRelayerEnabled: config.registry.relayerEnabled,
  };
}
