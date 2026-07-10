# The one-core-action engine

## Current implementation

### The actual action

Anky's native core action is not merely “write for eight minutes.” The executable contract is:

> Accept one forward grapheme at a time, encode its wall-clock delta locally, persist every accepted progression, classify the session complete after eight minutes, and seal it after stillness.

The writing ritual, `.anky` protocol, local draft store, post-session flow, level credit, companion feedback, haptics, and write-before-scroll gate are all coordinated by `WriteViewModel`. As a result, the current “engine” is partly pure protocol code and partly product orchestration.

Evidence:

- `apps/ios/Anky/Features/Write/WriteViewModel.swift:1-1161`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:1-1170`.
- `apps/ios/Anky/Core/WriteBeforeScroll/WritingSessionEngine.swift:23-171`.
- `apps/android/app/src/main/java/inc/anky/android/core/protocol/WritingSessionEngine.kt:63-189`.

### State machine reconstructed from code

The code does not expose one cross-platform enum for the whole lifecycle. The following state machine is an evidence-based reconstruction:

```text
                         app/draft load
                              |
                              v
             +---------- recoverable draft ----------+
             |             | resume                   | discard
             |             v                          v
             |       active(fragment) <--------- idle/ready
             |             | accepted glyph             |
             +-------------+ save draft each progression+
                           |
                elapsed active time >= 480 s
                           v
                    active(complete)
                           |
          +----------------+----------------+
          | configured inactivity           | qualifying gate/lifecycle exit
          v                                 v
 sealed(fragment or complete) ----> archive/index/continuity credit
          |
          +----> post-seal choice/automatic request
          |           |
          |           v
          |      reflected or reflection pending/failed
          v
 return invitation / painting home / next writing
```

Important consequences:

- “complete” is a threshold flag, not a terminal state.
- “interrupted” is not named as a first-class state; a sub-threshold silence is persisted as a fragment.
- “sealed” happens before reflection.
- “discard” is explicit only around draft recovery/import-management paths, not a routine end action.

### Start condition

The session is initially empty. The first accepted extended grapheme:

- captures current epoch milliseconds;
- creates the protocol's first line;
- starts active elapsed time;
- persists the draft.

Entering the screen alone does not create an artifact. Resuming a saved draft reconstructs the writer and resets its wall cursor so time outside the active ritual is not counted.

Evidence:

- `apps/ios/Anky/Core/Protocol/AnkyWriter.swift:3-115`.
- `apps/android/app/src/main/java/inc/anky/android/core/protocol/AnkyWriter.kt:8-67`.
- `apps/ios/Anky/Features/Write/WriteViewModel.swift:217-280,384-468`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:607-637`.

### Progress capture and timing

Each line after the first stores the nonnegative delta from the previous accepted input. Duration is the sum of recorded deltas. Wall-clock gaps are intentionally excluded after an explicit resume.

iOS's `AnkyWriter.replaceSuffix` tracks a detached wall cursor during suffix replacement. Android's corresponding implementation is shorter and does not reproduce the same cursor adjustment. That creates a possible timing divergence after frozen continuation or autocorrect/backspace-tail rewrites.

The pure `WritingSessionEngine` exists on both platforms. iOS wraps `AnkyWriter` and adds snapshot behavior, but its snapshot also calls `UnlockPolicy`, coupling protocol state to write-before-scroll. Android's production `WriteViewModel` directly owns `AnkyWriter` behavior rather than consistently delegating to its `WritingSessionEngine`.

Evidence:

- `apps/ios/Anky/Core/Protocol/AnkyWriter.swift:45-105`.
- `apps/android/app/src/main/java/inc/anky/android/core/protocol/AnkyWriter.kt:34-67`.
- `apps/ios/Anky/Core/WriteBeforeScroll/WritingSessionEngine.swift:23-171`.
- `apps/android/app/src/main/java/inc/anky/android/core/protocol/WritingSessionEngine.kt:63-189`.

### Completion and interruption conditions

| Condition | iOS | Android | Result |
|---|---|---|---|
| Active duration reaches 480,000 ms | `AnkyDuration.isComplete`; haptic/visual acknowledgment | Corresponding hard-coded duration logic | Session remains active but is classified complete |
| No accepted glyph for configured silence | Preference range 1–8 seconds, default 8 | Hard-coded 8 seconds in write VM | Seal artifact; fragment if under 480 s |
| App backgrounds | Save draft; quick-pass/gate path may seal; close if silence already elapsed | lifecycle observer can seal passive quick pass | Ordinary draft survives; gate behavior differs |
| Explicit recovery discard | Deletes active draft | Local flow can clear/replace | No continuity artifact from discarded draft |
| Midnight/day change | No time-based deletion invariant | loader can clear prior-UTC-day active draft | Android data-loss divergence |

The protocol specification says a terminal line is an integer from 1,000 through 8,000 ms. TypeScript enforces that range. iOS `terminalMarkerMs` accepts any positive integer when parsing. Android's parser recognizes only exactly `8000`. Runtime preference support also diverges: iOS persists a terminal-silence selection; Android's preference schema omits it despite similar comments.

Evidence:

- `protocol/SPEC.md:30-45`.
- `protocol/implementations/typescript/src/parse.ts`.
- `apps/ios/Anky/Core/Protocol/AnkyDuration.swift:3-36`.
- `apps/android/app/src/main/java/inc/anky/android/core/protocol/AnkyParser.kt:3-35`.
- `apps/ios/Anky/Core/Storage/WritingPreferencesStore.swift:49-143`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/WritingPreferencesStore.kt:56-121`.

### Text-input restrictions

#### iOS

`WriteView` wraps a hidden/native text input and reconciles mutations:

- one extended grapheme is the normal append;
- newline is rejected;
- paste/multi-glyph insertion is rejected;
- caret is forced to the end;
- default backspace is rejected;
- optional backspace rewrites a suffix;
- autocorrect can replace a recognized tail;
- marked-text composition is temporarily allowed, then synchronized.

Rejected mutations restore the authoritative text and do not append protocol time. Haptics and companion feedback are product effects, not codec behavior.

Evidence:

- `apps/ios/Anky/Features/Write/WriteView.swift:885-1024`.
- `apps/ios/Anky/Features/Write/WriteViewModel.swift:217-299`.
- `apps/ios/Anky/Core/Storage/WritingPreferencesStore.swift:49-143`.

#### Android

`HiddenTextInput` limits normal input to one extended grapheme and rejects newline. `WriteScreen` maps unexpected mutations through a narrower mutation vocabulary. Multi-glyph/paste behavior and suffix-rewrite semantics are not identical to iOS. The current `AnkyApp` does not pass its stored `WritingPreferencesStore` to `WriteViewModel`, so preference source code is not proof that live writing honors it.

Evidence:

- `apps/android/app/src/main/java/inc/anky/android/feature/write/HiddenTextInput.kt`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteScreen.kt`.
- `apps/android/app/src/main/java/inc/anky/android/app/AnkyApp.kt:192-217`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:148-185`.

### Draft recovery and persistence

| Concern | iOS | Android |
|---|---|---|
| Save frequency | After accepted progression and lifecycle checkpoints | After accepted progression and lifecycle checkpoints |
| Path | `Documents/ActiveDrafts/dotAnky.anky` | `filesDir/ActiveDrafts/dotAnky.anky` |
| Write method | Atomic replace; malformed files can be quarantined | Direct `writeText` |
| Recovery UX | Explicit Resume/Discard overlay | View-model restoration; shell behavior less explicit |
| Time away | Frozen/excluded from next delta | `prepareToResume` exists |
| Prior-day draft | Preserved; comment says time must not delete drafts | Cleared when first timestamp is outside current UTC day |
| After fragment seal | Draft cleared after successful archive | Active draft retained |
| Continue reflected fragment | iOS rejects continuation | Android continuation path lacks the same guard |

Evidence:

- `apps/ios/Anky/Core/Storage/ActiveDraftStore.swift:13-60`.
- `apps/ios/Anky/Features/Write/WriteViewModel.swift:332-468,1136-1152`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/ActiveDraftStore.kt:7-40`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:607-735,1161-1170`.

These are semantic differences, not cosmetic platform adaptations. In particular, Android can both archive a fragment and keep the same bytes as an active draft; later sealing can create overlapping/replacement artifacts. A framework event log cannot inherit that ambiguity.

### Foreground/background handling

iOS's view observes `scenePhase` to persist and to close a session whose silence deadline passed while inactive. `AppRoot` locks protected state and can invoke `sealIfLeftInMotion` for a quick-pass window.

Android's application shell locks on lifecycle transitions. `WriteScreen` has a lifecycle observer for passive quick-pass sealing. Because the shell does not supply `gateSession`, that branch is not proven active in the current composition.

Neither platform uses a background network requirement to make local sealing valid. Reflection retries are separate.

Evidence:

- `apps/ios/Anky/Features/Write/WriteView.swift:174-197`.
- `apps/ios/Anky/AppRoot.swift:946-1005`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteScreen.kt`.
- `apps/android/app/src/main/java/inc/anky/android/app/AnkyApp.kt:192-217`.

### Write-before-scroll enforcement

The product gate is a consumer of writing state, not the core action itself.

Pure policy code includes:

- a full daily writing target;
- quick-pass eligibility based on six words or sentence punctuation;
- a 15-minute pass;
- up to three quick passes per day;
- subscription/unlock ladder behavior.

iOS enforcement uses FamilyControls, ManagedSettings, DeviceActivity, App Group state, shield configuration, and shield action extensions. A shield action cannot reliably deep-open the main app; the code uses a notification/shared-state bridge and contains documented platform limitations.

Android enforcement uses UsageStats access, a foreground polling service (approximately 800 ms), blocked-app selection, alarms, and a shield activity. Store policy and OEM process management are operational dependencies.

Evidence:

- `apps/ios/Anky/Core/WriteBeforeScroll/UnlockPolicy.swift:91-239`.
- `apps/ios/AnkyWriteBeforeScrollShieldAction/ShieldActionExtension.swift:47-203`.
- `apps/ios/Anky/Core/WriteBeforeScroll/SCREEN_TIME_SPIKE_NOTES.md`.
- `apps/android/app/src/main/java/inc/anky/android/core/gate/runtime/GateWatcherService.kt:30-224`.
- `apps/android/app/src/main/AndroidManifest.xml`.

This subsystem should remain an Anky reference/optional adapter. A gratitude, rehabilitation, or maintenance continuity app may invite return without blocking unrelated applications.

### Error states

Current errors include:

- malformed or empty draft/artifact;
- failed atomic/direct file write;
- failed archive/index update;
- identity/signing failure;
- reflection entitlement/rate/network/provider errors;
- platform permission denial for gate/notifications/biometrics;
- restore/import validation failure.

`WriteViewModel` generally keeps local state and surfaces a UI message on persistence failure. However, archive/index/level/draft-clearing effects do not form one database transaction. A crash between effects can leave a valid artifact with stale index, duplicate unreported level record, or retained draft. Index rebuilds mitigate some cases.

### Tests and existing invariants

Protocol and native tests cover fixtures, parsing, reconstruction, duration, hashing, writer operations, identity signing, local storage, mirror behavior, and portions of view-model logic. Android source-invariant tests also inspect forbidden logging/content patterns. iOS includes write-before-scroll and privacy tests.

Evidence:

- `protocol/implementations/typescript/test/protocol.test.ts`.
- `apps/ios/Anky/Tests`.
- `apps/android/app/src/test/java/inc/anky/android/protocol/ProtocolFixtureTest.kt`.
- `apps/android/app/src/test/java/inc/anky/android/write/WriteViewModelTest.kt`.
- `apps/android/app/src/test/java/inc/anky/android/privacy/SourceInvariantTest.kt`.

Missing or insufficiently protected invariants:

- live composition-root dependency wiring;
- shared Swift/Kotlin/TypeScript golden behavior for continuation and Unicode;
- stable pre/post-reflection artifact identity;
- fragment draft ownership after seal;
- no time-based draft deletion on Android;
- endpoint-bound authorization;
- crash consistency across archive/index/draft/level effects;
- first-launch action reachability on both platforms.

## Generic versus Anky-specific

| Generic continuity concept | Current manifestation | Anky-specific policy mixed into it |
|---|---|---|
| Action started | First accepted glyph | Text/grapheme and epoch-line grammar |
| Action progressed | Append line + save draft | Forward-only input, `SPACE`, per-character timing |
| Action checkpointed | Active draft store | `.anky` plaintext path |
| Completion condition met | Duration ≥ 480 s | Exactly eight minutes |
| Action interrupted | Silence below threshold | Configurable 1–8 s / Android 8 s |
| Action sealed | Archive exact bytes | Terminal-marker rules |
| Continuity event persisted | Archive/index/level side effects | Hash-named `.anky`, writing stats |
| Immediate feedback | Haptic, companion, optional reflection | Anky character and reflection tiers |
| Accumulation updated | Level seconds and painting | Kingdoms, lazure, write-before-scroll |
| Continuation invited | Home/gate/reminder/widget | Blocking selected apps and quick passes |

## Interpretation

The generic behavior is **not isolated enough to extract safely today**.

Reasons:

1. The pure-looking engine depends on `UnlockPolicy`.
2. Production view models duplicate or bypass engine logic.
3. Persistence, level credit, UI feedback, gate unlock, and sealing share methods.
4. Platform composition roots provide different collaborators.
5. “fragment,” “complete,” “sealed,” and “reflected” are implicit booleans/data presence rather than one domain transition log.
6. The exact artifact format carries action timing semantics.

The correct extraction unit is not the current `WriteViewModel` and not the entire `Core/Protocol` folder. It is a new, characterized reducer and effect boundary whose first adapter reproduces Anky.

## TOHSENO implication

A minimal generic kernel could use commands and emitted facts:

```ts
type ActionCommand<Progress> =
  | { type: "start"; at: Instant; initial: Progress }
  | { type: "progress"; at: Instant; value: Progress }
  | { type: "lifecycle"; at: Instant; phase: "active" | "inactive" }
  | { type: "interrupt"; at: Instant; reason: string }
  | { type: "resume"; at: Instant };

type ActionFact =
  | { type: "started"; at: Instant }
  | { type: "progressed"; activeElapsedMs: number }
  | { type: "checkpointRequired" }
  | { type: "completionConditionMet"; policyId: string }
  | { type: "sealed"; outcome: "completed" | "interrupted"; reason: string };

interface CoreActionPolicy<State, Progress> {
  reduce(state: State, command: ActionCommand<Progress>): {
    state: State;
    facts: ActionFact[];
  };
}
```

This interface must not know about files, RevenueCat, paintings, blocked apps, model providers, or native text fields. Anky's policy adapter can interpret glyph mutations and its timing rules.

## Recommendation

1. Write golden lifecycle traces before introducing the reducer.
2. Extract `UnlockPolicy` calls from protocol snapshots behind a post-progress observer.
3. Define outcome and seal reason explicitly.
4. Make checkpoint/archive/event append a crash-consistent repository operation, or document compensating recovery.
5. Keep native input adapters native; share mutation fixtures and domain facts, not a lowest-common-denominator UI.
6. Resolve Android fragment retention and prior-day deletion as product decisions before claiming parity.
7. Keep eight minutes, silence duration, backspace, paste, newline, `.anky`, reflection voice, and gate policy in the Anky manifest/reference layer.
