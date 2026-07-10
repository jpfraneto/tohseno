import { describe, expect, test } from "bun:test";
import type Stripe from "stripe";
import { stripeCheckoutSessionMismatch } from "../src/payments.ts";
import { testConfig } from "./helpers.ts";

type CheckedSession = Parameters<typeof stripeCheckoutSessionMismatch>[0];

function price(
  id: string,
  amount: number,
  type: "one_time" | "recurring",
  recurring: Stripe.Price.Recurring | null,
): Stripe.Price {
  return {
    id,
    active: true,
    billing_scheme: "per_unit",
    currency: "usd",
    custom_unit_amount: null,
    recurring,
    type,
    unit_amount: amount,
  } as unknown as Stripe.Price;
}

function session(
  submissionId: string,
  mode: "payment" | "subscription",
  amount: number,
  prices: Array<Stripe.Price | string>,
): CheckedSession {
  return {
    amount_total: amount,
    currency: "usd",
    mode,
    client_reference_id: submissionId,
    metadata: { submission_id: submissionId },
    line_items: {
      data: prices.map((entry) => ({ price: entry, quantity: 1 } as unknown as Stripe.LineItem)),
    } as Stripe.ApiList<Stripe.LineItem>,
  };
}

const config = testConfig({
  paymentsMode: "stripe",
  stripeSelfHostedPriceId: "price_self",
  stripeClientSetupPriceId: "price_setup",
  stripeClientMonthlyPriceId: "price_monthly",
});

const monthlyLicensed: Stripe.Price.Recurring = {
  interval: "month",
  interval_count: 1,
  meter: null,
  trial_period_days: null,
  usage_type: "licensed",
};

describe("Stripe expanded Price reconciliation", () => {
  test("accepts the exact self-hosted one-time Price", () => {
    const value = session("sub_self", "payment", 8_800, [
      price("price_self", 8_800, "one_time", null),
    ]);
    expect(stripeCheckoutSessionMismatch(value, {
      id: "sub_self",
      operating_mode: "self-hosted",
    }, config)).toEqual([]);
  });

  test("accepts exact client setup and licensed monthly Prices", () => {
    const value = session("sub_client", "subscription", 97_600, [
      price("price_setup", 88_800, "one_time", null),
      price("price_monthly", 8_800, "recurring", monthlyLicensed),
    ]);
    expect(stripeCheckoutSessionMismatch(value, {
      id: "sub_client",
      operating_mode: "client-owned",
    }, config)).toEqual([]);
  });

  test("rejects annual, metered, wrong-amount, and unexpanded Prices", () => {
    const annualMetered: Stripe.Price.Recurring = {
      ...monthlyLicensed,
      interval: "year",
      usage_type: "metered",
    };
    const value = session("sub_client_bad", "subscription", 97_600, [
      price("price_setup", 88_801, "one_time", null),
      price("price_monthly", 8_800, "recurring", annualMetered),
      "price_not_expanded",
    ]);
    const mismatches = stripeCheckoutSessionMismatch(value, {
      id: "sub_client_bad",
      operating_mode: "client-owned",
    }, config);
    expect(mismatches).toContain("price:price_setup:amount");
    expect(mismatches).toContain("price:price_monthly:interval");
    expect(mismatches).toContain("price:price_monthly:usage-type");
    expect(mismatches).toContain("expanded-price");
    expect(mismatches).toContain("line-item-count");
  });
});
