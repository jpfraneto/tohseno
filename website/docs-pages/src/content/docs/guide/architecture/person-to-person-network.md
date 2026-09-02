---
title: Person-to-person network
description: How source is deliberately shipped, independently verified, locally signed, installed, and forked.
---

The network moves native software as inspectable source plus narrow public evidence. It skips App Store submission and review for this direct path; it does not skip Xcode, Apple signing, provisioning, Trust, Developer Mode, or physical device verification.

## Publish deliberately

`tohseno init [path]` non-destructively prepares an ordinary Xcode project as a public candidate with a stable random ShotID. `tohseno deploy` is the explicit Ship/Update command. Private creation and evolution do not publish automatically.

The Mac creates a deterministic sanitized snapshot in a temporary owner-only directory. It excludes VCS internals, build output, DerivedData, user data, caches, environment files, private Tohseno state, pairing/log state, Apple signing material, and known secrets. `.gitignore` is not used as the security boundary.

## Companion approval

The catalog release binds active generation and witness, ShotID, BuilderID, immutable release ID, publication time, display metadata, artifact digest and byte length, source-tree commitment, Xcode recipe, platform facts, safety classification, permissions, optional exact parent release, expected checkpoint, and public-checkpoint digest.

Companion recomputes that manifest and the RegisterShot or AppendCheckpoint action. It approves only that exact release and required account bootstrap. The DeviceKey never leaves the phone.

## Public agreement

A release becomes discoverable only when these agree:

1. manifest DeviceKey signature;
2. current BuilderAccount authority;
3. activated generation, chain, and Registry;
4. canonical transaction receipt and block;
5. current Registry head and checkpoint sequence;
6. manifest public-checkpoint digest;
7. staged source bytes and declared SHA-256.

The chain witnesses Builder-controlled continuity. The signed catalog binds that continuity to one release. Content addressing binds the release to the fetched bytes.

## Recipient safety

Paths are normalized, ordered, relative, bounded, and collision-checked. Symlinks, hard links, special files, traversal, oversized content, archive ambiguity, and high-confidence secrets fail.

Only a narrow Green Install Profile may build automatically: an ordinary native iOS app with pinned dependencies and no arbitrary Run Script, custom executable, unsafe build rule/plugin, unsupported entitlement, or unsafe archive. Review-classified source requires explicit **I Reviewed the Source — Build**. Unsupported source does not build.

The recipient signs locally. Forking fixes the exact parent release, assigns new private/project and Shot identities, and records the parent relation. It never borrows parent authority.
