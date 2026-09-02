---
title: Trust boundaries
description: What the phone, Mac, relay, Registry, contracts, coding agent, and Apple each control.
---

Security comes from keeping responsibilities narrow.

## Human and phone

Companion proves possession of private pairing keys for private commands and the Builder DeviceKey for public Builder/Claim actions. The DeviceKey private scalar stays in the strongest compatible non-exportable, this-device-only Keychain/Secure Enclave mechanism. Test software keys must be visibly local/test-only.

The phone authorizes exact structured actions. It does not prove the Mac built what it claimed or that the chain accepted a transaction.

## Mac

The Mac owns execution truth: source observed before and after work, harness selection, build outputs, code signature, device selection, installation command, bundle inventory, local history, and publication snapshot. It does not possess the Builder DeviceKey and cannot authorize public publication by itself.

## Coding harness

The harness is an untrusted source mutator operating within one bounded request. Its output earns nothing until deterministic engine and Apple gates pass. It receives private intent and necessary local context, so its configured provider route is a real privacy boundary.

## Relay

The Companion relay sees mailbox/device routing IDs, opaque ciphertext sizes, timestamps, sequence and cursor metadata. It cannot decrypt commands, grant a capability, resolve a Shot, or execute work. APNs, when configured, is only a content-free wake hint.

## Registry service

The service stores catalog manifests, blobs, indexes and constrained transaction jobs. It can transport public source and submit allowlisted calls. It cannot sign as Builder or claimant. Its database is reconstructable index state, not public authority.

## Contracts and chain

BuilderAccount establishes live public action authority. ShotRegistry witnesses only controller, public checkpoint head, checkpoint count and nonce. `TohsenoClaimsV1` stores immutable edition and Claim facts. Contracts do not know local source paths, private intentions, Apple identities, devices, installations, or human uniqueness.

## Apple

Xcode, certificates, provisioning, Trust, Developer Mode, CoreDevice and iOS installation remain an external security boundary. A Tohseno Claim or Registry receipt cannot replace them.

## Website handoff

A Browser Draft, Pending Relay Intention, and Local Pending Intention are transport states. None is a Shot. Production handoff stays closed unless the exact release and installer pin are verified.
