# TOHSENO Companion 0.9.0

The iPhone app. It is a beautiful remote control for intent, not a mobile
TOHSENO: the Mac remains the factory, and this app implements no second
protocol, no second backend, no second synchronization mechanism, and no mobile
coding harness. Everything it does goes through the released
`sdk/apple/TohsenoCompanionKit`.

```text
Your Apps
    ↓
Choose App
    ↓
What should change?
    ↓
Evolve App
```

That is the whole product.

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

**First run.** One sentence about what connecting grants, twelve recovery words
shown exactly once, and one camera scan of the code from the Mac's Settings.
Then straight into Your Apps. Capabilities remain granular, signed, and
revocable underneath; that vocabulary never appears here.

**Your Apps.** Real icons where the Mac has them, one letter mark where it does
not, and at most one subtle status word — and only when something is actually
happening. A settled app says nothing.

**One app.** The app's name, its current state if it has one, and the box:

```text
ANKY

What should change?

┌────────────────────────────────────┐
│ Make the timer smaller and keep    │
│ the writing on screen…             │
└────────────────────────────────────┘

+ Add screenshots

                          Evolve App
```

No feedback-saving step. No version picker. No execution configuration. No
confirmation after the tap.

## What one tap means

`Evolve App` is fire-and-forget for the person and durable underneath. The SDK
signs the command, seals it, and persists it with its images *before* the tap
returns, so the app can be closed immediately.

| Situation | What the phone says |
| --- | --- |
| Mac reachable, factory free | `Evolving Anky…` |
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
being re-tested here.

## Not in this app

Shots, Expressions, Versions, executions, command identifiers, capabilities,
relay terminology, synchronization internals, and cryptographic identity are
all real underneath and none of them are shown. There is no source browser, no
build log, no model picker, and no chat.
