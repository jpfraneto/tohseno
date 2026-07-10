# The Anky lifecycle

## Scope

This trace follows executable entry points, not the older “Write/Reveal” descriptions in some READMEs. “Complete” below means the `.anky` duration reaches 480,000 ms. “Sealed” means inactivity or a lifecycle condition caused bytes to be archived. Those are separate transitions in the current implementation.

Status words are used strictly:

- **Implemented:** a live entry point invokes the behavior.
- **Partially implemented:** code exists but wiring, parity, or end-to-end behavior is incomplete.
- **Documented:** repository prose describes it, but the live call path does not establish it.
- **Planned:** a TODO/future specification exists.
- **Unclear:** OS or operational behavior cannot be proven from repository code.

## Current implementation

### iOS sequence

```text
Person       AppRoot       Identity/Keychain       Write VM/Stores       Backend/provider
  | launch      |                   |                     |                     |
  |------------>| onAppear          |                     |                     |
  |             | recover draft     |                     |                     |
  |             | load/create phrase+key                  |                     |
  |             |------------------>|                     |                     |
  |             | identify RevenueCat / reconcile gate    |                     |
  |             | onboarding screens + first live write   |                     |
  | type glyphs |----------------------------------------->| append + save draft |
  | ... 8 min   |                                         | mark complete; haptic
  | stop typing |                                         | silence timer        |
  |             |                                         | seal exact bytes     |
  |             |                                         | SHA-256 filename     |
  |             |                                         | archive/index/level  |
  |             |<----------------------------------------| post-seal choice     |
  | tap “Read”  |                                         | terminalize artifact |
  |             |                                         | sign exact POST body |
  |             |                                         |-------------------->|
  |             |                                         |  SSE markdown        |
  |             |                                         |<--------------------|
  |             |                                         | save reflection JSON |
  | return later| route painting home / write / recovery  |                     |
```

Evidence:

- `apps/ios/Anky/AppRoot.swift:868-1005` — launch and scene-phase composition.
- `apps/ios/Anky/AppRoot.swift:1260-1308` — onboarding completion around the live writing threshold.
- `apps/ios/Anky/Features/Write/WriteViewModel.swift:217-343,564-738,1136-1152`.
- `apps/ios/Anky/AppRoot.swift:1778-2025` — local sealing state followed by user-selected reflection.
- `apps/ios/Anky/Features/Reveal/RevealViewModel.swift:431-458` — terminalization before request.

### Android sequence and divergences

```text
Person       AnkyApp/AppContainer    Write VM/Stores          Backend/provider
  | launch          |                       |                         |
  |---------------->| configure entitlement; identity is loaded lazily
  |                 | deprecated onboarding wrapper auto-advances     |
  | type glyphs     |---------------------->| append + save draft      |
  | stop typing     |                       | hard-coded 8 s timer      |
  |                 |                       | archive SHA-256 bytes     |
  |                 |                       | keep active draft         |
  |                 |<----------------------| post-seal screen          |
  |                 |                       | auto-request if entitled  |
  |                 |                       |-------------------------->|
  |                 |                       | save markdown reflection  |
  | return later    | active draft may resume; a prior UTC-day draft
  |                 | may instead be cleared during initialization
```

The Android shell does not pass `writingPreferencesStore`, gate-session callbacks, level completion, or unlock callbacks into `WriteViewModel`; default/no-op arguments are used. Consequently, source files that implement those concerns do not prove live behavior.

Evidence:

- `apps/android/app/src/main/java/inc/anky/android/app/AppContainer.kt:44-180`.
- `apps/android/app/src/main/java/inc/anky/android/app/AnkyApp.kt:192-217,400-480,608-624`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:148-185,684-735,1161-1170`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/PostSessionSealingScreen.kt:90-94`.
- `apps/android/app/src/main/java/inc/anky/android/feature/onboarding/OnboardingScreen.kt:1415-1447`.

### Stage-by-stage trace

#### 1. Installation and first launch

**Entry point**

- iOS: `AnkyApp` creates `AppRoot`.
- Android: `AnkyApplication` creates `AppContainer`; `MainActivity` invokes `AnkyApp`.

**State transition**

- iOS loads app-open state, looks for a recoverable active draft, creates/loads identity, configures RevenueCat, checks biometric lock, inspects iCloud backup, and routes to onboarding or the painting home.
- Android creates long-lived stores and configures entitlement state when the unlocked shell appears. Its identity store is lazy, but RevenueCat configuration requests the account identifier and therefore normally creates the identity before the first core value.

**Storage and failure**

- Local preference stores remember onboarding/app-open state.
- Identity failures surface as app state/errors; neither platform has a password/login fallback.
- The precise persistence of Keychain/Keystore items after uninstall is an OS behavior and is **unclear** from code. No uninstall hook exists.

**Privacy and crypto**

Identity is created invisibly before a profile. This follows “practice before profile,” but the backend/RevenueCat identifier exists before the first completed action. No writing leaves the device at launch.

Evidence:

- `apps/ios/Anky/AnkyApp.swift:5-13`.
- `apps/ios/Anky/AppRoot.swift:868-945`.
- `apps/android/app/src/main/java/inc/anky/android/AnkyApplication.kt:6-13`.
- `apps/android/app/src/main/java/inc/anky/android/MainActivity.kt:17-54`.
- `apps/android/app/src/main/java/inc/anky/android/app/AnkyApp.kt:192-216`.

#### 2. Identity creation

Both apps generate 128 bits of entropy, encode a 12-word BIP-39 English mnemonic, derive `m/44'/60'/0'/0/0`, and expose the EIP-55 address. iOS stores the phrase in Keychain. Android encrypts the phrase with a Keystore-held AES-GCM key and writes the ciphertext/IV under app-private files.

No username, password, email, or profile record is required. The server creates account-related rows opportunistically when signed routes or RevenueCat events arrive.

Evidence:

- `apps/ios/Anky/Core/Identity/RecoveryPhrase.swift:31-99`.
- `apps/ios/Anky/Core/Identity/WriterIdentityStore.swift:18-49`.
- `apps/android/app/src/main/java/inc/anky/android/core/identity/RecoveryPhrase.kt:7-69`.
- `apps/android/app/src/main/java/inc/anky/android/core/identity/WriterIdentityStore.kt:12-90`.

#### 3. Onboarding

iOS presents explanation/gate screens and a live writing threshold before marking onboarding complete. It is the clearer realization of “action before account UI,” although the cryptographic key has already been created.

Android contains a full `AnkyOnboardingFlow`, but the live shell calls deprecated `AnkyOnboardingScreen`, which supplies false entitlement and no-op paywall/gate callbacks and auto-advances. Therefore Android onboarding parity is **partially implemented**, not implemented.

Failure handling is local UI state; there is no server dependency for completing onboarding. Screen-time permissions can be declined, which limits the gate but should not prevent writing.

Evidence:

- `apps/ios/Anky/Features/Onboarding/OnboardingView.swift`.
- `apps/ios/Anky/AppRoot.swift:1260-1308`.
- `apps/android/app/src/main/java/inc/anky/android/feature/onboarding/OnboardingScreen.kt:115-205,1415-1447`.
- `apps/android/app/src/main/java/inc/anky/android/app/AnkyApp.kt:608-624`.

#### 4. Writing-session start

The painting/home route opens the native writing surface directly. The writer's first accepted extended grapheme creates a first line containing the epoch millisecond timestamp and glyph. Later glyphs record elapsed wall-clock deltas.

Draft recovery changes the start path:

- iOS explicitly presents Resume/Discard. Resume freezes elapsed time while the app was absent and prepares the writer cursor before the next glyph.
- Android loads an active draft during view-model initialization. It can clear a draft whose first timestamp is not in the current UTC day.

Network access is not required to start or continue writing.

Evidence:

- `apps/ios/Anky/AppRoot.swift:179-204,499-541`.
- `apps/ios/Anky/Core/Protocol/AnkyWriter.swift:3-115`.
- `apps/ios/Anky/Features/Write/WriteViewModel.swift:384-468`.
- `apps/android/app/src/main/java/inc/anky/android/core/protocol/AnkyWriter.kt:8-67`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:607-637,1161-1170`.

#### 5. Keystroke capture and local draft persistence

Each accepted extended grapheme is converted to a protocol line and the draft is saved. Space is encoded as the literal `SPACE`; other glyphs are stored as text after the duration. The UI keeps the caret at the end and rejects newline/paste-like multi-character changes. Default backspace is disabled; iOS has an optional suffix-rewrite mode and autocorrect-tail reconciliation.

Storage:

- iOS: `Documents/ActiveDrafts/dotAnky.anky`; atomic replacement and quarantine behavior.
- Android: `filesDir/ActiveDrafts/dotAnky.anky`; direct text write.

The draft is plaintext within the platform application sandbox. It is local-first but not application-level encrypted.

Evidence:

- `apps/ios/Anky/Features/Write/WriteView.swift:885-1024`.
- `apps/ios/Anky/Features/Write/WriteViewModel.swift:217-299`.
- `apps/ios/Anky/Core/Storage/ActiveDraftStore.swift:13-60`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/HiddenTextInput.kt`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/ActiveDraftStore.kt:7-40`.

#### 6. Completion, interruption, and application lifecycle

Current state transitions are:

```text
idle
  └─ accepted first glyph → active(fragment)
active(fragment)
  ├─ elapsed ≥ 480,000 ms → active(complete)
  ├─ configured silence → sealed(fragment)
  └─ qualifying quick-pass background → sealed(fragment)
active(complete)
  ├─ silence → sealed(complete)
  └─ qualifying quick-pass background → sealed(complete)
```

There is no universal explicit “cancel.” A sub-eight-minute silence still creates an archived fragment. At eight minutes, iOS emits haptic/visual acknowledgment and continues recording until silence. Android similarly uses completeness as classification.

iOS calls draft persistence on non-active scene phases and `closeIfSilenceElapsed`; `AppRoot` can invoke `sealIfLeftInMotion` for gate-related background behavior. Android observes lifecycle for a passive quick-pass seal, but the live shell does not supply the gate session, so that branch is not established end to end.

Evidence:

- `apps/ios/Anky/Features/Write/WriteView.swift:174-197`.
- `apps/ios/Anky/AppRoot.swift:946-1005`.
- `apps/ios/Anky/Features/Write/WriteViewModel.swift:646-738,1070-1110`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteScreen.kt:151-175`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:252-330,684-735`.

#### 7. `.anky` artifact creation, hashing, and indexing

On seal, the protocol validator parses exact UTF-8 lines, SHA-256 hashes the bytes, and the archive names the file `<lowercase-hash>.anky`.

- iOS saves to `Documents/Ankys`, updates a session index under `Application Support/Anky/session-index.json`, invokes level credit, and clears the active draft after a successful archive.
- Android saves to `filesDir/Ankys`, updates its index, and intentionally retains the active draft for a fragment. Its live level callback is omitted from `AnkyApp`.

The SHA-256 name is a checksum/content identifier, not a signature. Direct `load(hash:)` implementations parse a file at that name but do not explicitly assert that recomputing its bytes yields the requested filename.

Evidence:

- `apps/ios/Anky/Core/Storage/LocalAnkyArchive.swift:32-102`.
- `apps/ios/Anky/Core/Storage/SessionIndexStore.swift:187-235`.
- `apps/ios/Anky/Features/Write/WriteViewModel.swift:671-738,1136-1152`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/LocalAnkyArchive.kt:12-29`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:684-735`.

#### 8. Reflection request

iOS shows a sealed-session choice. “Read” calls the reflection path only for an eligible entitlement; otherwise the paywall is shown. Before sending, `RevealViewModel` can append the configured terminal duration, save the resulting artifact under a new hash, remove the old artifact/index entry, and sign the new exact body.

Android's post-seal screen calls `beginSealedSessionReflection` on appearance. Entitled users therefore begin automatically; non-entitled users see the veil/paywall state. Android sends the archived bytes without the iOS terminalization rewrite.

Both sign:

- HTTP method `POST`;
- path `/anky`;
- exact body SHA-256;
- account address;
- request timestamp;
- client identifier;
- EIP-712 domain.

The full raw `.anky` body then crosses the local privacy boundary to the Anky backend and its selected model provider.

Evidence:

- `apps/ios/Anky/AppRoot.swift:1966-2025`.
- `apps/ios/Anky/Features/Reveal/RevealViewModel.swift:431-458`.
- `apps/ios/Anky/Core/Mirror/AnkyPostSigner.swift:21-75`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/PostSessionSealingScreen.kt:90-94`.
- `apps/android/app/src/main/java/inc/anky/android/core/identity/AnkyPostSigner.kt`.

#### 9. Reflection response and local storage

The server returns Markdown, either directly or as server-sent events. Clients parse the stream, display progress, and persist a JSON record keyed by artifact hash with title/body/tags/timestamp. A pending-request store retries temporary failures on roughly three-second intervals for up to about two minutes.

- iOS detects the backend's friendly fallback apology and refuses to persist it as a real reflection.
- Android does not have equivalent fallback detection and can save that text.
- Android's SSE error conversion drops the structured backend error code, making entitlement/retry behavior heuristic in some paths.

Reflection files are plaintext in the platform application sandbox:

- iOS: `Application Support/Anky/reflections/<hash>.json`.
- Android: `filesDir/Anky/reflections/<hash>.json`.

Evidence:

- `apps/ios/Anky/Core/Storage/ReflectionStore.swift:44-114`.
- `apps/ios/Anky/Features/Reveal/RevealViewModel.swift:336-356`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/ReflectionStore.kt:62-79`.
- `apps/android/app/src/main/java/inc/anky/android/core/mirror/MirrorClient.kt:125-127,192-219`.
- `backend/server.ts:1709-1725,2002-2324`.

#### 10. Accumulation, sharing, and return

Session-index entries feed the painting/journey/You screens. Level credit is synchronized through signed backend routes; accumulated writing since a level boundary can be sent to `/level/prepare` for a generated painting at later levels. Daily reminders, widgets, haptics, companion animations, and write-before-scroll gates invite return.

This accumulation is deeply Anky-specific:

- writing duration/word statistics;
- eight named static kingdoms followed by dynamic paintings;
- “inky/lazure” companion state;
- daily unlock ladders and quick passes;
- RevenueCat entitlement distinctions.

Exports are explicit user actions:

- individual or collection `.anky` files;
- readable Markdown;
- ZIP backups with artifacts, reflections, and a manifest;
- share sheets/Android content URIs.

No background raw-writing synchronization was found. Level routes receive artifact hash, duration/seconds, and idempotency data; the dynamic painting route is a distinct opt-in/feature path that receives reconstructed writing.

Evidence:

- `apps/ios/Anky/Core/Level/LevelPaintingCoordinator.swift:176-200`.
- `apps/android/app/src/main/java/inc/anky/android/feature/painting/PaintingHomeView.kt:191-210`.
- `backend/painting/routes.ts:191-240`.
- `apps/ios/Anky/Core/Storage/BackupImporter.swift:227-305` — `BackupExporter`.
- `apps/android/app/src/main/java/inc/anky/android/core/storage/Exporter.kt`.

#### 11. Recovery, restoration, and deletion

iOS implements:

- biometric-gated phrase reveal/import;
- optional synchronizable Keychain phrase backup;
- an AES-GCM/HKDF encrypted ZIP in iCloud Documents;
- an automatic restore prompt when local archive/reflections are empty and a backup is available;
- server-first signed account deletion, followed by local clearing only after server success.

Android implements:

- biometric-gated phrase reveal/import;
- manual ZIP/Markdown export/import;
- an AES-GCM/HKDF encrypted backup stored inside its own `filesDir`;
- no automatic off-device restore path found;
- a “delete everywhere” method that clears local state and RevenueCat login but does not invoke `DELETE /account`.

Backend account deletion removes account-indexed SQLite rows, but not generated painting files and not global RevenueCat event records. This means the iOS user-facing “everywhere” guarantee is also incomplete at the storage-asset boundary.

Evidence:

- `apps/ios/Anky/Features/You/YouViewModel.swift:135-354`.
- `apps/ios/Anky/Core/Storage/ICloudBackupStore.swift:36-207`.
- `apps/ios/Anky/AppRoot.swift:1210-1249`.
- `apps/android/app/src/main/java/inc/anky/android/feature/you/YouViewModel.kt:192-279,375-557,688-738`.
- `backend/account/routes.ts:41-78`.
- `backend/level/db.ts:517-560`.

### Failure and privacy boundary summary

| Boundary | Offline behavior | Durable failure state | Privacy implication |
|---|---|---|---|
| Writing/draft | Fully available | Draft file | Plaintext app-private writing |
| Seal/archive | Fully local | Fragment/complete archive or retained draft | Hash is local checksum |
| Reflection | Requires server/provider | Pending request + UI error | Full writing disclosed to backend/provider |
| Level ledger | Requires server eventually | Idempotency/local level state | Address + hash + timing stored server-side |
| Dynamic painting | Requires server/providers | Generation metadata/files | Accumulated raw writing transiently disclosed; generated assets durable |
| Recovery | Platform-specific | Encrypted backup/phrase | iOS optional cloud; Android primarily installation-local |
| Export | User-driven | Share destination | Privacy transfers to user-selected destination |

## Interpretation

The real domain lifecycle is not simply “start → complete.” It is:

```text
action started
→ action progressed (draft checkpointed)
→ completeness threshold may be reached
→ session sealed with outcome fragment or complete
→ artifact persisted
→ reflection optionally/automatically requested
→ reflection persisted
→ accumulation credited
→ continuation invitation shown
```

“Completion,” “sealing,” “reflection,” and “continuity credit” must be separate events. Current code sometimes collapses them:

- the artifact hash can change when reflection is requested on iOS;
- Android retains the draft after fragment archival;
- a level callback is part of the write view model rather than a post-persistence subscriber;
- background/gate behavior can seal an action.

## TOHSENO implication

A generic continuity engine needs an explicit outcome and transaction boundary:

```text
persist artifact bytes
→ atomically append immutable ContinuityEvent
→ publish post-persist effects (reflection, progress, sync, invitation)
```

Network reflection, entitlement, and return gates must not decide whether the local event exists. Platform lifecycle adapters should emit domain commands rather than owning completion semantics.

## Recommendation

Characterize both live shells before extraction:

1. Freeze first-launch ordering, identity creation, first-value reachability, and draft recovery in composition tests.
2. Decide whether a sub-threshold silence is an `interrupted` or `completed(fragment)` outcome; encode it explicitly.
3. Make event persistence the one durable boundary and give reflection/level/painting independent idempotent consumers.
4. Preserve old `.anky` bytes and hashes through dual-read compatibility; do not rewrite existing artifacts merely to attach new metadata.
5. Align Android's actual onboarding, preferences, gate, level, deletion, and post-seal policy only as a separately reviewed Anky parity change—not as a side effect of framework extraction.
