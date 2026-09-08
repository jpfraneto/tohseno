# TOHSENO EVOLUTION 0040 — MAKE THE WORKSHOP A RUNTIME

You are working inside the canonical Tohseno repository.

This is an implementation task, not a planning exercise.

Do not stop after analysis, an ADR, a report, mockups, or partial scaffolding. Read the current repository deeply, understand the existing architecture and accepted constraints, then implement this evolution end-to-end, verify it, and leave the repository in a coherent working state.

Do not create a branch or staging environment. Work on the currently checked-out canonical development path. Do not invent a parallel product, alternate Studio, second factory, second identity system, or replacement protocol.

Do not perform production deployment, contract deployment, irreversible infrastructure mutation, App Store submission, DMG promotion, npm publication, or other release ceremony unless an existing repository rule explicitly requires it for local correctness. The purpose of this run is to evolve the product and implementation and prove it locally.

The name Tohseno comes from One Shot.

Honor that here.

---

# 0. BEGIN BY READING THE REAL REPOSITORY

Before modifying anything:

* inspect `git status`
* inspect the current HEAD and recent history
* read `AGENTS.md`
* read the current `README.md`
* read `docs/ARCHITECTURE.md`
* read `docs/STATE.md`
* read the accepted ADRs relevant to the current product, especially:

  * ADR 0034: person-to-person native software
  * ADR 0038: npm CLI / `tohseno init`
  * ADR 0039: One Shot living workshop
  * the current native macOS / intelligence / harness decisions
  * the Companion authority, pairing, relay, Claim, Ship, Update, intended-device, and security decisions
* inspect the actual current implementations of:

  * the native Mac Living Workshop
  * the Mac root/state model
  * the Companion Workshop
  * Companion pairing and DeviceKey
  * the current durable encrypted Mac ↔ Companion command channel
  * harness detection and routing
  * the current Intelligence settings UI
  * app adoption
  * `tohseno init`
  * generated apps
  * the Apple SDK packages
  * existing local-network or transport code

Treat the repository as authoritative over assumptions in this prompt whenever a lower-level implementation detail has changed.

Do not reinterpret the product's existing security truths merely to make this task easier.

---

# 1. THE PRODUCT MODEL HAS EVOLVED

The deepest definition of Tohseno is now:

> A Tohseno Shot is software that rearranges the capabilities of the computers around you into an experience.

That sentence is not marketing decoration.

It is the architectural direction.

A Shot is no longer conceptually equivalent to “an iPhone app.”

A traditional iPhone app is a perfectly valid, focused Shot whose required capabilities happen to fit inside one iPhone.

For example:

```text
Traditional iPhone app

Shot
└── iPhone
    ├── compute
    ├── display
    ├── touch
    └── network
```

But another Shot might eventually be:

```text
Apple TV → world
Mac      → game server + intelligence
iPhones  → controllers
```

Another might be:

```text
iPhone → camera + microphone + RTMP stream
Mac    → chat + control room + overlays
```

Another:

```text
iPhone → scanner
Mac    → reconstruction / local AI
iPad   → editing surface
```

The important abstraction is not the device.

The important abstraction is the **Workshop**.

A Workshop is the trusted set of devices and capabilities presently available to a person.

The four concepts should become explicit:

```text
SHOT
software/intention

    ↓ enters

WORKSHOP
trusted devices and their real capabilities

    ↓ resolves into

SESSION
the temporary running arrangement of those capabilities

    ↓ may use

INTELLIGENCE
one capability available from the Mac
```

ADR 0039 made the Workshop a truthful product projection.

This evolution must make it a **real programmable runtime**.

---

# 2. NON-NEGOTIABLE COMPATIBILITY

This is an additive evolution.

Do NOT break the existing person-to-person software network.

Preserve all existing truths around:

* Shot identity and lineage
* source snapshots
* one Ship followed by Updates
* Claims
* Builder DeviceKey authority
* Registry
* Robinhood Chain contracts
* current frozen protocol encodings
* recipient-local source verification
* Xcode build
* recipient-local Apple signing
* intended iPhone
* Apple Trust / Developer Mode / provisioning boundaries
* current Companion pairing
* existing durable encrypted commands
* current adoption of living Xcode projects
* `tohseno init`
* `tohseno deploy`

No contract ABI changes are required by this work.

No new public-chain primitive is required.

No new cryptocurrency primitive is required.

No new public identity system is required.

The Workshop Runtime is primarily an **owner-local runtime**.

Do not put ephemeral runtime events onchain.

Do not put camera frames, controller input, UI events, model prompts, local capability state, or Workshop Session traffic through the Registry.

---

# 3. CREATE ADR 0040: THE WORKSHOP RUNTIME

Create the next available ADR if 0040 is still free. A suitable title is:

`ADR 0040: The Workshop is a capability runtime`

The ADR should canonize the following model.

## Shot

A coherent piece of software distributed through Tohseno.

A Shot may have one surface or many.

An ordinary adopted Xcode iOS project remains a valid Shot without modification.

## Workshop

The set of trusted computing devices currently available to the person running Tohseno.

The Workshop exposes truthful capabilities.

It must never claim a device or capability exists merely because the UI would look nicer.

## Workshop Device

A real participant such as:

* this Mac
* paired intended iPhone
* eventually iPad
* eventually Apple TV
* eventually another explicitly authorized device

A device has identity, platform, connection state, and capabilities.

## Capability

A typed resource provided by a device.

Examples include:

```text
display
touch
keyboard
camera
microphone
motion
location
filesystem
compute
audio
intelligence
```

Do not assume every capability is usable merely because the hardware exists.

Represent separately:

* declared capability
* hardware availability
* OS permission state where applicable
* runtime reachability
* authorization
* ready-to-use state

Unknown stays unknown.

Denied stays denied.

No silent permission fiction.

## Workshop Session

An ephemeral runtime arrangement produced when a Shot asks the Workshop for capabilities.

A Session is not a public protocol object and is not Shot lineage.

It exists while software is awake.

It may disappear and reconnect without changing the Shot.

---

# 4. TWO PLANES, ONE WORKSHOP

This distinction is critical.

The existing Mac ↔ Companion system contains durable, encrypted, signed and/or idempotent command machinery for actions that matter to product authority.

Preserve it.

That is the **authority/control evidence plane**.

It remains appropriate for things like:

* create/evolve requests
* Ship approval
* Update approval
* Claims
* installation authority
* durable queued commands
* public actions
* actions that must survive temporary disconnection
* anything whose exact evidence matters

Do NOT turn every runtime event into one of those commands.

Add an **ephemeral Session plane** for live software behavior.

The Session plane is for things such as:

```text
controller.axis.changed
button.pressed
camera.changed
overlay.selected
stream.health
cursor.moved
scene.changed
preview.frame
game.input
workshop.pulse
```

The Session plane must be:

* local-first
* low latency
* encrypted
* authenticated
* typed
* reconnectable
* non-authoritative
* non-durable by default
* incapable of silently crossing a Ship / Claim / payment / install / approval boundary

A Workshop event must never become a loophole around Companion human authority.

This is one Workshop with two classes of communication, not two products.

---

# 5. IMPLEMENT A REAL LOCAL WORKSHOP CONNECTION

Use Apple's supported networking primitives appropriate to the repository's current deployment targets.

Prefer modern `Network.framework` concepts for:

* service listener
* service discovery
* nearby/local connection
* Apple peer-to-peer networking where appropriate
* encrypted transport

Use Bonjour/service discovery where appropriate and provide the required local-network declarations and truthful permission copy.

Keep transport details encapsulated behind Workshop runtime types.

The rest of Tohseno should not reason about IP addresses, Bonjour instance strings, ports, or connection implementation details.

Conceptually, application-level code should see:

```swift
workshop.devices
workshop.session
workshop.send(...)
workshop.events
```

not:

```swift
192.168.1.13:49291
```

Do not create a cloud relay for the Session plane in this evolution.

Same-LAN / nearby Workshop operation is enough for the first implementation.

The architecture may leave a clean future boundary for remote Workshop Sessions, but do not implement speculative WAN transport, WebRTC infrastructure, TURN servers, or another relay now.

---

# 6. AUTHENTICATE THE DEVICE, NOT JUST THE SOCKET

A local connection is not trusted merely because it appeared through Bonjour.

Reuse the existing Companion pairing / DeviceKey / intended-device truth.

Do not invent an unrelated Workshop account.

The local runtime handshake must cryptographically bind the Session to the already paired Workshop participants.

Study the existing pairing material before choosing the exact mechanism.

If an existing shared secret or key agreement is appropriate for deriving session credentials, reuse it safely with domain-separated derivation.

If the existing architecture instead makes DeviceKey challenge/response the correct mechanism, use that.

The requirements are:

```text
wrong iPhone              → rejected
unpaired Companion        → rejected
stale/revoked pairing     → rejected
tampered handshake        → rejected
paired intended iPhone    → accepted
```

Session traffic must be encrypted.

Do not leak wallet addresses, recovery words, raw private identity material, source code, prompts, or unnecessary stable identifiers through Bonjour TXT records.

Discovery metadata should be intentionally minimal.

Document the exact threat model and authentication flow in ADR 0040.

---

# 7. INTRODUCE SHARED WORKSHOP TYPES

Create or extend the correct Swift package/module so macOS and iOS share one set of runtime semantics.

Do not duplicate nearly-identical Workshop models independently in the Mac app and Companion.

Choose names that fit the repository, but the conceptual API should contain equivalents of:

```swift
WorkshopDevice
WorkshopDeviceID
WorkshopPlatform
WorkshopCapability
WorkshopCapabilityState
WorkshopConnectionState
WorkshopSession
WorkshopSessionID
WorkshopEvent
WorkshopEnvelope
WorkshopRuntime
```

Events need:

* schema/version
* session ID
* sender device ID
* event type
* monotonically meaningful ordering or sequence metadata where required
* payload
* timestamp only where semantically useful

Do not overengineer global distributed consistency.

This is an ephemeral realtime bus, not a blockchain and not a database.

A simple versioned typed envelope is sufficient.

Allow binary payloads at the transport boundary so the architecture does not need to be replaced later for richer media, but do not implement video transport in this task.

---

# 8. MAKE CAPABILITIES FIRST-CLASS

The Workshop Runtime should know what each real device can currently contribute.

For the first implementation, represent at least:

### Mac

```text
display
keyboard
filesystem
compute
intelligence
```

### paired iPhone / Companion

```text
display
touch
camera
microphone
motion
```

Do NOT request Camera/Microphone/Motion permissions on startup merely to turn these rows green.

A capability can truthfully say something equivalent to:

```text
Camera
Hardware present
Permission not requested
```

until software actually requests it.

Device presence and capability authorization are separate truths.

Build the model in a way that naturally supports future:

```text
iPad
Apple TV
visionOS
additional iPhones
```

but do not fabricate those devices into runtime UI today.

---

# 9. A NORMAL XCODE APP IS A FOCUSED SHOT

Preserve the critical property of `tohseno init`:

It can adopt an ordinary Xcode project non-destructively.

Do not force an existing project to become multi-device.

If an adopted project contains no Workshop-specific declaration, its semantics are simply:

> This is a focused Shot whose runtime currently lives in its existing Apple target(s).

That should require no new code and should not break builds.

Do not automatically inject packages, change project structure, alter entitlements, add targets, or rewrite source merely because `tohseno init` was called.

If Workshop metadata belongs in existing Tohseno owner-local/adoption metadata, extend that representation additively.

Do not modify frozen/public Shot encoding merely to store runtime hints.

If there is no current suitable metadata boundary, create the smallest owner-local, source-visible declaration consistent with existing repository conventions and document it.

But keep the default zero-config.

The important semantic rule is:

> Every existing app can be a Shot. Only Shots that want Workshop capabilities need to opt into Workshop APIs.

---

# 10. CREATE A WORKSHOP SDK BOUNDARY FOR SHOTS

Establish the developer-facing boundary that future generated/adopted software can use to participate in the Workshop.

Prefer a small Swift package with no unnecessary dependencies.

The desired ergonomics are approximately:

```swift
let workshop = TohsenoWorkshop.current

let session = try await workshop.join()

for await event in session.events {
    // react
}

try await session.send(
    .event("controller.button", payload: ...)
)
```

And conceptually:

```swift
let devices = workshop.devices
let intelligence = workshop.capability(.intelligence)
```

Do not freeze these exact names if the repository has a better naming convention.

The API should be tiny enough that a future app generated by Tohseno can use it without understanding the transport.

An app that never imports the Workshop SDK remains a completely valid Shot.

---

# 11. DO NOT CHEAT ON THIRD-PARTY SHOT AUTHORIZATION

A future Shot running as its own iOS app cannot simply read Companion's private Keychain material.

Do not weaken iOS isolation.

Do not place the Companion DeviceKey in an App Group available to arbitrary Shots.

Do not export the DeviceKey.

Do not give every locally installed app unrestricted Workshop access.

If this evolution implements standalone Shot enrollment, use an explicit capability-grant model.

A sound shape is:

1. the Shot creates its own app-local key
2. it discovers the local Workshop
3. it asks to join as its exact Shot identity
4. Companion presents a truthful one-time authorization such as:
   `Allow <Shot> to join your Workshop?`
5. Companion issues/signs a grant binding the Shot identity to the app-local public key and intended device/workshop
6. the Shot proves possession of its private key to the Mac
7. the Mac verifies the grant using already trusted Companion authority
8. subsequent runtime Sessions use the resulting authorization
9. authorization can be revoked

The exact implementation should adapt to the existing pairing architecture.

If completing this securely in the current pass would require violating an accepted security boundary, implement the shared Runtime fully between Mac and Companion and establish/test the grant types and interfaces without pretending standalone Shot enrollment is complete.

Never fake the secure boundary for the sake of a demo.

---

# 12. INTELLIGENCE BECOMES A MAC CAPABILITY

This repository already has harness detection and intelligence routing.

Do not rebuild it.

Reframe it.

The Mac contributes an `intelligence` capability to the Workshop when a real usable local/BYO harness exists.

The mental model is:

> Tohseno borrows the intelligence already available in your workshop.

For this MVP, intelligence comes only from providers already genuinely available/authenticated on the person's Mac through the supported harness architecture.

Examples may include Codex, Claude Code, OpenCode, local endpoints, or whatever adapters the current repository actually supports.

Do not invent an adapter merely because a provider name appears in this prompt.

Do not ask the person to paste API keys if their current authenticated local harness already works.

Do not meter or paywall local/BYO intelligence.

Do not silently route to paid managed inference.

The existing router should resolve real available intelligence.

Creation/evolution may use the selected/default available harness exactly as the current factory already does.

An adopted app can still be built/shipped without intelligence.

The Workshop itself can still open without intelligence.

But One Shot **creation/evolution** should truthfully indicate when no usable intelligence exists instead of pretending Tohseno can generate software.

---

# 13. SIMPLIFY THE INTELLIGENCE UI NOW

The current managed inference / balance UI is ahead of the product.

Do not delete valuable backend plumbing unnecessarily.

But remove it from the primary current experience.

The visible Intelligence surface should communicate approximately:

```text
INTELLIGENCE

Tohseno uses intelligence already available on this Mac.

● Codex
  Available

● Claude Code
  Available

○ Tohseno Intelligence
  Coming soon
```

The exact providers shown must be derived from real detected harnesses.

If only one provider exists, show one.

If none exists, tell the truth.

Keep advanced custom executable / local OpenAI-compatible endpoint configuration reachable if it is already supported and useful, but subordinate it under an Advanced affordance.

For now, hide/remove from the normal product surface:

* managed balance emphasis
* Add $10
* Add $25
* Add $50
* promotional balance
* “Welcome Compute”
* deposit flow
* payment-first inference language

Replace the managed product in the current visible experience with exactly this conceptual state:

```text
○ Tohseno Intelligence
  Coming soon
```

Do not implement Bankr payments in this evolution.

Do not implement USDC deposits in this evolution.

Do not implement surplus-intelligence markets in this evolution.

Leave clean architectural seams for them.

---

# 14. THE LIVING WORKSHOP UI MUST NOW SHOW REAL COMPUTING TRUTH

ADR 0039 already created the living workshop.

Do not redesign it back into cards/settings/admin dashboards.

Deepen it.

The scene should now visibly understand that the Workshop contains devices and capabilities.

The Mac and paired iPhone are not decorative illustrations.

They are runtime participants.

When connected, the scene may show the relationship as live.

When disconnected, it must visibly be disconnected.

When no iPhone is paired, it must not show a successful one.

When Intelligence is present, the Mac can truthfully communicate it.

When Intelligence is absent, do not create a glowing AI actor.

Avoid turning the main scene into a technical diagnostic screen.

The user should perceive:

```text
my Mac is here
my iPhone is here
they are connected
this Mac has intelligence
these Shots live in this workshop
```

before they perceive protocols and configuration.

Detailed capability information can appear contextually, for example when selecting a device or inspecting a Shot.

A focused existing iPhone Shot could communicate something like:

```text
Lives on
This iPhone
```

A future multisurface Shot may eventually say:

```text
Uses
This Mac · This iPhone
```

Do not invent multisurface runtime state for existing apps that do not declare it.

---

# 15. COMPANION BECOMES A REAL POCKET PARTICIPANT

ADR 0039 already calls Companion the pocket view/controller of the same Workshop.

Make this technically true.

When the local Workshop Session exists, Companion should have a real live connection state separate from merely “the durable command backend eventually received something.”

Its UI should be able to truthfully indicate:

```text
Mac workshop nearby
Connected
```

or:

```text
Mac workshop unavailable
```

Do not make connection status depend on animation.

Do not let Session connectivity imply public Registry connectivity.

Do not let local Session connectivity imply Companion approval of anything.

The Companion remains human authority for authority-sensitive actions.

It additionally becomes a real-time participant for runtime events.

---

# 16. BUILD ONE SMALL VERTICAL PROOF: WORKSHOP PULSE

Do not attempt the livestreaming product yet.

Do not attempt the Apple TV game yet.

Build the primitive once and prove it with the smallest experience that cannot be faked.

Add a developer-accessible / internal Workshop test called something like:

**Workshop Pulse**

It should use the actual authenticated Session plane.

On the Mac:

```text
WORKSHOP PULSE

This Mac  ●────────●  This iPhone
              live

[ Send pulse to iPhone ]

Last event from iPhone: 0.032s ago
```

On the Companion:

```text
WORKSHOP PULSE

Connected to Mac workshop

[ Send pulse to Mac ]
```

Behavior:

* tapping Send Pulse on iPhone produces an immediate visible state reaction on Mac
* tapping Send Pulse on Mac produces an immediate visible reaction on iPhone
* use a subtle haptic on iPhone when appropriate
* show measured round-trip or last-event timing if cheaply available
* disconnect Wi-Fi / make peer unavailable → state changes truthfully
* reconnect → Session can re-establish
* wrong/unpaired device cannot participate

This is not a fake preview.

It must traverse the real runtime transport.

The point of Workshop Pulse is:

> We have proven that two computers in a Tohseno Workshop can behave like parts of one piece of software.

Once this works, the architectural primitive exists.

Keep Workshop Pulse subordinate. It is a proof/debugging experience, not the product homepage.

---

# 17. DESIGN FOR THE LIVESTREAM SHOT WITHOUT BUILDING IT

After implementing the Runtime, verify architecturally that this future Shot would be possible without replacing the Runtime:

```text
LIVE

iPhone
- camera
- microphone
- front/back switching
- overlays
- direct stream output

Mac
- chat
- stream health
- overlay controls
- editable/forkable presentation skin

Workshop Session
- camera control events
- overlay state
- stream status
- moderation/control events
```

High-bandwidth media should not need to pass through the generic typed-event channel unless the Shot actually wants that.

For example, the iPhone may eventually stream directly to RTMP while the Workshop Session carries only controls/status.

Do not add RTMP dependencies now.

Likewise verify that the model can later express:

```text
Apple TV → game world
Mac      → game server / intelligence
iPhones  → controllers
```

without implementing tvOS in this evolution.

---

# 18. SHOT CAPABILITY DECLARATIONS

Introduce the smallest useful representation for a Shot to describe Workshop needs.

Do not require it for existing apps.

Conceptually it needs to be able to say something like:

```yaml
surfaces:
  iphone:
    capabilities:
      - camera
      - microphone

  mac:
    capabilities:
      - display
      - intelligence

session:
  realtime: true
```

Do not blindly adopt YAML or this exact schema if the repository already has a more appropriate manifest format.

The rules are more important than syntax:

* additive
* versioned
* human-readable where practical
* deterministic
* does not change frozen public Shot identity unless an existing canonical-source rule intentionally makes it part of source
* optional for focused legacy/adopted apps
* capable of adding more Apple device roles later
* distinguishes requirements from preferences
* capability names are typed/canonical rather than arbitrary UI strings

The resolver must never claim a Shot is runnable if a required real capability cannot be satisfied.

---

# 19. ONE SHOT COMPOSER CONSEQUENCE

Do not turn the One Shot composer into a giant device configuration form.

The user should still be able to write:

```text
Make a game where the Apple TV is the world and our iPhones are controllers.
```

Eventually Tohseno's intelligence should understand and produce the corresponding software arrangement.

The user should not need to manually choose:

```text
tvOS target
NWConnection
controller protocol
Bonjour service
```

That is precisely what the Workshop abstraction is for.

For this evolution, keep the existing creation path working and introduce the architectural context needed for the harness to understand Workshop concepts.

Update relevant builder/system context so generated software knows:

* a Shot may be focused or multisurface
* Workshop capabilities exist
* existing focused iOS output remains valid
* never invent unavailable devices
* use the Workshop SDK when genuinely needed

Do not force every generated app to become multi-device.

Simplicity remains a feature.

---

# 20. THE USER EXPERIENCE SHOULD MAKE THIS SENTENCE OBVIOUS

Without adding a long manifesto to the UI, the product should increasingly make this sentence feel true:

> A Tohseno Shot is software that rearranges the capabilities of the computers around you into an experience.

And the corollary:

> An app is a Shot that happens to fit inside one computer.

Use these as design tests.

Do not plaster both sentences everywhere.

The implementation should make them evident.

---

# 21. TESTING REQUIREMENTS

Add meaningful tests around the new domain and runtime.

At minimum cover:

* device model identity
* capability state derivation
* focused Shot fallback
* required capability resolution
* Session creation
* typed event serialization/version rejection
* handshake success for correct paired device
* handshake rejection for unpaired/wrong identity
* revoked/stale authorization where applicable
* disconnect
* reconnect
* no authority action through Session events
* Intelligence available from a real detected harness model
* Intelligence unavailable
* Tohseno Intelligence shown as coming soon rather than usable
* Workshop Pulse state mapping
* Mac scene truthfulness
* Companion scene truthfulness
* accessibility labels for connection/capability state
* existing distribution/adoption/Claim/Ship tests continue to pass

Do not rely only on snapshots.

Add pure-state tests where possible.

Use fixtures explicitly marked as fixtures for UI previews.

Run every validation command mandated by the current `AGENTS.md`.

Fix failures caused by this work.

Do not weaken tests merely to get green.

---

# 22. DOCUMENTATION

Update the canonical docs so the repository no longer has two conflicting definitions of the Workshop.

Update as appropriate:

* ADR index
* architecture
* state
* mental model docs
* Mac product docs
* Companion docs
* CLI docs if Shot semantics are relevant
* AGENTS authority pointers if repository convention requires it

Document this distinction clearly:

```text
ADR 0039
Workshop as truthful product composition

ADR 0040
Workshop as actual local capability/runtime fabric
```

Document that the Session plane is separate from the durable authority plane but shares the same Workshop identity and security model.

Document the focused Shot rule:

> Existing Xcode apps remain first-class Tohseno Shots without modification.

Document the future examples as examples, not implemented claims.

---

# 23. EXPLICIT NON-GOALS FOR THIS PASS

Do not implement:

```text
RTMP livestream product
Twitch integration
YouTube integration
chat provider integrations
Apple TV client
tvOS game
multi-iPhone game controller product
video streaming over Workshop Runtime
remote/WAN Workshop Sessions
WebRTC/TURN infrastructure
USDC deposits
Bankr payments
Tohseno Intelligence purchasing
surplus intelligence market
GPU marketplace
new smart contracts
new token mechanics
App Store distribution
new release ceremony
```

Do not spend the run beautifying unrelated surfaces.

Do not rewrite working protocol code for architectural aesthetics.

Do not delete existing functional behavior merely because it looks old; migrate its projection cleanly if this evolution touches it.

---

# 24. IMPLEMENTATION PRIORITY

If tradeoffs arise, prioritize in this order:

1. preserve existing protocol/security truth
2. real authenticated local Mac ↔ iPhone Session
3. shared Workshop device/capability model
4. Workshop Pulse working end-to-end
5. intelligence represented as a real Mac capability
6. simplified Intelligence UI
7. Living Workshop reflects the new runtime truth
8. optional Shot capability declaration
9. developer-facing Workshop SDK
10. documentation/polish

Do not sacrifice items 1–4 for decorative UI.

---

# 25. DEFINITION OF DONE

This evolution is complete when all of the following are true.

Opening Tohseno on the Mac still gives me the Living Workshop.

My actually paired iPhone appears only according to real state.

The Mac and Companion can establish a real authenticated local Workshop Session.

Workshop Pulse can send an actual live event:

```text
iPhone → Mac
```

and another:

```text
Mac → iPhone
```

without using the public Registry or pretending a durable authority command is a realtime bus.

The runtime has typed devices and capabilities.

The Mac exposes Intelligence only when a real supported local/BYO harness is available.

The primary Intelligence UI does not ask me to buy credits.

It shows real local intelligence and:

```text
○ Tohseno Intelligence
  Coming soon
```

An existing ordinary Xcode app remains valid after:

```text
tohseno init
```

without being rewritten into a complex multisurface application.

Such an app is understood as a focused Shot.

The architecture has a clean, tested path for a future Shot to ask for multiple Workshop capabilities.

No chain ABI changed.

No security boundary was weakened.

No fake Workshop state was introduced.

Existing tests still pass.

New tests prove the runtime.

The repository documentation agrees with the implementation.

---

# 26. FINAL EXECUTION REPORT

At the end, create a concise implementation report describing:

* baseline HEAD you started from
* files/modules changed
* ADR added
* exact Workshop Runtime architecture
* authentication mechanism actually implemented
* Session transport actually implemented
* capability model
* Intelligence changes
* UI changes
* Workshop Pulse proof
* tests run and outcomes
* anything that remains deliberately not implemented
* any physical-device step that genuinely cannot be proven without the owner present

Do not call something physically verified if it was only unit-tested or run in a simulator.

If the physical iPhone is available and repository tooling already supports owner-attended validation without unsafe automation, prepare the exact minimal verification path.

Do not manufacture evidence.

---

# THE NORTH STAR

When this task is finished, Tohseno should no longer merely **look like** a workshop.

For the first time, the Mac and iPhone should actually constitute one.

The existing network answers:

> How does software move from one person to another?

The Workshop Runtime should begin answering:

> Once that software arrives, what computers can it become?

A Shot can still be a tiny iPhone app.

But it can now grow toward:

```text
Apple TV → world
Mac      → game server
iPhones  → controllers
```

or:

```text
iPhone → camera
Mac    → studio
```

or configurations we have not imagined yet.

Build the primitive, prove it with Workshop Pulse, keep every existing distribution truth intact, and leave Tohseno ready for the first genuinely multi-device Shot.

One Shot.

