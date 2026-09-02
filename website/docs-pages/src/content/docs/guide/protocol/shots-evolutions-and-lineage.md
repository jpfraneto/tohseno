---
title: Shots, Evolutions, and lineage
description: The frozen record chain, additive coherent-intention lineage, exact bases, Genomes, Expressions, Versions, Organs, and Feedback.
---

## Frozen Shot records

`tohseno.shot/1` begins at sequence `1` with `previous = null`. Every later record references the immediately preceding Evolution commitment. `bundle_version` equals `sequence`; ShotID, BuilderID, bundle ID, and Fascia identifier remain stable. Timestamps are exact UTC `YYYY-MM-DDTHH:MM:SSZ`.

The one legacy-adoption exception starts at historical `N + 1`, declares `origin.kind = legacy_adoption`, commits the adopted source, and keeps `previous = null` because no prior protocol commitment existed. Later records continue normally. Missing history is never fabricated.

## Additive lineage `/2`

A signed lineage is an append-only sequence. Action 1/null is the commitment to original intention, initial controller/key, origin, and time. Each action carries one closed payload, its RFC 8785/SHA-256 digest, the previous action commitment, ShotID, current Builder actor, time, and private/public handling declaration.

The reducer rejects gaps, replayed links, backward time, changed ShotID, wrong actor/signer, and action-specific invalid transitions. An unanchored segment can prove its own bytes, signatures, and adjacency but not the authority supplied by a missing prefix.

## Intention, Genome, and Version

The original Intention may appear once and must match the first commitment. It cannot be replaced.

A Shot-specific Genome changes only through a proposal and explicit acceptance. Revision 1 has no base; later proposals name the current revision and digest. Ordinary implementation work cannot mutate the Genome implicitly.

A Version binds the exact Shot and Expression, expression-local ordinal, accepted Genome, source digest, materialization provenance, capability graph, successful VerificationResult, known incompleteness, actor, and optional build identity. A failed VerificationResult is honest history but cannot authorize a Version.

## Evolution and exact base

Feedback binds one exact ExpressionID and VersionID. An EvolutionaryIntent references selected signed Feedback action commitments and a precise `from_version_id`. A completed Evolution binds the target Version and any accepted Genome change. Exact-base mismatch is refusal, never an implicit rebase.

## Organs

Organs are immutable capability declarations per `(ExpressionID, organ_id)`, not mutable source folders. The full declarations are sorted by `organ_id`, RFC 8785 encoded, and SHA-256 committed as the capability graph. Every declared acceptance test requires a matching verification gate. Changing the graph requires an organ-scoped desired change.

Availability states are observations, not a ladder: private is not weaker public, and on-chain anchoring does not imply that artifact bytes are available.
