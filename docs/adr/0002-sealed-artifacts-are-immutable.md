# ADR 0002: Sealed continuity artifacts are immutable

- Status: Accepted
- Date: 2026-07-10

## Context

If archived bytes can be modified to prepare reflection, their digest, filename, backup references, and proofs can drift. It becomes impossible to say which exact private artifact an event recorded.

## Decision

Once an artifact is sealed and committed, its canonical bytes and sealing metadata are immutable. Any normalization, transcoding, terminalization, redaction, or reflection-preparation output is a new artifact with its own digest and an explicit derivation relation.

Mutable in-progress checkpoints are not sealed artifacts. The commit boundary promotes a checkpoint/output into an immutable artifact and event reference.

## Consequences

- Stores verify digest-to-bytes relationships when loading by digest.
- Importers preserve exact bytes when integrity must be retained and clearly label transformations.
- Reflections link to the exact input artifact without modifying it.
- Deletion may remove an artifact where policy permits; it does not rewrite it.
- Storage adapters need atomic commit or deterministic reconciliation around artifact, event, and checkpoint clearing.

## Non-goals

This ADR does not require every artifact to be embedded in an event record or stored forever. Large media may use immutable blob references, and retention remains an app policy.
