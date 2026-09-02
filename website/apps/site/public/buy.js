const TOKEN_ADDRESS = "0x364415f884fc93775a4c1825c1a3af1f0c2d8ba3";
const ADDRESS = /^0x[0-9a-f]{40}$/;
const NATIVE_TOKEN = "0x0000000000000000000000000000000000000000";

const elements = {
  connect: document.querySelector("[data-connect-wallet]"),
  walletState: document.querySelector("[data-wallet-state]"),
  form: document.querySelector("[data-swap-form]"),
  amount: document.querySelector("[data-amount]"),
  currency: document.querySelector("[data-currency]"),
  chain: document.querySelector("[data-chain]"),
  balance: document.querySelector("[data-balance]"),
  max: document.querySelector("[data-max]"),
  quoteButton: document.querySelector("[data-quote]"),
  output: document.querySelector("[data-output-amount]"),
  quotePanel: document.querySelector("[data-quote-panel]"),
  editQuote: document.querySelector("[data-edit-quote]"),
  minimum: document.querySelector("[data-minimum]"),
  route: document.querySelector("[data-route]"),
  time: document.querySelector("[data-time]"),
  impact: document.querySelector("[data-impact]"),
  impactWarning: document.querySelector("[data-impact-warning]"),
  riskWrap: document.querySelector("[data-risk-confirm-wrap]"),
  risk: document.querySelector("[data-risk-confirm]"),
  execute: document.querySelector("[data-execute]"),
  executionState: document.querySelector("[data-execution-state]"),
  walletDialog: document.querySelector("[data-wallet-dialog]"),
  walletOptions: document.querySelector("[data-wallet-options]"),
  copyContract: document.querySelector("[data-copy-contract]"),
  copyLabel: document.querySelector("[data-copy-label]"),
  marketPrice: document.querySelector("[data-market-price]"),
  marketLiquidity: document.querySelector("[data-market-liquidity]"),
  marketVolume: document.querySelector("[data-market-volume]"),
};

const state = {
  config: null,
  provider: null,
  account: null,
  chainId: null,
  balance: null,
  quote: null,
  routeTooExpensive: false,
  providers: new Map(),
};

function setText(element, value) {
  if (element) element.textContent = value;
}

function shortAddress(address) {
  return `${address.slice(0, 6)}…${address.slice(-4)}`;
}

function errorMessage(error) {
  if (error?.code === 4001) return "Nothing was signed. Your wallet rejected the request.";
  if (error?.code === -32002) return "Your wallet already has a connection request open.";
  if (error instanceof Error && error.message) return error.message;
  return "The request could not be completed.";
}

function decimalToUnits(value, decimals) {
  const normalized = value.trim();
  if (!/^(?:\d+\.?\d*|\.\d+)$/.test(normalized)) throw new Error("Enter an amount using numbers only.");
  const [whole = "0", fraction = ""] = normalized.split(".");
  if (fraction.length > decimals) throw new Error(`This asset supports at most ${decimals} decimal places.`);
  const digits = `${whole || "0"}${fraction.padEnd(decimals, "0")}`.replace(/^0+(?=\d)/, "");
  const amount = BigInt(digits || "0");
  if (amount < 1n) throw new Error("Enter an amount greater than zero.");
  return amount;
}

function unitsToDecimal(amount, decimals, maximumFraction = 6) {
  const padded = amount.toString().padStart(decimals + 1, "0");
  const whole = padded.slice(0, -decimals) || "0";
  const fraction = decimals ? padded.slice(-decimals).slice(0, maximumFraction).replace(/0+$/, "") : "";
  return fraction ? `${whole}.${fraction}` : whole;
}

function readableAmount(value, maximumFraction = 6) {
  if (!value || !/^\d+(?:\.\d+)?$/.test(value)) return "—";
  const number = Number(value);
  if (!Number.isFinite(number)) return value;
  if (number === 0) return "0";
  if (number < 0.000001) return number.toExponential(3);
  return new Intl.NumberFormat("en-US", {
    maximumFractionDigits: maximumFraction,
    maximumSignificantDigits: 9,
  }).format(number);
}

function usd(value) {
  if (typeof value !== "number" || !Number.isFinite(value)) return "—";
  if (value > 0 && value < 0.01) return "<$0.01";
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: value < 1 ? 4 : 2,
  }).format(value);
}

function currentChain() {
  return state.config?.chains.find((chain) => chain.id === state.chainId) ?? null;
}

function currentCurrency() {
  const chain = currentChain();
  return chain?.currencies.find((currency) => currency.address === elements.currency.value) ?? null;
}

function clearQuote() {
  state.quote = null;
  state.routeTooExpensive = false;
  elements.form.hidden = false;
  elements.quotePanel.hidden = true;
  elements.riskWrap.hidden = true;
  elements.risk.checked = false;
  elements.impactWarning.hidden = true;
  elements.execute.disabled = false;
  setText(elements.output, "—");
  setText(elements.executionState, "");
  elements.executionState.removeAttribute("data-state");
}

function setControls() {
  const ready = Boolean(state.account && currentChain());
  elements.amount.disabled = !ready;
  elements.currency.disabled = !ready;
  elements.chain.disabled = !state.account || !state.config;
  elements.max.disabled = !ready || state.balance === null;
  let validAmount = false;
  if (ready && currentCurrency()) {
    try {
      const amount = decimalToUnits(elements.amount.value, currentCurrency().decimals);
      validAmount = state.balance === null || amount <= state.balance;
    } catch {
      validAmount = false;
    }
  }
  elements.quoteButton.disabled = !ready || !validAmount;
}

function renderMarket() {
  const market = state.config?.market;
  if (!market) return;
  const price = Number(market.priceUsd);
  setText(elements.marketPrice, Number.isFinite(price) ? usd(price) : "—");
  setText(elements.marketLiquidity, usd(market.liquidityUsd));
  setText(elements.marketVolume, usd(market.volume24hUsd));
}

function renderChains() {
  if (!state.config) return;
  const selected = state.chainId;
  elements.chain.replaceChildren();
  const priority = new Map([[1, 0], [8453, 1], [42161, 2], [10, 3], [137, 4], [56, 5], [4663, 6]]);
  const chains = [...state.config.chains].sort((left, right) => {
    const leftRank = priority.has(left.id) ? priority.get(left.id) : 100;
    const rightRank = priority.has(right.id) ? priority.get(right.id) : 100;
    return leftRank - rightRank || left.name.localeCompare(right.name);
  });
  if (selected && !chains.some((chain) => chain.id === selected)) {
    const unsupported = document.createElement("option");
    unsupported.value = String(selected);
    unsupported.textContent = `Unsupported chain · ${selected}`;
    unsupported.disabled = true;
    elements.chain.append(unsupported);
  }
  for (const chain of chains) {
    const option = document.createElement("option");
    option.value = String(chain.id);
    option.textContent = chain.name;
    elements.chain.append(option);
  }
  if (selected) elements.chain.value = String(selected);
}

function renderCurrencies() {
  const chain = currentChain();
  const previous = elements.currency.value;
  elements.currency.replaceChildren();
  if (!chain) {
    const option = document.createElement("option");
    option.textContent = "—";
    elements.currency.append(option);
    return;
  }
  for (const currency of chain.currencies) {
    const option = document.createElement("option");
    option.value = currency.address;
    option.textContent = currency.symbol;
    elements.currency.append(option);
  }
  elements.currency.value = chain.currencies.some((currency) => currency.address === previous)
    ? previous
    : chain.currency.address;
}

async function walletRequest(method, params = []) {
  if (!state.provider) throw new Error("Connect a wallet first.");
  return state.provider.request({ method, params });
}

async function loadBalance() {
  const currency = currentCurrency();
  if (!state.account || !currency) {
    state.balance = null;
    setText(elements.balance, "Balance —");
    setControls();
    return;
  }
  try {
    let raw;
    if (currency.isNative || currency.address === NATIVE_TOKEN) {
      raw = await walletRequest("eth_getBalance", [state.account, "latest"]);
    } else {
      const accountWord = state.account.slice(2).padStart(64, "0");
      raw = await walletRequest("eth_call", [{ to: currency.address, data: `0x70a08231${accountWord}` }, "latest"]);
    }
    state.balance = BigInt(raw);
    setText(elements.balance, `Balance ${unitsToDecimal(state.balance, currency.decimals)} ${currency.symbol}`);
  } catch {
    state.balance = null;
    setText(elements.balance, "Balance unavailable");
  }
  setControls();
}

async function refreshWallet() {
  if (!state.provider) return;
  const [accounts, chainHex] = await Promise.all([
    walletRequest("eth_accounts"),
    walletRequest("eth_chainId"),
  ]);
  const account = Array.isArray(accounts) && typeof accounts[0] === "string"
    ? accounts[0].toLowerCase()
    : null;
  state.account = account && ADDRESS.test(account) ? account : null;
  state.chainId = Number.parseInt(chainHex, 16);
  clearQuote();
  renderChains();
  renderCurrencies();
  if (state.account) {
    setText(elements.connect, shortAddress(state.account));
    if (currentChain()) {
      setText(elements.walletState, `Connected on ${currentChain().name}. Quotes use the selected asset on this chain.`);
    } else {
      setText(elements.walletState, "This chain has no live route. Choose a supported source chain below.");
    }
  } else {
    setText(elements.connect, "Connect wallet");
    setText(elements.walletState, "Connect an EVM wallet to find a route.");
  }
  await loadBalance();
}

function attachProviderEvents(provider) {
  if (typeof provider?.on !== "function") return;
  provider.on("accountsChanged", () => refreshWallet().catch(() => {}));
  provider.on("chainChanged", () => refreshWallet().catch(() => {}));
  provider.on("disconnect", () => {
    state.account = null;
    clearQuote();
    setText(elements.connect, "Connect wallet");
    setText(elements.walletState, "Wallet disconnected.");
    setControls();
  });
}

async function selectProvider(entry) {
  state.provider = entry.provider;
  attachProviderEvents(entry.provider);
  localStorage.setItem("tohseno.buy.wallet", entry.info.rdns ?? entry.info.uuid);
  elements.walletDialog.close();
  try {
    const accounts = await walletRequest("eth_requestAccounts");
    if (!Array.isArray(accounts) || !accounts[0]) throw new Error("The wallet did not return an account.");
    await refreshWallet();
  } catch (error) {
    setText(elements.walletState, errorMessage(error));
  }
}

function registerProvider(detail) {
  if (!detail?.provider || typeof detail.provider.request !== "function") return;
  const info = detail.info ?? {};
  const key = info.uuid ?? info.rdns ?? `wallet-${state.providers.size}`;
  if (state.providers.has(key)) return;
  state.providers.set(key, {
    provider: detail.provider,
    info: {
      uuid: key,
      rdns: info.rdns,
      name: typeof info.name === "string" ? info.name : "Browser wallet",
      icon: typeof info.icon === "string" ? info.icon : null,
    },
  });
  renderWalletOptions();
  const remembered = localStorage.getItem("tohseno.buy.wallet");
  if (!state.provider && remembered && (remembered === info.rdns || remembered === key)) {
    state.provider = detail.provider;
    attachProviderEvents(detail.provider);
    refreshWallet().catch(() => {});
  }
}

function renderWalletOptions() {
  elements.walletOptions.replaceChildren();
  const providers = [...state.providers.values()];
  if (!providers.length) {
    const empty = document.createElement("p");
    empty.textContent = "No installed wallet was detected. Open this page in an EVM wallet browser or install a browser wallet.";
    elements.walletOptions.append(empty);
    return;
  }
  for (const entry of providers) {
    const button = document.createElement("button");
    button.type = "button";
    if (entry.info.icon) {
      const icon = document.createElement("img");
      icon.src = entry.info.icon;
      icon.alt = "";
      button.append(icon);
    }
    const name = document.createElement("span");
    name.textContent = entry.info.name;
    button.append(name);
    button.addEventListener("click", () => selectProvider(entry));
    elements.walletOptions.append(button);
  }
}

async function switchChain(chainId) {
  const chain = state.config?.chains.find((candidate) => candidate.id === chainId);
  if (!chain) throw new Error("That chain is not available for routing.");
  const hexId = `0x${chainId.toString(16)}`;
  try {
    await walletRequest("wallet_switchEthereumChain", [{ chainId: hexId }]);
  } catch (error) {
    if (error?.code !== 4902) throw error;
    await walletRequest("wallet_addEthereumChain", [{
      chainId: hexId,
      chainName: chain.name,
      rpcUrls: [chain.rpcUrl],
      blockExplorerUrls: [chain.explorerUrl],
      nativeCurrency: {
        name: chain.currency.name,
        symbol: chain.currency.symbol,
        decimals: chain.currency.decimals,
      },
    }]);
  }
  await refreshWallet();
}

async function api(path, init) {
  const response = await fetch(path, {
    ...init,
    headers: { "content-type": "application/json", ...init?.headers },
  });
  const body = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(body.error || "The route service did not respond.");
  return body;
}

function renderQuote(quote) {
  state.quote = quote;
  const details = quote.details;
  const output = details.currencyOut;
  const outputFormatted = output.amountFormatted;
  const minimumBase = output.minimumAmount;
  const decimals = output.currency.decimals;
  const minimumFormatted = minimumBase ? unitsToDecimal(BigInt(minimumBase), decimals, 6) : "—";
  setText(elements.output, `${readableAmount(outputFormatted)} TOHSENO`);
  setText(elements.minimum, `${readableAmount(minimumFormatted)} TOHSENO`);
  setText(elements.route, `${details.route.bridge} → ${details.route.destination}`);
  setText(elements.time, details.timeEstimate ? `≈ ${details.timeEstimate} sec` : "Route dependent");
  const impact = Number(details.totalImpact?.percent);
  const loss = Number.isFinite(impact) && impact < 0 ? Math.abs(impact) : 0;
  setText(elements.impact, Number.isFinite(impact) ? `${impact.toFixed(2)}%` : "—");
  elements.form.hidden = true;
  elements.quotePanel.hidden = false;
  state.routeTooExpensive = loss > 50;
  if (loss > 15) {
    elements.impactWarning.hidden = false;
    setText(elements.impactWarning, state.routeTooExpensive
      ? `This route loses about ${loss.toFixed(1)}% to execution, bridging, and price impact. It is blocked here—use a larger amount or a lower-fee source chain.`
      : `This route loses about ${loss.toFixed(1)}% to execution, bridging, and price impact. Consider another amount or source chain.`);
    elements.riskWrap.hidden = state.routeTooExpensive;
  } else {
    elements.impactWarning.hidden = true;
    elements.riskWrap.hidden = true;
  }
  elements.execute.disabled = state.routeTooExpensive || (loss > 15 && !elements.risk.checked);
}

async function fetchQuote() {
  const currency = currentCurrency();
  if (!state.account || !currentChain() || !currency) throw new Error("Connect a supported wallet and choose an asset.");
  const amount = decimalToUnits(elements.amount.value, currency.decimals);
  if (state.balance !== null && amount > state.balance) throw new Error(`Your ${currency.symbol} balance is too low for that amount.`);
  clearQuote();
  elements.quoteButton.disabled = true;
  setText(elements.quoteButton, "Checking liquidity…");
  try {
    const response = await api("/api/buy/v1/quote", {
      method: "POST",
      body: JSON.stringify({
        user: state.account,
        originChainId: state.chainId,
        originCurrency: currency.address,
        amount: amount.toString(),
      }),
    });
    renderQuote(response.quote);
  } finally {
    setText(elements.quoteButton, "Review route");
    setControls();
  }
}

function quantity(value) {
  return `0x${BigInt(value).toString(16)}`;
}

function delay(milliseconds) {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

async function waitForReceipt(hash) {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    const receipt = await walletRequest("eth_getTransactionReceipt", [hash]);
    if (receipt) {
      if (receipt.status === "0x0") throw new Error("A route transaction reverted onchain.");
      return receipt;
    }
    await delay(1_500);
  }
  throw new Error(`Transaction ${shortAddress(hash)} was sent but confirmation is taking longer than expected. Check your wallet.`);
}

function relayState(value) {
  const status = value?.status;
  if (typeof status === "string") return status.toLowerCase();
  if (status && typeof status.status === "string") return status.status.toLowerCase();
  return "pending";
}

async function waitForSettlement(requestId) {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    const response = await api(`/api/buy/v1/status?requestId=${encodeURIComponent(requestId)}`);
    const status = relayState(response.status);
    if (["success", "filled", "complete", "completed"].includes(status)) return;
    if (["failure", "failed", "refund", "refunded"].includes(status)) {
      throw new Error(`Relay reported ${status}. Check the request in your wallet before trying again.`);
    }
    setText(elements.executionState, `Settlement in progress · ${status}`);
    await delay(1_500);
  }
  setText(elements.executionState, "Transactions were sent. Settlement is still pending; keep this wallet open and verify the final balance before retrying.");
}

async function executeQuote() {
  if (!state.quote || !state.account || state.routeTooExpensive) return;
  const activeAccount = state.account;
  elements.execute.disabled = true;
  setText(elements.execute, "Follow wallet prompts…");
  setText(elements.executionState, "Preparing the first wallet confirmation.");
  try {
    const chainHex = await walletRequest("eth_chainId");
    if (Number.parseInt(chainHex, 16) !== state.chainId) await switchChain(state.chainId);
    let requestId = null;
    for (const step of state.quote.steps) {
      if (step.requestId) requestId = step.requestId;
      for (const item of step.items) {
        if (item.status === "complete") continue;
        setText(elements.executionState, step.description || step.action);
        const data = item.data;
        const hash = await walletRequest("eth_sendTransaction", [{
          from: activeAccount,
          to: data.to,
          data: data.data,
          value: quantity(data.value),
        }]);
        setText(elements.executionState, `${step.id === "approve" ? "Approval" : "Transaction"} sent · ${shortAddress(hash)}. Waiting for confirmation…`);
        await waitForReceipt(hash);
      }
    }
    if (requestId) await waitForSettlement(requestId);
    setText(elements.executionState, "$TOHSENO arrived. Verify the balance in your wallet before leaving this page.");
    elements.executionState.dataset.state = "success";
    setText(elements.execute, "Purchase complete");
    await loadBalance();
  } catch (error) {
    setText(elements.executionState, errorMessage(error));
    elements.executionState.dataset.state = "error";
    setText(elements.execute, "Try again");
    elements.execute.disabled = false;
  }
}

elements.connect.addEventListener("click", () => {
  renderWalletOptions();
  elements.walletDialog.showModal();
});

elements.form.addEventListener("submit", async (event) => {
  event.preventDefault();
  try {
    await fetchQuote();
  } catch (error) {
    clearQuote();
    elements.form.hidden = true;
    elements.quotePanel.hidden = false;
    setText(elements.impactWarning, errorMessage(error));
    elements.impactWarning.hidden = false;
    elements.execute.disabled = true;
  }
});

elements.amount.addEventListener("input", () => {
  clearQuote();
  setControls();
});

elements.currency.addEventListener("change", async () => {
  clearQuote();
  await loadBalance();
});

elements.chain.addEventListener("change", async () => {
  clearQuote();
  try {
    await switchChain(Number(elements.chain.value));
  } catch (error) {
    setText(elements.walletState, errorMessage(error));
    renderChains();
  }
});

elements.max.addEventListener("click", () => {
  const currency = currentCurrency();
  if (!currency || state.balance === null) return;
  const spendable = currency.isNative ? state.balance * 98n / 100n : state.balance;
  elements.amount.value = unitsToDecimal(spendable, currency.decimals, currency.decimals);
  clearQuote();
  setControls();
});

elements.risk.addEventListener("change", () => {
  elements.execute.disabled = state.routeTooExpensive || !elements.risk.checked;
});

elements.execute.addEventListener("click", executeQuote);

elements.editQuote.addEventListener("click", () => {
  clearQuote();
  setControls();
  elements.amount.focus();
});

elements.copyContract.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(TOKEN_ADDRESS);
    setText(elements.copyLabel, "COPIED");
    window.setTimeout(() => setText(elements.copyLabel, "COPY"), 1_800);
  } catch {
    setText(elements.copyLabel, "COPY FAILED");
  }
});

window.addEventListener("eip6963:announceProvider", (event) => registerProvider(event.detail));
window.dispatchEvent(new Event("eip6963:requestProvider"));

window.setTimeout(() => {
  if (window.ethereum && ![...state.providers.values()].some((entry) => entry.provider === window.ethereum)) {
    registerProvider({ provider: window.ethereum, info: { uuid: "legacy-provider", name: "Browser wallet" } });
  }
}, 100);

api("/api/buy/v1/config")
  .then((config) => {
    state.config = config;
    renderMarket();
    renderChains();
    if (state.provider) refreshWallet().catch(() => {});
    else setControls();
  })
  .catch((error) => {
    setText(elements.walletState, `Live routing is unavailable: ${errorMessage(error)}`);
    elements.connect.disabled = true;
  });
