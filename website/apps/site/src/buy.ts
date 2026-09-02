import { HttpError, withSecurityHeaders } from "./security.ts";

export const TOHSENO_TOKEN = "0x364415f884fc93775a4c1825c1a3af1f0c2d8ba3" as const;
export const ROBINHOOD_CHAIN_ID = 4663 as const;
export const TOHSENO_POOL_ID = "0x0b2b50f640fc821024a000f441b0e5c97db5f48e326aaf8472f21295079d7be9" as const;

const RELAY_ORIGIN = "https://api.relay.link";
const DEXSCREENER_MARKET_URL = `${RELAY_ORIGIN.replace("api.relay.link", "api.dexscreener.com")}/token-pairs/v1/robinhood/${TOHSENO_TOKEN}`;
const ADDRESS = /^0x[0-9a-f]{40}$/;
const REQUEST_ID = /^0x[0-9a-f]{64}$/;
const UINT = /^\d{1,78}$/;
const HEX_DATA = /^0x(?:[0-9a-f]{2})*$/;
const NATIVE_TOKEN = "0x0000000000000000000000000000000000000000";
const MAX_UINT256 = (1n << 256n) - 1n;
const CHAIN_CACHE_MS = 5 * 60_000;
const MARKET_CACHE_MS = 30_000;

type JsonObject = Record<string, unknown>;
type FetchLike = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

interface BuyCurrency {
  address: `0x${string}`;
  symbol: string;
  name: string;
  decimals: number;
  isNative: boolean;
}

interface BuyChain {
  id: number;
  name: string;
  rpcUrl: string;
  explorerUrl: string;
  currency: BuyCurrency;
  currencies: BuyCurrency[];
  depository?: `0x${string}`;
}

interface Cached<T> {
  at: number;
  value: T;
}

export interface BuyRouter {
  handles(pathname: string): boolean;
  fetch(request: Request): Promise<Response>;
}

class UpstreamError extends Error {
  constructor(readonly status: number, message: string) {
    super(message);
  }
}

function record(value: unknown): JsonObject | undefined {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as JsonObject
    : undefined;
}

function cleanText(value: unknown, maximum = 80): string | undefined {
  if (typeof value !== "string") return undefined;
  const cleaned = value.replace(/[\u0000-\u001f\u007f]/g, "").trim();
  return cleaned && cleaned.length <= maximum ? cleaned : undefined;
}

function cleanURL(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  try {
    const parsed = new URL(value);
    if (parsed.protocol !== "https:") return undefined;
    parsed.username = "";
    parsed.password = "";
    return parsed.toString().replace(/\/$/, "");
  } catch {
    return undefined;
  }
}

function cleanAddress(value: unknown): `0x${string}` | undefined {
  if (typeof value !== "string") return undefined;
  const normalized = value.toLowerCase();
  return ADDRESS.test(normalized) ? normalized as `0x${string}` : undefined;
}

function cleanCurrency(value: unknown, native = false): BuyCurrency | undefined {
  const candidate = record(value);
  if (!candidate) return undefined;
  const address = cleanAddress(candidate.address);
  const symbol = cleanText(candidate.symbol, 16);
  const name = cleanText(candidate.name, 64);
  const decimals = candidate.decimals;
  if (!address || !symbol || !name || !Number.isInteger(decimals) || (decimals as number) < 0 || (decimals as number) > 36) {
    return undefined;
  }
  return {
    address,
    symbol,
    name,
    decimals: decimals as number,
    isNative: native || address === NATIVE_TOKEN,
  };
}

function cleanChain(value: unknown): BuyChain | undefined {
  const candidate = record(value);
  if (!candidate || candidate.vmType !== "evm" || candidate.disabled !== false || candidate.depositEnabled !== true) {
    return undefined;
  }
  const id = candidate.id;
  const name = cleanText(candidate.displayName ?? candidate.name, 48);
  const rpcUrl = cleanURL(candidate.httpRpcUrl);
  const explorerUrl = cleanURL(candidate.explorerUrl);
  const currency = cleanCurrency(candidate.currency, true);
  if (!Number.isSafeInteger(id) || (id as number) < 1 || !name || !rpcUrl || !explorerUrl || !currency) {
    return undefined;
  }
  const currencies = new Map<string, BuyCurrency>([[currency.address, currency]]);
  if (Array.isArray(candidate.featuredTokens)) {
    for (const token of candidate.featuredTokens) {
      const cleaned = cleanCurrency(token);
      if (cleaned && !currencies.has(cleaned.address) && currencies.size < 8) {
        currencies.set(cleaned.address, cleaned);
      }
    }
  }
  const protocol = record(candidate.protocol);
  const v2 = record(protocol?.v2);
  return {
    id: id as number,
    name,
    rpcUrl,
    explorerUrl,
    currency,
    currencies: [...currencies.values()],
    depository: cleanAddress(v2?.depository),
  };
}

function json(data: unknown, status = 200, cacheControl = "no-store"): Response {
  return withSecurityHeaders(new Response(JSON.stringify(data), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": cacheControl,
    },
  }));
}

function methodNotAllowed(allow: string): Response {
  const response = json({ error: "Method not allowed" }, 405);
  const headers = new Headers(response.headers);
  headers.set("allow", allow);
  return new Response(response.body, { status: response.status, headers });
}

async function upstreamJSON(fetchFn: FetchLike, url: string, init?: RequestInit): Promise<unknown> {
  let response: Response;
  try {
    response = await fetchFn(url, {
      ...init,
      signal: AbortSignal.timeout(8_000),
      headers: {
        accept: "application/json",
        ...init?.headers,
      },
    });
  } catch {
    throw new UpstreamError(503, "The route provider is temporarily unavailable.");
  }
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    throw new UpstreamError(502, "The route provider returned an invalid response.");
  }
  if (!response.ok) {
    const message = cleanText(record(body)?.message, 180)
      ?? cleanText(record(body)?.error, 180)
      ?? "No executable route was found for that amount.";
    throw new UpstreamError(response.status >= 500 ? 503 : 422, message);
  }
  return body;
}

function publicChain(chain: BuyChain): Omit<BuyChain, "depository"> {
  const { depository: _depository, ...value } = chain;
  return value;
}

function cleanAmount(value: unknown): string {
  if (typeof value !== "string" || !UINT.test(value)) {
    throw new HttpError(400, "amount must be a positive base-unit integer");
  }
  const amount = BigInt(value);
  if (amount < 1n || amount > MAX_UINT256) {
    throw new HttpError(400, "amount is outside the supported range");
  }
  return value;
}

function cleanQuoteCurrency(value: unknown): JsonObject {
  const candidate = record(value);
  const currency = record(candidate?.currency);
  const address = cleanAddress(currency?.address);
  const chainId = currency?.chainId;
  const symbol = cleanText(currency?.symbol, 16);
  const name = cleanText(currency?.name, 64);
  const decimals = currency?.decimals;
  if (!candidate || !address || !Number.isSafeInteger(chainId) || !symbol || !name || !Number.isInteger(decimals)) {
    throw new UpstreamError(502, "The route provider returned invalid currency details.");
  }
  return {
    currency: { address, chainId, symbol, name, decimals },
    amount: cleanText(candidate.amount, 80),
    amountFormatted: cleanText(candidate.amountFormatted, 80),
    amountUsd: cleanText(candidate.amountUsd, 80),
    minimumAmount: cleanText(candidate.minimumAmount, 80),
  };
}

function cleanFees(value: unknown): JsonObject {
  const output: JsonObject = {};
  const fees = record(value);
  if (!fees) return output;
  for (const [key, rawFee] of Object.entries(fees)) {
    if (!/^[a-zA-Z][a-zA-Z0-9]{0,31}$/.test(key)) continue;
    const fee = record(rawFee);
    const currency = record(fee?.currency);
    if (!fee || !currency) continue;
    output[key] = {
      amountFormatted: cleanText(fee.amountFormatted, 80),
      amountUsd: cleanText(fee.amountUsd, 80),
      symbol: cleanText(currency.symbol, 16),
    };
  }
  return output;
}

function cleanPercent(value: unknown): JsonObject {
  const candidate = record(value);
  return {
    usd: cleanText(candidate?.usd, 80),
    percent: cleanText(candidate?.percent, 32),
  };
}

function cleanQuote(
  raw: unknown,
  user: `0x${string}`,
  chain: BuyChain,
  originCurrency: `0x${string}`,
  amount: string,
): JsonObject {
  const quote = record(raw);
  const details = record(quote?.details);
  const currencyIn = cleanQuoteCurrency(details?.currencyIn);
  const currencyOut = cleanQuoteCurrency(details?.currencyOut);
  const inputCurrency = record(currencyIn.currency)!;
  const outputCurrency = record(currencyOut.currency)!;
  if (inputCurrency.chainId !== chain.id || inputCurrency.address !== originCurrency) {
    throw new UpstreamError(502, "The route provider changed the input asset.");
  }
  if (outputCurrency.chainId !== ROBINHOOD_CHAIN_ID || outputCurrency.address !== TOHSENO_TOKEN) {
    throw new UpstreamError(502, "The route provider changed the destination asset.");
  }
  if (!Array.isArray(quote?.steps) || quote.steps.length < 1 || quote.steps.length > 4) {
    throw new UpstreamError(502, "The route provider returned an unsupported execution plan.");
  }
  const allowedStepIds = new Set(["approve", "approval", "deposit", "swap"]);
  const steps = quote.steps.map((rawStep): JsonObject => {
    const step = record(rawStep);
    const id = cleanText(step?.id, 24);
    const requestId = typeof step?.requestId === "string" && REQUEST_ID.test(step.requestId.toLowerCase())
      ? step.requestId.toLowerCase()
      : undefined;
    if (!step || !id || !allowedStepIds.has(id) || step.kind !== "transaction" || !Array.isArray(step.items) || step.items.length < 1 || step.items.length > 2) {
      throw new UpstreamError(502, "The route requires a wallet action this interface does not support.");
    }
    const items = step.items.map((rawItem): JsonObject => {
      const item = record(rawItem);
      const data = record(item?.data);
      const from = cleanAddress(data?.from);
      const to = cleanAddress(data?.to);
      const calldata = typeof data?.data === "string" ? data.data.toLowerCase() : "";
      const value = typeof data?.value === "string" ? data.value : "";
      if (!item || !data || from !== user || !to || data.chainId !== chain.id || !HEX_DATA.test(calldata) || calldata.length > 200_002 || !UINT.test(value)) {
        throw new UpstreamError(502, "The route provider returned invalid transaction data.");
      }
      if (id === "approve") {
        if (to !== originCurrency || calldata.length !== 138 || !calldata.startsWith("0x095ea7b3")) {
          throw new UpstreamError(502, "The route provider returned an unexpected token approval.");
        }
        const approved = BigInt(`0x${calldata.slice(-64)}`);
        if (approved > BigInt(amount)) {
          throw new UpstreamError(502, "The route requested an approval larger than the purchase amount.");
        }
      }
      const check = record(item.check);
      const endpoint = cleanText(check?.endpoint, 180);
      return {
        status: item.status === "complete" ? "complete" : "incomplete",
        data: { from, to, data: calldata, value, chainId: chain.id },
        check: endpoint && /^\/intents\/status\/v3\?requestId=0x[0-9a-f]{64}$/.test(endpoint)
          ? { endpoint, method: "GET" }
          : undefined,
      };
    });
    return {
      id,
      action: cleanText(step.action, 100) ?? "Confirm in your wallet",
      description: cleanText(step.description, 180) ?? "Confirm this route step",
      kind: "transaction",
      requestId,
      items,
    };
  });
  const route = record(details?.route);
  const destinationRoute = record(route?.destination);
  return {
    details: {
      currencyIn,
      currencyOut,
      totalImpact: cleanPercent(details?.totalImpact),
      swapImpact: cleanPercent(details?.swapImpact),
      timeEstimate: typeof details?.timeEstimate === "number" ? Math.max(0, Math.round(details.timeEstimate)) : undefined,
      route: {
        bridge: "Relay",
        destination: cleanText(destinationRoute?.router, 40) ?? "onchain liquidity",
      },
    },
    fees: cleanFees(quote?.fees),
    steps,
  };
}

function rateLimiter(now: () => number): () => boolean {
  let minute = Math.floor(now() / 60_000);
  let requests = 0;
  return () => {
    const currentMinute = Math.floor(now() / 60_000);
    if (currentMinute !== minute) {
      minute = currentMinute;
      requests = 0;
    }
    requests += 1;
    return requests <= 120;
  };
}

export function createBuyRouter(
  fetchFn: FetchLike = fetch,
  now: () => number = Date.now,
): BuyRouter {
  let chainCache: Cached<BuyChain[]> | undefined;
  let marketCache: Cached<JsonObject | null> | undefined;
  const admitQuote = rateLimiter(now);

  const chains = async (): Promise<BuyChain[]> => {
    if (chainCache && now() - chainCache.at < CHAIN_CACHE_MS) return chainCache.value;
    const raw = await upstreamJSON(fetchFn, `${RELAY_ORIGIN}/chains`);
    const values = record(raw)?.chains;
    if (!Array.isArray(values)) throw new UpstreamError(502, "The route provider returned an invalid chain list.");
    const cleaned = values.map(cleanChain).filter((value): value is BuyChain => value !== undefined);
    if (!cleaned.some((chain) => chain.id === ROBINHOOD_CHAIN_ID)) {
      throw new UpstreamError(503, "Robinhood Chain is not currently available through the route provider.");
    }
    cleaned.sort((left, right) => left.name.localeCompare(right.name));
    chainCache = { at: now(), value: cleaned };
    return cleaned;
  };

  const market = async (): Promise<JsonObject | null> => {
    if (marketCache && now() - marketCache.at < MARKET_CACHE_MS) return marketCache.value;
    try {
      const raw = await upstreamJSON(fetchFn, DEXSCREENER_MARKET_URL);
      const pair = Array.isArray(raw)
        ? raw.map(record).find((value) => cleanText(value?.pairAddress, 80)?.toLowerCase() === TOHSENO_POOL_ID)
        : undefined;
      const liquidity = record(pair?.liquidity);
      const volume = record(pair?.volume);
      const change = record(pair?.priceChange);
      const cleaned = pair ? {
        priceUsd: cleanText(pair.priceUsd, 40),
        liquidityUsd: typeof liquidity?.usd === "number" ? liquidity.usd : undefined,
        volume24hUsd: typeof volume?.h24 === "number" ? volume.h24 : undefined,
        change24hPercent: typeof change?.h24 === "number" ? change.h24 : undefined,
        dex: cleanText(pair.dexId, 32),
        pair: TOHSENO_POOL_ID,
      } : null;
      marketCache = { at: now(), value: cleaned };
      return cleaned;
    } catch {
      return marketCache?.value ?? null;
    }
  };

  return {
    handles(pathname: string): boolean {
      return pathname.startsWith("/api/buy/v1/");
    },

    async fetch(request: Request): Promise<Response> {
      const url = new URL(request.url);
      try {
        if (url.pathname === "/api/buy/v1/config") {
          if (request.method !== "GET" && request.method !== "HEAD") return methodNotAllowed("GET, HEAD");
          const [availableChains, currentMarket] = await Promise.all([chains(), market()]);
          const response = json({
            schema: "tohseno.buy-config/1",
            token: {
              chainId: ROBINHOOD_CHAIN_ID,
              address: TOHSENO_TOKEN,
              symbol: "TOHSENO",
              decimals: 18,
              poolId: TOHSENO_POOL_ID,
            },
            chains: availableChains.map(publicChain),
            market: currentMarket,
            routing: { provider: "Relay", destinationLiquidity: "KyberSwap / Uniswap v4" },
          }, 200, "public, max-age=20");
          return request.method === "HEAD" ? new Response(null, { status: response.status, headers: response.headers }) : response;
        }

        if (url.pathname === "/api/buy/v1/quote") {
          if (request.method !== "POST") return methodNotAllowed("POST");
          if (!admitQuote()) return json({ error: "Quote traffic is temporarily busy. Try again in a minute." }, 429);
          const contentLength = Number(request.headers.get("content-length") ?? "0");
          if (Number.isFinite(contentLength) && contentLength > 4_096) throw new HttpError(413, "Quote request is too large");
          let body: JsonObject;
          try {
            const text = await request.text();
            if (text.length > 4_096) throw new HttpError(413, "Quote request is too large");
            body = record(JSON.parse(text)) ?? {};
          } catch (error) {
            if (error instanceof HttpError) throw error;
            throw new HttpError(400, "Quote request must be valid JSON");
          }
          const user = cleanAddress(body.user);
          const originCurrency = cleanAddress(body.originCurrency);
          const originChainId = body.originChainId;
          const amount = cleanAmount(body.amount);
          if (!user) throw new HttpError(400, "user must be an EVM wallet address");
          if (!originCurrency) throw new HttpError(400, "originCurrency must be an EVM token address");
          if (!Number.isSafeInteger(originChainId)) throw new HttpError(400, "originChainId must be a supported EVM chain ID");
          const chain = (await chains()).find((candidate) => candidate.id === originChainId);
          if (!chain) throw new HttpError(400, "The connected chain is not available for routing");
          if (!chain.currencies.some((currency) => currency.address === originCurrency)) {
            throw new HttpError(400, "Choose one of the supported assets shown for this chain");
          }
          const raw = await upstreamJSON(fetchFn, `${RELAY_ORIGIN}/quote/v2`, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({
              user,
              recipient: user,
              originChainId,
              destinationChainId: ROBINHOOD_CHAIN_ID,
              originCurrency,
              destinationCurrency: TOHSENO_TOKEN,
              amount,
              tradeType: "EXACT_INPUT",
              usePermit: false,
            }),
          });
          return json({ schema: "tohseno.buy-quote/1", quote: cleanQuote(raw, user, chain, originCurrency, amount) });
        }

        if (url.pathname === "/api/buy/v1/status") {
          if (request.method !== "GET" && request.method !== "HEAD") return methodNotAllowed("GET, HEAD");
          const requestId = url.searchParams.get("requestId")?.toLowerCase();
          if (!requestId || !REQUEST_ID.test(requestId)) throw new HttpError(400, "requestId is invalid");
          const status = await upstreamJSON(fetchFn, `${RELAY_ORIGIN}/intents/status/v3?requestId=${requestId}`);
          const response = json({ schema: "tohseno.buy-status/1", status });
          return request.method === "HEAD" ? new Response(null, { status: response.status, headers: response.headers }) : response;
        }

        throw new HttpError(404, "Not found");
      } catch (error) {
        if (error instanceof HttpError) return json({ error: error.message }, error.status);
        if (error instanceof UpstreamError) return json({ error: error.message }, error.status);
        return json({ error: "The route could not be prepared" }, 500);
      }
    },
  };
}
