---
title: Release and activation
description: The distinct ceremonies for a Mac artifact, public website pin, contract generation, Registry activation, and Claims activation.
---

Source completeness is not release evidence.

## Native Mac release

A distributable candidate requires:

1. exact clean source commit and successful required verification matrix;
2. universal application/factory payload assembled from that source;
3. exact Developer ID signature and hardened-runtime verification;
4. Apple notarization accepted and ticket stapled;
5. mounted-DMG manifest, secret, architecture, identity and Finder-layout checks;
6. Gatekeeper acceptance from the mounted artifact;
7. immutable artifact publication and origin round-trip SHA-256 match;
8. website channel pinned to that exact HTTPS URL and digest;
9. independent clean-Mac product acceptance for the intended channel.

A release candidate is visibly labeled. Stable remains closed until its own gates pass. A rejected candidate remains immutable evidence and is not silently recycled.

## Generation 0.8 activation

The reproducible `ContractGeneration` definition is frozen separately from deployment. The current active generation uses a pinned 2-of-3 release-authority policy and threshold-signed activation binding exact definition digest, predicted/observed coordinates, instantiated runtime hashes, deployment transactions and blocks, activation block, and EIP-7951 P-256 probe.

Committed activation records are immutable. A defect requires a successor generation or successor activation, never editing the old record.

## Claims activation

`TohsenoClaimsV1` is additive and separately activated. Its activation binds its exact chain address, runtime hash, expected active ShotRegistry, source/generation facts and deployment evidence. Activation is necessary but not sufficient for public Claims.

Claims remain dark until constrained production writes, released matching clients, website/read models, canonical indexing, and owner-attended two-identity physical acceptance all agree. A deployed contract, signed activation, local test, simulator gesture, pending transaction, or database row cannot substitute for those facts.

## Repository guard

There is no open-ended new contract-generation or deployment ceremony on `main`; `scripts/deploy-candidate.sh` fails closed by design. Do not add or bypass one from documentation instructions.

The current exact release and activation picture is summarized in [current status](/guide/reference/current-status/) and remains governed by `release/` evidence.
