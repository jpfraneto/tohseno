# TOHSENO contract generation 0.8.0 human-audit brief

Status: engagement-ready scope; no provider selected or contacted; no spend
authorized.

## Objective

Perform an independent human or competitive security review of the exact
frozen TOHSENO generation `0.8.0` before any production deployment. The review
must challenge both contract correctness and the operational assumptions that
connect the contracts to Robinhood Chain's EIP-7951 implementation.

This is a small, tightly coupled identity and public-witness system: six
Solidity files, 905 physical lines and 36,821 bytes. It holds no funds, executes
no arbitrary calls, and has no administrator, proxy, pause, upgrade or token
logic. Immutability makes source changes generation-breaking rather than
patches.

## Immutable scope

- repository: `https://github.com/jpfraneto/tohseno`
- source commit: `862ca6cd3d396271b56b336fee0513ddcf6ecc64`
- generation definition:
  `contracts/generations/0.8.0/generation.json`
- generation-definition digest:
  `0x618fbd1ef9f826a2a642f94733192b130023e7f85db95d9ab066768e8abdf895`
- source-tree digest:
  `0x5d8c56423f9b9cb97d8e05834a6a2e776034b1257a186e47f25869bf509910c3`
- compiler: Solidity `0.8.30`, Cancun, optimizer 10,000 runs, no IR, no
  bytecode/CBOR metadata
- target: Robinhood Chain mainnet, chain ID `4663`

Files:

- `contracts/src/P256Verifier.sol`
- `contracts/src/EIP712Domain.sol`
- `contracts/src/IERC1271.sol`
- `contracts/src/BuilderAccount.sol`
- `contracts/src/BuilderAccountFactory.sol`
- `contracts/src/ShotRegistry.sol`

Any source change creates a new semantic generation and requires a new frozen
definition plus review of the changed candidate. Do not audit a moving branch.

## System model

`P256Verifier` strictly parses versioned 129-byte P-256 signatures, validates
curve points and scalar ranges, enforces low-s, and calls the EIP-7951
precompile at `0x100`. `BuilderAccount` is an ERC-1271 identity controller with
permissioned device keys, nonces, deadlines, counters and delayed recovery.
`BuilderAccountFactory` deploys accounts deterministically. `ShotRegistry` is a
neutral ERC-1271-controlled commit/reveal witness for ancestry-free public
checkpoints and controller transfer.

The registry intentionally accepts generic deployed ERC-1271 controllers;
client generation/runtime policy determines whether one is a recognized
TOHSENO BuilderAccount. The chain contract does not make that classification.

## Required review questions

1. P-256 input layout, point/scalar validation, low-s enforcement, official
   vector behavior and exact return-data handling.
2. ERC-1271 authorization, malformed or oversized return data, reverts,
   authority-controlled gas griefing and EOA/contract recovery behavior.
3. EIP-712 chain/contract/domain separation and exact field order for every
   device, recovery, registry and transfer action.
4. Device permission escalation, last-device/last-admin invariants, counter
   accuracy, revocation and epoch invalidation.
5. Recovery setup/change/initiate/veto/finalize state transitions, three-day
   delay boundaries, stale signatures and permissionless finalization.
6. Nonce, deadline, replay and cross-function/cross-generation behavior.
7. CREATE2 salts, init code, idempotence, counterfactual identities and factory
   front-running behavior.
8. Registry commitment copying, reset, expiry and relayer behavior; inclusive
   timestamp boundaries; registration nonce and checkpoint sequencing.
9. Append and transfer integrity, stale heads, old-controller invalidation and
   generic ERC-1271/EIP-7702 controller handling.
10. Arithmetic, timestamp, reentrancy, denial-of-service and event/evidence
    completeness.
11. Whether existing fuzz/invariant tests miss any state transition or
    adversarial composition.
12. Whether the mandatory actual-RPC EIP-7951 gate is sufficient and whether
    any deployment/runtime assumption remains unbound.

## Known internal observations

The internal pre-audit found no critical, high or medium issue and recorded
four informational/operational boundaries:

- neutral registry acceptance is broader than recognized TOHSENO identity;
- high-level ERC-1271 calls can allocate authority-controlled return data;
- protocol-permission ERC-1271 is not itself digest-purpose restricted;
- production safety depends on the fresh actual-RPC EIP-7951 gate.

Slither 0.11.6 reported 27 patterns across 102 detectors. All were triaged as
required exact comparisons, guarded assignments, intended time windows,
reviewed assembly/low-level calls or constants. Auditors must independently
challenge those dispositions rather than accepting them as exclusions.

Aderyn 0.6.8 ran 88 detectors. Its sole high detector flagged
`abi.encodePacked` in the factory's CREATE2 init-code construction; the internal
disposition is that the compiled creation-code prefix is invariant and the
constructor suffix is fixed-width ABI for two `uint256`s. Low detectors flagged
`ecrecover` despite strict low-s/`v`, guarded recovery-address assignments, and
style/API suggestions. Auditors must independently verify these dispositions.

## Available evidence

- `contracts/audits/PREAUDIT_0_8_0_2026-07-31.md`
- `contracts/generations/0.8.0/generation.json`
- `protocol/SPECIFICATION.md`
- `protocol/CONFORMANCE.md`
- `docs/adr/0006-public-witness-and-contract-generation.md`
- 80 Foundry unit, fuzz and invariant tests
- fixed-seed gas snapshot and deterministic ABI/bytecode checks
- actual-RPC P-256 probe implementation and historical evidence
- independent Python/OpenSSL activation verifier and negative suite

## Required deliverables

- report naming the exact source commit and generation digest;
- finding table with severity, impact, exploit preconditions, affected lines,
  reproducible proof and remediation;
- explicit review of every required question above;
- list of accepted assumptions and excluded surfaces;
- statement of tests/tools/methods used;
- fix verification for any revised successor generation;
- final attestation that names the exact reviewed digest.

Critical, high and medium findings block deployment. Every low and
informational item needs an explicit disposition. An audit is not a warranty
and does not replace production canaries or continuing review.

## Current primary-source engagement paths

No contact or purchase has been authorized. Current official options include:

- Spearbit managed review request:
  `https://docs.spearbit.com/spearbook/anatomy-of-a-spearbit-review/form-submission`
- Cantina security review or competition:
  `https://docs.cantina.xyz/` and
  `https://cantina.xyz/solutions/competitions`
- Sherlock audit contest intake:
  `https://docs.sherlock.xyz/audits/protocols/how-it-works-for-protocols`
- Code4rena competitive audit:
  `https://code4rena.com/competitive-audit`
- Cyfrin private or CodeHawks competitive review:
  `https://www.cyfrin.io/`

Provider choice, budget, contact details, disclosure timing and any external
message require an explicit human decision. This brief may be attached without
private keys, wallet credentials, unpublished vulnerabilities or user data.
