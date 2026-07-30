# ADR 0004: a Shot is the persistent identity of a committed coherent intention

- Status: accepted
- Date: 2026-07-29
- Supersedes: the parts of ADR 0003 that identify a Shot with one app folder
- Preserves: ADR 0003's visible-folder workflow and immutable completed worlds
- Superseded in part by: ADR 0006 for undeployed v0.7 contract-ABI
  compatibility, public identity boundaries, and the narrowed witness

## Context

The Genesis candidate already has a strong identity and verification substrate:
a random stable Shot ID, a chain-scoped Builder identity, exact input and source
commitments, signed immutable Apple-app records, crash-safe completed worlds,
offline lineage verification, a reusable Apple Fascia, and narrow public
contracts.

The v1 names are narrower than the thing they preserve. `ShotRecord` represents
one accepted state of one Apple application, while the Shot ID already survives
facts such as the folder name and public relations. Treating the app, bundle
identifier, repository, current source, or token as the Shot would make those
replaceable expressions into identity.

The factory also has a static `Genome` used to instruct materializing agents.
That is the constitution of the press. It is not the changing, owner-accepted
interpretation of one Shot's enduring constraints.

## Decision

A Shot is the persistent identity created when an authorized actor commits a
coherent human intention into reality.

The original intention is preserved as exact source material. A Shot-specific
genome is a separate, revisioned interpretation of what must remain true. An
expression is one concrete manifestation. A version is one immutable state of
one expression. Feedback binds to the exact version that produced an
experience. An evolutionary intent proposes change. An evolution is the
authorized, verified transition between accepted versions.

New canonical history is an append-only sequence of deterministically encoded,
content-addressed, signed lineage actions. Current-state files are derived
snapshots. Actions can refer to unavailable or private artifacts by digest and
must describe availability honestly.

The accepted machine-readable genome revision is canonical. `GENOME.md` is its
deterministic rendering, and drift fails verification. An owner edit is input
to a new genome proposal; it does not mutate the accepted revision in place.

The existing `tohseno.shot/1` record remains frozen and valid. It is interpreted
through a compatibility adapter as an accepted version record for the initial
Apple expression. Existing signatures are neither rewritten nor wrapped in
claims of historical facts that were not recorded.

The existing Builder identity remains the authority system. Ownership actions
change continuity authority; they do not claim authorship of generated source.

The existing Apple Fascia remains the concrete Apple capability substrate.
Neutral organ declarations can describe what a capability provides, owns,
requires, emits, consumes, satisfies, and tests, but do not replace the
normative Fascia sources or broaden current materialization beyond native Apple
software.

The existing public contracts remain optional witnesses:

- the registry can bind an authorized controller to a public accepted head;
- lineage checkpoints can be content commitments;
- relations can point to handles, app-store records, or tokens;
- no private intention, feedback, repository, transcript, or subjective genome
  interpretation belongs on-chain.

A token association is a signed relationship. It is never a Shot identifier,
ownership key, expression identifier, or requirement for a Shot. The existing
v1 Appcoin ABI is loaded as a Token Association compatibility action.
Accordingly, Anky may be an independently owned Shot and associate `$ANKY` on
Base (`eip155:8453`) without identifying the token contract with Anky, TOHSENO,
any `$TOHSENO` association, or either Shot's controller. No token address is
implied by that example.

Nodes validate and preserve public lineage actions and report referenced
artifact availability. The candidate node stores action records only; a future
node may additionally preserve artifact bytes without changing lineage law.
Nodes may possess partial histories and different artifact subsets. They agree
on deterministic byte, signature, and available-segment validity.
Authority is a separate result: a segment with an unavailable parent remains
explicitly unresolved until enough causal context is present to reduce it
under the candidate authority policy. Nodes do not decide whether an intention
is metaphysically coherent, run a global mutable database, or require
distributed consensus over one universal head.

## Canonical object meanings

- **Coherent Intention** — the preserved human declaration that something
  should exist.
- **Shot** — the persistent identity created when that intention is committed
  into reality.
- **Commitment** — the signed action that begins the Shot.
- **Genome** — the current accepted operational interpretation of what must
  remain true.
- **Expression** — one concrete manifestation of the Shot.
- **Organ** — a bounded declared capability used by a software expression.
- **Version** — one immutable identifiable state of an expression.
- **Feedback** — experience attached to the exact expression version that
  produced it.
- **Evolutionary Intent** — an authorized proposal for changing an existing
  Shot.
- **Evolution** — a verified transition from one accepted version to another.
- **Lineage** — the signed history connecting origin, ownership, expressions,
  states, experience, and change.
- **Ownership** — authority to approve continuity-changing actions.
- **Token Association** — an optional chain-specific economic relationship
  that does not replace Shot identity.

## Continuity rules

1. The Shot ID does not change when a folder, name, repository, bundle,
   platform, expression, owner-facing description, deployment, or token
   relationship changes.
2. An expression has its own stable ID and version sequence.
3. A normal evolution preserves the accepted genome. A genome mutation requires
   an explicit proposal and acceptance by current authority.
4. A failed materialization cannot produce an accepted Version action.
5. A descendant creates a new Shot ID and signs its parent relationship. A
   copied folder without that action is only a copy.
6. Receiving or importing public records does not transfer ownership.
7. Derived state must be reproducible. Divergence in a cache cannot supersede
   signed canonical actions.
8. Missing, unknown, private, local, public, replicated, verified, and anchored
   are distinct states. None may be upgraded by implication.
9. A partial public segment may be retained after schema and signature
   verification, but it is not authority-verified until its causal context is
   available and valid.

## Generated-repository rule

The visible folder remains the builder's direct working surface. It now also
contains exact human-readable intention and genome surfaces, a concise next
evolutionary-intent surface, immutable version and feedback locations, and a
private working area excluded from publication. Machine-readable counterparts
and signed lineage live under `.tohseno`.

Shot-level documents and private material are not application source. They are
excluded from living-tree source materialization and capability scanning while
remaining committed through their own canonical records.

## Compatibility

The v1 Apple record, signature, Fascia, conformance, embedded metadata, public
actions, contract ABIs, legacy N+1 adoption, `latest_shot` TOML alias, and stable
v0.6 state isolation remain supported.

Migration creates neutral projections and marks unavailable historical fields
as unknown. It does not alter completed Evolution directories, manufacture
old signatures, or silently publish private material.

A portable Shot bundle is a verified protocol projection with explicit
omissions and availability, not an ownership transfer and not merely a source
archive. Public projection never includes exact private intention material
merely because its digest was publicly committed, and never relabels private
availability.

## Consequences

TOHSENO can preserve a Shot across more than one expression without turning the
current Apple factory into a generic generator. Exact feedback and evolution
become verifiable lineage rather than adjacent notes. Nodes can replicate
useful public continuity without a blockchain or shared database becoming the
Shot. Existing candidate records remain meaningful and verifiable.

The protocol must carry more explicit identifiers and availability metadata.
Owner-key rotation and contract ownership transfer require authority proofs
beyond the current initial-key-only offline verifier. Until that proof chain is
implemented and verified, the limitation remains explicit rather than being
papered over.
