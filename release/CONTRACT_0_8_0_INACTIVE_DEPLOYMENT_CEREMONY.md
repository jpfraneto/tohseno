# Contract generation 0.8.0 inactive-deployment ceremony

Status: completed once under ADR 0009. The authorization is consumed. This is
not a reusable deployment command or activation authority.

Security note: the public singleton CREATE2 coordinates can be occupied by an
exact-code third-party deployment. Accepted ADR 0008 defines the required
fail-closed abandonment rule.

This runbook is subordinate to `protocol/`, accepted ADRs, `AGENTS.md`, and the
owner's eventual accepted deployment-authority record. It deliberately contains
no signer invocation or broadcast command. Current `main` forbids a deployment
command, and its tombstones remain unchanged.

## Purpose and boundary

The only proposed transaction outcome is deployment of the exact immutable
generation 0.8.0 `BuilderAccountFactory` and `ShotRegistry` to Robinhood Chain
mainnet as an inactive, untrusted candidate. Deployment does not activate the
generation, create a Builder identity, publish a Shot, register a controller,
or launch a token.

The transaction payer supplies gas only. It receives no protocol authority,
upgrade right, pause right, ownership, release key, Builder identity, registry
controller, or Bankr relationship.

## Frozen coordinates

- chain: Robinhood Chain mainnet, chain ID `4663`
- generation: `0.8.0`
- generation definition digest:
  `0x618fbd1ef9f826a2a642f94733192b130023e7f85db95d9ab066768e8abdf895`
- source commit: `862ca6cd3d396271b56b336fee0513ddcf6ecc64`
- source tree digest:
  `0x5d8c56423f9b9cb97d8e05834a6a2e776034b1257a186e47f25869bf509910c3`
- payer: `0x1eaF00a3F027275077253713C5bF7d0fAC44207F`
- canonical CREATE2 deployer:
  `0x4e59b44847b379578588920ca78fbf26c0b4956c`
- deployer runtime length: `69` bytes
- deployer runtime Keccak-256:
  `0x2fa86add0aed31f33a762c9d88e807c475bd51d0f52bd0955754b2608f7e4989`
- factory salt:
  `0x5fd2db5a85724a3ec24a912bfbf24fc577d26b53aafaf22e45dd425901249ef4`
- factory init-code Keccak-256:
  `0x4a06468df5e31a81a23c5874258de8b3c3c70031a27c98fb8ef677f061c854eb`
- factory address: `0xb1bd208cd2af98e701f43d06aaa889d3a594df65`
- factory runtime Keccak-256:
  `0xb880403ff0da9a3f4c0c982cdea56fb198912c66bc436305e20861378f45c5f4`
- registry salt:
  `0x9c40694097bc8150c4d1b158a4947ebce701ac78a428182953119033cb17f8c4`
- registry init-code Keccak-256:
  `0x865bf1508195e35de9e0b0620f0bbc9026485a6022e755369ce6fad1c825cf6b`
- registry address: `0x3fe6508ba2660bc575080024f402c192a2e035a0`
- registry compiler runtime-template Keccak-256:
  `0xb6279467956165a9520803fdfe5a2254c98569b71541bf67d57fcd54ab3a2524`

ADR 0010 governs the distinction between this compiler template and
constructor-patched instance bytes.

Any different fact means stop. Do not adapt the ceremony to new bytes or
coordinates; define, audit, and authorize a new semantic generation.

## Conditions before a ceremony implementation may exist

Every item must be proven and referenced from the final authority record:

1. two independent AI audit reports exist and every finding is reproduced and
   dispositioned against the exact generation;
2. no Critical or High code finding remains, and every Medium finding has an
   accepted fail-closed disposition; human/competitive review remains required
   before activation;
3. release-authority preparer and verifier tests pass; production 2-of-3 keys
   and the approved policy digest remain required before activation;
4. repository tests and deterministic artifact checks pass from a clean or
   fully understood source state;
5. accepted ADR 0009 explicitly authorizes this exact single-generation
   inactive deployment;
6. the one-time operator sequence has no implicit RPC, chain, generation,
   payer, deployer, salt, bytecode, address, retry, or activation path, and each
   signed transaction is decoded and checked before submission; and
7. the owner explicitly schedules the transaction ceremony. Wallet funding or
   this document's existence is not that authorization.

## Roles and separation

- The owner accepts policy and final facts. The agent or operator must not infer
  acceptance.
- The transaction operator runs the reviewed ceremony implementation and
  controls only the limited-purpose payer.
- An independent observer reproduces hashes, simulations, nonce and on-chain
  results from a separate process or machine.
- Release-authority custodians do not participate in deployment signing and do
  not expose their offline keys during deployment.

No one role may substitute a transaction signature for release approval.

## Stage A — deterministic rebuild and fresh target gate

Immediately before any signing:

1. identify the exact reviewed source commit and understood worktree;
2. rebuild with the frozen compiler profile and byte-compare every artifact;
3. reproduce the generation definition and source-tree digests;
4. require an explicit HTTPS RPC and prove chain ID 4663;
5. obtain one fresh canonical block and run the complete positive, negative,
   point-at-infinity, and exact 6,900-gas EIP-7951 probe against that block;
6. recheck that the probe block remains canonical;
7. verify the canonical CREATE2 deployer's exact runtime bytes;
8. verify that both predicted targets contain no code at the pinned block; and
9. stop if any prior transaction, nonce change, target code, reorganization,
   RPC inconsistency, or artifact drift appears.

Historical files under `contracts/audits/` are useful comparisons only and
cannot satisfy this stage.

## Stage B — exact simulations and owner display

Without accessing the signer, construct two candidate calls. Both have chain ID
4663, sender equal to the frozen payer, recipient equal to the canonical CREATE2
deployer, and value zero.

1. Factory data is exactly `factory_salt || factory_init_code`.
2. Registry data is exactly `registry_salt || registry_init_code`.

At the same fresh state boundary, obtain the payer's confirmed nonce, balance,
fee data, both gas estimates, and both `eth_call` results. Each call must return
only its predicted address. The owner-facing review must show, without
abbreviation:

- chain ID and canonical block hash;
- payer and starting nonce;
- generation definition digest;
- deployer address and runtime hash;
- each salt, init-code hash, calldata byte length, predicted address, expected
  runtime hash, gas limit, maximum fee per gas, maximum total cost, and value;
- combined maximum cost and remaining payer balance; and
- the fresh P-256 evidence path and SHA-256 digest.

Gas limits must be derived from the successful fresh estimates under the
reviewed implementation's fixed bounded policy. They may not be silently
re-estimated after confirmation. Fee caps may not exceed the displayed values.

## Stage C — single-use confirmation

After the independent observer reproduces Stage B, the owner enters one
single-use confirmation containing every binding value. The reviewed
implementation must generate the phrase; operators must not hand-edit it.
Its closed grammar is:

```text
DEPLOY INACTIVE TOHSENO 0.8.0 ON CHAIN 4663 FROM <payer> NONCE <n> FACTORY <factory> REGISTRY <registry> GENERATION <definition-digest> P256 <probe-sha256> MAX-WEI <combined-maximum-cost>
```

Matching is byte-exact and case-sensitive. The confirmation expires if the
canonical block, nonce, balance, fee caps, gas limits, target code, simulation,
probe, source state, or implementation digest changes. It is consumed on the
first signing attempt even if a response later becomes uncertain.

## Stage D — sequential broadcast and uncertain outcomes

Only the accepted, independently reviewed ceremony implementation may access
the payer. It signs and broadcasts at most one factory transaction first.

1. Persist the signed transaction bytes in owner-controlled restricted storage
   before submission, without printing them in chat or logs.
2. Submit once. Never use automatic retry.
3. Wait for a canonical successful receipt, then verify sender, nonce, chain,
   recipient, zero value, exact input, block, target runtime bytes and runtime
   hash.
4. Only after factory verification, repeat the same one-at-a-time process for
   the registry at the next confirmed payer nonce.

If submission times out or returns an ambiguous error, stop. Query the exact
transaction hash when known, the payer's latest and pending nonce, receipts,
and target code from at least two RPC observations. Do not sign or submit a
replacement until the original outcome is proven absent and the owner issues a
new explicit authorization under a newly generated confirmation. A target with
the expected runtime is evidence to investigate, not permission to assume who
deployed it.

## Stage E — inactive deployment evidence

After both receipts are canonical, preserve:

- reviewed source and ceremony implementation commits and digests;
- complete fresh P-256 evidence and digest;
- all Stage B display facts and the fact of owner confirmation, but no secret;
- both signed transaction bytes in restricted owner storage;
- transaction hashes, senders, nonces, recipients, values, exact inputs,
  receipts, gas used, effective gas prices, block numbers and block hashes;
- observed target runtime bytes and hashes at a canonical block at or after both
  deployments; and
- independent observer reproduction results.

Update public state only to `deployed_inactive_untrusted`. Do not change client
trust roots, `active_generation`, publication behavior, app metadata, node
discovery, or Bankr behavior. Predicted coordinates become observed deployment
coordinates only after the evidence above verifies.

## Post-deployment boundary

The next phase is the dedicated production canary, including the real-time
three-day recovery delay. Any defect abandons generation 0.8.0 and starts a new
generation and audit cycle. Activation is a later threshold-signed release and
client trust-root decision; it is never an automatic consequence of this
ceremony.

## Completed outcome

The exact one-time ceremony completed on 2026-08-01 UTC. Public evidence is
`contracts/audits/robinhood-inactive-deployment-0.8.0-20260801T021920Z.json`.
The factory transaction is
`0x259f8f6d7fc09b392e46928066d172c3cce8f436c7a63591c64ec9c58409a5ef`;
the registry transaction is
`0x1f7d2ccc24f66e9826b2a2729808a2fcc58dfd2f830ee10631ebf30eac72de91`.
Both receipts succeeded, both canonical blocks were rechecked, exact inputs
matched the frozen creation bytes, and the registry instance was reproduced
byte-for-byte by replaying the signed transaction on a fork of its parent
block. The candidate remains inactive and untrusted.
