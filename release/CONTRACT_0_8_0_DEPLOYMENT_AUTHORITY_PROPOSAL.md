# Contract generation 0.8.0 deployment-authority proposal

Status: accepted and consumed by one inactive deployment under ADR 0009; not
activation authority.

This proposal is intentionally non-normative. `protocol/` and accepted ADRs
remain authoritative. It exists to make the owner's future decision exact and
reviewable without weakening the deployment tombstone on `main`.

The corresponding prepared operator procedure is
`release/CONTRACT_0_8_0_INACTIVE_DEPLOYMENT_CEREMONY.md`. It contains no signer
or broadcast command and has no authority until this proposal's conditions are
met and the repository's accepted policy process explicitly permits the narrow
ceremony implementation.

## Exact proposed authorization

Authorize one fail-closed production ceremony that deploys the frozen TOHSENO
contract generation `0.8.0` to Robinhood Chain mainnet as an **inactive,
untrusted candidate** using the canonical CREATE2 deployer.

- chain ID: `4663`
- explicit RPC: `https://rpc.mainnet.chain.robinhood.com`
- generation-definition digest:
  `0x618fbd1ef9f826a2a642f94733192b130023e7f85db95d9ab066768e8abdf895`
- generation source commit: `862ca6cd3d396271b56b336fee0513ddcf6ecc64`
- transaction payer: `0x1eaF00a3F027275077253713C5bF7d0fAC44207F`
- canonical CREATE2 deployer: `0x4e59b44847b379578588920ca78fbf26c0b4956c`
- expected deployer runtime Keccak-256:
  `0x2fa86add0aed31f33a762c9d88e807c475bd51d0f52bd0955754b2608f7e4989`
- predicted BuilderAccountFactory:
  `0xb1bd208cd2af98e701f43d06aaa889d3a594df65`
- predicted ShotRegistry:
  `0x3fe6508ba2660bc575080024f402c192a2e035a0`

The payer is only a transaction sender. It gains no protocol authority,
Builder identity, registry ownership, upgrade right, pause right or activation
right.

## Conditions precedent

The inactive-deployment authority requires all of the following evidence to
name the exact generation digest above:

1. the independent AI audit report exists and every finding is reproduced and
   dispositioned;
2. two independent AI audits are complete with no unresolved Critical or High
   code finding, and every Medium finding has an accepted fail-closed
   disposition; human/competitive review remains mandatory before activation;
3. the release-authority runbook and verifier tests pass; the production
   2-of-3 policy and offline keys remain mandatory before activation rather
   than inactive deployment;
4. all repository tests, deterministic artifact checks and independent
   activation-verifier tests pass from an understood worktree;
5. the one-time operator sequence is closed over the explicit RPC, chain,
   generation, deployer, salts, init code, addresses, payer, nonces and cost
   cap; decoded signed transactions are rechecked before one-time submission;
6. immediately before broadcast, a fresh canonical Robinhood block passes the
   complete EIP-7951 positive, negative, point-at-infinity and exact 6,900-gas
   probe;
7. the canonical CREATE2 deployer runtime matches exactly and both predicted
   addresses still contain no code;
8. simulation proves the exact transactions, sender nonce, gas, balance,
   init-code hashes, addresses and expected post-deployment runtime hashes;
9. the owner confirms those final simulation facts in the ceremony record.
10. ADR 0008's CREATE2-provenance and pre-activation-state policy is accepted,
    and the independently reviewed ceremony proves the authorized payer's
    successful receipts rather than accepting code equality alone.

Failure or drift in any condition voids the ceremony authorization. Source or
bytecode drift requires a new semantic generation and a new audit cycle.

## Narrow effect

Acceptance authorizes at most one broadcast attempt for each exact deployment
transaction. It does not authorize automatic retry after an uncertain RPC
response. The operator must first inspect the payer nonce, receipts and target
code. It does not authorize:

- activating generation 0.8.0;
- deploying any other bytecode, generation, proxy or test variant;
- creating or signing release-authority keys on the transaction payer;
- publishing a Shot, registering a BuilderID or launching a token;
- spending Base USDC or native assets on another chain;
- changing protocol law or treating predicted addresses as deployed.

Unexpected code at either predicted address before the authorized payer's
successful receipt—including exact expected code deployed by another
sender—voids the generation 0.8.0 ceremony. It must not be adopted or retried;
the coordinates are abandoned under accepted ADR 0008.

After deployment, the contracts remain inactive. Activation requires the full
real-time three-day canary, a canonical activation record, threshold approval
from the separately trusted release policy, independent verification and an
explicit client trust-root change.

## Repository implementation boundary

Current `main` expressly has no deployment command and its retired deployment
scripts fail closed. ADR 0009 authorizes a one-time operator ceremony without
adding a deployment command to the repository. A general-purpose deploy
command remains forbidden.

## Owner decision record

- decision: `accepted_inactive_deployment_only`
- accepted generation digest:
  `0x618fbd1ef9f826a2a642f94733192b130023e7f85db95d9ab066768e8abdf895`
- accepted release-authority policy digest: `null_before_activation`
- accepted ceremony implementation: `one-time operator sequence under ADR 0009`
- decision timestamp: `2026-07-31T22:19:20-04:00`
- evidence reference: owner deployment direction and ADR 0009

The authority was consumed by the successful factory transaction
`0x259f8f6d7fc09b392e46928066d172c3cce8f436c7a63591c64ec9c58409a5ef`
and registry transaction
`0x1f7d2ccc24f66e9826b2a2729808a2fcc58dfd2f830ee10631ebf30eac72de91`.
No retry or additional deployment authority remains.

This decision does not authorize activation, canary spending beyond a later
approved canary budget, Shot publication, or Bankr token launch.
