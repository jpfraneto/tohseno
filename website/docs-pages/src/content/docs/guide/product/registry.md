---
title: Registry
description: Discover public software facts, privately follow Builders, and receive high-signal updates.
---

Registry is the living public world around software. It is not a second factory and not an App Store grid.

## Discover

Discover is a deterministic public timeline with a closed event set:

- `shot.shipped` — the one birth of a public Shot;
- `shot.updated` — each later public release;
- `shot.forked` — an exact parent-release relationship;
- `claim.edition_closed` — a finite or timed edition reached its boundary.

Events are ordered from canonical receipt and block facts, with idempotent reorg handling. Individual Claims do not flood the timeline; counts and closure summarize them.

## Following

Following filters public events using exact BuilderIDs held as a private preference in the encrypted Mac/Companion relationship. Follow and Unfollow are idempotent, durable while either peer is offline, and survive handle changes.

There is no public follow graph, follower count, leaderboard, popularity score, token, or server-owned social identity.

## Updates

Updates is a private high-signal inbox for facts that directly involve the person: a claimed app changed, preparation is ready, a fork of their Shot shipped, their edition closed, an alias changed, publication needs approval, or a private evolution completed. Stable IDs and paired-device reconciliation prevent restart spam.

Generic Discover traffic and individual Claims do not enter Updates.

## Verification boundary

The service database is an index. A security-sensitive client independently checks the signed activation, active chain and contract, current Builder authority, canonical transaction receipt, current Registry head, Companion-signed catalog, public checkpoint digest, and exact source bytes.

Catalog reachability is not labeled chain verification. A card can be visible before the client has enough evidence to install it; the decisive verification runs when a person chooses Claim, Install, or Fork.

See [current status](/guide/reference/current-status/) for which public reads and writes are actually activated now.
