# State of this repository

Written 2026-07-30, amended through 2026-08-05. This is the plain-language
answer to "what is going on here" for someone returning after time away. When
something below stops being true, update this file in the same change that
makes it untrue.

Source on `main` now contains the post-0.8.5 one-shot correction in ADR 0013.
It is not in the immutable `v0.8.5` archive or public installer yet; those
remain byte-identical to the published release. A new pinned immutable release
is required before installed users receive the behavior described under “App
birth in current source” below.

## What ships today

TOHSENO 0.8.5 ships for macOS. A person runs one install command
(`curl -fsSL https://tohseno.com/oneshot.sh | bash`), which verifies a
pinned, immutable release archive, installs the `tohseno` command-line tool
and its Apple identity helper transactionally, and starts Studio, a
localhost-only browser view over the same engine. The installer script is
pinned to an exact version and checksum; releasing means updating that pin,
not rebuilding from moving sources. Shot execution carries two laws: the
selected harness always launches with its own permission prompts bypassed
(`--yolo` for Codex, `--dangerously-skip-permissions` for Claude Code) so an
unattended Shot never stalls on an approval nobody is present to grant, and
running a Shot is identity-bound — prepare and run both refuse, before any
side effect, an app whose recorded Builder is missing or is not the local
identity, so no execution can land anonymously or under someone else's
Builder. When the unattended runner reaches its final outcome — accepted,
unsealed, cancelled, or stalled — it announces the ending with a native macOS
notification, a courtesy signal that never alters the durable record. Studio
opens an evolution by installing its retained Simulator artifact directly, so
reopening an app is seconds, not a rebuild.

What the user then gets is a local loop that needs no phone, no account, and
no TOHSENO server: they declare one intention in plain words, their own coding
agent (Claude Code or Codex, installed and authenticated by them — TOHSENO
never holds agent credentials) materializes a native iPhone app from it, and
the tool records an immutable version of that app. They can attach private
feedback to an exact version, evolve the app into a new version without losing
its origin, and verify, export, and import the whole signed history offline.
The folder on disk is the product: the app's identity and history live in
files inside it. The full end-to-end flow was re-verified on 2026-07-30 by
`scripts/test-ontology-lifecycle.sh` against real Xcode builds.

## App birth in current source

Source on `main` now interprets a Shot before accepting its first Genome. A
create execution preserves exact Intention material, discovers the local Xcode,
SDK, Simulator, connected-iPhone, and last-known-device capability context,
then invokes the selected harness in conception mode. The harness must return
strict `tohseno.conception-output/1` containing an app-specific Birth Plan,
Genome, product organs, target actors, stable requirement ledger, forbidden
substitutions, and Experience Contract. The engine deterministically validates
and internally accepts that exact proposal as the next phase of the same Shot;
there is no `--accept-genome` ceremony and no deterministic generic factory
template.

Taking the Shot starts a detached local runner immediately; no Terminal window,
Enter press, or macOS Terminal-automation consent is part of the default path.
Materialization begins immediately after internal acceptance of the
app-specific proposal.
The harness builds and tests the Release product, traverses the required
target-user journeys, writes evidence-bound `tohseno.experience-trial/1`, and
receives focused repair tasks until the independent criteria pass or the
bounded repair limit is reached. The engine owns sealing. A first accepted
Version requires protocol conformance, intent fidelity, and experience
verification; product gaps, missing must-level journeys, forbidden
substitutions, or missing required physical-device evidence leave an unsealed
candidate. The delivery-required recording path waits for a paired iPhone and
installs and launches the exact verified candidate before signing Version
acceptance. A missing phone leaves the Shot in flight; install or launch failure
leaves it unaccepted. Evolution retains its public meaning: a new intention
applied to an already accepted app.

The static Constitution is now distinct from the app-specific Genome. Generic
identity and continuity organs are protocol substrate and cannot satisfy
product requirements. The Apple Fascia remains deterministic truth about the
built artifact; it does not author camera, AR, network, navigation, or other
product choices. These source changes do not alter old commitments, signatures,
accepted directories, or historical verifier semantics.

## Web-to-local intention handoff active in production

Main now contains an additive first-surface flow: a Browser Draft is retained
in IndexedDB, frozen into the noncanonical `tohseno.intent-package/1`,
encrypted in the browser, temporarily relayed as bounded ciphertext, imported
into durable local pending state, and opened in the existing Studio. Neither
the browser nor relay creates a Shot. The existing engine creates the Shot
only after local onboarding and the person's single explicit TAKE THE SHOT
action. Conception and Genome acceptance are internal phases of that local
run, not additional relay or web authority.

This capability is active in production. The public installer is
byte-identical to the immutable, claim-capable `v0.8.5` release, and the Bun
relay uses the owner-controlled durable volume at the canonical HTTPS origin.
The relay still defaults off in source, and production startup rejects an
enabled relay unless durable storage, HTTPS, and the matching installer gate
are all explicit. The completed activation evidence is recorded in
`release/WEB_INTENTION_HANDOFF_ACTIVATION.md`.

Local pending intentions live under the same machine data root as identities
and installed state (`~/.tohseno` unless `TOHSENO_DATA_ROOT` is set), outside
the installer-owned release directories. Update and uninstall preserve them.

## The contracts, their deployment, and their activation

`contracts/src/` holds six successor Solidity source files. On 2026-08-01 UTC the
exact generation 0.8.0 `BuilderAccountFactory` and `ShotRegistry` were deployed
to Robinhood Chain mainnet as an inactive, untrusted candidate. Public evidence
is `contracts/audits/robinhood-inactive-deployment-0.8.0-20260801T021920Z.json`.

On 2026-08-02 UTC the owner ceremony activated the generation. A 2-of-3
release-authority policy was constructed and its digest
(`0xf144…943c`) explicitly approved by the owner; a sequence-1 activation
binding the deployment evidence, a fresh EIP-7951 probe, and activation block
25511561 was threshold-signed and accepted by two independent verifier
implementations; and the engine now pins that policy digest as its compiled-in
trust root, verifying the complete chain on every resolution. All artifacts
and owner-decision evidence live in `release/contract-activations/`. Two
recorded owner deviations: all three authority keys were generated on the
owner's Mac rather than separate offline devices, and the 72-hour production
canary was explicitly waived before signing — so the on-chain recovery path
has not yet been exercised, and a retroactive canary remains advisable.

`BuilderAccount` is a non-upgradeable smart account controlled by P-256 device
keys (the kind Apple hardware produces). It keeps separate counts of active
devices and active admin devices, refuses to revoke the last device ever and
the last admin device unless a recovery authority is configured, and replaces
the entire key set only through a recovery flow with a mandatory three-day
delay that any admin key can veto. `BuilderAccountFactory` deploys such
accounts at addresses predictable before deployment. `ShotRegistry` is a
neutral public witness where a controller can register a Shot (an app's
permanent identity) using a commit-then-reveal scheme hardened against
observers replaying or resetting other people's commitments, then append
hash-chained checkpoints and transfer control. It accepts any contract that
answers the standard signature-check interface — it deliberately does not pin
one account implementation, so a future account fix does not strand the
registry. `P256Verifier` wraps the EIP-7951 P-256 precompile and fails closed:
a chain without the precompile looks identical to an invalid signature, so
deployment anywhere requires a live probe of the actual target chain proving
correct verify results and the exact 6,900-gas cost.

## The two contract generations

The v0.7 contract generation shipped inside the 0.7.0/0.7.1 release artifacts
as sources, ABIs, and predicted addresses. It was never deployed, was
superseded after security review, and will never be deployed. Main's
deployment and release-build commands for it fail closed on purpose.

Generation 0.8.0 is the remediated successor: the sources currently in
`contracts/src/`, with a reproducible build definition committed at
`contracts/generations/0.8.0/generation.json`. Two independent AI audits are
complete; a human/competitive review remains outstanding. Post-deployment
verification also found and fixed an off-chain activation-validator
assumption: compiler runtime templates contain zero placeholders for Solidity
immutables, while instantiated runtime bytes are constructor-patched. ADR 0010
governs the exact distinction, and the signed activation binds the locally
instantiated BuilderAccount runtime hash per the owner's canary waiver.

The engine resolves the generation from its compiled-in trust root (three
constants in `engine/src/contract_generation.rs`, now carrying the approved
digest and ceremony artifacts) and verifies the complete
policy-plus-threshold-signed-activation chain on every resolution; a partial
or non-verifying trust root refuses to resolve. `tohseno protocol` and Studio
report the generation as active. What activation does NOT change yet: the 0.8
secure public identity workflow, registry RPC/relayer paths, and publication
receipts are separate unimplemented work (the post-activation gap audit's
Gaps 2–4), so every public-action surface still fails closed with an honest
"not implemented" reason. The private local lifecycle is unchanged: on a
machine with no identity, the first Shot creates a local, explicitly
test-only Builder identity by default (it can never authorize a public
action); an explicit `TOHSENO_IDENTITY_BACKEND=secure-enclave` request fails
with "not implemented" rather than "inactive", and legacy v0.7 identities
remain non-authoritative under every generation.

## What v0.7 retirement means for someone who already installed

If an installed 0.7.x CLI shows a predicted BuilderID or account address, that
prediction belongs to the retired generation and will never exist on-chain. It
is not a public identity, not ownership evidence, and not a future deployment
coordinate; the CLI now says so whenever it prints one. Everything local
remains valid: private Shots, identities, and signed history still verify
offline against the frozen v0.7.1 release inputs, which are kept byte-for-byte
in the repository and at the v0.7.1 tag for exactly that purpose.

## Capability policy for generated apps

The installed older release involved in the Anky dogfood run carried a
notification-only embedded Genome bundle. Repository HEAD replaces that model
with a data-driven Apple capability catalog and explicit states:
`supported`, `supported_with_permission`, `supported_with_entitlement`,
`hardware_specific`, `simulator_unavailable`,
`unknown_until_physical_device`, `unsupported_by_current_sdk`, and
`unsupported_by_factory`. Simulator absence and unknown hardware are not
product prohibitions. A true unsupported must capability is reported before
substantial materialization as a visible factory capability gap.

Only a paired device with an active `devicectl` tunnel is current connected
hardware. A paired but disconnected network device is local last-known
context, not live trial evidence; stored profiles contain product type and OS
facts but no UDID or private device identifier.

The Birth Plan supplies intent-level purpose. The scanner obtains structural
evidence from tokenized executable Swift, structured Info.plist, entitlements,
and Xcode settings. The engine reconciles both into the exact mechanical
`TOHSENO/capabilities.json` and final signed Fascia. Comments, documentation,
arbitrary asset text, XML namespace URLs, `eyeSocket`, `violetCurls`, and
`AVAudioEngine.connect` no longer claim network access; real `NWConnection`,
`URLSession`, camera, ARKit, microphone, and speech pipelines remain detected
and protected. Missing usage descriptions, undeclared sensitive behavior,
stale declarations, and under-scoped endpoints still fail closed with the
gate, category, file, token or structural fact, expected declaration, reason,
and app-versus-factory classification.

The catalog includes camera, microphone, speech, AR and RealityKit, spatial
audio, motion, haptics, Vision, persistence, notifications, App Intents,
widgets, NFC, Nearby Interaction, peer connectivity, Family Controls,
location, HealthKit, Bluetooth, HomeKit, CloudKit, StoreKit, network, and
secure storage materials. Native Apple frameworks are the current default.
Uninspected external runtime dependencies remain an explicit factory
capability gap; privacy, tracking, Builder-secret, and silent-identity
boundaries remain fail-closed.

## Deliberately deferred

Public activation and everything downstream of it (durable public BuilderIDs,
the public witness registry, publication receipts) wait on the remaining audit,
canary, and activation chain described above. The ADR 0013 unattended
one-action birth and required phone-delivery correction on main still needs a
new immutable CLI release before installed users receive it. Device-key
replacement for the frozen v0.7 identities is
closed — the successor generation's recovery design (ADR 0006) is the answer,
and no signed identity-supersession flow will be built until a real migration
needs one.

## After the first dogfooding pass (2026-07-31)

The full ceremony — intention → Shot → execution → usable app → contact →
version-bound feedback → evolutionary intent → Evolution — was run end to
end with a real coding harness on the subscription route, three Shots and
one Evolution landing as verified accepted Versions. The pass surfaced and
fixed five flow breaks: fresh-machine identity creation (above), retrying a
failed first execution (the engine's own standing orders no longer count as
builder work), feedback-to-evolution continuity (a successful seal now
mirrors its engine substitutions back into the living folder, and evolve
proves the feedback selection before any recording side effect), honest
handling of a Terminal window that cannot open, and `tohseno retire --local`
so an explicitly local retirement needs no phone. `tohseno doctor` now
checks signing, harness, and identity state, and Shot inputs are validated
before any folder exists. The 0.8.5 signing correction prefers a usable paid
Xcode team and derives any free-team three-app wall only from TOHSENO bundles
actually present on the connected iPhone. Simulator-only apps and accepted
local Shot history do not consume device slots. `docs/DOGFOOD_REPORT.md`
carries the original findings and their resolution status.

## Half-finished or worth knowing

The notice that the v0.7 generation is retired exists in the repository
(`release/V0_7_CONTRACT_GENERATION_NOTICE.md`) but still has to be added by a
release operator to the already-published external v0.7 release notes; the
repository cannot do that itself. The branch
`archive/codex-0.8-cutover-pre-remediation` (now also on origin) preserves an
abandoned pre-remediation draft of the 0.8 cutover for the record only — it
must not be merged. Engine regression coverage now exercises declared camera,
microphone, location, contacts, HealthKit, Bluetooth, StoreKit, CloudKit,
local Bonjour pairing, remote endpoint scoping, Apple authentication, native
Core Data, and the continuing tracking prohibition through the shared source
policy path. The sealed Apple Fascia artifact still labels itself candidate 0.7.0 by design —
sealed artifacts are never edited in place; its next accepted revision is
expected to add generation-scoped publication-receipt verification.
