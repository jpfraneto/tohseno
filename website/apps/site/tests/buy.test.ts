import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createApplication } from "../server.ts";
import { loadConfig } from "../config.ts";
import {
  createBuyRouter,
  ROBINHOOD_CHAIN_ID,
  TOHSENO_POOL_ID,
  TOHSENO_TOKEN,
} from "../src/buy.ts";

const USER = "0x03508bb71268bba25ecacc8f620e01866650532c";
const USDC = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
const DEPOSITORY = "0x4cd00e387622c35bddb9b4c962c136462338bc31";
const REQUEST_ID = `0x${"17".repeat(32)}`;

function chain(id: number, name: string, currency: Record<string, unknown>) {
  return {
    id,
    name: name.toLowerCase(),
    displayName: name,
    httpRpcUrl: `https://${name.toLowerCase().replaceAll(" ", "-")}.example`,
    explorerUrl: `https://explorer.${name.toLowerCase().replaceAll(" ", "-")}.example`,
    depositEnabled: true,
    disabled: false,
    vmType: "evm",
    currency,
    featuredTokens: id === 1 ? [{ address: USDC, symbol: "USDC", name: "USD Coin", decimals: 6 }] : [],
    protocol: { v2: { depository: DEPOSITORY } },
  };
}

const chains = {
  chains: [
    chain(1, "Ethereum", {
      address: "0x0000000000000000000000000000000000000000",
      symbol: "ETH",
      name: "Ether",
      decimals: 18,
    }),
    chain(ROBINHOOD_CHAIN_ID, "Robinhood Chain", {
      address: "0x0000000000000000000000000000000000000000",
      symbol: "ETH",
      name: "Ether",
      decimals: 18,
    }),
  ],
};

function quote(approved = 10_000_000n) {
  const approvalData = `0x095ea7b3${DEPOSITORY.slice(2).padStart(64, "0")}${approved.toString(16).padStart(64, "0")}`;
  return {
    details: {
      currencyIn: {
        currency: { chainId: 1, address: USDC, symbol: "USDC", name: "USD Coin", decimals: 6 },
        amount: "10000000",
        amountFormatted: "10.0",
        amountUsd: "10.00",
        minimumAmount: "10000000",
      },
      currencyOut: {
        currency: { chainId: ROBINHOOD_CHAIN_ID, address: TOHSENO_TOKEN, symbol: "TOHSENO", name: "TOHSENO", decimals: 18 },
        amount: "30000000000000000000000000",
        amountFormatted: "30000000.0",
        amountUsd: "7.00",
        minimumAmount: "29400000000000000000000000",
      },
      totalImpact: { usd: "-3.00", percent: "-30.00" },
      swapImpact: { usd: "-0.20", percent: "-2.00" },
      timeEstimate: 2,
      route: { destination: { router: "kyberswap" } },
    },
    fees: {
      relayer: {
        currency: { symbol: "USDC" },
        amountFormatted: "2.8",
        amountUsd: "2.80",
      },
    },
    steps: [
      {
        id: "approve",
        action: "Confirm transaction in your wallet",
        description: "Sign an approval for USDC",
        kind: "transaction",
        requestId: REQUEST_ID,
        items: [{
          status: "incomplete",
          data: { from: USER, to: USDC, data: approvalData, value: "0", chainId: 1 },
        }],
      },
      {
        id: "deposit",
        action: "Confirm transaction in your wallet",
        description: "Deposit funds for TOHSENO",
        kind: "transaction",
        requestId: REQUEST_ID,
        items: [{
          status: "incomplete",
          data: { from: USER, to: DEPOSITORY, data: "0x12", value: "0", chainId: 1 },
          check: { endpoint: `/intents/status/v3?requestId=${REQUEST_ID}`, method: "GET" },
        }],
      },
    ],
  };
}

function mockFetch(currentQuote = quote()): typeof fetch {
  return (async (input, init) => {
    const url = String(input);
    if (url.endsWith("/chains")) return Response.json(chains);
    if (url.includes("api.dexscreener.com")) {
      return Response.json([{
        pairAddress: TOHSENO_POOL_ID,
        priceUsd: "0.0000002",
        liquidity: { usd: 17_000 },
        volume: { h24: 24 },
        priceChange: { h24: 1.2 },
        dexId: "uniswap",
      }]);
    }
    if (url.endsWith("/quote/v2")) {
      const sent = JSON.parse(String(init?.body));
      expect(sent).toMatchObject({
        user: USER,
        recipient: USER,
        originChainId: 1,
        destinationChainId: ROBINHOOD_CHAIN_ID,
        originCurrency: USDC,
        destinationCurrency: TOHSENO_TOKEN,
        amount: "10000000",
        usePermit: false,
      });
      return Response.json(currentQuote);
    }
    if (url.includes("/intents/status/v3")) return Response.json({ status: "success" });
    return Response.json({ message: "not found" }, { status: 404 });
  }) as typeof fetch;
}

describe("wallet-routed $TOHSENO purchase", () => {
  test("serves the wallet-routed purchase as part of the public site", async () => {
    const application = await createApplication({
      config: loadConfig({ NODE_ENV: "test", PORT: "3000", BASE_URL: "http://localhost:3000" }),
    });
    const response = await application.fetch(new Request("http://localhost:3000/buy"));
    expect(response.status).toBe(200);
    expect((await application.fetch(new Request("http://localhost:3000/buy.js"))).status).toBe(200);
    expect((await application.fetch(new Request("http://localhost:3000/buy.css"))).status).toBe(200);

    const body = await response.text();
    const style = readFileSync(fileURLToPath(new URL("../public/buy.css", import.meta.url)), "utf8");
    const script = readFileSync(fileURLToPath(new URL("../public/buy.js", import.meta.url)), "utf8");
    expect(body).toContain('<h1 id="buy-title">Buy <em>$TOHSENO.</em></h1>');
    expect(body).toContain("Never a gate to the network.");
    expect(body).toContain("You do not need $TOHSENO to build, publish, verify, Claim, or install software.");
    expect(body).toContain("Start where<br>your wallet is.");
    expect(body).toContain("Know what you are buying.");
    expect(body).toContain(TOHSENO_TOKEN);
    expect(body).not.toContain("hero-art");
    expect(body).not.toContain("buy-hero");
    expect(body).toContain("<footer");
    expect(body).toContain('src="/landing-assets/wordmark.svg"');
    expect(body).toMatch(/property="og:image" content="http:\/\/localhost:3000\/og-buy\.png\?v=[0-9a-f]{8}"/);
    expect(body).not.toMatch(/\{\{[A-Z0-9_]+\}\}/);
    expect(style).toContain("--cream: #f7f4ee");
    expect(style).toContain("--orange: #f04a13");
    expect(style).toContain("overflow-x: hidden");
    expect(style).toContain("@media (max-width: 680px)");
    expect(style).toContain(".mascot-note");
    expect(style).toContain(".swap-card");
    expect(script).toContain('querySelector("[data-edit-quote]")');
  });

  test("exposes supported EVM chains and current exact-pool market data", async () => {
    const router = createBuyRouter(mockFetch());
    const response = await router.fetch(new Request("https://tohseno.com/api/buy/v1/config"));
    expect(response.status).toBe(200);
    const body = await response.json();
    expect(body.token).toEqual({
      chainId: ROBINHOOD_CHAIN_ID,
      address: TOHSENO_TOKEN,
      symbol: "TOHSENO",
      decimals: 18,
      poolId: TOHSENO_POOL_ID,
    });
    expect(body.chains).toHaveLength(2);
    expect(body.chains[0].depository).toBeUndefined();
    expect(body.market).toMatchObject({ liquidityUsd: 17_000, pair: TOHSENO_POOL_ID });
  });

  test("pins the destination and returns only bounded wallet transactions", async () => {
    const router = createBuyRouter(mockFetch());
    const response = await router.fetch(new Request("https://tohseno.com/api/buy/v1/quote", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ user: USER, originChainId: 1, originCurrency: USDC, amount: "10000000" }),
    }));
    expect(response.status).toBe(200);
    const body = await response.json();
    expect(body.quote.details.currencyOut.currency).toMatchObject({
      chainId: ROBINHOOD_CHAIN_ID,
      address: TOHSENO_TOKEN,
    });
    expect(body.quote.steps.map((step: { id: string }) => step.id)).toEqual(["approve", "deposit"]);
    expect(body.quote.steps[0].items[0].data.to).toBe(USDC);
    expect(body.quote.steps[1].items[0].check.endpoint).toContain(REQUEST_ID);
  });

  test("rejects a route that asks for more approval than the purchase", async () => {
    const router = createBuyRouter(mockFetch(quote(10_000_001n)));
    const response = await router.fetch(new Request("https://tohseno.com/api/buy/v1/quote", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ user: USER, originChainId: 1, originCurrency: USDC, amount: "10000000" }),
    }));
    expect(response.status).toBe(502);
    expect(await response.json()).toEqual({ error: "The route requested an approval larger than the purchase amount." });
  });
});
