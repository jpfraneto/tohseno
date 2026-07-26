# ADR 0005: Signed Shot records precede registry projections

- Status: Accepted
- Date: 2026-07-25

## Context

Local repository materialization, app runtime state, distribution lifecycle,
and public discovery are different domains. Collapsing them would make local
ownership depend on public infrastructure and would turn build or runtime
events into false distribution claims.

## Decision

A Shot has one stable ID and evolves through append-only signed public records.
Its distribution lifecycle is exactly `EVOLVING`, `PUBLISHED`, and
`APP_STORE`. Public records use deterministic canonical JSON, domain-separated
signatures, computed content hashes, explicit Builder authority, and per-Shot
previous-hash links.

Record schemas, canonicalization rules, signature domains, and public projection
schemas carry intrinsic protocol versions. Those versions describe how record
bytes are interpreted; they are not Shot lifecycle states, release versions,
or Evolution numbers.

These records are portable Builder attestations: a signature binds the
declared key to the record bytes but does not independently prove ownership,
claim accuracy, or which competing genesis history is globally preferred.
Registries are deterministic projections of one accepted verified history.
Nodes are replaceable indexes and transports for those records. The
repository's Bun and SQLite node is a reference implementation only.

Local repository creation is named repository creation or materialization,
never publication. Generated apps remain fully usable without protocol
participation.

## Consequences

- A node outage or operator cannot revoke ownership of a Shot.
- Given the same accepted record history, another implementation can replay it
  and derive the same public projection.
- Public disclosure is explicit and exported copies may persist; private
  prompts and app-user content have no wire fields.
- Builder, app-runtime, release, Apple, and external-action identities remain
  separate.
- Appcoin links identify deployment-agnostic external assets. They neither
  define the protocol nor authorize deployment.
- Production trust roots and cross-node resolution of competing valid genesis
  histories remain Open; protocol v1 supplies no consensus rule.
