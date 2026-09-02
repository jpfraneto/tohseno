---
title: Private and public data
description: The default privacy boundary and the exact information deliberately made public by Ship and Claim.
---

Private is the default.

## Stays private

- Raw intentions, prompts, feedback, reference images, private lineage and local activity.
- Absolute source paths, dirty working-tree details, harness logs and execution receipts.
- Phone and Mac pairing secrets, mailbox capabilities, recovery words, DeviceKey private material and InstallationKey private material.
- Apple Account credentials, signing private keys, provisioning profiles, certificate material, team and physical installation evidence.
- Device names, IP addresses, private app data, local Follow preferences and Updates read state.
- Managed provider secrets, operator tokens, payment credentials and private balance authorization.

An exact intention may be disclosed to the configured coding route because implementation requires it. That is a chosen execution boundary, not publication.

## A public Ship contains

The sanitized source artifact and a closed Companion-signed catalog: public display metadata, ShotID, BuilderID, exact release identity, source/artifact commitments, bounded Xcode build recipe, minimum platform/device family, dependency facts, safety classification, permissions, optional parent release, public checkpoint and witness coordinates.

The public checkpoint itself is far narrower and contains no source or content digest. The signed catalog and content-addressed blob bind software bytes off-chain to that narrow on-chain witness.

## A public Claim contains

Claim deliberately exposes the relationship between one Tohseno account and one Shot at an exact release/checkpoint, plus a canonical Claim-mark commitment, per-Shot claim number and global non-transferable token ID.

It does not publish the physical phone, Mac, source path, Apple identity, install fact, private prompt, device name, raw gesture points, timing, pressure, motion, or behavioral inference. Public profile pages do not automatically aggregate every Claim even though canonical receipts are directly queryable.

## App-local Git is not public Registry

The generated app's `.tohseno/` directory includes Git-visible durable continuity plus explicitly ignored private subpaths. Committing that repository is not the same as Ship. Conversely, excluding `.tohseno/` from a source-tree digest avoids self-reference; it does not mean the whole directory should be ignored.

## No secret by inference

When the system lacks evidence, it records absent, unknown, private, or not checked. It does not replace missing private or historical facts with guessed prose.
