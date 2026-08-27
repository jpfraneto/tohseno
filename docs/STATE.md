# State of this repository

Written 2026-07-30, amended through 2026-08-26. This is the plain-language
answer to “what is going on here” for someone returning after time away. When
something below stops being true, update this file in the same change.

## The product: App → Intent → App on your iPhone

ADR 0016 defines what a person sees. The whole product is:

```text
tohseno create [my-app]   describe the app     → it installs on your iPhone
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

ADR 0015 defines the persistent local boundary, refined for 1.0.0 by ADR 0020.
TOHSENO is an intention-led app factory whose private backend is the owner's
Mac. A persistent **Local Workspace Service** hosts loopback-only Studio, owns
the durable private command and event journals, monitors execution, and
reconciles paired Companions. CLI, Studio, and mobile commands converge on one
Rust `ShotApplicationService`; none implements a second factory or shells out
to another frontend.

`tohseno create [name]` begins a factory birth from `--prompt`,
`--prompt-file`, or bounded piped UTF-8. With no intention in an interactive
session it starts the service and opens the creation composer; an exact regular
`./MASTER_PROMPT.md` prefills that composer through the durable
pending-intention store, and never starts a build on its own. Non-interactive
invocation without an intention fails instead of waiting or creating a partial
Shot. Exact intentions and up to eight safely checked reference images are
preserved before execution.

The name is optional. When it is omitted, the service reserves a collision-safe
technical slug from the intention and the existing implementation model is
instructed to choose and apply a concise user-facing product name based on the
app's primary use. This happens inside the one bounded implementation pass;
there is no preliminary naming or planning invocation.

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
after a process crash. ADR 0019 bounds the operation to one implementation
harness and at most one targeted repair for a concrete code/build defect. Both
share one 60-minute wall-clock harness budget; fifteen minutes without source
progress stops the current harness. Device, signing, provisioning, network,
lineage, and protocol conditions never invoke intelligence. Source files or a
successful harness exit alone are not completion: TOHSENO performs the finite
deterministic build, recording, installation, and launch. A missing development
iPhone yields
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

The harness reads one small `.tohseno/TASK.md`: the exact intention, continuity
and data-preservation rules, and the app identity needed to build. It does not
author a Genome, plan, ontology, or new Experience Trial. For births, the
engine stages the exact Fascia sources and resource placeholders before the
harness starts; they are not machine-discovery work for the harness. A repair
does not repeat Xcode acceptance and cannot replace the intention-wide state
transition draft. Every terminal execution records a private
`.tohseno/executions/<execution-id>/state-transition.json`; absent or invalid
harness evidence becomes `unknown` without repair. Details shows the receipt
and wall-clock execution elapsed from the first durable event, so sleep,
restarts, and repair attempts cannot reset the visible clock.

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

ADR 0024 distinguishes Version source from repository durability. `.tohseno/`
is excluded from the app source-tree commitment to prevent self-reference, but
the directory itself is not blanket-gitignored. Safe app identity, state,
expression, capability, protocol-version, and immutable Evolution structure
remain Git-visible. Exact intentions, inline-private lineage, references,
feedback, executions, logs, retained artifacts, and `.tohseno/private/` stay
explicitly ignored. Git visibility is not registry publication.

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
disclosure keeps exact status, execution phase, identities, total wall-clock
elapsed time, and the State Transition Receipt available to the owner, and now
carries the **execution receipt**: the verbatim intention that was sent, the
harness, model, and route that actually ran it, the tokens it burned and any
additional charge, and the deterministic gate that refused a candidate, quoted
with the engine's own evidence. Every value is read from that execution's own
durable records, so changing the configured harness afterwards cannot rewrite
what already happened, and a fact the factory never recorded is shown as
absent rather than as zero. Mutations use exact Origin and
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

## Cable genesis, trial, Pro, and npm

ADR 0020 makes no-argument `tohseno` the product door. A fresh installation
walks one durable cable state machine through Xcode, Trust, Developer Mode,
Apple Account signing, Companion build/install, CoreDevice URL launch, and
secure pairing. Only a derived device observation is durable; recovery words
exist only on the iPhone. The normal Mac-to-phone QR and Studio Add iPhone
surface are deleted.

Trial authority is the private versioned ledger at
`service/entitlement/state-v1.json`. The clock starts only after physical
Companion install and pairing. The complete factory is available for at most
seven local calendar days. An accepted, installed, launched Version counts on
at most one distinct date. Five days move to `trial_qualified` and lock the
next new mutation. Expiry with fewer days moves to `trial_expired` and offers
no purchase. Existing apps and read-only integrity, diagnostics, export,
billing recovery, and safe uninstall remain available.

Create/Evolve admission is enforced in `ShotApplicationService`, below CLI,
Studio, and Companion. Commands durably accepted before the boundary finish;
new commands do not enter the journal afterward. The phone receives only the
encrypted private `product.entitlement` projection. A pre-1.0.0 paired user
starts a deterministic fresh trial with zero fabricated days. Debug source
checkouts may explicitly set `TOHSENO_DEVELOPMENT_ENTITLEMENT=1`; that path is
absent from release builds.

Hosted billing lives in the existing Bun site. Checkout claims are short-lived
and workspace-signed, containing only a derived installation binding and plan.
Stripe webhooks are signature/timestamp checked and produce P-256 server-signed
receipts. The local service verifies a release-pinned public key and exact
installation bind before changing entitlement. Monthly is $9.99 and yearly is
$99. Billing is configuration-gated and currently inactive.

The repository prepares `packages/cli` 1.0.2 as the dependency-free npm peer
of native TOHSENO 1.0.2. A fresh global Mac install enters the existing
first-run guide directly from npm postinstall. The bootstrap uses one fixed
HTTPS manifest, refuses redirects and unapproved origins, verifies exact size,
SHA-256, release layout/checksums, and Apple signing policy, and installs only
into the existing user-owned layout. It now also requires the manifest's native
version to equal 1.0.2, so publishing npm before the signed native manifest
fails closed. The public npm `latest` and native manifest remain 1.0.1 and
1.0.0 respectively until the separate 1.0.2 owner release actions occur.

## Private Companion channel

The Companion is a signed remote interface, not a Builder identity, wallet,
mobile IDE, source browser, coding harness, public social client, or substitute
for Apple device trust. Its recoverable BIP-39 12-word identity derives
domain-separated Ed25519, X25519, and local-storage keys. Restoring those words
does not restore a workspace capability; the device must pair again.

After cable installation, the Mac launches a signed, one-use invitation that
expires after approximately two minutes using CoreDevice's supported URL
payload. It contains public rendezvous material and an allowlisted relay
identifier—not recovery words, private keys, filesystem paths, arbitrary URLs,
or permanent credentials. The owner grants
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

## The public website

`tohseno.com` is the public introduction to the local factory. It explains the
App → Intent → App path, personal software, independent app ownership, the
bounded build method, and the optional public record. Its install action copies
`npm i -g tohseno`; it does not collect an intention or create a Shot.

The navigation keeps visitors on the page for the product explanation. Its
Open Source section explains that the factory can be inspected, run locally,
and changed, then links to the repository. The app source remains the owner's
ordinary SwiftUI repository, whether the owner keeps it private or publishes
it.

The ADR 0011 encrypted Browser Draft relay remains a transport API rather than
a public factory surface. It does not make the website an origin of Shots, and
possession of a one-time claim capability remains its whole authorization.
The published `install.sh` and `oneshot.sh` are byte-identical and pin the
authorized 1.0.0 release.

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

This is an honest client-state claim, not an audit-quality claim. The release
records also say the owner waived the required pre-activation canary, all three
release-authority keys were created on one Mac, and no independent human or
formal contract audit is claimed. The contracts are immutable; a defect
requires abandonment or a successor rather than repair in place.

Activation means the client trusts those exact factory and registry instances;
it does not make a downloadable app registry operational. Secure public
BuilderAccount creation, registry RPC/transaction orchestration,
app-metadata publication receipts, source-repository hosting, catalog lookup,
download, and node checkpoint/receipt inventory are not implemented. The
contract stores no app code. The existing Workshop can create and verify a
manually transported source capsule and launch the tester-built app in
Simulator; it is neither registry submission nor iPhone distribution.

## Published release

Native **1.0.0** is the current public product release. The signed `v1.0.0`
GitHub release, native manifest, and public installer pin were published and
independently checked before activation. The public website and installer
serve native 1.0.0. npm's independently versioned front door is 1.0.1 and
delegates to that native release. Publication evidence for the npm patch is in
`release/NPM_1_0_1_PUBLICATION.json`.

Native and npm **1.0.2** are the next coherent release candidate in this
source tree. Its workflow accepts only `v1.0.2`, and
`release/V1_0_2_READINESS.json` remains explicitly unauthorized and blocked
until clean verification, immutable artifacts, exact public manifest bytes,
npm publication, and website activation are recorded. Source preparation is
not a claim that 1.0.2 is publicly available.

Production billing remains configuration-gated and inactive. Companion relay
and APNs activation remain separate operational decisions; the 1.0.0 release
does not infer them from artifact publication. The readiness record and
publication evidence are in `release/V1_0_0_READINESS.json`, with the operator
steps retained in `docs/runbooks/`.
