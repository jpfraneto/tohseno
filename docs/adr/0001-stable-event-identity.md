# ADR 0001: Separate stable event identity from artifact hashes

- Status: Accepted
- Date: 2026-07-10

## Context

Anky uses artifact hashes as filenames, reflection keys, ledger references, and idempotency inputs. iOS may prepare different sealed bytes before reflection, changing the hash. A digest therefore cannot simultaneously be immutable lifecycle identity and an identifier for exact bytes.

## Decision

Every `ContinuityEvent` has a stable event ID generated independently from artifact bytes. An event references one or more artifacts by media/codec and digest. Derived, normalized, or replacement artifacts express an explicit relation to the event and prior artifact; they never replace the event ID.

Artifact digests remain integrity/content identifiers for exact canonical bytes. They are not used as the primary lifecycle identity.

## Consequences

- Reflections, accumulation, idempotent subscribers, proofs, and synchronization refer to stable event IDs and optionally pin exact artifact digests.
- Existing `.anky` hashes remain readable compatibility references; migration uses sidecars/mappings rather than rewriting bytes.
- Imports must distinguish preservation of exact bytes from creation of a normalized derived artifact.
- A digest change does not silently create a new action event, and an event cannot conceal an artifact mutation.

## Non-goals

This ADR does not choose UUID/ULID or another event-ID encoding, nor does it finalize cross-device event merging. Those require contract fixtures and synchronization decisions.
