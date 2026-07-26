# ADR 0002: Keep private intent and app-runtime content out of shared services

- Status: Accepted
- Date: 2026-07-10

## Context

TOHSENO interprets private intent locally and can optionally index deliberately
public Shot records. Centralizing owner prompts or generated-app user content
would contradict independent ownership and create an unnecessary disclosure
boundary.

## Decision

Private intention and reference inputs remain in the local, gitignored Shot
provenance boundary. Structured logs remain content-free.

The reference node accepts only the closed public protocol record schemas. It
has no generated-app runtime endpoint and no designated field for prompts,
credentials, unpublished source, or app-user content. App-specific networking,
when declared, belongs to the independently owned app and is not a TOHSENO
factory service.

## Consequences

- Factory operation never requires transmitting the owner's raw intention.
- The public protocol cannot become an accidental prompt or user-content
  warehouse.
- Unknown node fields fail closed.
- The Builder remains responsible for reviewing arbitrary public summary text.
- Generated apps remain usable when every TOHSENO node is unavailable.

## Non-goals

This ADR does not prohibit a generated app from declaring its own backend or
public mechanics. It requires those mechanics to be explicit, independently
owned, and outside the TOHSENO factory and reference node.
