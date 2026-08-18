# ADR 0016: The user-facing abstraction is App → Intent → App on your iPhone

Status: accepted

Date: 2026-08-18

Supersedes: ADR 0015 as the description of the current user-facing product
surface. ADR 0015's persistent Local Workspace Service, durable command
journal, private Companion channel, capability model, transport suite, and
relay boundary all remain accepted and unchanged. This decision changes what
those things look like to a person, not what they are.

## Context

The factory that ADR 0015 restored works. Its user-facing surface did not.

Studio had grown into an IDE-shaped dashboard for administering TOHSENO. Three
simultaneous regions — YOUR SHOTS, INTENT / SHOT ACTIVITY, CURRENT APP /
EXECUTION — exposed Shots, Expressions, Versions, execution identifiers, a
twelve-step execution pipeline, harness and inference-route summaries, paired
devices, and separate Feedback, Marketing, and Evolution forms. Evolving an
app could take six deliberate actions: write Feedback, save it, choose it,
create an Evolution, confirm the exact Version binding, and submit.

Every one of those concepts is real and internally necessary. None of them is
the product. A person with an idea should not have to learn TOHSENO's ontology
to act on it, and a permanent CONNECT IPHONE button proves the interface was
organized around administering the system rather than using it.

The Companion made the same problem visible from the other side: the
repository contained the private protocol, the SDK, and a raw conformance
fixture, but no product a person could open on a phone.

## Decision

The canonical user-facing abstraction is:

**App → Intent → App on your iPhone.**

Externally, TOHSENO speaks in App, Create, Evolve, Waiting, Building, Ready,
Installing, Installed, Failed, Retry, and Details. Internally, the canonical
persisted objects are unchanged: Shots, Expressions, Versions, Feedback,
executions, lineage, and their exact byte encodings are neither renamed nor
migrated. Studio and the Companion are intentionally thin projections over the
same durable local application service.

### One presentation projection

`application/src/presentation.rs` collapses every internal execution phase into
six human states — `waiting`, `building`, `ready_for_phone`, `installing`,
`installed`, `failed` — and the workspace snapshot publishes one `presentation`
per app. Studio renders it verbatim and interprets no phase for itself. The
frozen companion snapshot schema is not changed to carry it; the Companion
derives the same states from the same execution vocabulary and both sides
assert against `fixtures/presentation-v1.json`, so the Mac and the phone can
never describe one app differently.

### Feedback stops being a ceremony

Writing what should change and pressing Evolve App is the whole operation. The
evolutionary transaction already preserves that exact intention in canonical
lineage, so no Feedback record is fabricated alongside it merely to satisfy the
internal distinction. Exact-Version Feedback remains a real, unchanged
capability through `tohseno advanced feedback` and the Companion
`feedback.write` grant; it is no longer a step between having an idea and
acting on it. The Studio-only feedback, marketing, and feedback-action HTTP
endpoints were removed with the UI that was their only caller.

### The exact base is bound for the person

Evolutions bind the current accepted Expression and Version at submission.
Nobody selects a Version. ADR 0015's stale semantics are unchanged and remain
fail-closed: a base that genuinely moved is refused, never silently rebased,
and both surfaces say so in one sentence while `Show details` keeps the exact
protocol reason.

### Waiting for the iPhone has no button

When source, build, test, and verification have succeeded but the development
iPhone is absent, the person is told the app is ready and asked for a cable.
There is no Install, Resume, or Continue control, because the persistent
service resumes delivery by itself. `waiting_for_device` still means what it
meant: not acceptance.

### One durable factory lease

Expensive local work is serialized by a single advisory lease file under the
private machine data root. It is deliberately the smallest mechanism that can
express "this Mac has your request but is busy": no queue, no scheduler, no
new command state, and no new protocol record. A runner that cannot take the
lease stays in its durable `queued` phase, which every surface already presents
as *Waiting to build…*, and starts by itself when the lease frees. The lease is
released while a verified candidate waits for a cable, so an absent phone never
blocks unrelated source work, and it is released by process exit, so a crashed
runner cannot strand the factory. The command journal remains the durability
and idempotency authority.

### Details is where complexity lives

Observability is not destroyed, it is moved. A deliberate Details surface
exposes exact status, internal execution phase, execution and app identities,
timestamps, accepted Version, selected harness, and inference route, plus a
pointer to the bounded operational log. Raw harness output, source files, and
private prompts still never reach a browser or a phone.

### Pairing becomes setup

Connecting an iPhone moves out of the permanent chrome and into Settings and
first run. The permission is described in one sentence. The underlying
capabilities remain granular, signed, workspace-scoped, and revocable, and
revocation is still checked before command admission and event delivery.

### The Companion is a product

`companion/apple/TohsenoCompanion` is a dedicated branded app built on the
released `TohsenoCompanionKit`: Your Apps, then one app, then one intent box,
then Evolve App. It implements no second protocol, backend, synchronization
mechanism, or mobile coding harness. The SDK's conformance fixture stays a
conformance fixture. The product lives in a library target so its behaviour is
tested without a Simulator.

### Scriptable use is unchanged

Explicit `--prompt`, `--prompt-file`, piped stdin, `--image`, `--wait`, and
`--json` forms are preserved exactly. Only the human defaults changed:
`tohseno create my-app` and `tohseno evolve my-app` with nothing else open the
composer. An exact `./MASTER_PROMPT.md` may prefill that composer through the
existing pending-intention store, and never starts a build on its own.

## Consequences

Deletion is part of this decision. The Studio dashboard is removed rather than
hidden: its three-region grid, execution pipeline renderer, per-execution
polling, Feedback and Marketing forms, exact-Version binding controls, and
device administration panel no longer exist, and an unreferenced second Studio
server implementation was deleted. Studio's own tests now assert the absence of
protocol vocabulary on the normal path, the absence of an extra install button,
and upper bounds on each asset's size, so the dashboard cannot quietly return.

If a future change adds more normal-path concepts than it removes, it is
probably wrong.

Nothing here weakens the permission model, the acceptance rules, or the
verification gates. No contract generation, public-witness deployment, release
publication, DNS change, installer repin, or production APNs activation is
authorized by this decision, and every existing activation gate remains
fail-closed. Historical protocol bytes, frozen fixtures, Builder identities,
signatures, and `tohseno-node` validation are unchanged.
