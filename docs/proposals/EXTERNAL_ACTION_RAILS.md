# Proposal: optional external action rails

- Status: **Proposed** architecture; first capability, provider, network, and
  key model are **Open**
- Does not implement: contracts, programs, RPC clients, trading, transfers,
  brokerage access, exchange access, deployment, or mainnet/testnet actions

## Start with the operation, not the provider

An external rail is a narrow, manifest-declared operation that supports the one
continuity action. It is not a generic wallet, trading terminal, chain router, or
“do stuff” API.

Base, Solana, Hyperliquid, and Robinhood do not represent the same boundary:

- Base is an EVM chain target;
- Solana is a chain and program target with a different account/signature model;
- Hyperliquid may involve exchange/API execution and chain-specific behavior;
- Robinhood is an external brokerage/provider boundary unless an exact on-chain
  product and operation are named.

Putting all four behind an undifferentiated `smart_contracts/` enum would hide
custody, signing, fee, finality, disclosure, rollback, and legal differences.

## Eventual repository separation

Only after the first operation is approved should code adopt a shape such as:

```text
packages/external-actions/      typed operation and receipt contracts
adapters/base/                  Base RPC and transaction adapter
adapters/solana/                Solana RPC and transaction adapter
adapters/hyperliquid/           declared Hyperliquid API/chain adapter
adapters/robinhood/             declared Robinhood provider adapter
smart_contracts/base/           actual reviewed EVM source, if required
smart_contracts/solana/         actual reviewed program source, if required
```

The `smart_contracts` directory is reserved for deployable on-chain source. A
provider adapter does not become a smart contract merely because it can move or
trade an asset. No provider subdirectory should claim availability until a
precise operation, environment, contract, tests, and ownership decision exist.

## Key and authority boundary

Practice identity, browser delegation, owner/update approval, and asset custody
are distinct roles. One raw key must not be reused across apps, browsers, Base,
Solana, exchanges, and brokerages. A shared mnemonic would be a common recovery
and compromise root even if domain-separated child keys reduce address linkage.

Every external operation must declare:

- operation ID, purpose, provider/network, environment, and version;
- exact recipient/program/contract and closed arguments;
- signer role and where the key lives;
- simulation or preview requirements;
- amount, fee, slippage, frequency, and cumulative limits where applicable;
- phone step-up and human-readable confirmation;
- nonce, idempotency, finality, reconciliation, and retry behavior;
- public disclosure, including durable address/hash/timing correlation;
- failure fallback, disable switch, revocation, export, ownership, and ejection;
- who pays and which action requires explicit owner approval.

Financial execution cannot inherit authority from a generic browser session.
Automated tests may not deploy, trade, transfer, or spend real funds.

## Relationship to continuity

External rails are optional subscribers after a local commit. They cannot be the
canonical event store or a prerequisite for action, local record, recovery, or
owner export. Public-chain hashes, timestamps, and transaction graphs are
durable and linkable disclosure; even a content digest can correlate or enable
guessing. On-chain projection therefore requires explicit manifest consent and
minimal disclosure.

## First-rail selection test

Before implementation, answer:

1. What exact continuity transition does the operation serve?
2. Why can it not be satisfied locally?
3. What valid manifest diff represents it?
4. What leaves the device, becomes public, costs money, or becomes irreversible?
5. Which key and account does the owner control?
6. What happens when the provider disappears or the operation fails?
7. Can the app be ejected and the rail disabled without loss of local value?

The first rail should use a sandbox or testnet, one named operation, strict
limits, explicit step-up, deterministic fixtures, and no administrative upgrade
power unless separately approved.

## Acceptance evidence

- changed chain/environment, recipient, program/contract, amount, fee, slippage,
  calldata/arguments, nonce, or expiry fails;
- simulation mismatch and stale quote fail;
- retries cannot duplicate an irreversible action;
- app/network/purpose keys differ according to versioned test vectors;
- logs, URLs, and the TOHSENO control plane receive no private content or key;
- public disclosure is presented before approval and captured in the manifest;
- provider/network failure leaves the local continuity loop intact;
- ejection works without TOHSENO accounts or credentials.

## Non-goals

- choosing a universal chain;
- using one address as a person's cross-app identity;
- storing prompts or private continuity artifacts on-chain;
- a general-purpose wallet, trading interface, or autonomous financial agent;
- creating paid infrastructure or publishing contracts without owner approval.
