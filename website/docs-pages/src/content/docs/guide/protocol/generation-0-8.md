---
title: Contract generation 0.8
description: Reproducible contract definitions, active generation evidence, BuilderAccount, ShotRegistry v2, commit/reveal, and checkpoint sequencing.
---

Generation 0.8.0 is the active client-trusted public-witness generation. It adds successor BuilderAccount and ShotRegistry behavior without rewriting frozen v0.7 encodings.

## Definition is not activation

`tohseno.contract-generation/1` closes over exact source inventory, compiler and EVM profile, ABI/bytecode artifacts, creation hashes, runtime templates, target chain, EIP-7951 P-256 requirement, and conditional CREATE2 coordinates.

Its RFC 8785/SHA-256 digest proves a reproducible build definition. It contains no transaction, block, signature, authority, deployment status, or trust root. Predicted addresses remain arithmetic until a separately signed activation binds observed deployed code and an activation block.

The engine pins the digest of a 2-of-3 release-authority policy and verifies the threshold-signed activation under that trust root.

## BuilderAccount

BuilderAccount validates authorized P-256 actions and provides separated device administration and delayed recovery. It is deployed lazily for a first public Builder action. A constrained relayer may deploy the exact predicted account; a correct front-run deployment is idempotent. A random EOA is never substituted for BuilderID.

## ShotRegistry v2

The EIP-712 domain is:

```text
name = "TOHSENO ShotRegistry"
version = "2"
chainId = 4663
```

Registration is permissionless commit plus signed reveal. The commitment binds controller, independent random ShotID, fresh private salt, Registry, chain, and deadline. Reveal is valid from 60 seconds after commit through the earlier of 24 hours or the deadline. Successful reveal deletes the commitment and creates checkpoint 1.

Append requires the exact previous head and increments `checkpointSequence` by one. Transfer changes controller while preserving head and checkpoint count.

## Sequence separation

Registry checkpoint sequence is witness-local. It is never compared with local lineage sequence, Version ordinal, `CFBundleVersion`, Git count, or App Store history. A Shot first shipped from local Version 12 still registers at checkpoint 1.

The Registry head is the digest of `tohseno.public-checkpoint/1`, never a private lineage action. Live controller eligibility, ERC-1271 acceptance, receipt evidence, and active-generation verification supply authority around that head.
