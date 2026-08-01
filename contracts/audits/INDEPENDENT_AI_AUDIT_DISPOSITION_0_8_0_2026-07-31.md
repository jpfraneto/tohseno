# Independent AI audit disposition — generation 0.8.0

Status: one Medium operational finding remains open. This record is not a
human audit, deployment authority, activation authority, or permission to use
a signer.

## Reviewed candidate

- source commit: `862ca6cd3d396271b56b336fee0513ddcf6ecc64`
- generation digest:
  `0x618fbd1ef9f826a2a642f94733192b130023e7f85db95d9ab066768e8abdf895`
- source-tree digest:
  `0x5d8c56423f9b9cb97d8e05834a6a2e776034b1257a186e47f25869bf509910c3`

Independent reports:

- Claude Fable 5:
  `contracts/audits/FABLE_5_AUDIT_0_8_0_2026-07-31.md`
  (`c627f9fdb40e9d8c3c624f05eb4e5a22206fe45fd3b4d1fa1c51d32d606693e5`)
- GPT-5.6-Sol:
  `contracts/audits/GPT_5_6_SOL_AUDIT_0_8_0_2026-07-31.md`
  (`c032475b73ab9f94450d86f4a91489d531ad76efe2209aa491ba4ad5ae3a8d53`)

Both are AI reviews. Together they complete the independent AI-review gate;
they do not silently satisfy the separate human/competitive-review gate in the
current deployment-authority proposal.

## Disposition table

| Finding | Severity | Reproduced | Disposition | Release effect |
|---|---:|---:|---|---|
| GPT M-01: public CREATE2 top-level predeployment and pre-activation state | Medium, operational | Yes | **Open.** Proposed ADR 0008 defines strict payer-receipt provenance, no adoption/retry after third-party predeployment, activation-block filtering, and generation abandonment on collision. It is not accepted policy yet. | Blocks deployment. |
| Fable TOH-01 / GPT I-03: P-256 mock did not inspect all five words | Low / informational | Yes | **Remediated in tests.** The mock now has a strict mode checking `digest,r,s,x,y`, and `testWrapperUsesExactOfficialEip7951InputOrder` uses the pinned official positive vector. No frozen source changed. The deployed-wrapper canary remains mandatory. | Closed locally; canary condition remains. |
| GPT L-02: Rust Builder action accepted unusable contract coordinates | Low | Yes | **Remediated.** Normative specification and conformance now require `action.account == domain.verifyingContract` and forbid self-recovery. Rust rejects both, the schema documents the semantic rule, and negative tests pass. EIP-712 type strings/digests and frozen contract bytes are unchanged. | Closed. |
| Fable TOH-02: one compromised DEVICE_ADMIN defeats recovery | Low | Yes | **Accepted design risk, documentation required.** Existing tests prove admin rotation, revocation, and veto behavior. ADR 0006 deliberately grants these powers. Product UX must state that recovery protects device loss, not compromise of an active admin. | Does not block inactive deployment; blocks public onboarding until disclosed. |
| Fable TOH-03: no recovery plus last-key loss bricks account | Low | Yes | **Accepted operational risk.** Contract guards prevent voluntary last-device removal but cannot recover a lost sole key before recovery setup. Clients must require or strongly gate recovery setup before durable use. | Does not block inactive deployment; blocks public onboarding policy. |
| Fable TOH-07 / GPT L-01: transfer requires no recipient acceptance | Low / informational | Yes | **Accepted neutral-witness behavior with client constraint.** Controller state is mutation authority, not recipient endorsement. Clients must label an inbound transfer unaccepted until the recipient produces a later valid action. A two-phase transfer is deferred to a successor generation if consent becomes protocol law. | Does not block inactive deployment; blocks misleading UI/indexing. |
| Dynamic ERC-1271 return-data allocation | Informational | Yes | **Accepted for 0.8.0.** Only the selected controller or recovery authority can grief its own action/relayer and can already refuse it. Bounded copying is future hardening. | No block. |
| Digest-agnostic ERC-1271 protocol permission | Informational | Yes | **Accepted generic ERC-1271 behavior.** Signing clients must be purpose-closed and never request opaque external digests. | No block; client requirement. |
| BuilderAccount is not an asset wallet | Informational | Yes | **Accepted.** There is no execution or recovery path for force-sent ETH or tokens. UI and documentation must say not to send assets. | No block; UX requirement. |
| Persistent expired commitment storage | Informational | Yes | **Accepted.** Writers pay for their own storage and no registry path iterates commitments. | No block. |
| Client-chosen long deadlines | Informational | Yes | **Accepted with client cap.** Nonces bound replay; clients must generate short bounded deadlines. | No block; client requirement. |
| Same-transaction created/destroyed controller and generic neutral controllers | Informational | Yes | **Accepted neutrality/liveness boundary.** Approved TOHSENO controller classification remains an off-chain runtime/provenance check. | No block; verifier requirement. |

## Reproduction evidence

The coordinating session independently verified:

- the six working-tree Solidity sources are byte-identical to source commit
  `862ca6c`;
- all six source SHA-256 hashes match the generation definition;
- the wrapper input-order weakness existed in the prior mock;
- the admin can cancel after recovery maturity and change recovery immediately;
- transfer authenticates only the current controller;
- `BuilderAccountActionV2::digest` previously accepted a mismatched domain;
- both top-level CREATE2 calls are publicly reproducible from the frozen
  deployer, salt, and init code; and
- at fresh Robinhood block `24668739`, chain ID was `4663`, both predicted
  top-level addresses still returned empty code, payer nonce was `0`, and payer
  balance was `777000000000000` wei. This is a read-only historical observation,
  not reusable ceremony evidence.

Post-remediation verification:

- `forge fmt --check --root contracts`: pass;
- `forge test --root contracts -vvv`: 81 passed, 0 failed;
- `cargo fmt --all --check`: pass;
- `cargo test --locked -p tohseno-protocol --all-targets`: 72 passed, 0 failed;
- `cargo clippy -p tohseno-protocol --all-targets -- -D warnings`: pass.

## Remaining decision

Audit M-01 remains unresolved until a human owner accepts or rejects proposed
ADR 0008 and the resulting ceremony receives independent review. Acceptance
would disposition the risk as fail-closed denial: third-party deployment or
provenance drift abandons 0.8.0 rather than being adopted. Rejection requires a
new generation/topology and a new audit cycle.
