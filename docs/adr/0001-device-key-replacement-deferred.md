# ADR 0001: DeviceKey replacement and account recovery are deferred

Status: closed for the successor generation by ADR 0006; retained as the
frozen `0.7.0` (`GENESIS`) decision

## Context

The protocol defines `AuthorizeDevice`, `RevokeDevice`, `SetRecovery`, and
`RecoverAccount`, but the candidate does not yet have a complete,
independently verifiable authorization chain for Evolution signatures:

- the BuilderAccount and registry are not deployed, so there is no
  evidence-backed contract nonce or receipt;
- an Evolution carries no canonical local DeviceKey authorization proof;
- the offline verifier can reproduce authority only for the initial P-256 key
  bound into the CREATE2 BuilderID prediction;
- the encrypted BIP-39 vault stores recovery material locally, but does not
  configure or exercise account recovery.

Caller-selected nonces and detached action files do not close those gaps. They
can be stale, replayed, orphaned, or mistaken for completed replacement.

## Decision

The GENESIS CLI exposes no DeviceKey authorize, revoke, rotate, or recover
command. It does not create detached replacement-action files. Local signing,
descriptor validation, and offline verification accept only the original
DeviceKey that reproduces the frozen v0.7 predicted BuilderID. A
cryptographically valid signature by any other key fails
`record.device_authority`.

`identity backup` and `identity import-backup` are described only as encrypted
local recovery-authority backups. They do not activate recovery, replace a
key, or prove an account transition.

## Historical requirements for the frozen v0.7 client

Replacement or recovery remains blocked in the frozen v0.7 client because it
does not define and test all of the following together:

1. a canonical, bounded authorization/revocation proof carried with or
   unambiguously referenced by each affected Evolution;
2. an authoritative nonce and deadline source tied to verified deployed
   BuilderAccount state, with replay and rollback handling;
3. deterministic offline verification of the complete key-state transition
   chain, including revocation ordering;
4. recovery-authority setup and recovery signatures with public receipts;
5. atomic local activation of a replacement key only after those proofs pass.

Adding an action encoder, accepting a caller nonce, importing backup words, or
writing a signed file is not sufficient evidence to lift this decision.

## Successor resolution

ADR 0006 closes this ADR for contract generation 0.8 through permissioned
device administration, exact active-device/admin invariants, delayed
ERC-1271-capable recovery, an active-admin veto, epoch-based revocation, and
action-specific nonces and deadlines. No contract generation is active, so
this closure does not enable a current public CLI mutation or reinterpret a
frozen v0.7 artifact. A future owner interface and offline authority-proof
format remain separate implementation work, not a continuation of this
contract-design deferral.
