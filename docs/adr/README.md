# Architecture decisions

Accepted ADRs are authoritative architecture decisions beneath `protocol/`.

[ADR 0039](0039-one-shot-living-workshop.md) makes the native Mac and
Companion a shared living software workshop: Mac factory, intended iPhone,
keeper/human authority, real app objects, network threshold, and one existing
One Shot command path. It migrates the Apps, Registry, Updates, and Profile
capabilities without creating a second factory, restoring Studio, changing any
protocol or ABI, or turning animation into installation/publication evidence.

[ADR 0038](0038-npm-cli-init-first.md) makes npm the direct CLI installation
door for builders with an existing Xcode project. npm installs only the
dependency-free launcher with no postinstall side effects; interactive
`tohseno init` teaches the real Xcode, ShotID, Companion, Ship, Update, and
`deploy` path one Enter-gated line at a time while structured and
non-interactive use remains unblocked. Adoption begins only after the intended
iPhone's CoreDevice app inventory contains the exact Companion bundle and its
specific one-use private pairing has completed.

[ADR 0037](0037-network-mediated-release-trust.md) defines network trust as
exact-release evidence: additive external identity bindings, content-addressed
machine Verification Reports, bounded DeviceKey-signed human Release
Attestations, and private social context. It preserves Claim semantics and
forbids centralized safety verdicts, stale review inheritance, popularity
scores, or external identities becoming Builder authority.

[ADR 0036](0036-destination-driven-apple-delivery.md) makes a privately
associated physical iPhone the installation destination while USB and local
network remain observed transports. It retains verified artifacts until the
exact target is reachable, fails closed on ambiguity, and keeps Apple pairing,
Trust, Developer Mode, signing, and installation constraints visible.

[ADR 0035](0035-claiming-software.md) adds Claim as a public, intentional,
non-transferable software encounter; makes a Shot ship exactly once and update
thereafter; opens one immutable Claim Edition at first Ship; and turns Registry
into Discover, private Following, and a high-signal private Updates inbox. The
additive Claims contract has separate threshold-signed activation and changes
no frozen protocol encoding or deployed generation-0.8 ABI.

[ADR 0034](0034-person-to-person-native-software.md) makes Tohseno the
person-to-person native software network: the Mac remains the factory, the
Companion owns Builder publication authority, generation 0.8 remains the
public checkpoint witness, and a signed content-addressed catalog carries
buildable source between people. It adds no contract generation and preserves
Apple's signing, Xcode, Trust, Developer Mode, and physical-install truth.

[ADR 0033](0033-living-project-connection.md) makes the durable connection
between an iPhone app, its owner-local source project, the Companion, one coding
harness, and truthful Xcode/device delivery the primary product. Adopted
projects use a private versioned record and signed project command; they are
never fabricated as protocol Shots, and creation remains a secondary path.

[ADR 0031](0031-public-release-candidate-for-clean-mac-acceptance.md) allows
the exact signed, notarized, digest-pinned DMG to use an explicitly labeled
public prerelease channel for testing the real website-to-Finder path on a
clean Mac. Stable promotion and all remaining gates stay closed until that
acceptance passes.

[ADR 0030](0030-system-aware-direct-native-download.md) makes an ordinary,
system-aware **Download for this Mac** link the canonical website installation
door. It removes shell-command copying from the normal landing path while
preserving the immutable DMG, exact digest, notarization, Gatekeeper, Finder,
and fail-closed release boundaries.

[ADR 0029](0029-first-shot-before-factory.md) makes TAKE A SHOT the real
first creation surface before the factory whenever the workspace has never
recorded a Shot. It uses the existing Return-to-send command path, accepts up
to eight dropped or picked PNG/JPEG references, and offers one persisted,
secondary Skip without changing protocol or activating publication.

[ADR 0028](0028-finder-first-install-and-native-welcome.md) makes the native
one-liner an Enter/Escape, progress-bar, verified-DMG handoff to Finder and
gives a genuinely empty first open the small TAKE A SHOT invitation. ADR 0029
supersedes its passive, no-state welcome composition; the Finder handoff stays
accepted and publication remains inactive.

[ADR 0027](0027-native-app-workspace-and-device-stage.md) makes the native
selected-app surface a Build/App/Source workspace with bounded owner-local
activity, changed source files, an honest latest Simulator capture, and a
permanent automatic cable-handoff card. It keeps internal execution concepts
and raw harness output out of the normal path and does not restore the deleted
Studio dashboard or claim an interactive embedded Simulator.

[ADR 0026](0026-keyboard-first-local-registry-and-native-installer.md) makes
plain Return submit each focused native composer, adds the truthful local
Registry/Builder track-record destination, and defines the fail-closed
one-line native installer without activating publication.

[ADR 0025](0025-native-macos-app-factory-managed-balance.md) makes the native
SwiftUI `Tohseno.app` the primary product while keeping the persistent Rust
service as the only factory. It removes npm/browser first run, mandatory
Companion setup, successful-day qualification, and subscription gating of
local/BYO execution from the consumer path; moves execution choice under
Advanced; and defines the append-only creation-balance, Stripe-pack, managed
proxy, Bankr, migration, packaging, and release-truth boundaries. No external
service or release is activated by the decision.

[ADR 0024](0024-app-local-tohseno-git-boundary.md) makes `.tohseno/` an
integral Git-visible part of each app repository while retaining exact ignores
for intentions, inline-private lineage, references, feedback, executions,
logs, artifacts, and `.tohseno/private/`. Git visibility never substitutes for
an explicit public-registry publication flow.

[ADR 0023](0023-per-shot-harness-and-model-choice.md) added one compact Studio
creation choice for an installed coding harness and model. ADR 0025 supersedes
that visible product placement but preserves its exact durable per-command
selection and ADR 0019's bounded implementation/repair ceiling.

[ADR 0022](0022-optional-model-chosen-app-name.md) makes the app name optional.
When omitted, the factory reserves a local technical slug and gives the one
existing implementation model responsibility for choosing the user-facing
product name from the exact intention, with no extra planning invocation.

[ADR 0021](0021-npm-install-enters-first-run.md) governed the legacy
npm-to-first-run transition. ADR 0025 removed it as the normal consumer door,
and ADR 0038 supersedes its remaining automatic postinstall behavior with a
side-effect-free CLI installation and init-first guide.

[ADR 0020](0020-cable-genesis-earned-pro-npm-front-door.md) defined TOHSENO
0.9.9's product boundary: the first Mac↔iPhone relationship begins
through the cable while the existing cryptographic pairing remains intact;
the complete factory is available for at most seven calendar days; five
accepted physical-delivery days qualify a person for $9.99 monthly or $99
yearly Pro; factory admission is enforced below every UI; Apple membership is
independent; and the npm package is a thin verified native-release front door.
It authorizes no publication or production external activation. ADR 0025
supersedes its Companion genesis, qualification/subscription gate, and npm
consumer-door decisions while retaining the underlying compatibility code.

[ADR 0019](0019-bounded-intent-to-usable-app.md) defines the current execution
hot path: one implementation harness, at most one concrete code/build repair,
one shared wall-clock budget, deterministic build and delivery, and one private
State Transition Receipt. CLI, Studio, and Companion remain origins of the
same durable application operation. It supersedes ADR 0017's older supervision
defaults and harness-authored Experience Trial requirement without changing
the public protocol or accepted history.

[ADR 0018](0018-the-companion-links-a-browser.md) records that the website
will never hold an account. When the public terminal can send more than once,
it will be the published Companion linking a browser to a phone: the phone
keeps the identity and issues a scoped, revocable capability, generated apps
stay free of identities and grants, and a linked browser still reaches only
the Mac that phone is paired with. It is accepted and deliberately not
implemented, and it lists what must be true before it can be.

[ADR 0017](0017-the-engine-composes-the-genome.md) defines how a birth runs:
the engine composes and accepts each Shot's Genome and Expression itself, and
the single harness invocation reads the exact human intention. It supersedes
[ADR 0012](0012-intention-led-app-birth.md)'s Conception phase while keeping
ADR 0012's intention-led birth and engine-owned acceptance intact. It also
bounds one unattended harness invocation by stall and total runtime.

[ADR 0016](0016-app-intent-app-on-your-iphone.md) defines the current
user-facing surface: the canonical abstraction is App → Intent → App on your
iPhone, and Studio and the Companion are thin projections over the same durable
local application service. It supersedes ADR 0015 as the description of what a
person sees, deliberately including what was deleted to get there. It does not
change ADR 0015's service, journal, capability, transport, or relay
architecture.

[ADR 0015](0015-persistent-local-factory-private-companion.md) defines the
current internal boundary: one persistent local app factory with CLI, loopback
Studio, and a private paired-companion channel. It supersedes
[ADR 0014](0014-app-version-feedback-product-boundary.md) as the description
of current `create`, `evolve`, and Studio behavior while preserving ADR 0014's
exact app-local recording format through explicit `init` and `record`
commands. Recording-only folders are not silently migrated into factory Shots.

[ADR 0011](0011-encrypted-web-to-local-intention-handoff.md) still defines the
historical web-to-local intention transport.
[ADR 0006](0006-public-witness-and-contract-generation.md) remains
authoritative for public-witness and contract-generation boundaries. None of
these ADR summaries override canonical encodings or validation rules in
`protocol/`.
