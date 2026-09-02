---
title: Protocol authority and scope
description: What the Tohseno protocol defines, what it deliberately leaves to products and deployments, and how versions coexist.
---

This section is explanatory. The normative sources are `protocol/SPECIFICATION.md`, `protocol/CONFORMANCE.md`, closed schemas, implementation code, and frozen test vectors. If prose here differs, `protocol/` wins.

## Two versioned layers

The repository retains frozen v0.7 byte law for released private artifacts and offline verification. Additive `/2` records, ShotRegistry v2 actions, public checkpoints, generation definitions, and activation records define successor behavior.

Decoders dispatch on the exact schema and generation. Changing an EIP-712 domain version does not reinterpret a v0.7 object as a successor action. Frozen bytes remain readable and testable; additive successors receive new schema versions.

## What the protocol defines

- Shot, Expression, Version, Builder and device identities.
- Immutable and append-only record structures.
- Exact UTF-8 JSON shapes and canonical RFC 8785 encodings.
- SHA-256, Keccak-256, CREATE2, EIP-712, P-256 and recovery laws.
- Source, input, Fascia, lineage and public-checkpoint commitments.
- Neutral reduction and conformance checks.
- Generation 0.8 build-definition and activation evidence formats.
- Pairing-request and installation-continuity record formats.

## What it does not define

The protocol does not define a terminal UI, cloud account, coding harness, RPC selection, relayer policy, Apple signing workflow, server filesystem, product pricing, or global app-folder layout. Those belong to the engine, product, operators, and accepted ADRs.

The pure `tohseno-protocol` Rust crate has no CLI, network RPC, Apple signing, harness, server, or global filesystem policy.

## Strict JSON

Every wire object is UTF-8 JSON. Schemas are closed. Unknown or duplicate members, trailing JSON values, wrong-width hexadecimal, and uppercase hexadecimal are invalid. Byte strings are lowercase `0x`-prefixed hex. Cross-language `u64` JSON values are restricted to `0..=9007199254740991`.

JSON Schema validation is necessary but not enough. Curve membership, low-s arithmetic, cross-field equality, time, lineage, deployed state, authority, and receipts require semantic verification.

## Authority is evidence, not a type

A canonical object, predicted address, valid detached signature, reproducible contract definition, or signed activation each proves only its own claim. Public Builder/Shot authority requires the relevant pieces to agree with live state under a client-trusted activation.
