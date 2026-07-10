export const PRODUCT = Object.freeze({
  repositoryUrl: "https://github.com/jpfraneto/tohseno",
  skillPath: "skills/continuity-app/SKILL.md",
  manifestVersion: "0.1.0",
  maxMarkdownBytes: 256 * 1024,
  minMarkdownCharacters: 32,
  capabilityLifetimeDays: 90,
  prices: {
    selfHosted: { display: "$88 once", amount: 8_800, currency: "usd" },
    clientOwned: {
      display: "Founding price: $888 setup + $88/month",
      setupAmount: 88_800,
      monthlyAmount: 8_800,
      currency: "usd",
    },
    ankyOperated: { display: "Selective" },
  },
  copy: {
    META_DESCRIPTION: "Describe one repeated action. Receive the private contract and operating path for a continuity app.",
    BRAND: "TOHSENO",
    HEADER_NOTE: "Continuity application compiler / early product shell",
    HEADLINE_LINE_1: "Describe the action.",
    HEADLINE_LINE_2: "Receive the app.",
    INTRO_1: "Paste the Markdown document that describes the app you believe should exist.",
    INTRO_2: "TOHSENO turns one repeated action into a continuity app.",
    CYCLE_ARIA_LABEL: "The continuity cycle",
    CYCLE_ACT: "act",
    CYCLE_RECORD: "record",
    CYCLE_REFLECT: "reflect",
    CYCLE_CONTINUE: "continue",
    INTAKE_EYEBROW: "MASTER_PROMPT.md",
    MARKDOWN_LABEL: "Describe the application",
    MARKDOWN_HELP: "Paste meaningful Markdown, up to",
    MAX_MARKDOWN_UNIT: "UTF-8 bytes",
    MARKDOWN_PLACEHOLDER: "# The application I believe should exist",
    FILE_BUTTON: "Load a .md file",
    FILE_NOTE: "The file is read in this browser and fills the field above.",
    EMAIL_LABEL: "Your email",
    EMAIL_HELP: "Used only to operate this request and return its private status.",
    OPERATING_MODE_LEGEND: "Choose ownership and operation",
    OPERATING_MODE_HELP: "The source and data boundaries differ; the core product contract does not.",
    SELF_HOSTED_TITLE: "SELF-HOSTED",
    SELF_HOSTED_DESCRIPTION: "Your agent receives the private capsule, source contract, and operator instructions. You own and run everything.",
    CLIENT_TITLE: "CLIENT-OWNED",
    CLIENT_DESCRIPTION: "You own the developer accounts, source, domain, application identities, infrastructure, and data plane. TOHSENO operates the system through scoped access.",
    ANKY_TITLE: "ANKY-OPERATED",
    ANKY_DESCRIPTION: "Anky, Inc. may adopt, publish, support, and operate the application as a genuine first-party product. This is an application, not an automatic publishing service.",
    PRIMARY_BUTTON: "CREATE CONTINUITY APP",
    SUBMITTING_BUTTON: "CREATING PRIVATE INTAKE…",
    NOSCRIPT_NOTE: "JavaScript is optional when pasting Markdown. File loading and automatic redirect require it.",
    PRIVACY_STATEMENT: "Your source document is encrypted at rest. It is never published at a content-hash URL. TOHSENO does not receive the private continuity data created by the eventual users of your app.",
    PRIVACY_LINK: "Read the intake privacy notice",
    CURRENT_STATUS_LABEL: "CURRENT STATUS",
    CURRENT_STATUS_COPY: "This repository provides the product shell, manifest contract, private intake, and operator path. It does not yet generate a complete native application.",
    PAYMENT_DISABLED_NOTICE: "Private intake is open. Checkout is temporarily unavailable; no payment will be taken.",
    PAYMENT_TEST_NOTICE: "Private intake is open. Checkout is in test mode; no real payment will be taken.",
    PAYMENT_BOUNDARY_NOTICE: "Creating this private intake does not charge you. Payment is a separate secure Checkout step when it is available.",
    FOOTER_DOCTRINE: "One app. One primary action. Ejectable from birth.",
    FOOTER_OPERATOR: "Currently operated by Anky, Inc.",
  },
});

export type NodeEnvironment = "development" | "test" | "production";
export type PaymentsMode = "disabled" | "mock" | "stripe";
export type EmailMode = "disabled" | "console" | "resend";

export interface AppConfig {
  nodeEnv: NodeEnvironment;
  port: number;
  baseUrl: string;
  databasePath: string;
  trustProxy: boolean;
  dataKeyBase64: string;
  operatorToken: string;
  paymentsMode: PaymentsMode;
  stripeSecretKey?: string;
  stripeWebhookSecret?: string;
  stripeSelfHostedPriceId?: string;
  stripeClientSetupPriceId?: string;
  stripeClientMonthlyPriceId?: string;
  emailMode: EmailMode;
  resendApiKey?: string;
  emailFrom?: string;
}

type Environment = Record<string, string | undefined>;

function oneOf<T extends string>(name: string, value: string | undefined, values: readonly T[], fallback: T): T {
  const candidate = value ?? fallback;
  if (!values.includes(candidate as T)) {
    throw new Error(`${name} must be one of: ${values.join(", ")}`);
  }
  return candidate as T;
}

function optional(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function requireSecret(name: string, value: string | undefined): string {
  const secret = optional(value);
  if (secret) return secret;
  throw new Error(`${name} is required; run bun run generate-secrets and set it in the environment`);
}

function validateDataKey(value: string): void {
  let decoded: Uint8Array;
  try {
    decoded = Uint8Array.from(Buffer.from(value, "base64"));
  } catch {
    throw new Error("TOHSENO_DATA_KEY must be base64-encoded");
  }
  if (decoded.byteLength !== 32 || Buffer.from(decoded).toString("base64") !== value) {
    throw new Error("TOHSENO_DATA_KEY must be a canonical base64-encoded 32-byte key");
  }
}

export function loadConfig(env: Environment = process.env): AppConfig {
  const nodeEnv = oneOf("NODE_ENV", env.NODE_ENV, ["development", "test", "production"] as const, "development");
  const paymentsMode = oneOf("PAYMENTS_MODE", env.PAYMENTS_MODE, ["disabled", "mock", "stripe"] as const, "disabled");
  const emailMode = oneOf("EMAIL_MODE", env.EMAIL_MODE, ["disabled", "console", "resend"] as const, "disabled");
  if (nodeEnv === "production" && paymentsMode === "mock") {
    throw new Error("PAYMENTS_MODE=mock is forbidden when NODE_ENV=production");
  }
  if (nodeEnv === "production" && emailMode === "console") {
    throw new Error("EMAIL_MODE=console is forbidden when NODE_ENV=production");
  }

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
  if (env.DATABASE_PATH !== undefined && env.DATABASE_PATH.trim().length === 0) {
    throw new Error("DATABASE_PATH must not be empty");
  }

  const dataKeyBase64 = requireSecret("TOHSENO_DATA_KEY", env.TOHSENO_DATA_KEY);
  validateDataKey(dataKeyBase64);
  const operatorToken = requireSecret("TOHSENO_OPERATOR_TOKEN", env.TOHSENO_OPERATOR_TOKEN);
  if (operatorToken.length < 32) throw new Error("TOHSENO_OPERATOR_TOKEN must contain at least 32 characters");

  const config: AppConfig = {
    nodeEnv,
    port,
    baseUrl: parsedBase.origin,
    databasePath: env.DATABASE_PATH ?? "./data/tohseno.sqlite",
    trustProxy: env.TRUST_PROXY === "true",
    dataKeyBase64,
    operatorToken,
    paymentsMode,
    emailMode,
  };

  const optionalFields: Array<[keyof AppConfig, string | undefined]> = [
    ["stripeSecretKey", optional(env.STRIPE_SECRET_KEY)],
    ["stripeWebhookSecret", optional(env.STRIPE_WEBHOOK_SECRET)],
    ["stripeSelfHostedPriceId", optional(env.STRIPE_SELF_HOSTED_PRICE_ID)],
    ["stripeClientSetupPriceId", optional(env.STRIPE_CLIENT_SETUP_PRICE_ID)],
    ["stripeClientMonthlyPriceId", optional(env.STRIPE_CLIENT_MONTHLY_PRICE_ID)],
    ["resendApiKey", optional(env.RESEND_API_KEY)],
    ["emailFrom", optional(env.EMAIL_FROM)],
  ];
  for (const [key, value] of optionalFields) {
    if (value !== undefined) Object.assign(config, { [key]: value });
  }

  if (emailMode === "resend" && (!config.resendApiKey || !config.emailFrom)) {
    throw new Error("EMAIL_MODE=resend requires RESEND_API_KEY and EMAIL_FROM");
  }
  return config;
}

export function safeStartupSummary(config: AppConfig): Record<string, string | number | boolean> {
  return {
    service: "tohseno",
    environment: config.nodeEnv,
    port: config.port,
    baseUrl: config.baseUrl,
    databasePath: config.databasePath,
    trustProxy: config.trustProxy,
    paymentsMode: config.paymentsMode,
    emailMode: config.emailMode,
  };
}
