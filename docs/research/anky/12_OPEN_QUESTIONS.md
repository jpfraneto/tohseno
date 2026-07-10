# Open questions and validation record

## Current implementation uncertainties

These are not claims that functionality is absent; they are boundaries the repository cannot establish by itself.

1. **Which mobile source is currently shipped?** The working tree contains an in-progress relocation from `apps/write-8-minutes/{ios,android}` to `apps/{ios,android}`. This study uses the latter because it is the live tree, but store build/version provenance requires release records.
2. **What survives an actual uninstall on each supported OS version?** Source defines Keychain/Keystore accessibility and backup flags, but OS uninstall/keychain behavior must be tested on real devices.
3. **Are iOS and Android encrypted backup envelopes actually interoperable?** Algorithms/context look intentionally aligned; no golden cross-platform encrypted ZIP round trip was found.
4. **How many backend replicas are deployed?** Replay memory is process-local. Railway/runtime topology and rolling-restart behavior are operational state, not source code.
5. **What do external providers contractually retain today?** Code requests/declares ZDR. Current vendor account settings and data-processing terms require operational verification.
6. **Which server files/backups exist outside `/data`?** Code identifies SQLite and painting directories. Volume snapshots, platform logs, observability exports, and provider control planes require an operational inventory.
7. **Does Android's Glance implementation have a currently registered widget receiver elsewhere?** Glance-related code exists, but no receiver was found in the inspected manifest; this should be confirmed against generated/variant manifests and the shipped artifact.
8. **Are Apple FamilyControls distribution entitlements approved for the current bundle IDs?** The source documents the requirement; the repository cannot prove App Store entitlement state.

## Human product decisions

### P0: required before extracting domain/storage code

| Decision | Current evidence/conflict | Recommended default |
|---|---|---|
| What is the stable continuity event ID? | `.anky` hash changes when iOS terminalizes before reflection | New random/ordered event ID in a sidecar envelope; artifact digest remains separate |
| Is terminal stillness part of canonical artifact bytes? | TypeScript accepts 1–8 s, Swift any positive, Kotlin only 8 s; Android reflects raw archive | Freeze `.anky v0`; choose one versioned v1 rule without rewriting old bytes |
| What is a sub-eight-minute silence? | Persisted as a fragment, but “interruption” is implicit | Explicit `interrupted` outcome with preserved artifact |
| Does reaching eight minutes end immediately? | Current code only acknowledges completion; silence seals later | Preserve current Anky behavior unless product intentionally changes the ritual |
| Are fragments reflectable? | Backend tiers support them; immediate mobile paths differ from older Reveal rules | Yes only through an explicit Anky policy, separately from completion |
| Is reflection opt-in or automatic? | iOS requires “Read”; Android auto-starts for entitled users | Opt-in at the private disclosure boundary |
| What does “trustworthy event/proof” mean? | Current key signs self-supplied bytes/claims; no independent witness | Call it practice-key self-attestation until a stronger witness model is chosen |
| Must live local content be application-encrypted? | Current archives/reflections are plaintext in the platform sandbox | Define threat model; target app-encrypted repositories for TOHSENO without changing legacy artifact bytes |

Evidence:

- `apps/ios/Anky/Features/Reveal/RevealViewModel.swift:431-458`.
- `protocol/implementations/typescript/src/parse.ts`.
- `apps/ios/Anky/Core/Protocol/AnkyDuration.swift:3-36`.
- `apps/android/app/src/main/java/inc/anky/android/core/protocol/AnkyParser.kt:3-35`.
- `apps/ios/Anky/AppRoot.swift:1966-2025`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/PostSessionSealingScreen.kt:90-94`.

### P1: required before identity/auth/recovery publication

| Decision | Current evidence/conflict | Recommended default |
|---|---|---|
| When should a TOHSENO practice identity be created? | Anky normally creates it on first launch/before value | Allow first committed event as generic default; preserve first launch in Anky compatibility |
| Is Base EOA the framework default? | It is deeply compatible with Anky/RevenueCat/backend but not necessary for local continuity | No; make `anky.base.eoa.v1` one frozen suite adapter |
| How are cross-app identities related? | No bridge model; address reuse would silently link | Distinct per-app scopes; explicit signed relation with selective disclosure |
| What happens when importing a phrase over existing content? | Both platforms retain archives; server/subscription ownership changes; iOS stages old phrase, Android does not | Require an identity/content migration plan and rollback before applying |
| What is Android's off-device recovery destination? | Manual phrase/ZIP; encrypted backup is in `filesDir`; backup disabled | Product must choose explicit user file, encrypted Drive-like provider, or no automatic recovery |
| How should operation authorization be versioned? | Typed method/path are hard-coded to `POST /anky` even for GET/DELETE | Server-first dual-version endpoint-bound statements with durable nonce/idempotency |
| What does “delete everywhere” include? | Android omits server DELETE; server leaves painting files; provider relies on ZDR | Enumerated deletion report covering device, cloud backup, DB, generated files, vendor identifiers, and limitations |

Evidence:

- `protocol/identity/SPEC.md:3-24`.
- native `WriterIdentityStore` implementations.
- `apps/ios/Anky/Core/Mirror/AnkyPostSigner.swift:63-71`.
- `apps/ios/Anky/Core/Level/LevelSyncClient.swift:197-233`.
- `backend/server.ts:924-1028`.
- `apps/android/app/src/main/java/inc/anky/android/feature/you/YouViewModel.kt:688-738`.
- `backend/painting/config.ts:84-110`.

### P2: required before a public scaffold/skill

| Decision | Why it matters | Recommended default |
|---|---|---|
| Which second app proves genericity? | Package names should survive a non-text, non-duration ritual | Gratitude Lock is grounded and materially different if media/encryption scope is acceptable |
| Is accumulation always numeric? | Anky uses seconds/levels/paintings; that would bias the framework | No; allow app-defined local projections such as private cards/constellations |
| Are screen-time gates a framework feature? | They require fragile, platform-specific permissions and express Anky's bargain | Anky reference/optional adapter only |
| Are payments part of core? | RevenueCat currently gates reflection/progression, not writing | Optional entitlement port; never gate action, local event, export, or recovery |
| Does every app require server reflection? | Gratitude can reflect locally; private provider disclosure has real cost | No; support none/local/monorepo server |
| Which public package APIs are stable at v1? | Publishing identity/storage too early creates migration obligations | Public manifest/validator first; keep domain/artifact/identity/storage internal through second app |
| When is a one-line installer acceptable? | Remote installers can overwrite projects and magnify skill errors | Only after signed/versioned releases, dry-run, checksums, and non-destructive conflict handling |

## Technical questions for Phase 1

1. Can one Unicode fixture corpus reproduce IME composition and suffix replacement across Swift/Kotlin/TypeScript without conflating UI and codec behavior?
2. Should `.anky v0` parsing preserve each implementation's accepted legacy inputs or adopt a strict reader plus explicitly labeled repair importer?
3. What crash-consistent primitive is available on each platform for artifact + event + checkpoint clearing?
4. How will legacy reflection hashes be related to new stable event IDs when the pre-reflection artifact was replaced?
5. Can server idempotency safely cache a reflection result under the privacy covenant, and for how long/encrypted by what key?
6. How should backup import distinguish byte-preservation from user-text normalization?
7. What is the least revealing server receipt useful for progress/coordination?
8. Which metadata must be locally stored to make provider consent/provenance auditable without creating a new tracking layer?
9. How will old and endpoint-bound signatures coexist during staggered mobile releases?
10. Which generated painting assets and external provider identifiers can be deleted or made unlinkable in practice?

## Validation performed on 2026-07-10

No deployment, store submission, connected-device mutation, production endpoint call, or blockchain transaction was performed.

### Passed

| Command | Result |
|---|---|
| `cd protocol/implementations/typescript && bun test` | 26 passed, 0 failed |
| `cd protocol/implementations/typescript && bun run typecheck` | Passed |
| `cd backend && bun test` | 139 passed, 0 failed |
| `cd backend && bun run typecheck` | Passed |
| `cd apps/android && ./gradlew :app:assembleDebug` | Build succeeded |
| `xcodebuild -project apps/ios/Anky.xcodeproj -scheme Anky -destination 'generic/platform=iOS Simulator' -derivedDataPath /tmp/anky-ios-derived build CODE_SIGNING_ALLOWED=NO` | Build succeeded for app, Screen Time extensions, and widget target |
| `cd smart_contracts && forge test` | 16 passed, 0 failed |
| `cd apps/landing && bun run lint` | Passed |
| `cd apps/landing && bun run build` | Passed |
| `cd livestream && bun run typecheck` | Passed |
| `cd livestream && bun run build` | Passed |
| `cd apps/prompt-tester && bun install --frozen-lockfile && bun run typecheck` | Locked dependencies installed locally; typecheck passed; lockfile unchanged |

Bun commands used Bun 1.2.18 as installed in the workspace environment.

### Failed or incomplete

#### Android JVM tests

Command:

```sh
cd apps/android && ./gradlew :app:test
```

Result: 591 tests ran; 8 `SourceInvariantTest` cases failed:

- `iosImageAssetsHaveAndroidDrawableResources`
- `activeDraftStoreUsesIosStylePrimaryDirectoryAndOpenLegacyFallback`
- `shortSessionTryAgainRoutesThroughRootRetryWritingLikeIos`
- `mapRefreshFallsBackToExistingIndexLikeCurrentIos`
- `youDeleteAccountAndDataMatchesCurrentIosDestructiveFlow`
- `firstLaunchOnboardingMatchesCurrentIosPages`
- `companionTapOnlyTogglesPresenceLikeIos`
- `youHomeRowsMatchCurrentIosPromptAndLegalShape`

The compiler also warned that the live `AnkyApp.kt:611` call uses deprecated `AnkyOnboardingScreen`. This corroborates the runtime-wiring finding. No production code was changed to address these failures.

#### Swift package tests

Command:

```sh
swift test --package-path apps/ios --scratch-path /tmp/anky-ios-swiftpm
```

Result: dependency resolution and library compilation progressed, then the test target failed to compile because `apps/ios/Anky/Tests/DraftRecoveryTests.swift:2` uses `@testable import Anky`, while the Swift package exposes `AnkyProtocol`/`AnkyCore`, not the Xcode application module `Anky`. The generic Xcode application build passed separately. Therefore the documented Swift-package test command is currently not a valid full test entry point.

#### Not run

- Android connected instrumentation tests: require a configured emulator/device and can alter installed app/device state; JVM tests and debug assembly were used for this documentation phase.
- iOS device/simulator UI tests: the documented generic build was run; no repository-documented noninteractive UI-test command was found, and the documented Swift-package test command failed as above.
- Browser extension and Gratitude Lock automated tests: no test command or automated suite was found.
- Live reflection/provider/prompt evaluation: intentionally not run because it would send data/use external paid services; backend tests use injected/recorded providers.
- Release bundles, store signing, Railway deployment, contract deployment, and store submission: explicitly out of scope.

## Documentation validation

The documentation validation pass checks:

- all 13 required Markdown names and the JSON example exist and are nonempty;
- the JSON example parses with Bun;
- every concrete backticked current-repository path exists;
- every cited numeric line/range is within the referenced file's line count;
- no placeholder/TODO document is present;
- only `docs/tohseno` contains changes attributable to this study;
- production dependencies and source files were not edited;
- proposed paths are labeled as proposed rather than current.

The repository had extensive pre-existing tracked/untracked changes outside `docs/tohseno`; those were preserved and are not part of this work.

## Interpretation

The open questions are not reasons to design a broader framework. They are reasons to keep the initial public promise small. Most can be resolved through Phase 1 fixtures plus a handful of explicit product decisions.

The validation failures reinforce the roadmap:

- tests currently protect source-shape parity that the live Android tree no longer satisfies;
- the Swift package/test boundary includes an Xcode-app test it cannot import;
- builds can pass while lifecycle/composition invariants fail.

## TOHSENO implication

TOHSENO should publish no identity/proof/storage guarantee until the corresponding question above has a tested answer. The manifest can be public earlier because it makes those unresolved guarantees explicit rather than hiding them.

## Recommendation

The next human review should answer the P0 table first. The next engineering change should be Phase 1 characterization fixtures and composition-root tests—not a package move or refactor.

