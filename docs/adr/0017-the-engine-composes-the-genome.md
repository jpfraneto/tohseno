# ADR 0017: The engine composes the Genome; the harness reads the intention

Status: accepted

Date: 2026-08-19

Supersedes: [ADR 0012](0012-intention-led-app-birth.md)'s conception phase.
ADR 0012's core claim — that an app is born from one exact preserved human
intention, and that the engine rather than a harness owns acceptance — remains
accepted and unchanged. What this decision removes is the separate planning
conversation ADR 0012 introduced to get there.

## Context

ADR 0012 put a Conception phase in front of every birth. Before any app code
existed, the selected intelligence had to read `.tohseno/CONCEPTION.md` and
return one strict JSON object containing a complete Birth Plan and Experience
Contract: target actors, stable requirement IDs with origins, planned Apple
capabilities, product journeys, a topologically ordered organ graph, forbidden
substitutions, a completion contract, and a proposed Genome. The engine then
validated that object against the JSON Schemas plus roughly fifteen
cross-object rules — byte-exact intention excerpts, capability↔organ↔scenario
coverage, physical-verification coverage, RFC 8785 canonical digests — and ran
up to five repair passes against whichever rule failed first.

The intent was sound: make the intelligence commit to an interpretation the
engine could check before it wrote code.

In practice it inverted the cost of a Shot. The hardest, most failure-prone
part of a birth became the part that produced no app. A run against a real
intention on 2026-08-18 spent eight hours and thirty-nine minutes in this phase
and changed zero files; its final record reads `outcome: failed`,
`files_changed: []`, with the Shot tree byte-identical before and after. The
last event before the gap reports "no source-tree file change is visible yet"
two minutes in. Nothing had gone wrong with the product idea. The harness was
grinding on a schema.

Two properties made that outcome likely rather than unlucky:

- The rules are conjunctive and cross-referential. Satisfying one commonly
  breaks another, so repair passes do not converge monotonically.
- Nothing in the phase is checkable against reality. A Birth Plan can satisfy
  every rule and still describe the wrong app, and it can violate a rule while
  describing the right one. The gate measured internal consistency, not the
  product.

Meanwhile the gates that do measure the product — Release build, install on
the owner's iPhone, launch, and the declared trial — sat behind the phase that
never finished.

## Decision

The engine composes each Shot's initial Genome, Birth Plan, and Experience
Contract deterministically from the preserved intention. No intelligence is
asked to author them, and there is no separate planning invocation.

`tohseno create` now runs:

```text
preserve the exact intention
    ↓
engine composes and accepts the Genome and Expression   (no harness)
    ↓
one harness invocation against the exact intention
    ↓
build · test · verify · install · accept
```

The synthesized substrate is deliberately thin. It asserts one must-level
requirement — that the app fulfils the preserved intention — one owner actor,
one journey, one scenario, and the two protocol-substrate organs that carry
installation identity and signed continuity. It claims no Apple capabilities
and quotes no text from the intention, so there is no citation that could be
fabricated and no interpretation that could narrow the product.

The materialization task states this explicitly: the intention is
authoritative, interpreting it is the harness's work, and `genome.json` and
`birth-plan.json` are engine-composed protocol substrate that never narrow the
intention. A harness that reads the plan expecting a specification will find
nothing to under-build from.

### Why the Genome stays

`protocol/SPECIFICATION.md` binds `genome_digest32` into every VersionRecord.
A Shot cannot hold a Version without an accepted Genome. Removing the concept
would change the record encoding, the ledger, the conformance vectors, the
contracts, and the Companion SDK, and would invalidate the signed lineage of
every Shot already recorded. The Genome is protocol substrate; it was only ever
the *authoring* of it that needed an intelligence, and it no longer does.

### Supervision

An unattended run has nobody watching it, so the supervisor now bounds one
harness invocation two ways, measured against the Shot tree rather than harness
output:

- `TOHSENO_HARNESS_STALL_SECS` (default 30 minutes) — the harness wrote nothing
  to the Shot for this long.
- `TOHSENO_HARNESS_MAX_RUNTIME_SECS` (default 4 hours) — one invocation reached
  no acceptance gate in this long.

Either bound sends SIGTERM and records the reason as the execution's validation
evidence, so the completion record says why the run ended instead of reporting
an unexplained absence of an accepted Version. A harness that prints
continuously while writing nothing is not working, which is why progress is
measured by tree hash.

## Consequences

- A birth is one harness invocation. The first thing any intelligence sees is
  the exact text the human wrote.
- Acceptance is unchanged and remains the engine's. Release build, install,
  launch, the Experience Trial, protocol conformance, and offline verification
  all still gate a Version. Only the pre-build planning gate is gone.
- Every acceptance rule that governed a Birth Plan still exists and still runs.
  A Shot carrying a rich, intention-derived plan validates exactly as before;
  `engine/src/anky_fixture.rs` keeps proving that. The author changed, not the
  rules.
- `ExecutionMode::Conception`, `ExecutionPhase::Conception`, and the
  `conception` presentation state are no longer produced but remain readable,
  so private records written by releases through 0.9.0 still load.
- `.tohseno/CONCEPTION.md` is no longer written. The private planning artifacts
  (`conception-input.json`, `conception-output.json`,
  `accepted-conception-output.json`, `birth-plan.json`,
  `experience-contract.json`) keep their filenames and schemas; they are now
  engine output rather than harness output.
- The thin plan makes the Experience Trial correspondingly thin: one scenario
  instead of a per-capability matrix. Deep capability-level verification is now
  the harness's judgement under the intention, checked by the real gates,
  rather than a matrix declared before any code existed.
