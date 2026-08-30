# TOHSENO Companion 1.1.0

The iPhone app. It is the human authority and remote control, not a mobile
factory: the Mac remains the factory, and this app implements no second
protocol, no second backend, no second synchronization mechanism, and no mobile
coding harness. Everything it does goes through the released
`sdk/apple/TohsenoCompanionKit`.

```text
Apps · Registry · Profile
  Apps     → Create / choose app / Evolve
  Registry → exact public release → Install or Fork
  Profile  → DeviceKey-signed profile / permissioned alias request
```

## Layout

```text
Sources/TohsenoCompanionApp/   the product surface (a library, so it is testable)
Tests/TohsenoCompanionAppTests/  what the product does, not what the SDK does
App/                            a thin @main shell and its Xcode project
```

Keeping the product in a library means `swift test` exercises the real screens'
model on a Mac, without a Simulator, and the shell contains nothing but client
construction.

The dedicated app target replaces nothing: the SDK's
`Examples/CompanionConformanceApp` remains the deliberately raw conformance
fixture for protocol verification, and stays that way.

## Screens

**First run.** The Mac installs and launches Companion through the cable with
a signed, expiring `tohseno://pair/v1/…` payload. The iPhone shows twelve
recovery words exactly once and pairs only after they are confirmed saved.
Those words never travel to the Mac, URL, relay, build settings, or logs. Then
straight into Your Apps. Capabilities remain granular, signed, and
revocable underneath; that vocabulary never appears here.

**Your Apps.** After a short restoration state, the Companion uses the whole
screen for a compact, scrollable icon grid. Each app has the real icon where
the Mac has one, a one-letter mark where it does not, a short name, and one
status-colored dot. There are no oversized cards, generic feed, or decorative
creation dock competing with the apps. A read-only Sync button beside Mac connected requests the newest
encrypted workspace projection and reports when the Mac cannot be reached.
Opening an app uses native stack navigation, including the iPhone's standard
left-edge swipe to return to Your Apps, and never submits a build.

Generated-app creation is one clear Companion action routed to the same Mac
factory. The evolution composer includes native speech
recognition and screenshot attachments; spoken text stays editable and a
failed transcription is never reported as success.

**One app.** The app's name, its current state if it has one, and the box:

```text
ANKY

What should change?

┌────────────────────────────────────┐
│ Make the timer smaller and keep    │
│ the writing on screen…             │
└────────────────────────────────────┘

+ Add screenshots

                             Evolve
```

No feedback-saving step. No version picker. No execution configuration. The
screen says that opening is read-only; only the explicit Evolve App button
sends one request. It stays disabled while that app already has work in flight.

Legacy entitlement projections remain decodable but do not replace the current
person-to-person product or gate local/BYO execution.

**Registry.** The phone browses real catalog releases and opens one exact
ShotID/release digest. Install and Fork queue a signed, encrypted,
replay-protected command containing only those identifiers and the bounded
action. The Mac independently resolves and verifies the official release; the
phone never supplies an executable URL or source tree.

**Profile.** A separate P-256 Builder DeviceKey is created with the strongest
this-device-only Secure Enclave/Keychain mechanism. Its private scalar never
leaves the iPhone and is not a pairing, recovery, installation, or Apple
signing key. Publication approval displays the complete bounded release/action
summary, recomputes digests, and signs only after the person's explicit tap.
Public profile updates and alias claims use the same exact-digest, low-s P-256
boundary; aliases remain pending until explicit server policy review.

## What one tap means

`Evolve` is fire-and-forget for the person and durable underneath. The SDK
signs the command, seals it, and persists it with its images *before* the tap
returns, so the app can be closed immediately.

| Situation | What the phone says |
| --- | --- |
| Mac reachable, factory free | `Building Anky…` |
| Mac unreachable | `Waiting for your Mac…` |
| Mac reachable, factory busy | `Waiting…` |
| Verified but no cable | `Anky is ready.` / `Connect this iPhone to your Mac to install the update.` |
| Accepted | `Anky updated ✓` |

`Waiting for your Mac…` is derived from `unacknowledgedCommandCount()`, not
guessed from connection state: the phone is authoritative for its outbox until
the Mac acknowledges, so it can say that honestly. The request is never
re-sent by hand — reconciliation on launch and foreground delivers it.

The exact accepted base is bound at submission. If it genuinely moved first the
Mac refuses the command, and the phone says:

```text
This app changed while your request was waiting. Review it and try again.
```

It is never silently rebased.

## One projection, two surfaces

`TohsenoPresentation` mirrors `application/src/presentation.rs`. The frozen
companion snapshot schema does not carry the human state, so the phone derives
it from the same execution states, and both sides assert against
`Resources/presentation-v1.json` (a copy of the repository's
`fixtures/presentation-v1.json`). The Mac and the phone therefore cannot
describe one app differently. The copy differs on purpose: the Mac speaks to
the person about their iPhone, and the iPhone speaks about itself.

## Build and test

```sh
swift build --package-path companion/apple/TohsenoCompanion
swift test --package-path companion/apple/TohsenoCompanion

xcodebuild \
  -project companion/apple/TohsenoCompanion/App/TohsenoCompanion.xcodeproj \
  -scheme TohsenoCompanion \
  -destination 'platform=iOS Simulator,name=iPhone 16' \
  build
```

The tests drive the model against a stand-in Mac. They cover the product's
promises — one tap sends exactly one command, an empty box cannot be sent, an
unreachable Mac is not an error, a busy Mac just waits, a refusal becomes one
human sentence, and pairing lands directly in Your Apps. Crypto, envelopes,
durability, and reconciliation stay covered by the SDK's own tests rather than
being re-tested here. The SDK tests additionally cover Builder DeviceKey,
publication canonicalization, active-generation digest interoperability, and
durable Install/Fork commands; the website Registry tests consume compatible
P-256 canonical payloads.

## Not in this app

Expressions, Versions, executions, command identifiers, capabilities, relay
terminology, and synchronization internals are not normal UI. Shot/release and
Builder trust facts appear only where needed for public Registry and Profile.
There is no source browser, build log, model picker, or chat on the phone.
