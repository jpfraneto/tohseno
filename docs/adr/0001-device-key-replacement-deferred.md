# ADR 0001: DeviceKey replacement and account recovery are deferred

Status: accepted for `1.0.0-rc.1` (`GENESIS`)

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
DeviceKey that reproduces the stable BuilderID. A cryptographically valid
signature by any other key fails `record.device_authority`.

`identity backup` and `identity import-backup` are described only as encrypted
local recovery-authority backups. They do not activate recovery, replace a
key, or prove an account transition.

## Required work before replacement can be enabled

Replacement or recovery remains blocked until one candidate version defines
and tests all of the following together:

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
