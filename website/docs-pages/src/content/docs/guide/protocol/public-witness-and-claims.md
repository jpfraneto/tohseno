---
title: Public witness and Claims
description: The ancestry-free public checkpoint, signed catalog, additive Claims contract, exact actions, and non-transferable receipt.
---

## Public checkpoint

`tohseno.public-checkpoint/1` is a narrow ancestry-free projection containing only fixed protocol/schema/scope, generation/chain/Registry coordinates, random ShotID, witness-local sequence, prior public checkpoint, and canonical publication time.

It contains no local lineage head, intention, Genome, source, build, artifact, Expression, Version, Feedback, token relation, controller, installation/end-user data, content, or free text—and no hashes derived from private values.

Its commitment is `SHA-256(RFC8785(public_checkpoint))` and becomes the Registry head. Checkpoint 1 has no predecessor; each continuation names the prior public checkpoint and advances by one. The checkpoint alone does not prove authority or on-chain acceptance.

## Signed catalog

The off-chain `tohseno.catalog-release/1` binds that public witness to exact release metadata, source artifact bytes, source-tree commitment, closed Xcode recipe, safety classification, permissions, and optional parent. Companion signs the complete structured manifest. After publication, clients pair it with independent receipt and live-state evidence.

## Additive Claims contract

`TohsenoClaimsV1` references the exact active ShotRegistry but does not change generation 0.8 bytecode or ABI. It is non-upgradeable and separately activated. It has no owner mint, policy editor, supply override, pause, confiscation, arbitrary URI replacement, or transfer/approval path.

`OpenClaimEdition` binds the Claims domain, chain, trusted Registry, ShotID, immutable policy, current controller, nonce, and deadline. The current Shot controller authorizes it through ERC-1271.

`ClaimSoftware` binds the trusted Registry, ShotID, claimant Tohseno account, exact release digest, exact current public checkpoint, Claim-mark commitment, claimant nonce, and deadline. Execution checks that the checkpoint remains current. Duplicate Claim, closed/exhausted edition, stale nonce/head, wrong chain/contract, expiry, or invalid signature reverts.

Token IDs begin at one. Claim number is sequential per Shot and exists only after canonical execution. Transfer and approval paths revert. The contract stores no app name, icon, source, raw gesture, device, or installation information.

## Activation and relaying

Clients trust Claims only through a signed activation that binds chain, address, runtime hash, expected Registry, source/generation, deployment evidence, and activation ordering. The relayer can submit only exact allowlisted account-bootstrap, edition, and Claim calls. It stores jobs first, retries idempotently, and never holds a DeviceKey.

Current write availability is separate from implemented code and signed activation; see [current status](/guide/reference/current-status/).
