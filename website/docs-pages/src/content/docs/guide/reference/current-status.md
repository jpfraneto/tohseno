---
title: Current status
description: A dated separation of implemented source, active public evidence, release-candidate availability, disabled writes, and remaining physical gates.
---

> Snapshot: 2026-08-31. `docs/STATE.md` and `release/V1_2_0_READINESS.json` are the live repository authorities and may advance after this page.

## Product direction

The current source line is claimable person-to-person native software: adopt or create on the Mac, evolve through the paired Companion, deliberately Ship inspectable source, witness it through active generation 0.8, optionally Claim one exact encounter through the additive Claims contract, and build/sign/install locally for the recipient.

## Native release

`v1.2.0-rc.1` is an explicitly labeled release candidate built from clean source commit `122f121732497e7a2f60e7daeb1b57882ebf9964`. Its immutable DMG was Developer ID signed, Apple-notarized, stapled, mounted and Gatekeeper-verified, published as a GitHub prerelease, and activated on the website candidate channel.

DMG SHA-256:

```text
7b98f99ddb004de7c8e031f7eb44216f0470f56a8333b63b6913d4c66154b212
```

The **stable 1.2.0 download is not activated**. Clean-Mac product acceptance and physical iPhone acceptance for RC1 are not complete, so stable promotion is false.

## Registry generation

Generation 0.8.0 is the client-trusted active contract generation under a pinned 2-of-3 release-authority policy and signed activation. Registry reads in the dark production deployment are healthy.

The production Registry relayer and production public Registry channel are disabled. Source implementation and active generation do not by themselves make public Ship/Update writes available.

## Claims

The additive `TohsenoClaimsV1` contract is deployed at:

```text
0x5012703d48d99224ac0035d58bc373de9e8b1934
```

Its signed activation and runtime/Registry binding verify in the dark deployment. The Claims index is enabled and has zero records in the captured readiness evidence.

**Production Claims writes and the Claims relayer are disabled.** The product must not advertise a live Claim flow or report a simulated/pending result as Claimed.

## Remaining activation evidence

Stable/public writes remain blocked until owner-attended acceptance proves the complete ordinary path:

- one real production Ship and immutable edition;
- a second identity's physical Companion Claim and canonical receipt;
- Claim while the recipient Mac is offline, then exact-release preparation;
- recipient-local Apple signing and physical iPhone installation;
- a later Update preserving the existing Claim and edition;
- private Follow reconciliation;
- live receipt/metadata website paths;
- exactly one Ship in the public timeline;
- clean-Mac product acceptance.

No local test, simulator, operator row, pending transaction, deployed bytecode, or source-complete feature substitutes for that evidence.

[Read the exact readiness record](https://github.com/jpfraneto/tohseno/blob/main/release/V1_2_0_READINESS.json).
