# State of this repository

Written 2026-07-30, amended through 2026-09-02. This is the plain-language
answer to “what is going on here” for someone returning after time away. When
something below stops being true, update this file in the same change.

## Current product direction: claimable person-to-person software

ADR 0035 retains ADR 0034's signed, buildable person-to-person source and adds
one public act: Claim. ADR 0036 now makes the intended iPhone—not its current
USB or Wi-Fi transport—the private delivery destination, and ADR 0037 defines
future network trust as exact-release evidence rather than a safety score. The Mac is
the factory, Companion is the human authority, generation 0.8 plus the signed
catalog remains the public Shot witness, and a separately activated additive
Claims contract records non-transferable exact encounters. A Shot ships once,
later releases are Updates, and its first Ship fixes one immutable Claim
Edition. Install remains private physical evidence, never an on-chain fact.

The implemented vertical slice is:

```text
Adopt .xcodeproj/.xcworkspace on Mac
  -> private stable project_<uuid> record, repository untouched
  -> encrypted workspace projection to paired Companion
  -> signed project.evolve.request + durable encrypted phone outbox
  -> authenticated/idempotent Mac admission, persisted before receipt
  -> configured harness in the exact source directory
  -> signed iphoneos xcodebuild + codesign verification
  -> exact devicectl install + bundle-inventory verification
  -> durable status/history on Mac and iPhone

Explicit Ship
  -> deterministic sanitized source snapshot
  -> exact catalog + Registry action + immutable Claim Edition approved by Companion DeviceKey
  -> active generation-0.8 BuilderAccount/RegisterShot or AppendCheckpoint
  -> first release emits one Ship; later public releases emit Update
  -> canonical Discover/Shot timeline backed by receipt/head/source evidence
  -> recipient Companion Claim circle + Tohseno smart-account signature
  -> separately activated Claims contract mints one non-transferable receipt
  -> canonical Claim durably queues the exact release while the Mac may be offline
  -> recipient Mac independently verifies chain, receipt, manifest, and source
  -> narrow Xcode build + recipient-local Apple signing + physical install proof
```

Adoption inspects schemes, bundle ID, deployment target, signing-team setting,
Git revision/dirty paths, repository instruction files, and a real unsigned
Simulator build. It keeps a versioned external record under
`~/.tohseno/service/living-projects-v1`; choosing the same container/scheme
again preserves the same project ID. Evolution records retain the exact
request, attachment copies, starting state, pre-existing dirty paths, harness,
changed-file observation, build attempts, per-device installation attempts,
completion/recovery text, and follow-up relationship. Rollback is explicitly
unavailable rather than risking unrelated dirty work.

Pairing remains the existing one-use, two-minute, signed and encrypted flow.
Phone identity/secrets live in iOS Keychain; the Mac identity lives in Keychain;
durable projections/outboxes are encrypted in Application Support; revocation
increments local authority immediately and revokes both relay mailboxes. Mac
Settings now lists, renames, revokes, and starts QR pairing for additional
personal devices.

Initial Companion setup persists a one-way digest of the exact physical
CoreDevice that received Companion. The install path now consumes that private
bootstrap evidence: when it exists, only the matching phone is eligible across
USB or Xcode local-network transport, even if other phones are visible; when it
is absent, another phone is never substituted. Older records without the
digest retain the exactly-one-reachable-device compatibility fallback and fail
closed on multiple devices. `Installed` requires a successful devicectl
install followed by an exact bundle-ID inventory query. Device absence, lock,
Trust, Developer Mode, and unresolved legacy ambiguity retain the verified
artifact as **Ready to install** with the smallest action. The background retry
performs no device polling unless a saved build is actually waiting. ADR 0036's
full versioned, multi-target, Companion-ID-bound replacement model remains
unimplemented and the bootstrap selector has no physical acceptance evidence.

The frozen 1.1.0 source candidate includes the public catalog/blob service,
constrained active-generation relayer, DeviceKey publication approval,
deterministic source security, live receipt/head verification, Install/Fork,
recipient signing/refresh, native Registry and Profile surfaces, signed profile
updates, permissioned alias requests, and the isolated network E2E harness. It
changes no public protocol encoding or deployed ABI. Source completion does not
claim the 1.1.0 DMG is signed/notarized/published or that second-person physical
acceptance has occurred; those remain release evidence, not software inference.

The current 1.2.0 source line adds `TohsenoClaimsV1`, exact Rust/Swift/Solidity/
TypeScript action and Claim-mark agreement, separately signed Claims
activation, atomic first-Ship edition opening, one-Ship public timelines,
canonical Claims indexing, the Companion circle and claimed-software cabinet,
durable offline exact-release preparation, private follows, and a reconciled
high-signal Updates inbox. It also adds a constrained owner deployment and
threshold ceremony without modifying generation 0.8. Claims remains dark:
the exact contract and threshold-signed activation are live for verified reads,
but the production Claims relayer and every Claims write remain disabled. The
dedicated Claims and Registry relayer addresses are funded for the bounded
owner-attended walkthrough but remain dark outside that acceptance window. The
signed, notarized, stapled 1.2 release candidate is public only for the
owner-attended physical walkthrough; there is still no canonical physical
Claim. Exact blockers live in `release/V1_2_0_READINESS.json`; 1.1 evidence
remains bound to its recorded source commit.

RC7, built from exact source commit `ff7e67f`, now releases the stable
deploy-time app slug, exact app
selection for Companion alias requests, an authenticated append-only alias
approval endpoint, and live root Registry routes such as `/your-app`. The app
page explains the four-step recipient path, pins Claim to the exact release,
and distinguishes signed provenance and bounded machine facts from unavailable
human Release Attestations. The Mac and CLI now describe Wi-Fi/USB
reachability instead of presenting every normal installation as a cable
operation. RC7 is Developer ID signed, Apple-notarized, stapled, mounted,
Gatekeeper-accepted, published as immutable prerelease `v1.2.0-rc.7`, and
active on the production release-candidate download channel with SHA-256
`e161400ff522693e6a6290365533233ee3e6c9686a07fdca28bfd7be48c80b69`.
The exact public bytes round-trip to the local artifact. Registry and Claims
reads are live while both funded relayers remain disabled. No alias, Ship,
Claim, friend installation, or physical acceptance has occurred yet.

## Native macOS transition

ADRs 0025 through 0032 describe the prior native transition retained beneath
ADR 0033. They make a
native SwiftUI `Tohseno.app` the primary consumer surface over the existing
persistent Rust factory. It supersedes npm/browser first run, mandatory
five-successful-day qualification, subscription gating of
local/BYO work, and the visible Studio model selector. It preserves the one
application service, durable journal, bounded engine transition, Apple
build/sign/install path, exact protocol history, app-local Git boundary, CLI
recovery surface, and private Companion implementation.

The source implementation now exists at `macos/Tohseno`: a native SwiftUI
navigation/client with six-state presentation, create/evolve composers,
references, details, balance/settings, restoration, SSE updates, and exactly-once
submission guards. It authenticates to the same loopback service through a
signed-app helper, one-use workspace challenge, and short-lived per-instance
scoped session. First open verifies and atomically installs a bundled universal
factory release with rollback; scripts assemble, sign, notarize, verify, and
package a DMG.

On first open, the native window explains that Tohseno turns an intention into
a native iPhone app, keeps source and history on the owner's Mac, and uses
Tohseno Companion to connect the phone to the local factory. The one existing
genesis path observes the Apple readiness gates, builds, signs, installs, and
launches the real Companion, and waits for its private pairing proof while the
Mac app shows staged progress. It does not install the disposable readiness
app used by the retained compatibility API. The running application also has a
menu-bar item drawn from the repository's Tohseno SVG.

When the workspace has no apps, the native window now explains the connection
and offers **Adopt Existing App** first and **Create a First App** second. The
former selects an existing Xcode container; the latter retains the existing
keyboard-first generated Shot composer. The deleted first-run Take a Shot gate,
Skip preference, and intimidating empty textarea do not return.

Selecting an app now opens a native Build/App/Source workspace. Build is the
default and shows the simple Intent → Source → Simulator → Your iPhone path,
up to 200 owner-local source files changed against the request's own Git
baseline, and the existing bounded semantic journal without its internal
phase or raw harness output. App contains the explicit **What should change?**
action; Source opens the real local Git working tree. An iPhone stage remains
visible beside every tab: it uses the actual latest verified Simulator
capture, labels it non-interactive, and keeps the connect-and-unlock cable
handoff central while automatic installation is pending.

Every intention composer is keyboard-first: plain Return sends through
the existing exactly-once application-service path and Shift–Return inserts a
line. The creation settings are open by default so discovered Codex and other
authenticated harnesses are visible; discovery checks bounded standalone,
Homebrew/global, Volta, npm-global, Bun, and installed NVM locations without
executing a user shell. The Registry destination presents deterministic public
**Discover**, private **Following**, and a durable private **Updates** inbox;
the old quick New Shot composer and local app grid do not return. Catalog reachability is not
shown as chain verification: the exact manifest, receipt, active generation,
current Builder DeviceKey authority, current Shot state, and source bytes are
verified by the Mac only after the person chooses Claim, Install, or Fork.
Workshop and creation actions remain in Your Apps; Registry is not a second
factory.

Retirement preserves source and accepted history. The native Diagnostics
archive exposes those apps and can restore one to the library without silently
reinstalling it on a phone.

The same source also retains Companion-independent Apple readiness for
compatibility clients; the primary native path uses the paired Companion as
governed by ADR 0032. It implements known/custom/loopback/managed intelligence adapters with exact durable
selection; server-priced estimates and caps; installation-signed managed
claims; append-only paid/promotional micro-USD balance; Stripe packs; a narrow
Bankr completion proxy; protected grant/revocation/reconciliation operations;
and a public Mac download route that is disabled unless an HTTPS artifact and
exact digest are configured.

The rejected `v1.0.2-rc.1` candidate was built from clean commit
`d275d1f5948eef89cf2b422a90f9bc780dd38ac1`. It contains ADR 0029's real
first-Shot gate, keyboard submission, and eight-reference path in addition to
the native Build/App/Source workspace and Finder-first handoff. It is signed
with Developer ID Team `84V63LKV45`, accepted by Apple notarization submission
`d95ff96d-a536-4777-814d-8c43ae4f7ecd`, stapled, and packaged as a locally
verified universal DMG with SHA-256
`6a168ba9da2c89c0b852e252370f19a60cfd92f8a71d0230161d093f0b04a4dc`.
The exact `AGENTS.md` verification matrix passed at source commit
`232ab31fc280851d36c31675ec72f778bb421c7e`; the later clean artifact commit
changes release evidence only. A read-only mount passed manifest,
secret-pattern, universal-binary, exact Team ID, hardened-runtime, Developer
ID, Gatekeeper, stapled-ticket, and Applications-alias verification. Finder
installed the exact app into `/Applications`, Gatekeeper accepted first launch
on both the build Mac and an independent clean Mac, and that independent test
then rejected its product experience under ADR 0032.

Before that rejection, the exact candidate completed three fresh Codex `gpt-5.6-sol` births and a
same-app Version 2 evolution on the paired physical iPhone 15. The native Mac
app was terminated during admitted work and reopened from Finder without
losing the service, execution, source, or history. Each app passed its bounded
implementation and repair limits, deterministic device and Simulator gates,
signed physical-device build, install, launch, and acceptance. Independent
inspection verifies intention bytes, embedded metadata, the selected records,
source materialization, and contiguous lineage for all three apps. A deliberate
evolution against obsolete Version 1 was durably rejected with HTTP 409
`stale_base` and created no execution.

Managed-compute staging now exists at the separate Railway `staging`
environment with a durable `/data/managed-compute` root, a Keychain-held
operator token whose digest alone is configured server-side, trusted checkout
return URLs, exact Stripe test-mode one-time Prices for $10/$25/$50, and a
test webhook endpoint for checkout, refund, and dispute events. Managed
compute remains disabled because no least-privilege Bankr LLM Gateway key or
model allowlist is available, Bankr credits and real inference are unverified,
and no backup/restore drill or real-service scenario matrix has passed. The
Railway volume API reports no manual backup and no schedule; the current CLI
credential can inspect that state but is not authorized to create a backup or
set daily/weekly schedules, so backup administration still requires an
authorized Railway owner session.

Superseded local artifacts remain recorded in
`release/V1_0_2_READINESS.json` and must not be distributed. Exact tag
`v1.0.2` is covered by active deletion and non-fast-forward protection but has
not been created. ADR 0030's system-aware direct-download landing is deployed
and healthy. Under ADR 0031, the exact signed, notarized, stapled candidate was
published as GitHub prerelease `v1.0.2-rc.1`; a fresh origin round trip matched
SHA-256 `6a168ba9da2c89c0b852e252370f19a60cfd92f8a71d0230161d093f0b04a4dc`.
Independent clean-Mac installation reached Gatekeeper and first open but failed
product acceptance for the DMG composition, naming, onboarding, Companion
setup, progress, menu-bar presence, guided creation, Codex discovery, and
Registry expectations now governed by ADR 0032. Production deployment
`69a33859-d149-423f-8d74-79028defad3a` disabled that candidate. The rejected
prerelease remains immutable evidence and must not be distributed again.

Replacement prerelease `v1.0.2-rc.2` targets clean artifact commit
`15381917bc8851752e59306fe2d7f6ff9d2e9ec8`. Apple accepted notarization
submission `8586451e-b32e-4064-9b0a-805c9978165f`; the stapled universal app
passed mounted-DMG signature, manifest, Gatekeeper, and Finder-layout checks.
Its immutable 46,622,258-byte DMG has SHA-256
`352c154675e59ab98cc5c0da4d804fa259564e66217ce9ddd28f7939f6c1bdcb`, and
both the GitHub origin and `tohseno.com` round trips matched it. Production
deployment `86a81e20-b180-467e-92b1-fed4cf7ed0a4` is healthy and exposes that
exact digest as a **release-candidate**. Independent clean-Mac Companion and
creation acceptance for RC2 remains unverified; stable `v1.0.2` is not
published.

## Retained generated-Shot product machinery

ADR 0016 still defines the retained generated-Shot projection. Under ADR 0033
it is secondary to adopting and evolving a living project:

```text
Open Tohseno.app → describe an app   → it installs on your iPhone
Open that app    → describe a change → the update installs
```

Each primary create/evolve path opens one screen with one box. Externally that path speaks only in App,
Create, Evolve, Waiting, Building, Ready, Installing, Installed, Failed, Retry,
and Details. The optional Registry destination deliberately uses Shot,
Evolution, Builder, and Registry to inspect verified local history; it does
not expose execution-pipeline or planning internals. Expressions, executions,
Feedback records, harnesses, inference routes, and pairing internals remain
off the normal path. If a change adds more primary-path
concepts than it removes, it is probably wrong.

`application/src/presentation.rs` is the single projection that collapses every
internal execution phase into six human states; the workspace snapshot carries
one `presentation` per app, and native Mac, Studio, and Companion render the
same contract from `fixtures/presentation-v1.json`. Studio's own tests
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

## Retained legacy cable, entitlement, recurring billing, and npm code

The behavior in this section remains readable for installed-release
compatibility but is superseded as a consumer product by ADR 0025. It is not a
prerequisite or admission gate for native first run or local/BYO work.

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

The repository prepares `packages/cli` 1.2.1 as the dependency-free npm CLI
door. A global install writes only the JavaScript launcher: it performs no
postinstall download, service mutation, Companion setup, or GUI launch. The
no-argument command shows `cd`, `tohseno init`, then `tohseno deploy`.
Interactive init explains Xcode adoption, the stable candidate ShotID,
Companion authority, and one Ship followed by Updates one Enter-gated line at
a time; JSON and non-interactive use do not pause. Before adoption it selects
only the private intended CoreDevice, verifies the exact
`com.tohseno.companion` bundle in that iPhone's installed-app inventory, and
requires completion of the specific one-use private pairing session. Missing
Companion directs the person to `tohseno companion install`; unreadable
inventory remains unknown, and remembered state or another phone cannot count.
At first operational use,
the launcher uses one fixed CLI manifest, refuses redirects and unapproved
origins, verifies exact size, SHA-256, release layout/checksums, and Apple
signing policy, and activates the command runtime only in the existing
user-owned layout. Public npm `latest` is 1.2.1. Its exact signed runtime
archives, production manifest, npm tarball, no-side-effect install, lazy runtime
activation, source identity, signature, `init` Enter gate, and Companion-check
explanation were independently observed through a fresh public install. No
physical iPhone or Companion inventory was available during that smoke, so
physical acceptance remains unobserved. The publication evidence is recorded
in `release/NPM_1_2_1_PUBLICATION.json`; the earlier 1.2.0 observation remains
in `release/NPM_1_2_0_PUBLICATION.json`.

## Private Companion channel

This channel is optional for native readiness, create, evolve, build, generated
app installation, and local/BYO admission. It is required for the
builder-network `init`/`deploy` path because Companion holds publication
authority. Existing pairings remain valid only while the intended iPhone still
contains the exact Companion app and the private pairing remains active.

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
bounded build method, and the optional public record. Current website source
centers one ordinary `/download/macos` link. Browser platform detection labels
it **Download for this Mac** on macOS and states the real Mac requirement on
iPhone, iPad, Windows, Android, Linux, ChromeOS, and unknown systems. The
fallback without JavaScript remains **Download for Mac**. The route remains
fail-closed until the operator configures the immutable notarized DMG URL,
exact SHA-256, and release channel. It currently serves the exact verified
`v1.2.0-rc.6` bytes on the labeled release-candidate channel; stable `v1.2.0`
remains unpublished.
The website does not collect an intention or create a Shot. The
retained `/oneshot.sh` is a legacy/claim transport and no longer appears on the
normal landing path.

The same fail-closed artifact configuration controls the retained `/install`
and `/download` shell compatibility aliases. Once activated, they emit the
interactive native installer previously used by the normal landing path: it
asks for Enter or Escape, shows one download progress bar, and invisibly
verifies SHA-256, the exact bundle and Developer ID Team, and Gatekeeper
acceptance. It places the verified DMG in Downloads, prints that exact path,
and reveals it in Finder so the person performs the familiar drag into
Applications. It does not copy, replace, or open an application, request
administrator access, or edit a shell profile. HEAD exposes only
status/instruction headers. They currently use the same verified 1.2 RC4 pin.

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
successor contract-generation or generation deployment command is active on
current source. ADR 0035's narrow additive Claims deployment script is
separate, owner-attended, and has not produced a trusted deployment or
activation.

This is an honest client-state claim, not an audit-quality claim. The release
records also say the owner waived the required pre-activation canary, all three
release-authority keys were created on one Mac, and no independent human or
formal contract audit is claimed. The contracts are immutable; a defect
requires abandonment or a successor rather than repair in place.

Activation means the client trusts those exact factory and registry instances.
ADR 0034 now connects them to secure public BuilderAccount bootstrap, constrained
Registry transaction orchestration, signed catalog receipts, content-addressed
source hosting, discovery, exact download, and independent recipient
verification. The contract still stores no app code, private intention,
artifact digest, or installation identity; those facts remain off-chain under
the signed catalog and local privacy boundaries.

The additive `TohsenoClaimsV1` contract is now deployed at
`0x5012703d48d99224ac0035d58bc373de9e8b1934` on chain 4663 and its exact
threshold-signed activation is pinned and verified by the production read
service. Its index is canonical and empty before the physical walkthrough.
The Claims and Registry relayers each have a dedicated configured and funded
address. Both relayers remain disabled outside the owner-attended physical
acceptance window.

## Published release

Native **1.0.0** remains the current stable native product release. The signed
`v1.0.0` GitHub release, native manifest, and public installer pin were
published and independently checked before activation. npm's independently
versioned CLI door is now 1.2.1. It installs only its side-effect-free launcher
and lazily activates the exact Developer ID signed 1.2.1 command runtime from
the separate CLI manifest; it does not promote the stable native-app DMG.
Publication evidence is preserved in `release/NPM_1_0_1_PUBLICATION.json` and
the successive `release/NPM_1_2_0_PUBLICATION.json` and
`release/NPM_1_2_1_PUBLICATION.json` observations.

Native **1.1.0** remains an unreleased source candidate frozen to exact commit
`ee16e1e2cef95a2598632bc9444d5011998aebae`; its readiness record remains false
and its evidence must not be rewritten to describe later source.

Native **1.2.0** is the current product release target in source. It must not
reuse 1.0.2 artifact evidence or infer release truth from the 1.1 candidate.
Release candidate `v1.2.0-rc.1` is rejected because its Mac bundle omitted the
Apple identity source package required to resolve and build Companion; the
resulting failure was incorrectly collapsed into a generic Apple Account
message. Replacement `v1.2.0-rc.2` repaired the bundle and retained staged
diagnostics, but owner-attended first-open acceptance found that it still
entered technical device setup without welcoming the person or first
explaining the system. RC3 replaced it with a living-mark welcome, the
intention → Mac → iPhone relationship, explicit local-source and publication
promises, and plain-language purpose for each setup requirement while keeping
the persistent progress and diagnostics. Owner-attended Registry acceptance
then rejected RC3 because the Mac app rejected valid Registry responses and
did not expose the CLI activation, `tohseno init`, `tohseno deploy`, Companion
approval, and recipient-signing path. RC4 repairs Registry decoding, adds the
signed one-click CLI activation surface, presents the exact commands and
one-time pairing QR, and distinguishes healthy public reads from the disabled
publication relayer. Candidate `v1.2.0-rc.4` was built from exact clean and
fully CI-gated commit `7abf702c`, signed with Developer ID
Team `84V63LKV45`, accepted by Apple notarization submission
`58a308f5-68c8-4118-8e1d-18ba28ea9e58`, stapled, mounted,
Gatekeeper-checked, and published as a 52,294,358-byte universal DMG with
SHA-256
`f9ccbc05ba2d81060c107f65bb402fb21c34451d983e95018f827d546c19855c`.
Both the GitHub origin and tohseno.com round trips matched. Owner-attended init
acceptance then rejected RC4 because its CLI abandoned the local request after
ten seconds while Xcode correctly continued a real Simulator build, producing
a false connection error and no continuing feedback. RC5 keeps that bounded
request attached, explains that Xcode is resolving packages and building for
Simulator, and reports elapsed progress every ten seconds. Candidate
`v1.2.0-rc.5` was built from exact clean and fully CI-gated commit `c4e6d35`,
signed with Developer ID Team `84V63LKV45`, accepted by Apple notarization
submission `0d02d61f-ee5f-4b25-9bf2-e9331848c325`, stapled, mounted,
Gatekeeper-checked, and published as a 52,314,436-byte universal DMG with
SHA-256
`92ccf48a158db0c1f105d0cdbab9b06b0b94d22bd5b7d1a0e59e70bce3d8fc2a`.
Both the GitHub origin and tohseno.com round trips matched. Owner-attended
deploy acceptance then rejected RC5 because the snapshot scanner treated the
short byte prefix `ghp_` inside compressed PNG data as a complete credential.
RC6 requires complete high-confidence token shapes while retaining fail-closed
checks for real tokens and private-key material. A dry run against Anky's real
956-file, 460,333,699-byte source tree completed without uploading, signing, or
publishing. Candidate `v1.2.0-rc.6` was built from exact clean and fully
CI-gated commit `fddab7d`, signed with Developer ID Team `84V63LKV45`, accepted
by Apple notarization submission `be3cb4d7-16dc-40cb-9a09-2505ad0be7b9`,
stapled, mounted, Gatekeeper-checked, and published as a 52,314,624-byte
universal DMG with SHA-256
`082c9d1c8e44574cdb48132753e243c372ab21b675674527c0391589af7df2de`.
Both the GitHub origin and tohseno.com round trips matched. RC6 is active only
on the labeled release-candidate channel for the owner-attended walkthrough.
Stable 1.2 still requires the production proof in ADR 0035: real first Ship and
edition, second identity Claim, offline-Mac preparation, recipient-signed
physical install, later Update preservation, Follow reconciliation, live
receipt paths, and exactly one Ship. Registry/Claims writes and Claim
advertising remain dark until that order creates no broken window.

Legacy recurring billing and the new managed Stripe/Bankr service remain
separately configuration-gated and inactive. Companion relay
and APNs activation remain separate operational decisions; the 1.0.0 release
does not infer them from artifact publication. The readiness record and
publication evidence are in `release/V1_0_0_READINESS.json`, with the operator
steps retained in `docs/runbooks/`.
