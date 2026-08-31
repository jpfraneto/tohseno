# Tohseno person-to-person native software architecture

This is the concrete runtime map for the public network and private iPhone-to-Mac factory. It is
descriptive, not protocol authority: [`protocol/`](../protocol/) and the
accepted [ADRs](adr/README.md) win if this document disagrees with them.

ADR 0034 connects ADR 0033's private adopted-project boundary to a signed
public source catalog. ADR 0035 adds a separately activated, additive Claims
receipt and one-Ship software timeline without changing frozen protocol
encodings or the deployed generation-0.8 ABI. The primary implemented paths
are:

```text
Native SwiftUI Tohseno.app -> Adopt existing Xcode project
        |
        | bounded native loopback session + events
        v
Persistent Rust Local Workspace Service
        |
        v
LivingProjectService → harness → xcodebuild/codesign/devicectl/verify
        |                                  |
       +── private versioned history      +── exactly one resolved owner iPhone

Builder Companion DeviceKey -> signed catalog + Registry authorization
        |                                      |
        v                                      v
Mac deterministic source snapshot -> public Registry/catalog/blob service
        |                                      |
        +-> first Ship + immutable edition     +-> canonical Discover timeline
                                                       |
                             exact Shot/release Claim link
                                                       |
                                                       v
Recipient Companion circle -> Claims receipt -> durable private Mac intention
                                                       |
                                                       v
                           Mac verifies chain/receipt/source independently
                                                       |
                                                       v
                                   Xcode build + recipient signing + iPhone
```

The existing `ShotApplicationService → Engine` remains the one generated-app
factory. Existing source starts a truthful new local root at `tohseno init`;
it does not receive fabricated historical lineage.

The native app and CLI are Mac clients. Companion and its encrypted relay are
the normal human authorization path. Apple release signing/notarization,
physical-device checks, on-chain publication, and service activation remain
separate evidence; source existence does not imply any of them.

```text
iPhone Companion
  signed command + encrypted durable outbox
                    |
                    v
          content-blind relay
                    |
                    v
        Local Workspace Service
  authenticate -> admit -> journal -> execute
                    |
                    v
       exact adopted source folder / private project record
```

The Companion is the human's Tohseno authority. The Mac is the factory. The
Registry is the shared public witness and discovery layer. The relay is a
bounded opaque mailbox; it is neither a factory nor an authority over a
project or Shot.

## Runtime components

### Native Mac app and native session

[`macos/Tohseno`](../macos/Tohseno/) is a macOS 14 SwiftUI package with an
ordinary app executable and a testable core. It renders the six-state workspace
projection, uses native navigation/settings/file pickers and URL routing,
persists its selected app, submits create/evolve exactly once per gesture, and
receives live status from the service event stream. It performs no build,
recording, installation, or protocol transition itself.

[`cli/src/native_client.rs`](../cli/src/native_client.rs) is the bundled helper.
In release it verifies the signed parent app against the bundle's exact Team-ID
requirement, asks the workspace service for a one-use challenge, signs it with
the existing workspace key, and receives a 15-minute token bound to the current
service instance and scopes. [`cli/src/native_session.rs`](../cli/src/native_session.rs)
owns that separate authorization state; browser Origin/CSRF sessions remain a
different boundary.

### Adopted project record and execution

[`cli/src/living_project.rs`](../cli/src/living_project.rs) owns the private
`tohseno.private-living-project-store/1` records. Adoption canonicalizes an
exact Xcode project/workspace, asks only for a genuinely ambiguous app scheme,
reads Xcode build settings, Git state, and bounded repository instructions,
then performs a real Simulator build. It writes no metadata into the selected
repository.

An adopted-project request is a private signed `project.evolve.request`,
deliberately separate from public Shot evolution. It is persisted and indexed
by command ID before acknowledgement. The selected harness receives a bounded
execution packet and runs in the exact source root. After it exits
successfully, a signed generic-iOS Xcode build and codesign gate must pass.
CoreDevice installation is attempted only against exactly one reachable
iPhone; the exact bundle must appear in inventory before the state can become
Installed. The saved artifact and retry state survive service/Mac restart.

### Public release, catalog, and recipient execution

[`network/`](../network/) owns the closed `tohseno.catalog-release/1` model,
deterministic sanitized tar creation/extraction, narrow Xcode build-safety
classifier, and exact public release evidence verifier. The catalog signature
binds source bytes and tree, build recipe, permissions, optional immutable fork
parent, active generation, ShotID, BuilderID, checkpoint sequence, and public
checkpoint digest. It contains no private intention, absolute path, prompt,
Apple credential, or installation fact.

[`cli/src/network_commands.rs`](../cli/src/network_commands.rs) implements
`init`, `deploy`, `status`, `install`, and `fork`. Publication jobs and
Companion approvals are private durable records. Receive accepts only official
Tohseno links/IDs, verifies active activation and deployed runtime hashes, live
BuilderAccount key authority, exact transaction receipt/block/event, current
Registry head, manifest signature, and source digest before safe extraction.
An install-only import cannot become a new public Shot; a fork gets a new
random ShotID and retains the exact signed parent release.

[`website/apps/site/src/registry.ts`](../website/apps/site/src/registry.ts)
provides the durable catalog, private expiring staging, immutable blob
transport, verified receipt metadata, signed Builder profiles, permissioned
alias requests, and the constrained transaction state machine. The operator
store is not authority. Release finalization reads fresh chain state, and Mac
clients verify it again.

Recipient build and signing stay in
[`cli/src/living_project.rs`](../cli/src/living_project.rs). Green sources can
build automatically; named non-Green behavior requires an explicit Mac review,
and unsupported behavior never enters `xcodebuild`. The build uses the
recipient's locally selected development team and a recipient-local bundle
namespace. Installed means `devicectl` succeeded and exact bundle inventory on
one physical iPhone agrees. Repeating the same exact install refreshes local
signing without AI or a Registry mutation.

### Claim, timelines, and private attention

[`contracts/src/TohsenoClaimsV1.sol`](../contracts/src/TohsenoClaimsV1.sol) is
an additive, non-upgradeable ERC-721/ERC-5192 receipt with all transfer and
approval paths closed. One immutable edition opens at first Ship. A Claim
binds one Tohseno BuilderAccount, Shot, exact release, current public
checkpoint, claimant nonce, deadline, and SHA-256 Claim-mark commitment. The
contract reads the unchanged active ShotRegistry and accepts only the exact
ERC-1271 P-256 authorities; its signed activation is independent from the
generation-0.8 activation.

[`website/apps/site/src/claims.ts`](../website/apps/site/src/claims.ts) owns
the closed activation verifier, canonical reorg-aware Claims index, constrained
durable bootstrap/edition/Claim jobs, exact receipt and metadata routes, and
normalized mark rendering. First Ship publication is atomic in authority
order: Registry receipt, Claims edition receipt, then source/catalog promotion.
An Update cannot alter the edition. Claims service configuration is absent by
default and partial activation, indexer, or relayer configuration aborts.

The Registry projects canonical chain order as `shot.shipped`, `shot.updated`,
`shot.forked`, and `claim.edition_closed`; exactly one Ship exists per Shot.
Individual Claims do not flood Discover. Exact BuilderID follows live only in
private Mac/Companion preference state. The durable private Updates store
accepts only relationship-backed evidence and reconciles stable IDs/read state
over the encrypted Companion channel; generic Discover traffic never enters
it.

Companion normalizes the circle to exactly 64 fixed-width points and discards
raw touch behavior after canonical geometry exists. It signs the independently
recomputed EIP-712 action, waits for canonical mint evidence, persists the
receipt/cabinet, and only then queues the existing install request for that
exact release. The outbox may wait for an offline Mac. Claim is public;
preparation, Apple signing, device identity, and installation remain private.

### First open, readiness, and distribution

[`cli/src/native_install.rs`](../cli/src/native_install.rs) verifies the exact
bundled factory manifest, rejects symlinks/special files, stages an immutable
installer-owned release, and atomically publishes both stable executables and
`current`. Any activation error restores the previous selection; state and app
roots are never release payloads.

[`cli/src/device_readiness.rs`](../cli/src/device_readiness.rs) independently
projects macOS/Xcode/license/components, connected device, unlock/Trust,
Developer Mode, signing team, and a real deterministic minimal build/install/
launch/remove result. It exposes one instruction and at most one action at a
time and does not read or mutate Companion pairing. Packaging under
[`macos/Tohseno/Packaging`](../macos/Tohseno/Packaging/) builds universal
executables, writes integrity metadata, supports inside-out hardened signing
and notarization, creates the DMG, and verifies architecture, signatures,
stapling, and forbidden-secret absence.

### Intelligence and managed balance

The engine's one `HarnessAdapter` enum now resolves known tools, a bounded
owner-selected executable, a consented loopback OpenAI-compatible endpoint, or
managed OpenAI-compatible execution. Exact adapter/model/route and, for
managed work, pricing timestamp, estimate range, privacy tier, and maximum are
stored with the prepared execution. Recovery never rediscovers or changes the
route. [`cli/src/local_openai_harness.rs`](../cli/src/local_openai_harness.rs)
uses a bounded file-plan protocol and safe atomic source writes; secrets are
Keychain references rather than arguments.

[`website/apps/site/src/managed.ts`](../website/apps/site/src/managed.ts) is the
optional server boundary. Installation-signed claims obtain live allowlisted
catalog/pricing, Stripe hosted pack Checkout, or a reservation. The append-only
integer micro-USD ledger separates paid/promotional value and records holds,
actual charges, releases, adjustments, grants/revocations, and reconciliation.
A short-lived one-use capability reaches only the narrow Bankr completion
proxy; no Bankr credential enters a local artifact. Ambiguous outcomes stay
held for protected operator reconciliation. The public `/download/macos`
route is independent and fail-closed until immutable URL and digest config
exist.

### iPhone Companion

The shipping iOS product is
[`companion/apple/TohsenoCompanion`](../companion/apple/TohsenoCompanion/).
[`CompanionModel.swift`](../companion/apple/TohsenoCompanion/Sources/TohsenoCompanionApp/CompanionModel.swift)
owns the thin product flow, and
[`CompanionBackend.swift`](../companion/apple/TohsenoCompanion/Sources/TohsenoCompanionApp/CompanionBackend.swift)
constructs the production client. The transport and persistence implementation
lives in
[`sdk/apple/TohsenoCompanionKit`](../sdk/apple/TohsenoCompanionKit/), principally
[`Client.swift`](../sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Client.swift)
and
[`Storage.swift`](../sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Storage.swift).

The app reads a privacy-safe workspace projection, binds an evolution to the
displayed Shot, Expression, accepted Version, and ordinal, and asks the SDK to
queue it. The SDK returns only after the signed command, encrypted envelope,
and any encrypted reference payloads have been persisted. Network delivery is
then best-effort and repeatable.

### Local Workspace Service, Studio, and factory

[`cli/src/workspace_service.rs`](../cli/src/workspace_service.rs) is the
loopback-only, long-lived Mac service. It owns the one
[`ShotApplicationService`](../application/src/application_service.rs), opens
the command journal, recovers it before accepting traffic, serves Studio, and
polls paired relay mailboxes independently of any Terminal or browser window.
The installed process is the user LaunchAgent
`com.tohseno.workspace-service`; its lifecycle and paths are in
[`cli/src/service_commands.rs`](../cli/src/service_commands.rs).

Studio in [`studio/`](../studio/) is a thin browser projection of that service.
It is not a second factory. CLI, Studio, and Companion commands all converge on
the same application service. The removed dashboard, pipeline renderer,
Feedback/Marketing forms, and manual Version controls are intentionally absent
under ADR 0016.

The application layer in [`application/`](../application/) owns admission,
idempotency, recovery, the machine-wide factory lease, and detached execution.
The engine in [`engine/`](../engine/) owns Shot state, the exact lifecycle,
build/test/verification/delivery gates, and accepted lineage. Actual unattended
work enters through `tohseno shot run`, launched by
[`application/src/execution_manager.rs`](../application/src/execution_manager.rs).

### Companion relay

[`website/apps/companion-relay`](../website/apps/companion-relay/) is a separate
Bun service. Routes are in
[`src/routes.ts`](../website/apps/companion-relay/src/routes.ts), filesystem
mailboxes are in
[`src/storage.ts`](../website/apps/companion-relay/src/storage.ts), and
production fail-closed configuration is in
[`config.ts`](../website/apps/companion-relay/config.ts).

The relay validates routing metadata, bearer capabilities, sizes, cursors,
sender sequence watermarks, expiry, and capacity. It persists opaque bytes and
cannot decrypt a command, inspect an intention, admit a Shot mutation, or start
execution. Retention is bounded. APNs, when configured, is only a content-free
wake-up hint; correctness uses mailbox reconciliation.

### Identities, keys, and pairing

The shared private wire contract and its Rust verifier live in
[`companion/`](../companion/). The important modules are
[`identity.rs`](../companion/src/identity.rs),
[`pairing.rs`](../companion/src/pairing.rs),
[`command.rs`](../companion/src/command.rs),
[`capability.rs`](../companion/src/capability.rs), and
[`envelope.rs`](../companion/src/envelope.rs). Swift implements the same bytes
in the correspondingly named SDK files and is checked against
[`companion/test-vectors/companion-v1.json`](../companion/test-vectors/companion-v1.json).

The Mac's workspace seed is held through Keychain by
[`cli/src/workspace_identity.rs`](../cli/src/workspace_identity.rs). The phone's
BIP-39 identity derives domain-separated Ed25519 signing, X25519 agreement, and
local storage keys; its identity stays in Keychain. Commands are signed by the
phone and envelopes are end-to-end encrypted to the Mac. Command receipts and
workspace events are signed by the Mac and encrypted to the phone.

Pairing is a signed, one-use, short-lived invitation. The relay carries an
opaque encrypted response. The Mac verifies proof of the phone identity and
issues a revocable capability grant plus two directional mailbox capability
sets. Recovery words restore the phone identity, not the workspace grant;
pairing must be repeated.

### Shots, commands, and execution

A Shot is the factory identity and accepted history behind an ordinary app
folder. Its layout and engine persistence are implemented by
[`engine/src/shot_layout.rs`](../engine/src/shot_layout.rs),
[`engine/src/ledger.rs`](../engine/src/ledger.rs), and
[`engine/src/machine.rs`](../engine/src/machine.rs). An evolution is bound to
one exact accepted base. Both the Companion coordinator and application service
check it; stale work is durably rejected and never silently rebased.

The application command state machine is in
[`application/src/command.rs`](../application/src/command.rs). Its filesystem
journal is in [`application/src/journal.rs`](../application/src/journal.rs).
Before semantic work begins it stores immutable request metadata, canonical
payload bytes, and exact reference inputs. The execution manager then prepares
a stable app-local execution and starts a detached runner. The runner performs
the bounded harness work and deterministic gates defined by ADR 0019. A Version
is accepted only after those gates pass; harness exit alone is not success.

Details is the owner's disclosure, not a second product surface.
[`application/src/receipt.rs`](../application/src/receipt.rs) projects one
execution receipt per app — the preserved intention, the harness/model/route
that ran, metered tokens and additional charge, and each deterministic gate
that refused — assembled from that execution's own `execution.json`,
`completion.json`, `state-transition.json`, preserved `intent.md`, and, for
executions prepared before per-execution preservation existed, the durable
command journal found by recomputing the execution identity.
[`engine/src/harness_usage.rs`](../engine/src/harness_usage.rs) reads the token
total out of the private harness log that is already captured; no harness is
invoked differently in order to be metered, and a harness that reports nothing
stays unmetered rather than being recorded as zero.

## Persistence map

Default locations are shown; verification scripts override them with isolated
roots.

| State | Default location | Owner | Survives |
|---|---|---|---|
| Phone identity | iOS Keychain | Companion SDK | app termination and ordinary relaunch |
| Phone pairing, workspace projection, replay state, command outbox | protected, encrypted Application Support `TOHSENO/companion-state.bin` | Companion SDK | app termination and device restart |
| Exact encrypted reference/envelope copies | protected Application Support `TOHSENO/outbox/` | Companion SDK | app termination; removed after a verified Mac receipt |
| Pairing rendezvous and opaque mailboxes | configured absolute `COMPANION_RELAY_ROOT` | Companion Relay | relay process restart, within retention and available storage |
| Workspace identity | Keychain plus `~/.tohseno/service/workspace.json` reference | Local Workspace Service | service and Mac restart |
| Paired devices, relay cursors, admitted envelopes/commands, reference inbox, Mac outbox | `~/.tohseno/service/{devices,inbox,outbox}` | Companion coordinator | service and Mac restart |
| Durable commands and exact inputs | `~/.tohseno/service/command-journal/<command-id>/` | application service | service and Mac restart |
| Visible app and accepted Shot state | normally `~/Desktop/Tohseno/<app>/` | engine | service restart; the folder remains ejectable |
| Prepared/running execution, events, completion, private receipt | `<app>/.tohseno/executions/<execution-id>/` | execution manager and engine | service restart and, except for the limitation below, Mac restart |
| Factory serialization lease | private machine data root | application service | released automatically when the owning process exits |
| Native UI restoration | macOS app scene/default state | native Mac app | window closure and app relaunch |
| Native challenge/session | service memory, scoped to service instance | Local Workspace Service | intentionally expires; not valid after service restart |
| Installed factory programs | `~/.tohseno/releases/`, `current`, and stable `bin/` | native/legacy installer | app and service restart; rollback preserves prior selection |
| Intelligence configuration | `~/.tohseno/service/intelligence-v1.json` plus optional Keychain references | Local Workspace Service | service and Mac restart |
| Managed balance authority | configured server `MANAGED_COMPUTE_ROOT` | website managed service | process restart and operator backup/restore |
| Local network imports and delivery evidence | `~/.tohseno/service/living-projects-v1/` and visible `~/Developer/Tohseno/` source | Local Workspace Service | service/Mac restart; owner source remains visible |
| Publication approval jobs | `~/.tohseno/service/network-publications-v1/` | Mac + Companion signatures | service/Mac/phone reconnect |
| Public catalog, staging, blobs, profiles, alias requests | configured durable `REGISTRY_ROOT` | Registry service; signed/chain facts remain independently verifiable | process restart and operator backup/restore |
| Private follows and high-signal Updates | `~/.tohseno/service/network-preferences-v1/` | Local Workspace Service + encrypted Companion reconciliation | service/Mac/phone reconnect |
| Canonical Claims index and durable relayer jobs | configured durable Registry root | Claims service; chain receipts remain independently verifiable | process restart and canonical rebuild |

On service startup, command recovery runs before the loopback listener opens.
A prepared execution is started; a live detached runner is reattached; a
verified candidate waiting for a device resumes deterministic delivery. A Mac
restart during arbitrary harness mutation is not replayed: if no runner remains
for an in-flight harness phase, the execution is finalized as failed to avoid a
second intelligence pass over unknown partial mutations.

## Repository map and authority

The current product core is:

- `macos/Tohseno/` — primary native Mac projection and packaging.
- `companion/apple/TohsenoCompanion/` — primary phone request projection.
- `sdk/apple/TohsenoCompanionKit/` and `companion/` — Swift/Rust private wire,
  cryptography, state, and conformance vectors.
- `website/apps/companion-relay/` — content-blind internet mailbox.
- `network/`, `website/apps/site/src/registry.ts`, and
  `website/apps/site/src/claims.ts` — public release/Claim models, sanitized
  source transport, canonical indexes, verifiers, and constrained relayers.
- `cli/`, `application/`, and `engine/` — Local Workspace Service, durable
  application boundary, and Mac factory.
- `studio/` — thin local UI served by the service.

Important but separate from this private loop:

- `protocol/` is normative public protocol law; `node/`, `contracts/`, and
  `release/` concern public evidence and releases.
- `website/apps/site/` is the public site, managed-balance/proxy boundary, and
  separate web-to-local intention handoff. A relay record is not a Shot.
- `apple-identity/`, `fascia/`, and `oneshot/` support Mac identity, generated
  iOS source, and installation/release respectively.

Compatibility and historical material is intentionally not a second active
architecture. `MASTER_PROMPT.md`, `genome/LAWS.md`, `history/`, historical
release documents, and readable legacy lifecycle variants remain for byte and
record compatibility. `sdk/.../Examples/CompanionConformanceApp` and
`cli/src/companion_simulator.rs` are verification fixtures, not alternate
products. Old names such as readable `Conception` execution variants do not
authorize the removed Conception phase for new births.

For the boundary-by-boundary operational trace, see
[`GOLDEN_PATH.md`](GOLDEN_PATH.md).
