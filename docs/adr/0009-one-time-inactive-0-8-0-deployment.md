# ADR 0009: authorize one inactive generation 0.8.0 deployment ceremony

- Status: accepted
- Date: 2026-07-31
- Owner direction: “polish the rough edges … and deploy the smart contracts …
  sharpen the rough edges and lets ship”
- Scope: one inactive-candidate deployment of the exact frozen generation
  `0.8.0` to Robinhood Chain mainnet

## Context

The owner has explicitly directed the project to finish and deploy the reviewed
contracts. Two independent AI reviews examined the exact frozen generation:
Claude Fable 5 returned Conditional Go with no Critical, High, or Medium code
finding; GPT-5.6-Sol found one Medium operational CREATE2 provenance/denial
risk, two Low integration risks, and informational hardening items.

ADR 0008 now accepts the fail-closed disposition for the Medium risk. The
P-256 wrapper-test gap and Rust action-coordinate mismatch were remediated
without changing any generation 0.8.0 Solidity source or bytecode. The owner
wants production-chain testing before activation, with a defective immutable
candidate abandoned rather than repaired in place.

The earlier preparatory proposal required the human/competitive audit and
offline release-authority policy before even placing inactive code on-chain.
Those remain appropriate activation gates but are not required to make an
untrusted, administrator-free candidate observable for the real three-day
canary. Deployment is not activation or publication authority.

## Decision

Authorize exactly one fail-closed ceremony for the generation identified by:

- generation digest:
  `0x618fbd1ef9f826a2a642f94733192b130023e7f85db95d9ab066768e8abdf895`;
- source commit: `862ca6cd3d396271b56b336fee0513ddcf6ecc64`;
- chain ID: `4663`;
- explicit RPC: `https://rpc.mainnet.chain.robinhood.com`;
- payer: `0x1eaF00a3F027275077253713C5bF7d0fAC44207F`;
- canonical CREATE2 deployer:
  `0x4e59b44847b379578588920ca78fbf26c0b4956c`;
- factory: `0xb1bd208cd2af98e701f43d06aaa889d3a594df65`;
- registry: `0x3fe6508ba2660bc575080024f402c192a2e035a0`.

The two independent AI reviews satisfy the review threshold for **inactive
deployment only**. A separate human or competitive audit remains mandatory
before activation. The 2-of-3 offline release-authority policy is likewise an
activation prerequisite, not a transaction-sender prerequisite.

The ceremony may access the existing limited-purpose payer in macOS Keychain
only after a fresh deterministic rebuild, complete EIP-7951 target probe,
deployer/runtime check, empty-target check, exact simulations, nonce/balance
check, and a maximum combined gas limit cost no greater than `0.0002` native
ETH. It may sign and submit the factory at nonce zero and, only after a verified
canonical successful receipt, the registry at nonce one. Each transaction is
submitted once. There is no automatic replacement or retry.

The signer must be moved directly from Keychain into an ephemeral encrypted
keystore without appearing in process arguments, shell output, repository
files, prompts, or logs. The ephemeral keystore and its password are destroyed
after signing. Raw signed transactions and public receipts are retained in an
owner-only ceremony directory outside the repository.

No general-purpose or reusable deployment command is added to `main`. The
retired deployment tombstones remain unchanged.

## Fail-closed outcomes

- Any source, artifact, hash, chain, precompile, deployer, target, payer, nonce,
  simulation, or cost drift stops before signing.
- Any unexpected code at either target applies ADR 0008: generation 0.8.0 is
  abandoned and no existing deployment is adopted.
- Any ambiguous submission stops. Chain state, payer nonce, transaction hash,
  and target code are inspected before further action.
- A successful deployment is recorded only as
  `deployed_inactive_untrusted`. It does not change a client trust root,
  activate publication, create a BuilderAccount, register a Shot, or launch a
  Bankr token.

## Activation and product boundary

Activation still requires the real-time three-day canary, a human/competitive
audit, the accepted 2-of-3 offline P-256 release policy, threshold-signed
activation evidence, independent verification, and an explicit client
trust-root change. Bankr token launch remains a separate Shot-scoped action
with its own Bankr API key, recipient, simulation, and confirmation.
