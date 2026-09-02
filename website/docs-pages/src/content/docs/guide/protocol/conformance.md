---
title: Deterministic conformance
description: What must be checked before a Tohseno artifact, action, generation, or lineage can be called conformant.
---

Conformance is offline and fail-closed. It never asks an LLM to infer protocol meaning from prose. A `tohseno.conformance/1` report is conformant only when every applicable check passes; `fail` and `not_checked` both prevent conformance.

## Local Evolution checks

The required families include:

- closed schema, shape, canonical bytes, digest and signature;
- complete device authority evidence;
- contiguous lineage and stable identities;
- exact Genesis input, source-tree and reusable Fascia commitments;
- no-symlink path law and required Fascia files/target membership;
- InstallationKey properties and finite capability declarations;
- dependency, storage, network and privacy boundaries;
- embedded provenance equality;
- Apple bundle/build identity and offline build.

Installation, launch, Apple signing, publication, and Registry witnessing are operational observations above the pure protocol. Reports distinguish implemented, automatically verified, manually observed, deployed, published, and pending.

## Generation and public witness checks

BuilderAccount v2 verifies exact schema/domain/action digest, recovery coordinates, live permission/nonces/deadlines/epoch, and current state.

ShotRegistry v2 verifies generation dispatch, domain, commit/reveal preimage, EIP-712 digest, detached low-s P-256 evidence, live ERC-1271 authority, checkpoint transition, sequence separation, privacy, public-checkpoint bytes, and receipt boundary.

Contract generation verifies exact definition digest, every source/artifact byte, compiler profile, CREATE2 arithmetic, EIP-7951 requirement, and absence of activation claims. Activation separately verifies definition coordinates, instantiated runtime hashes, deployment/block/probe evidence, causality, threshold policy, approvals, and client trust pin.

## Neutral lineage checks

The reducer verifies payload/action digests, signatures, adjacency, actor authority, original intention, explicit Genome acceptance, Organ graph and gates, exact Version verification, Feedback/version binding, Evolution scope, ownership changes, availability, token separation, and v1 adaptation without fabricated facts.

## Test vectors

Frozen JSON vectors make cross-language behavior testable. Vector generators write to standard output; committed files change only through a deliberate new version or accepted protocol change. Mutation tests cover duplicate JSON, bad lengths, changed bytes, high-s signatures, wrong domains, stale links, and other fail-closed cases.

See the exact [protocol conformance source](https://github.com/jpfraneto/tohseno/blob/main/protocol/CONFORMANCE.md).
