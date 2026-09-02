---
title: Ship, Claim, and Update
description: The public lifecycle and why publication, encounter, preparation, and installation stay separate.
---

## Ship happens once

The first accepted transition from a private/local candidate to a discoverable Shot is **Ship**. It produces exactly one `shot.shipped` event and an immutable `shipped_at` derived from canonical registration evidence.

That first Ship also opens exactly one immutable Claim Edition. The Builder selects one policy in the exact Companion approval:

| Policy | `maxClaims` | `closesAt` |
| --- | ---: | ---: |
| Open | `0` | `0` |
| Limited | greater than `0` | `0` |
| Timed | `0` | future timestamp |
| Limited and timed | greater than `0` | future timestamp |

Zero supply means unlimited; zero closing time means never. The edition cannot be reopened, edited, extended, reset, or replaced by an Update or ownership change. There are no prices, tiers, auctions, allowlists, paid mints, or administration controls.

## Update happens afterward

Every later accepted public release is an **Update** and emits `shot.updated`. It advances the public checkpoint but does not create another Ship or edition. The same developer command can end with `Shipped.` for the first release and `Updated.` later.

## Claim is one exact encounter

A Claim says one Tohseno smart-account identity encountered one Shot at an exact release and public checkpoint. It includes a Claim-mark commitment, is gas-sponsored through a constrained relayer, and—only after canonical execution—produces a non-transferable `TohsenoClaimsV1` ERC-721 receipt. One account can Claim a Shot once.

A Claim is not a purchase, license, unique-human proof, transferable asset, Shot ownership, Builder authority, or Apple installation evidence.

## Claim then prepares; it does not fake installation

Canonical Claim confirmation durably queues preparation of the exact release through the existing private Companion-to-Mac channel. The Mac independently verifies and downloads that release. If the Mac is offline, the request waits. If a newer Update exists, the claimed release is still prepared first and **Update available** is shown separately.

Xcode, local Apple signing, Trust, Developer Mode, cable reachability, and physical inventory verification still decide installation. A Claim can succeed when installation cannot. An install can be refreshed without changing the Claim.

## Fork

A Fork binds one exact parent release, creates a new random child ShotID, and remains private until explicitly shipped. Its first public release creates its own one Ship and one immutable edition. It never reuses parent authority or republishes the parent.

Current Claims and public-write availability is intentionally fail-closed; see [current status](/guide/reference/current-status/).
