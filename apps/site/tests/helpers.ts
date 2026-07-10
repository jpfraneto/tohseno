import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { AppConfig } from "../config.ts";
import { createApplication } from "../server.ts";
import type { TohsenoApplication } from "../server.ts";
import { openMigratedDatabase } from "../src/database.ts";
import type { EmailMessage, EmailProvider } from "../src/email.ts";
import { createPaymentProvider } from "../src/payments.ts";
import type {
  CheckoutResult,
  PaymentProvider,
  VerifiedPaymentEvent,
} from "../src/payments.ts";
import type { OperatingMode } from "../src/state-machine.ts";
import type { SubmissionRow } from "../src/submissions.ts";

const TEST_DATA_KEY = Buffer.alloc(32, 29).toString("base64");
const TEST_OPERATOR_TOKEN = "test-operator-token-with-at-least-thirty-two-bytes";

export function testConfig(overrides: Partial<AppConfig> = {}): AppConfig {
  return {
    nodeEnv: "test",
    port: 3000,
    baseUrl: "https://tohseno.test",
    databasePath: ":memory:",
    trustProxy: false,
    dataKeyBase64: TEST_DATA_KEY,
    operatorToken: TEST_OPERATOR_TOKEN,
    paymentsMode: "mock",
    emailMode: "disabled",
    ...overrides,
  };
}

export class FakeEmailProvider implements EmailProvider {
  readonly name = "fake" as const;
  readonly messages: EmailMessage[] = [];

  async send(message: EmailMessage): Promise<{ providerReference: string }> {
    this.messages.push(structuredClone(message));
    return { providerReference: `fake-email-${this.messages.length}` };
  }
}

export async function waitForEmailCount(
  provider: FakeEmailProvider,
  expected: number,
  timeoutMs = 1_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (provider.messages.length < expected && Date.now() < deadline) await Bun.sleep(1);
  if (provider.messages.length < expected) {
    throw new Error(`Timed out waiting for ${expected} email deliveries; observed ${provider.messages.length}`);
  }
}

export class VerifiedFakePaymentProvider implements PaymentProvider {
  readonly name = "stripe" as const;
  readonly checkouts: Array<Pick<SubmissionRow, "id" | "operating_mode">> = [];
  readonly webhookBodies: string[] = [];
  readonly expectedSignature: string;
  private checkoutSequence = 0;
  private readonly checkoutDetails = new Map<string, {
    submissionId: string;
    operatingMode: OperatingMode;
    amount: number;
    currency: string;
  }>();

  constructor(expectedSignature = "verified-test-signature") {
    this.expectedSignature = expectedSignature;
  }

  availability(mode: OperatingMode): { available: boolean; reason?: string } {
    return mode === "anky-operated"
      ? { available: false, reason: "Anky-operated applications do not use automatic Checkout." }
      : { available: true };
  }

  async createCheckout(
    submission: Pick<SubmissionRow, "id" | "operating_mode">,
    _checkoutAttempt: number,
  ): Promise<CheckoutResult> {
    this.checkouts.push({ ...submission });
    this.checkoutSequence += 1;
    const checkoutSessionId = `cs_test_${this.checkoutSequence}`;
    const amount = submission.operating_mode === "self-hosted" ? 8_800 : 97_600;
    this.checkoutDetails.set(checkoutSessionId, {
      submissionId: submission.id,
      operatingMode: submission.operating_mode,
      amount,
      currency: "usd",
    });
    return {
      checkoutSessionId,
      url: `https://checkout.example.test/${checkoutSessionId}`,
      amount,
      currency: "usd",
    };
  }

  async verifyWebhook(
    rawBody: string,
    signature: string | null,
  ): Promise<VerifiedPaymentEvent> {
    if (signature !== this.expectedSignature) throw new Error("Invalid signature");
    this.webhookBodies.push(rawBody);
    const payload = JSON.parse(rawBody) as {
      eventId: string;
      checkoutSessionId: string;
      type?: string;
      outcome?: VerifiedPaymentEvent["outcome"];
      amountTotal?: number;
      currency?: string;
      submissionId?: string;
      submissionReferenceValid?: boolean;
      checkoutMode?: string;
    };
    const details = this.checkoutDetails.get(payload.checkoutSessionId);
    if (!details) throw new Error("Unknown fake Checkout session");
    const type = payload.type ?? "checkout.session.completed";
    const defaultOutcome: VerifiedPaymentEvent["outcome"] = type === "checkout.session.expired"
      ? "expired"
      : type === "checkout.session.async_payment_failed"
        ? "failed"
        : "paid";
    return {
      provider: "stripe",
      eventId: payload.eventId,
      type,
      outcome: payload.outcome ?? defaultOutcome,
      checkoutSessionId: payload.checkoutSessionId,
      submissionId: payload.submissionId ?? details.submissionId,
      submissionReferenceValid: payload.submissionReferenceValid ?? true,
      checkoutMode: payload.checkoutMode ?? (details.operatingMode === "self-hosted" ? "payment" : "subscription"),
      amountTotal: payload.amountTotal ?? details.amount,
      currency: payload.currency ?? details.currency,
    };
  }
}

export interface SiteHarness {
  application: TohsenoApplication;
  config: AppConfig;
  databasePath: string;
  directory: string;
  emailProvider: FakeEmailProvider;
  paymentProvider: PaymentProvider;
  request(path: string, init?: RequestInit): Promise<Response>;
  persistedBytes(): Buffer;
  close(): Promise<void>;
}

export async function createSiteHarness(options: {
  config?: Partial<AppConfig>;
  paymentProvider?: PaymentProvider;
  emailProvider?: FakeEmailProvider;
} = {}): Promise<SiteHarness> {
  const directory = mkdtempSync(join(tmpdir(), "tohseno-site-test-"));
  const databasePath = join(directory, "tohseno.sqlite");
  const config = testConfig({ databasePath, ...options.config });
  const database = openMigratedDatabase(databasePath);
  const paymentProvider = options.paymentProvider ?? createPaymentProvider(config);
  const emailProvider = options.emailProvider ?? new FakeEmailProvider();
  const application = await createApplication({
    config,
    database,
    paymentProvider,
    emailProvider,
  });
  await Bun.sleep(0);

  let closed = false;
  return {
    application,
    config,
    databasePath,
    directory,
    emailProvider,
    paymentProvider,
    request(path: string, init?: RequestInit): Promise<Response> {
      return application.fetch(new Request(new URL(path, config.baseUrl), init));
    },
    persistedBytes(): Buffer {
      database.exec("PRAGMA wal_checkpoint(FULL)");
      const files = readdirSync(directory)
        .filter((name) => name.startsWith("tohseno.sqlite"))
        .sort()
        .map((name) => readFileSync(join(directory, name)));
      return Buffer.concat(files);
    },
    async close(): Promise<void> {
      if (closed) return;
      closed = true;
      await application.close();
      database.close();
      rmSync(directory, { recursive: true, force: true });
    },
  };
}

export interface HttpSubmission {
  submissionId: string;
  status: string;
  statusUrl: string;
  token: string;
  expiresAt: string;
}

export function syntheticMarkdown(label = "practice"): string {
  return `# ${label}\n\nThe person records one deliberate observation, reflects briefly, and returns tomorrow.`;
}

export async function submitThroughHttp(
  harness: SiteHarness,
  operatingMode: OperatingMode = "self-hosted",
  overrides: { markdown?: string; email?: string } = {},
): Promise<HttpSubmission> {
  const response = await harness.request("/api/submissions", {
    method: "POST",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      markdown: overrides.markdown ?? syntheticMarkdown(operatingMode),
      email: overrides.email ?? `${operatingMode}@example.test`,
      operatingMode,
    }),
  });
  if (response.status !== 201) {
    throw new Error(`Submission failed with ${response.status}: ${await response.text()}`);
  }
  const body = await response.json() as Omit<HttpSubmission, "token">;
  const token = new URL(body.statusUrl).pathname.split("/").at(-1);
  if (!token) throw new Error("Status URL did not contain a capability token");
  return { ...body, token };
}

export async function beginHttpCheckout(
  harness: SiteHarness,
  token: string,
): Promise<string> {
  const response = await harness.request("/api/checkout", {
    method: "POST",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ token }),
  });
  if (response.status !== 201) {
    throw new Error(`Checkout failed with ${response.status}: ${await response.text()}`);
  }
  const body = await response.json() as { checkoutUrl: string };
  const checkoutSessionId = new URL(body.checkoutUrl).pathname.split("/").at(-1);
  if (!checkoutSessionId) throw new Error("Checkout URL did not contain a session ID");
  return checkoutSessionId;
}

export async function completeMockCheckout(
  harness: SiteHarness,
  checkoutSessionId: string,
): Promise<{ processed: boolean; finalStatus?: string }> {
  const response = await harness.request("/api/payments/mock/complete", {
    method: "POST",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ checkoutSessionId }),
  });
  if (response.status !== 200) {
    throw new Error(`Mock completion failed with ${response.status}: ${await response.text()}`);
  }
  return response.json() as Promise<{ processed: boolean; finalStatus?: string }>;
}
