# ADR 0019: One bounded intent-to-usable-app transition

Status: accepted

Date: 2026-08-21

Supersedes ADR 0017's per-invocation supervision defaults and its requirement
that a new harness author an Experience Trial. It preserves ADR 0017's removal
of Conception, engine-composed Genome, and exact-intention authority.

## Context

The durable command service, exact-base evolution rule, one-factory lease, and
private Companion boundary already put every intention on one application
path. Work after admission was not similarly bounded. One intention could
cause an implementation harness followed by as many as five repairs, and an
installed developer configuration could retain eight. Each invocation could
receive four hours by default and twelve through an obscure override.

The materialization task also told the harness to build, test, launch, inspect,
produce a strict Experience Trial, repair mismatches, and repeat. TOHSENO then
ran many of the same gates again. Process-monotonic elapsed time made this hard
to see: on a sleeping Mac an execution could exist for six wall-clock hours
while its heartbeat reported only minutes.

## Decision

One admitted create or evolve command is one bounded transition:

```text
durably preserve exact intention
    ↓
one implementation harness invocation
    ↓
finite deterministic build and verification
    ↓
at most one targeted repair for a concrete code/build defect
    ↓
rerun deterministic gates once
    ↓
install · launch · record completion
```

The maximum is two harness invocations. Device absence, signing, provisioning,
network, protocol conformance, lineage, and other external or engine-owned
conditions never invoke repair intelligence.

Harness work has one shared wall-clock budget across implementation and repair:

- no source-tree progress for 15 minutes stops the current harness;
- total harness wall time defaults to 60 minutes;
- `TOHSENO_HARNESS_STALL_SECS` may raise the stall window only to 30 minutes;
- `TOHSENO_HARNESS_TOTAL_BUDGET_SECS` may raise the shared budget only to two
  hours.

Wall time derives from durable UTC timestamps and therefore includes sleep,
service recovery, process replacement, attempts, and device waiting. Details
shows total execution elapsed; heartbeats distinguish total execution time
from the current attempt.

The generated `.tohseno/TASK.md` is the whole harness constitution. It names
the exact intention, small continuity and data-preservation rules, app identity
needed by the build, and the State Transition draft. It does not teach Genome,
Birth Plan, Experience Contract, or protocol acceptance, and it tells the
harness not to load an outer TOHSENO workflow skill. For a birth, TOHSENO
deterministically stages its exact Fascia sources, documents, and resource
placeholders before the harness starts; the harness only adds those existing
files to the app target. It does not search the machine for protocol material.
TOHSENO owns final build, verification, recording, installation, and launch.

A targeted repair receives only the independently diagnosed criterion, does
not repeat `xcodebuild`, and cannot replace the first implementation pass's
State Transition draft.
Private `.tohseno` material is excluded from source progress and completion
diffs. Xcode folder resources are accepted as source membership and then
verified from the built bundle, avoiding a textual project-file false negative.

Older harness-authored Experience Trials remain readable and verifiable. A new
execution does not require one. The public protocol, accepted lineage, signed
Versions, Builder identity, and canonical byte encodings are unchanged.

Every terminal execution gets exactly one private
`.tohseno/executions/<execution-id>/state-transition.json`. The application
schema and migrations remain authoritative; this receipt only says whether
persistent state changed, summarizes the change, names migration paths when
present, and reports data safety. Missing or invalid harness output becomes an
`unknown` receipt and never causes a repair.

CLI, Studio, and Companion remain origins, not execution modes. All three
admit through the durable command journal and `ShotApplicationService`, then
use this same transition. The Mac remains the factory and the phone optional.

## Consequences

- A clean bounded failure replaces recursive autonomous diligence.
- The previously accepted app remains the accepted base when a candidate
  fails; a person may submit another intention.
- Missing devices wait without a harness and without holding the factory
  lease.
- Harness, model, route, and cost metadata remain private implementation facts
  under Details, so the coding harness stays replaceable.
- Local app ownership has no product-level count or retention limit. The one
  factory processes commands serially and never deletes an app because it is
  not installed.
- No billing, backend, queue, scheduler, daemon, transport, or public release
  ceremony is introduced by this decision.
