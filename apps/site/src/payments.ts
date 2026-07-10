import Stripe from "stripe";
import type { AppConfig } from "../config.ts";
import { PRODUCT } from "../config.ts";
import { generateOpaqueId } from "./crypto.ts";
import type { TohsenoDatabase } from "./database.ts";
import { queueSubmissionEmail } from "./email.ts";
import { appendAuditEvent, transitionOrder } from "./state-machine.ts";
import type { OperatingMode, OrderState } from "./state-machine.ts";
import type { SubmissionRow } from "./submissions.ts";

export interface CheckoutResult {
  checkoutSessionId: string;
  url: string;
  amount: number;
  currency: string;
}

export interface VerifiedPaymentEvent {
  provider: "stripe" | "mock";
  eventId: string;
  type: string;
  outcome: "paid" | "expired" | "failed" | "integrity-review" | "ignored";
  checkoutSessionId: string | null;
  submissionId: string | null;
  submissionReferenceValid: boolean;
  checkoutMode: string | null;
  amountTotal: number | null;
  currency: string | null;
}

export interface PaymentProvider {
  readonly name: "disabled" | "mock" | "stripe";
  availability(mode: OperatingMode): { available: boolean; reason?: string };
  createCheckout(submission: Pick<SubmissionRow, "id" | "operating_mode">, checkoutAttempt: number): Promise<CheckoutResult>;
  verifyWebhook(rawBody: string, signature: string | null): Promise<VerifiedPaymentEvent>;
}

export class PaymentConfigurationError extends Error {}

function currencyFor(mode: OperatingMode): string {
  if (mode === "self-hosted") return PRODUCT.prices.selfHosted.currency;
  if (mode === "client-owned") return PRODUCT.prices.clientOwned.currency;
  throw new PaymentConfigurationError("Anky-operated applications do not use Checkout");
}

export function stripeCheckoutSessionMismatch(
  session: Pick<Stripe.Checkout.Session, "amount_total" | "currency" | "mode" | "client_reference_id" | "metadata" | "line_items">,
  submission: Pick<SubmissionRow, "id" | "operating_mode">,
  config: AppConfig,
): string[] {
  const expectedAmount = submission.operating_mode === "self-hosted"
    ? PRODUCT.prices.selfHosted.amount
    : PRODUCT.prices.clientOwned.setupAmount + PRODUCT.prices.clientOwned.monthlyAmount;
  const expectedMode = submission.operating_mode === "self-hosted" ? "payment" : "subscription";
  const mismatches: string[] = [];
  if (session.amount_total !== expectedAmount) mismatches.push("amount");
  const expectedCurrency = currencyFor(submission.operating_mode);
  if (session.currency?.toLowerCase() !== expectedCurrency) mismatches.push("currency");
  if (session.mode !== expectedMode) mismatches.push("mode");
  if (session.client_reference_id !== submission.id || session.metadata?.submission_id !== submission.id) {
    mismatches.push("submission-reference");
  }
  const lines = session.line_items?.data ?? [];
  const prices = new Map<string, { price: Stripe.Price; quantity: number | null }>();
  for (const line of lines) {
    if (typeof line.price !== "object" || line.price === null || line.price.deleted) {
      mismatches.push("expanded-price");
      continue;
    }
    prices.set(line.price.id, { price: line.price, quantity: line.quantity });
  }
  const checkPrice = (
    id: string | undefined,
    amount: number,
    type: "one_time" | "recurring",
    recurringMonthly: boolean,
  ): void => {
    if (!id) {
      mismatches.push("price-id");
      return;
    }
    const line = prices.get(id);
    if (!line || line.quantity !== 1) {
      mismatches.push(`price:${id}:line-item`);
      return;
    }
    const price = line.price;
    if (!price.active) mismatches.push(`price:${id}:inactive`);
    if (price.type !== type) mismatches.push(`price:${id}:type`);
    if (price.unit_amount !== amount) mismatches.push(`price:${id}:amount`);
    if (price.currency.toLowerCase() !== expectedCurrency) mismatches.push(`price:${id}:currency`);
    if (price.billing_scheme !== "per_unit" || price.custom_unit_amount !== null) {
      mismatches.push(`price:${id}:billing-scheme`);
    }
    if (recurringMonthly) {
      if (price.recurring?.interval !== "month" || price.recurring.interval_count !== 1) {
        mismatches.push(`price:${id}:interval`);
      }
      if (price.recurring?.usage_type !== "licensed") mismatches.push(`price:${id}:usage-type`);
    }
    if (!recurringMonthly && price.recurring !== null) mismatches.push(`price:${id}:recurring`);
  };
  if (submission.operating_mode === "self-hosted") {
    checkPrice(config.stripeSelfHostedPriceId, PRODUCT.prices.selfHosted.amount, "one_time", false);
    if (lines.length !== 1) mismatches.push("line-item-count");
  } else {
    checkPrice(config.stripeClientSetupPriceId, PRODUCT.prices.clientOwned.setupAmount, "one_time", false);
    checkPrice(config.stripeClientMonthlyPriceId, PRODUCT.prices.clientOwned.monthlyAmount, "recurring", true);
    if (lines.length !== 2) mismatches.push("line-item-count");
  }
  return mismatches;
}

export class DisabledPaymentProvider implements PaymentProvider {
  readonly name = "disabled" as const;

  availability(mode: OperatingMode): { available: boolean; reason?: string } {
    return mode === "anky-operated"
      ? { available: false, reason: "Anky-operated applications do not use automatic Checkout." }
      : { available: false, reason: "Payments are currently disabled by the operator." };
  }

  async createCheckout(_submission: Pick<SubmissionRow, "id" | "operating_mode">, _checkoutAttempt: number): Promise<CheckoutResult> {
    throw new PaymentConfigurationError("Payments are currently disabled");
  }

  async verifyWebhook(_rawBody: string, _signature: string | null): Promise<VerifiedPaymentEvent> {
    throw new PaymentConfigurationError("Stripe webhooks are unavailable when payments are disabled");
  }
}

export class MockPaymentProvider implements PaymentProvider {
  readonly name = "mock" as const;

  constructor(private readonly baseUrl: string) {}

  availability(mode: OperatingMode): { available: boolean; reason?: string } {
    return mode === "anky-operated"
      ? { available: false, reason: "Anky-operated applications do not use automatic Checkout." }
      : { available: true };
  }

  async createCheckout(submission: Pick<SubmissionRow, "id" | "operating_mode">, _checkoutAttempt: number): Promise<CheckoutResult> {
    if (submission.operating_mode === "anky-operated") throw new PaymentConfigurationError("Anky-operated applications do not use Checkout");
    const session = generateOpaqueId("mock_checkout");
    const amount = submission.operating_mode === "self-hosted"
      ? PRODUCT.prices.selfHosted.amount
      : PRODUCT.prices.clientOwned.setupAmount + PRODUCT.prices.clientOwned.monthlyAmount;
    return {
      checkoutSessionId: session,
      url: `${this.baseUrl}/mock-checkout/${session}`,
      amount,
      currency: currencyFor(submission.operating_mode),
    };
  }

  async verifyWebhook(_rawBody: string, _signature: string | null): Promise<VerifiedPaymentEvent> {
    throw new PaymentConfigurationError("Mock payments use the development-only completion route");
  }
}

export function buildStripeCheckoutParams(
  config: AppConfig,
  submission: Pick<SubmissionRow, "id" | "operating_mode">,
): Stripe.Checkout.SessionCreateParams {
  const safeMetadata = { submission_id: submission.id };
  const common = {
    client_reference_id: submission.id,
    metadata: safeMetadata,
    success_url: `${config.baseUrl}/checkout/success?session_id={CHECKOUT_SESSION_ID}`,
    cancel_url: `${config.baseUrl}/checkout/cancel`,
    allow_promotion_codes: false,
    expand: ["line_items.data.price"],
  } satisfies Partial<Stripe.Checkout.SessionCreateParams>;

  if (submission.operating_mode === "self-hosted") {
    if (!config.stripeSelfHostedPriceId) throw new PaymentConfigurationError("STRIPE_SELF_HOSTED_PRICE_ID is not configured");
    return {
      ...common,
      mode: "payment",
      line_items: [{ price: config.stripeSelfHostedPriceId, quantity: 1 }],
      payment_intent_data: { metadata: safeMetadata },
    };
  }
  if (submission.operating_mode === "client-owned") {
    if (!config.stripeClientSetupPriceId || !config.stripeClientMonthlyPriceId) {
      throw new PaymentConfigurationError("STRIPE_CLIENT_SETUP_PRICE_ID and STRIPE_CLIENT_MONTHLY_PRICE_ID are required");
    }
    return {
      ...common,
      mode: "subscription",
      line_items: [
        { price: config.stripeClientSetupPriceId, quantity: 1 },
        { price: config.stripeClientMonthlyPriceId, quantity: 1 },
      ],
      subscription_data: { metadata: safeMetadata },
    };
  }
  throw new PaymentConfigurationError("Anky-operated applications do not use Checkout");
}

export class StripePaymentProvider implements PaymentProvider {
  readonly name = "stripe" as const;
  readonly #stripe: Stripe | null;

  constructor(private readonly config: AppConfig) {
    this.#stripe = config.stripeSecretKey ? new Stripe(config.stripeSecretKey) : null;
  }

  availability(mode: OperatingMode): { available: boolean; reason?: string } {
    if (mode === "anky-operated") return { available: false, reason: "Anky-operated applications do not use automatic Checkout." };
    if (!this.#stripe) return { available: false, reason: "STRIPE_SECRET_KEY is not configured." };
    try {
      buildStripeCheckoutParams(this.config, { id: "configuration-check", operating_mode: mode });
      if (!this.config.stripeWebhookSecret) return { available: false, reason: "STRIPE_WEBHOOK_SECRET is not configured." };
      return { available: true };
    } catch (error) {
      return { available: false, reason: error instanceof Error ? error.message : "Stripe price configuration is incomplete." };
    }
  }

  async createCheckout(submission: Pick<SubmissionRow, "id" | "operating_mode">, checkoutAttempt: number): Promise<CheckoutResult> {
    if (!this.#stripe) throw new PaymentConfigurationError("STRIPE_SECRET_KEY is not configured");
    const params = buildStripeCheckoutParams(this.config, submission);
    const session = await this.#stripe.checkout.sessions.create(params, {
      idempotencyKey: `tohseno-checkout-v1:${submission.id}:attempt-${checkoutAttempt}`,
    });
    if (!session.url) throw new Error("Stripe did not return a Checkout URL");
    const mismatches = stripeCheckoutSessionMismatch(session, submission, this.config);
    if (mismatches.length > 0) {
      try {
        await this.#stripe.checkout.sessions.expire(session.id);
      } catch {
        // The unsafe URL is never returned even if defensive expiration fails.
      }
      throw new PaymentConfigurationError(`Stripe Checkout configuration mismatch: ${mismatches.join(", ")}`);
    }
    const amount = submission.operating_mode === "self-hosted"
      ? PRODUCT.prices.selfHosted.amount
      : PRODUCT.prices.clientOwned.setupAmount + PRODUCT.prices.clientOwned.monthlyAmount;
    return {
      checkoutSessionId: session.id,
      url: session.url,
      amount,
      currency: currencyFor(submission.operating_mode),
    };
  }

  async verifyWebhook(rawBody: string, signature: string | null): Promise<VerifiedPaymentEvent> {
    if (!this.#stripe || !this.config.stripeWebhookSecret) {
      throw new PaymentConfigurationError("Stripe webhook verification is not configured");
    }
    if (!signature) throw new Error("Missing Stripe-Signature header");
    const event = this.#stripe.webhooks.constructEvent(rawBody, signature, this.config.stripeWebhookSecret);
    if (
      event.type !== "checkout.session.completed" &&
      event.type !== "checkout.session.async_payment_succeeded" &&
      event.type !== "checkout.session.async_payment_failed" &&
      event.type !== "checkout.session.expired"
    ) {
      return {
        provider: "stripe",
        eventId: event.id,
        type: event.type,
        outcome: "ignored",
        checkoutSessionId: null,
        submissionId: null,
        submissionReferenceValid: false,
        checkoutMode: null,
        amountTotal: null,
        currency: null,
      };
    }
    const session = event.data.object;
    const metadataSubmissionId = session.metadata?.submission_id ?? null;
    const clientReferenceId = session.client_reference_id;
    const submissionReferenceValid = metadataSubmissionId !== null &&
      clientReferenceId !== null && metadataSubmissionId === clientReferenceId;
    let outcome: VerifiedPaymentEvent["outcome"] = "ignored";
    if (event.type === "checkout.session.expired") outcome = "expired";
    else if (event.type === "checkout.session.async_payment_failed") outcome = "failed";
    else if (event.type === "checkout.session.async_payment_succeeded" || session.payment_status === "paid") outcome = "paid";
    else if (session.payment_status === "no_payment_required") outcome = "integrity-review";
    return {
      provider: "stripe",
      eventId: event.id,
      type: event.type,
      outcome,
      checkoutSessionId: session.id,
      submissionId: metadataSubmissionId,
      submissionReferenceValid,
      checkoutMode: session.mode,
      amountTotal: session.amount_total,
      currency: session.currency,
    };
  }
}

export function createPaymentProvider(config: AppConfig): PaymentProvider {
  if (config.paymentsMode === "mock") return new MockPaymentProvider(config.baseUrl);
  if (config.paymentsMode === "stripe") return new StripePaymentProvider(config);
  return new DisabledPaymentProvider();
}

export async function beginCheckout(
  database: TohsenoDatabase,
  provider: PaymentProvider,
  submission: SubmissionRow,
): Promise<CheckoutResult> {
  if (submission.operating_mode === "anky-operated") throw new PaymentConfigurationError("Anky-operated applications do not use Checkout");
  const availability = provider.availability(submission.operating_mode);
  if (!availability.available) throw new PaymentConfigurationError(availability.reason ?? "Payment configuration is unavailable");
  const active = database.query<{
    checkoutSessionId: string;
    url: string;
    amount: number;
    currency: string;
  }, [string, string]>(`
    SELECT p.checkout_session_id AS checkoutSessionId, p.checkout_url AS url,
           p.amount, p.currency
    FROM payments p JOIN submissions s ON s.id = p.submission_id
    WHERE p.submission_id = ? AND p.provider = ? AND p.status = 'pending' AND p.checkout_url IS NOT NULL
      AND s.status = 'PAYMENT_PENDING'
    ORDER BY p.attempt DESC LIMIT 1
  `).get(submission.id, provider.name);
  if (active) return active;
  if (submission.status !== "READY_FOR_PAYMENT") throw new PaymentConfigurationError("This order is not ready for payment");
  const expectedAmount = submission.operating_mode === "self-hosted"
    ? PRODUCT.prices.selfHosted.amount
    : PRODUCT.prices.clientOwned.setupAmount + PRODUCT.prices.clientOwned.monthlyAmount;
  const expectedCurrency = currencyFor(submission.operating_mode);
  const reserve = database.transaction((): { id: string; attempt: number } => {
    const current = database.query<{ status: OrderState }, [string]>(
      "SELECT status FROM submissions WHERE id = ?",
    ).get(submission.id);
    if (!current) throw new PaymentConfigurationError("Submission not found");
    if (current.status !== "READY_FOR_PAYMENT") {
      throw new PaymentConfigurationError("This order is not ready for payment");
    }
    const creating = database.query<{ id: string; attempt: number }, [string, string]>(`
      SELECT id, attempt FROM payments
      WHERE submission_id = ? AND provider = ? AND status = 'creating'
      ORDER BY attempt DESC LIMIT 1
    `).get(submission.id, provider.name);
    if (creating) return creating;
    const priorAttempt = database.query<{ maximum: number | null }, [string]>(
      "SELECT max(attempt) AS maximum FROM payments WHERE submission_id = ?",
    ).get(submission.id)?.maximum ?? 0;
    const attempt = priorAttempt + 1;
    const id = generateOpaqueId("pay");
    const now = new Date().toISOString();
    database.query(`
      INSERT INTO payments (
        id, submission_id, provider, provider_reference, checkout_session_id, checkout_url,
        attempt, amount, currency, status, idempotency_key, created_at, updated_at
      ) VALUES (?, ?, ?, NULL, ?, NULL, ?, ?, ?, 'creating', ?, ?, ?)
    `).run(
      id,
      submission.id,
      provider.name,
      `checkout_creating_${id}`,
      attempt,
      expectedAmount,
      expectedCurrency,
      `checkout:${provider.name}:${submission.id}:attempt-${attempt}`,
      now,
      now,
    );
    return { id, attempt };
  })();

  let checkout: CheckoutResult;
  try {
    checkout = await provider.createCheckout(submission, reserve.attempt);
    if (checkout.amount !== expectedAmount || checkout.currency.toLowerCase() !== expectedCurrency) {
      throw new PaymentConfigurationError("Checkout provider returned an unexpected amount or currency");
    }
  } catch (error) {
    if (error instanceof PaymentConfigurationError) {
      database.query("UPDATE payments SET status = 'failed', updated_at = ? WHERE id = ? AND status = 'creating'")
        .run(new Date().toISOString(), reserve.id);
    }
    throw error;
  }

  const persist = database.transaction(() => {
    const update = database.query(`
      UPDATE payments
      SET provider_reference = ?, checkout_session_id = ?, checkout_url = ?, status = 'pending', updated_at = ?
      WHERE id = ? AND status = 'creating'
    `).run(
      checkout.checkoutSessionId,
      checkout.checkoutSessionId,
      checkout.url,
      new Date().toISOString(),
      reserve.id,
    );
    if (update.changes !== 1) {
      const concurrent = database.query<{ status: string; checkout_session_id: string }, [string]>(
        "SELECT status, checkout_session_id FROM payments WHERE id = ?",
      ).get(reserve.id);
      if (concurrent?.status === "pending" && concurrent.checkout_session_id === checkout.checkoutSessionId) return;
      throw new Error("Checkout attempt is no longer reservable");
    }
    transitionOrder(database, submission.id, "PAYMENT_PENDING", "customer", {
      provider: provider.name,
      checkoutSessionId: checkout.checkoutSessionId,
      checkoutAttempt: reserve.attempt,
    });
  });
  persist();
  return checkout;
}

export interface PaymentConfirmation {
  processed: boolean;
  ignored?: boolean;
  requiresReview?: boolean;
  paymentAccepted?: boolean;
  deliveryReleased?: boolean;
  submissionId?: string;
  operatingMode?: OperatingMode;
  finalStatus?: string;
}

interface PaymentRow {
  id: string;
  submission_id: string;
  provider: string;
  status: string;
  attempt: number;
  amount: number;
  currency: string;
  operating_mode: OperatingMode;
  order_status: OrderState;
}

export function processVerifiedPaymentEvent(
  database: TohsenoDatabase,
  event: VerifiedPaymentEvent,
): PaymentConfirmation {
  const perform = database.transaction((): PaymentConfirmation => {
    if (event.outcome === "ignored" || !event.checkoutSessionId) {
      return { processed: false, ignored: true };
    }
    const prior = database.query<{ value: number }, [string, string]>(
      "SELECT 1 AS value FROM payment_events WHERE provider = ? AND provider_event_id = ?",
    ).get(event.provider, event.eventId);
    if (prior) {
      const existing = event.checkoutSessionId
        ? database.query<{ submission_id: string; operating_mode: OperatingMode; status: OrderState }, [string]>(`
            SELECT p.submission_id, s.operating_mode, s.status
            FROM payments p JOIN submissions s ON s.id = p.submission_id
            WHERE p.checkout_session_id = ?
          `).get(event.checkoutSessionId)
        : null;
      return existing
        ? {
            processed: false,
            submissionId: existing.submission_id,
            operatingMode: existing.operating_mode,
            finalStatus: existing.status,
          }
        : { processed: false };
    }
    const payment = database.query<PaymentRow, [string]>(`
      SELECT p.id, p.submission_id, p.provider, p.status, p.attempt, p.amount, p.currency,
             s.operating_mode, s.status AS order_status
      FROM payments p JOIN submissions s ON s.id = p.submission_id
      WHERE p.checkout_session_id = ?
    `).get(event.checkoutSessionId);
    if (!payment) return { processed: false, ignored: true };
    const now = new Date().toISOString();
    database.query(`
      INSERT INTO payment_events (provider, provider_event_id, checkout_session_id, outcome, created_at)
      VALUES (?, ?, ?, ?, ?)
    `).run(event.provider, event.eventId, event.checkoutSessionId, event.outcome, now);

    if (payment.status === "paid") {
      appendAuditEvent(database, payment.submission_id, "payment-provider", {
        action: "terminal-payment-event-after-paid",
        provider: event.provider,
        providerEventId: event.eventId,
        outcome: event.outcome,
      });
      return {
        processed: true,
        paymentAccepted: true,
        deliveryReleased: false,
        submissionId: payment.submission_id,
        operatingMode: payment.operating_mode,
        finalStatus: payment.order_status,
      };
    }

    const expectedMode = payment.operating_mode === "self-hosted" ? "payment" : "subscription";
    const integrityFailures: string[] = [];
    if (payment.provider !== event.provider) integrityFailures.push("provider");
    if (!event.submissionReferenceValid || event.submissionId !== payment.submission_id) integrityFailures.push("submission-reference");
    if (event.amountTotal !== payment.amount) integrityFailures.push("amount");
    if (event.currency?.toLowerCase() !== payment.currency.toLowerCase()) integrityFailures.push("currency");
    if (event.checkoutMode !== expectedMode) integrityFailures.push("checkout-mode");
    if (event.outcome === "integrity-review") integrityFailures.push("payment-status");

    if (integrityFailures.length > 0) {
      database.query("UPDATE payments SET status = 'requires_review', provider_reference = ?, updated_at = ? WHERE id = ?")
        .run(event.eventId, now, payment.id);
      if (payment.order_status === "PAYMENT_PENDING") {
        transitionOrder(database, payment.submission_id, "FAILED", "payment-provider", {
          provider: event.provider,
          providerEventId: event.eventId,
          integrityFailures,
        });
      } else {
        appendAuditEvent(database, payment.submission_id, "payment-provider", {
          action: "payment-integrity-review",
          provider: event.provider,
          providerEventId: event.eventId,
          integrityFailures,
        });
      }
      return {
        processed: true,
        requiresReview: true,
        paymentAccepted: false,
        deliveryReleased: false,
        submissionId: payment.submission_id,
        operatingMode: payment.operating_mode,
        finalStatus: payment.order_status === "PAYMENT_PENDING" ? "FAILED" : payment.order_status,
      };
    }

    if (event.outcome === "expired" || event.outcome === "failed") {
      database.query("UPDATE payments SET status = ?, provider_reference = ?, updated_at = ? WHERE id = ?")
        .run(event.outcome, event.eventId, now, payment.id);
      const latestAttempt = database.query<{ maximum: number }, [string]>(
        "SELECT max(attempt) AS maximum FROM payments WHERE submission_id = ?",
      ).get(payment.submission_id)?.maximum ?? payment.attempt;
      let finalStatus = payment.order_status;
      if (payment.order_status === "PAYMENT_PENDING" && payment.attempt === latestAttempt) {
        finalStatus = transitionOrder(database, payment.submission_id, "READY_FOR_PAYMENT", "payment-provider", {
          provider: event.provider,
          providerEventId: event.eventId,
          checkoutAttempt: payment.attempt,
          outcome: event.outcome,
        });
      }
      return {
        processed: true,
        paymentAccepted: false,
        deliveryReleased: false,
        submissionId: payment.submission_id,
        operatingMode: payment.operating_mode,
        finalStatus,
      };
    }

    database.query("UPDATE payments SET status = 'paid', provider_reference = ?, updated_at = ? WHERE id = ?")
      .run(event.eventId, now, payment.id);

    let orderStatus = payment.order_status;
    if (orderStatus === "READY_FOR_PAYMENT") {
      orderStatus = transitionOrder(database, payment.submission_id, "PAYMENT_PENDING", "payment-provider", {
        provider: event.provider,
        providerEventId: event.eventId,
        resumedByVerifiedPayment: true,
      });
    }
    if (orderStatus !== "PAYMENT_PENDING") {
      appendAuditEvent(database, payment.submission_id, "payment-provider", {
        action: "late-payment-requires-review",
        provider: event.provider,
        providerEventId: event.eventId,
        observedOrderState: orderStatus,
      });
      return {
        processed: true,
        requiresReview: true,
        paymentAccepted: true,
        deliveryReleased: false,
        submissionId: payment.submission_id,
        operatingMode: payment.operating_mode,
        finalStatus: orderStatus,
      };
    }

    transitionOrder(database, payment.submission_id, "PAID", "payment-provider", {
      provider: event.provider,
      providerEventId: event.eventId,
      amount: payment.amount,
      currency: payment.currency,
    });
    transitionOrder(database, payment.submission_id, "MANIFEST_LOCKED", "system", { manifestVersion: PRODUCT.manifestVersion });
    if (payment.operating_mode === "self-hosted") {
      transitionOrder(database, payment.submission_id, "GENERATING", "system", { output: "private-agent-capsule" });
      transitionOrder(database, payment.submission_id, "READY", "system", { readyArtifact: "capsule-and-source-contract" });
      queueSubmissionEmail(database, payment.submission_id, "payment-confirmed", `payment:${payment.id}:confirmed:v1`);
      queueSubmissionEmail(database, payment.submission_id, "self-hosted-ready", `payment:${payment.id}:ready:v1`);
      return {
        processed: true,
        paymentAccepted: true,
        deliveryReleased: true,
        submissionId: payment.submission_id,
        operatingMode: payment.operating_mode,
        finalStatus: "READY",
      };
    }
    transitionOrder(database, payment.submission_id, "NEEDS_CREDENTIALS", "system", { ownership: "client-owned" });
    queueSubmissionEmail(database, payment.submission_id, "payment-confirmed", `payment:${payment.id}:confirmed:v1`);
    queueSubmissionEmail(database, payment.submission_id, "client-credentials-required", `payment:${payment.id}:credentials:v1`);
    return {
      processed: true,
      paymentAccepted: true,
      deliveryReleased: true,
      submissionId: payment.submission_id,
      operatingMode: payment.operating_mode,
      finalStatus: "NEEDS_CREDENTIALS",
    };
  });
  return perform();
}

export function confirmCheckoutPayment(
  database: TohsenoDatabase,
  provider: "mock" | "stripe",
  providerEventId: string,
  checkoutSessionId: string,
): PaymentConfirmation {
  const payment = database.query<{
    submission_id: string;
    operating_mode: OperatingMode;
    amount: number;
    currency: string;
  }, [string]>(`
    SELECT p.submission_id, s.operating_mode, p.amount, p.currency
    FROM payments p JOIN submissions s ON s.id = p.submission_id
    WHERE p.checkout_session_id = ?
  `).get(checkoutSessionId);
  if (!payment) return { processed: false, ignored: true };
  return processVerifiedPaymentEvent(database, {
    provider,
    eventId: providerEventId,
    type: provider === "mock" ? "mock.checkout.completed" : "checkout.session.completed",
    outcome: "paid",
    checkoutSessionId,
    submissionId: payment.submission_id,
    submissionReferenceValid: true,
    checkoutMode: payment.operating_mode === "self-hosted" ? "payment" : "subscription",
    amountTotal: payment.amount,
    currency: payment.currency,
  });
}
