# ADR 0003: Store reflections separately from events and artifacts

- Status: Accepted
- Date: 2026-07-10

## Context

A reflection is derived feedback with a different lifecycle, provider/consent provenance, and deletion need from the action event and private artifact. Embedding it in or mutating the artifact would couple local continuity to a network/model outcome.

## Decision

`ContinuityReflection` has its own stable identity and record. It references a stable event and, when relevant, the exact input artifact digest. It records generation time, policy/provider provenance, and consent/disclosure metadata. It can be deleted independently without deleting or changing the underlying event/artifact.

An event is valid without a reflection. Provider unavailability, payment, refusal, or deletion of derived feedback cannot invalidate the committed local continuity fact.

## Consequences

- Reflection retries require explicit derivation/idempotency semantics rather than overwriting an event.
- Referential validation rejects accidental orphaning or a mismatched input digest.
- Local, deterministic, remote, and no-reflection policies fit the same event model.
- Privacy/deletion inventory treats provider disclosure and stored output separately.

## Non-goals

This ADR does not decide whether fragments may be reflected or whether remote reflection is always opt-in. Those are open product decisions.
