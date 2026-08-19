# State of this repository

Written 2026-07-30, amended through 2026-08-18. This is the plain-language
answer to “what is going on here” for someone returning after time away. When
something below stops being true, update this file in the same change.

## The product: App → Intent → App on your iPhone

ADR 0016 defines what a person sees. The whole product is:

```text
tohseno create my-app     describe the app     → it installs on your iPhone
tohseno evolve my-app     describe the change  → the update installs
```

Each opens one screen with one box. Externally TOHSENO speaks only in App,
Create, Evolve, Waiting, Building, Ready, Installing, Installed, Failed, Retry,
and Details. Shots, Expressions, Versions, executions, Feedback records,
harnesses, inference routes, lineage, and pairing internals are all real and
none of them appear on the normal path. If a change adds more normal-path
concepts than it removes, it is probably wrong.

`application/src/presentation.rs` is the single projection that collapses every
internal execution phase into six human states; the workspace snapshot carries
one `presentation` per app, Studio renders it verbatim, and the Companion
mirrors the same table from `fixtures/presentation-v1.json`. Studio's own tests
assert the absence of protocol vocabulary on the normal path and bound the size
of each asset, so the old dashboard cannot quietly return.

## Underneath: persistent local factory

ADR 0015 defines the current 0.9.0 internal boundary, unchanged by ADR 0016.
TOHSENO is an intention-led app factory whose private backend is the owner's
Mac. A persistent **Local Workspace Service** hosts loopback-only Studio, owns
the durable private command and event journals, monitors execution, and
reconciles paired Companions. CLI, Studio, and mobile commands converge on one
Rust `ShotApplicationService`; none implements a second factory or shells out
to another frontend.

`tohseno create <name>` begins a factory birth from `--prompt`,
`--prompt-file`, or bounded piped UTF-8. With no intention in an interactive
session it starts the service and opens the creation composer; an exact regular
`./MASTER_PROMPT.md` prefills that composer through the durable
pending-intention store, and never starts a build on its own. Non-interactive
invocation without an intention fails instead of waiting or creating a partial
Shot. Exact intentions and up to eight safely checked reference images are
preserved before execution.

`tohseno evolve <name>` begins an evolutionary transaction bound to one Shot,
one Expression, and one exact accepted base Version, and with no intention in
an interactive session opens that app's composer. The surfaces bind the current
accepted base at submission so nobody selects a Version by hand. Selected
Feedback action commitments and exact references are part of that durable,
idempotent command. If the base changes before admission the request is stale
and is rejected; the service never silently rebases it, and both surfaces
explain it in one sentence.

Writing what should change and pressing Evolve App is the whole operation.
Exact-Version Feedback remains a real capability through `tohseno advanced
feedback` and the Companion `feedback.write` grant, but it is no longer a step
between having an idea and acting on it, and no Feedback record is fabricated
alongside an evolution. The Studio-only feedback, marketing, and
feedback-action HTTP endpoints were removed with the UI that was their only
caller.

Every admitted command is journaled before its semantic action. Stable command,
Shot, and execution identities make retries idempotent and allow recovery
after a process crash. Completion still means that the applicable conception,
Birth Plan, Genome, materialization, build, test, experience, repair, delivery,
and acceptance gates passed. Source files or a successful harness exit alone
are not acceptance. A missing development iPhone yields
`waiting_for_device`, not success — presented as *Your app is ready, plug your
iPhone in* with no button to press, because the service resumes delivery by
itself.

Expensive local work is serialized by one advisory lease file under the private
machine data root (`application/src/factory_lease.rs`). It is deliberately the
smallest mechanism that can say “this Mac has your request but is busy”: no
queue, no scheduler, no new command state, no new protocol record. A runner
that cannot take the lease stays in its durable `queued` phase — presented
everywhere as *Waiting to build…* — and starts by itself when the lease frees.
The lease is released while a verified candidate waits for a cable, so an
absent phone never blocks unrelated source work, and it is released by process
exit, so a crashed runner cannot strand the factory. The command journal
remains the durability and idempotency authority.

## Recording-layer compatibility

ADR 0014's app-local recording format and safe-path rules remain accepted;
ADR 0015 changed only its user-facing role. The exact behavior is available
through:

```text
tohseno init <name>
tohseno record [name] --note "..."
tohseno record [name] --note-file note.md
```

The visible app directory remains an ordinary, ejectable working tree.
Recording excludes `.tohseno/` and `.git/` while preserving all other ordinary
files. Existing `.tohseno/recording-layer-v1` directories remain
`recording_only`; no implicit migration, fabricated Shot identity, or factory
execution is permitted. Historical accepted directories and records are read
and verified under the law that produced them, never rewritten or re-signed.

## Local Workspace Service and Studio

The installed service is a user LaunchAgent named
`com.tohseno.workspace-service`. Its stable program path is
`~/.tohseno/bin/tohseno service run`; it requires no `sudo`, starts at login,
and is restarted after unexpected failure. An explicit clean stop unloads the
job. Bounded content-free logs and private service state live beneath
`~/.tohseno/`, separately from visible app folders.

`tohseno studio` ensures the installed service is healthy, opens its verified
loopback origin, and returns to the Terminal. Studio is a visual client of the
same application service, and is now four views: Your Apps (`/`), the composer
(`/create`), one app (`/shots/{id}`), and Settings (`/settings`). Connecting an
iPhone moved into Settings; the three-region factory-control grid, execution
pipeline renderer, per-execution polling, Feedback and Marketing forms, and
exact-Version binding controls were deleted rather than hidden, along with an
unreferenced second Studio server implementation. A deliberate Details
disclosure keeps exact status, execution phase, identities, timestamps,
harness, and route available to the owner. Mutations use exact Origin and
anti-CSRF validation; the server does not bind a public interface or grant
permissive CORS. Live status uses the service event stream rather than
reloading the whole workspace once per second.

Service administration is explicit:

```text
tohseno service install|start|stop|restart|status|logs|run|uninstall
```

Ordinary service uninstall removes only a recognized installer-owned
LaunchAgent and preserves app folders, Builder identity, command journals,
and companion pairing state.

## Private Companion channel

The Companion is a signed remote interface, not a Builder identity, wallet,
mobile IDE, source browser, coding harness, public social client, or substitute
for Apple device trust. Its recoverable BIP-39 12-word identity derives
domain-separated Ed25519, X25519, and local-storage keys. Restoring those words
does not restore a workspace capability; the device must pair again.

Studio creates a signed, one-use invitation that expires after approximately
two minutes. Its `tohseno://pair/v1/…` QR contains public rendezvous material
and an allowlisted relay identifier—not recovery words, private keys,
filesystem paths, arbitrary URLs, or permanent credentials. The owner grants
explicit revocable capabilities for workspace/execution reads, feedback,
marketing notes, Shot creation, and evolution. Revocation is checked before
new command admission and event delivery.

Companion traffic is signed and end-to-end encrypted with the versioned suite
defined by `companion/` and the single shared Rust/Swift fixture at
`companion/test-vectors/companion-v1.json`. Mailbox cursors, sender sequences,
stable IDs, acknowledgements, durable outboxes, bounded retention, and snapshot
fallback make delivery reconnectable and idempotent. The Mac is authoritative
for workspace and Shot state; the phone is authoritative only for its unsent
drafts and outbox until acknowledgement. Pending phone envelopes are retried
verbatim, then safely resealed with the same signed command and reference bytes
before relay expiry. The canonical command-admission window is thirty days;
older unacknowledged material stays protected on the phone for explicit owner
resolution while inbound receipts and revocation continue to reconcile.

The separate Bun Companion Relay performs opaque rendezvous and mailbox
delivery. It cannot decrypt content, invoke an agent, build an app, or store
source. APNs, when separately configured, carries only a wake-up signal;
foreground reconciliation works without it. Production relay and APNs
activation remain fail-closed and have not been authorized by the source
change.

The branded iOS app now exists at `companion/apple/TohsenoCompanion`. It is
built on the released `sdk/apple/TohsenoCompanionKit` and implements no second
protocol, backend, synchronization mechanism, or mobile coding harness. Its
whole surface is Your Apps → choose an app → *What should change?* → Evolve
App, with first-run pairing and no confirmation step. The SDK's
`Examples/CompanionConformanceApp` remains the raw conformance fixture and was
not turned into a product. The product lives in a library target, so
`swift test --package-path companion/apple/TohsenoCompanion` exercises it
without a Simulator; `App/TohsenoCompanion.xcodeproj` is a thin shell that
builds, installs, and launches on an iOS Simulator. Released SDK source can
still be vendored into a Shot so generated apps do not depend on a mutable
`~/.tohseno/current` path.

## The public website is a terminal

`tohseno.com` is one prompt whose placeholder is `tohseno create my-app-name`.
It speaks the same vocabulary as the Mac and the phone and adds no concept of
its own. A person writes the whole intention first — text, pasted or dropped
images — and only then is asked where it should go.

Only one destination works: the ADR 0011 encrypted handoff to their Mac, which
prints the single-use `--claim` command and falls back to the unencrypted
private `.tohseno-intent` download when the relay is not activated. The demo
door replays `application/src/presentation.rs` with its exact headlines, and
`website/apps/site/tests/terminal.test.ts` checks every replayed state against
`fixtures/presentation-v1.json`, so the website is now a third surface bound to
the one presentation contract.

The third door, linking the iPhone Companion, is deliberately unbuilt and says
so. It is recorded here so it is not quietly redesigned: when the Companion is
published, scanning will link **the browser to the phone**, not a person to an
account. The phone keeps the identity and does the signing, the browser holds
only a capability the phone can revoke, and the phone forwards to the Mac it is
already paired with. A phone is a remote control for a factory, not a factory.
Nothing about it makes the website an origin of Shots, and ADR 0011's rule that
possession of a one-time capability is the whole authorization still holds.

The paid day and the sojourn moved into that terminal as commands and into the
static boot block, which is the only copy a crawler or a reader without
JavaScript sees. The published `install.sh` and `oneshot.sh` are unchanged and
still pin the authorized release.

## Public protocol and node remain separate

`protocol/` remains normative over every prose document. Its canonical byte
encodings, schemas, frozen vectors, historical Builder identity behavior, and
verification rules are not changed by private companion transport.

The existing `tohseno-node` remains the Public Node. It validates and preserves
only eligible public lineage evidence and continues to reject private actions.
Private capabilities, envelopes, snapshots, marketing notes, command
provenance, and companion events never enter that system. The web-to-local
handoff from ADR 0011 also remains transport rather than a Shot.

The remediated 0.8 contract generation was deployed to Robinhood Chain mainnet
as a candidate on 2026-08-01 and activated by the recorded owner ceremony on
2026-08-02. That evidence remains under `release/` and `contracts/`. No
contract generation or deployment command is active on current source.

## Repository source versus published release

Repository source and package metadata target **0.9.0**. That is not a claim
that a public 0.9.0 release, production relay, APNs provider, or new installer
pin exists. As of this document's date:

- the public installer still pins immutable **0.8.5**;
- the published website copies must remain byte-identical to that authorized
  installer until 0.9.0 activation;
- no v0.9.0 tag or GitHub release has been created;
- no production companion relay, APNs credentials, DNS, or deployment has been
  activated by this change;
- local 0.9.0 source must pass the readiness gates and be built from a clean,
  captured commit before an owner authorizes publication.

The ordered release, independent checksum verification, service-health,
relay/APNs activation, installer-pin, and rollback instructions are in
`release/V0_9_0_OPERATOR_RUNBOOK.md`. Until every gate is recorded, the public
0.8.5 pin is the honest production state.
