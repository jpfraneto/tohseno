# ADR 0010: Bind instantiated runtime hashes separately from compiler templates

Status: accepted

Date: 2026-07-31

## Context

The one-time generation 0.8.0 deployment installed the exact audited CREATE2
inputs on Robinhood Chain. `BuilderAccountFactory` matched the generation's
compiler runtime hash. `ShotRegistry` did not: replaying the exact signed
transaction on a fork of its parent block produced byte-for-byte the same
runtime as mainnet, but not the value returned by `forge inspect
deployedBytecode`.

The cause is deterministic. `EIP712Domain` stores `_hashedName` and
`_hashedVersion` as Solidity immutables. Compiler deployed bytecode contains
zero placeholders at those immutable-reference offsets; constructor execution
patches the installed runtime. `BuilderAccount` has the same distinction.
Generation 0.8.0 correctly commits the reproducible compiler artifacts and
creation inputs, but the activation validator incorrectly treated a template
hash as an instantiated runtime hash.

## Decision

`ContractBuild.runtime_code_keccak256` means the Keccak-256 of the compiler's
deployed-bytecode template. It remains an immutable build fact.

An activation's runtime hashes mean the exact instantiated runtime bytes. The
factory contains no immutable references and MUST equal its generation
template. BuilderAccount and ShotRegistry instance hashes MUST be nonzero,
MUST be reproduced from the exact generation-bound creation inputs during
release review, and MUST be approved explicitly by the release-authority
threshold. They MUST NOT be required to equal their zero-placeholder compiler
templates.

This decision changes no Solidity source, init code, CREATE2 coordinate,
deployment transaction, contract state, or generation digest. It does not
activate generation 0.8.0. The production canary and release ceremony must
still reproduce actual runtime bytes and pass every independent-review and
trust-root gate.

## Consequences

The already deployed inactive candidate can be assessed honestly without
rewriting its immutable generation. A runtime substitution still fails closed:
the factory hash is exact, CREATE2 addresses and deployment observations remain
generation-bound, actual instance hashes are signed, and clients compare live
code to the signed activation. Release tooling and independent review must
distinguish compiler templates from instantiated bytecode everywhere.
