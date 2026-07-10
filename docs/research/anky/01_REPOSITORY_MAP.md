# Repository map

## Current implementation

### Repository-level shape

There is no root JavaScript workspace manifest. Each JavaScript/TypeScript application owns its own `package.json` and Bun lockfile; native projects use Swift Package Manager/Xcode and Gradle; contracts use Foundry.

```text
anky/
├── apps/
│   ├── ios/                 SwiftUI application, extensions, widget, Swift packages/tests
│   ├── android/             Jetpack Compose application, service, Glance code, tests
│   ├── browser/             Chrome extension: forward-only writing before X
│   ├── gratitude-lock/      Small iOS gratitude ritual prototype
│   ├── landing/             React/Vite marketing and distribution site
│   └── prompt-tester/       Local Bun/OpenRouter prompt evaluation UI
├── backend/                 Bun + Hono API, reflection, ledger, subscription, paintings
├── protocol/                .anky and identity specifications, fixtures, TS implementation
├── smart_contracts/         Foundry ERC-721 public mirror/claim system
├── livestream/              Bun/React OBS overlay and control service
├── scripts/                 Protocol inspection, credits, sprite preprocessing
├── docs/                    Product, privacy, identity, API, and distribution documents
└── railway.toml             Backend Railway deployment definition
```

Evidence:

- `apps/ios/Package.swift:5-39` — two Swift library products and dependencies.
- `apps/android/settings.gradle.kts` and `apps/android/app/build.gradle.kts:37-159` — Android application build.
- `backend/package.json`, `apps/landing/package.json`, `apps/prompt-tester/package.json`, and `livestream/package.json` — independent Bun packages.
- `smart_contracts/foundry.toml` — Foundry project.
- `railway.toml` and `backend/Dockerfile` — hosted backend build/start.

### Major component inventory

| Component | Responsibility and consumers | Platform | Classification tendency |
|---|---|---|---|
| `apps/ios` | Production-style native Anky client; writing, identity, reflection, journey, purchases, exports, Screen Time gate | iOS 16+, extensions, widget | Mostly Anky reference/product; native storage and identity adapters are reusable candidates |
| `apps/android` | Native port; Compose writing, identity, reflection, journey, purchases, UsageStats gate | Android API 26–35 | Same; live wiring is not equivalent to iOS |
| `apps/browser` | Chrome MV3 content script that blocks the X timeline until a short forward-only writing ritual | Chromium extension | Anky reference experiment; no shared identity/backend |
| `apps/gratitude-lock` | In-memory gratitude → reflection → rest prototype with text/voice/photo inputs | iOS | Useful second ritual evidence, not a complete continuity implementation |
| `apps/landing` | Marketing site, gallery/memes, policy content, app/agent-skill distribution | Web | Anky-only distribution |
| `apps/prompt-tester` | Local prompt/model comparison tool using OpenRouter | Bun/web | Internal developer tool |
| `backend` | Signed reflection API, event ingestion, continuity ledger, entitlement webhooks, paintings, account deletion | Bun/Hono on Railway | Reflection/auth concepts configurable; current service is Anky-specific |
| `protocol` | Text artifact and Base EOA identity laws, cross-language fixtures, TypeScript codec/signing implementation | Cross-platform reference | Best current source for reusable contracts, but still Anky-named |
| `smart_contracts` | Base ERC-721 claims/mints, payments/gifts, public artifact/reflection hashes | EVM/Base | Optional external/public Anky product; not in the core mobile loop |
| `livestream` | WebSocket/API state service and React OBS overlay | Bun/web | Unrelated production/media tooling |
| `scripts` | Credits, artifact inspection, sprite preprocessing | Bun | Anky operations/development |
| `docs` | Product and technical laws, identity/privacy/API notes | Documentation | Intent evidence; may lag code |

### iOS application

The iOS project has a large SwiftUI composition root plus local Swift packages:

- `Anky/AnkyApp.swift:5-13` enters `AppRoot`.
- `Anky/AppRoot.swift` composes onboarding, writing, painting home, journey/You, RevenueCat, identity, iCloud recovery, gate state, and lifecycle handling.
- `Anky/Core/Protocol` contains the native `.anky` codec, hasher, validator, and session engine.
- `Anky/Core/Identity` contains BIP-39 recovery, HD key derivation, Keychain access, and typed-data signing.
- `Anky/Core/Storage` contains draft, archive, reflection, index, export/import, and encrypted iCloud backup stores.
- `Anky/Core/Mirror` contains signed reflection networking and pending request storage.
- `Anky/Core/WriteBeforeScroll` plus three extension targets implement Apple's FamilyControls/ManagedSettings/DeviceActivity gate.
- `Anky/Features/Write`, `Features/Reveal`, `Features/You`, `Features/PaintingHome`, and onboarding screens implement product flows.
- `apps/ios/AnkyGlanceWidgets` contains widget/live-activity code.
- `apps/ios/Anky/Tests` and Swift package tests cover protocol, identity, storage, mirror, write, gate, level, subscription, and privacy behavior.

The project declares an app target, three Screen Time extension targets, widget/live-activity support, and tests in `apps/ios/Anky.xcodeproj/project.pbxproj:862-984`. `Package.swift:8-37` exposes `AnkyProtocol` and `AnkyCore`; the app itself is still largely compiled from the Xcode target.

External iOS dependencies include Apple's CryptoKit/Keychain/CloudKit-family APIs, FamilyControls/ManagedSettings/DeviceActivity, StoreKit/RevenueCat, and `web3swift` 3.3.2.

### Android application

The Android app mirrors the iOS directory vocabulary but is a separate implementation:

- `AnkyApplication.kt:6-13` creates `AppContainer`.
- `MainActivity.kt:17-54` owns deep links and the Compose entry.
- `app/AppContainer.kt:44-180` constructs long-lived stores and services.
- `app/AnkyApp.kt:192-624` is the live navigation/composition root.
- `core/protocol`, `core/identity`, `core/storage`, and `core/mirror` duplicate native equivalents.
- `feature/write`, `feature/reveal`, `feature/you`, onboarding, painting, level, subscription, and paywall features implement Anky.
- `gate/runtime` uses UsageStats, a foreground watcher service, an overlay/shield activity, and alarms.
- `app/src/test` holds JVM tests; `app/src/androidTest` holds device UI tests.

All Android source paths above are relative to `apps/android/app/src/main/java/inc/anky/android`.

Dependencies in `apps/android/app/build.gradle.kts:37-159` include Compose, AndroidX DataStore/Glance/biometric/navigation/review, RevenueCat 9.23.1, OkHttp 4.12, BouncyCastle 1.78.1, and web3j 4.12.1. Release secrets come from environment variables or `local.properties`; the default backend is the Railway service.

The many `WIRING-*.md` files describe intended porting work, but the executable call sites in `AnkyApp.kt` remain the source of truth. In particular, copied classes with default parameters are not proof that a subsystem is live.

### Browser extension and gratitude prototype

`apps/browser/content.js:1-203` injects a forward-only editor over X, persists a local draft in Chrome storage, blocks paste/delete/newline, declares success after approximately nine seconds of continuity, and treats about 1.5 seconds of inactivity as interruption. `manifest.json` confirms a Chrome Manifest V3 content-script application. It does not create a practice key, use `.anky`, request a reflection, or synchronize with the native apps.

`apps/gratitude-lock/v0/ios/GratitudeLockByAnky/ContentView.swift:4-54` models an in-memory `ritual → reflection → rest` state, and `apps/gratitude-lock/v0/ios/GratitudeLockByAnky/RitualView.swift:4-144` accepts text, voice, or photo gratitude. No durable identity, event log, backend, or recovery path was found. It is nevertheless evidence that TOHSENO must support an action unlike timed text entry.

### Protocol

The protocol directory has two distinct contracts:

1. `protocol/SPEC.md` defines the line-oriented `.anky` format, completion marker, reconstruction, and SHA-256 content identity.
2. `protocol/identity/SPEC.md` defines `anky.base.eoa.v1`: 12-word BIP-39 English mnemonic, BIP-44 Ethereum path, secp256k1 EOA, EIP-55 address, and EIP-712 request authorization.

The TypeScript implementation in `protocol/implementations/typescript/src` uses `@scure/bip32`, `@scure/bip39`, and `viem`. Fixtures in `protocol/fixtures` and expected JSON in `protocol/expected` are consumed by protocol tests and analogous native fixtures.

Potentially reusable: the notion of exact artifact bytes, a codec, fixtures, digesting, request authorization, and a cross-language compatibility suite.

Anky-specific: the text grammar, eight-minute completeness marker, Ethereum/Base suite, `AnkyPost` EIP-712 domain, and route-specific fields.

### Backend services and storage

`backend/server.ts` is the main composition root. It registers:

- `GET /health`;
- signed `POST /anky` reflection, streaming or plain;
- signed level/continuity ledger routes in `backend/level/routes.ts`;
- painting routes in `backend/painting/routes.ts`;
- RevenueCat subscription/webhook routes in `backend/subscription`;
- funnel/event ingestion in `backend/events/routes.ts`;
- signed account deletion in `backend/account/routes.ts`;
- debug routes controlled by environment.

The backend uses SQLite at `/data/anky.sqlite` by default and initializes tables in `backend/level/db.ts:64-184`. Durable records include:

- session ledger and level state;
- painting metadata and generation logs;
- subscription state and RevenueCat events;
- funnel events;
- idempotency and quota state.

Generated painting files are stored below `/data/paintings/<safe-account>/<level>` by `backend/painting/config.ts:84-110` and the pipeline. Raw writing is sent transiently to reflection or painting providers; the code does not insert raw writing or reflection bodies into SQLite.

External services:

- OpenRouter is the primary reflection/model gateway.
- Bankr and Poiesis are optional providers gated by environment and zero-data-retention declarations.
- RevenueCat supplies entitlement state/webhooks.
- Railway supplies the container and persistent volume.
- Painting/model configuration names Google/Anthropic-family models through those gateways.

### Smart contracts

`smart_contracts/src/ANKY_MIRRORS.sol:49-430` defines an ERC-721 collection with a fixed maximum, Farcaster FID/wallet claim constraints, signer-authorized minting, public reflection/artifact hashes, payments, and gifts. It is deployed/tested with Foundry against Base-oriented configuration.

No mobile or backend call site was found that makes this contract part of writing, local persistence, or reflection. It is an optional public ownership/proof experiment. TOHSENO must not infer that every continuity event belongs on-chain.

### Infrastructure, generated files, and secrets

- `railway.toml` builds `backend/Dockerfile`, starts the compiled Bun server, and checks `/health`.
- `apps/landing/orbiter.json` and Vite configuration deploy the marketing site separately.
- Xcode `.build` directories, archives, Android `build` directories, web `dist`, Foundry `cache/broadcast`, prompt-tester runs, and evaluation cases are generated/local artifacts rather than architectural source.
- Root and component `.gitignore` files ignore `.env`, `*.p8`, prompt-tester secrets/runs, historical writings, and smart-contract secrets. Sensitive-looking ignored files are present locally; this study did not open them.
- Environment examples and release build files reveal a broad secrets surface: model-provider keys, RevenueCat credentials/webhook secrets, chain RPC/deployer keys, Apple signing material, Android release signing values, and backend signing/test toggles. These are external operational dependencies, not framework defaults.

Evidence:

- `.gitignore:1-50`.
- `backend/.env.example`.
- `apps/android/app/build.gradle.kts:37-77`.
- `smart_contracts/.env.example`.

### Tests and validation surfaces

| Area | Existing test surface |
|---|---|
| Protocol TypeScript | Bun tests over codec, duration, fixtures, and identity |
| Backend | Bun tests for auth, endpoint, provider routing, prompts, privacy, ledger, subscription, events, painting, account deletion |
| iOS | Swift package tests plus Xcode app/unit tests |
| Android | JVM unit tests and instrumentation/UI tests |
| Contracts | Foundry tests |
| Landing/livestream/prompt tool | Typecheck/lint/build scripts, with limited or no domain tests |

`apps/MOBILE_PARITY.md:139-152` reports a previous green count, but it is documentation, not a substitute for current execution. Final commands and results are recorded in `12_OPEN_QUESTIONS.md`.

## Dependency directions

Current dependency directions are mostly implicit:

```text
native UI/composition roots
  ├─ product state (journey, gates, purchase, onboarding)
  ├─ duplicated native protocol/identity/storage
  └─ HTTP clients ───────────────┐
                                 v
TypeScript protocol ────────> Bun backend
                                 ├─ SQLite/files
                                 ├─ RevenueCat
                                 └─ model/image providers

Foundry contracts, browser, landing, prompt tester, gratitude-lock, livestream
remain adjacent rather than runtime dependencies of the native loop.
```

The backend depends on the TypeScript protocol via a local file dependency. Native apps do not import that package; they reimplement the contract. No root build graph enforces synchronized changes across all three codecs.

## Interpretation

The monorepo is organized around shipping Anky, not around publishing portable primitives. That is appropriate for a seed implementation. The useful seams are visible, but dependency inversion is incomplete:

- protocol and identity behavior are reimplemented rather than shared through generated fixtures;
- giant composition roots know both infrastructure and product ritual;
- storage models link directly to `.anky` and reflection hashes;
- reflection authorization, quota, provider routing, and Anky prompt logic share one backend file;
- platform gates and payments are entangled with return/progression behavior.

Several README and parity documents describe earlier behavior. Examples include older storage paths and a now-obsolete reveal-oriented navigation. Recommendations must follow executable call sites, with documents used only as intent evidence.

## TOHSENO implication

TOHSENO should not reproduce this top-level shape verbatim. It should add an explicit dependency direction:

```text
app ritual → configurable continuity domain → opaque storage/identity/reflection ports
                                             ↓
                           Apple / Android / server / provider adapters
```

The first public contracts should be pure and small. Native UI, screen-time enforcement, RevenueCat, painting, and blockchain integrations should remain adapters or Anky reference code.

## Recommendation

Before extracting directories:

1. Make the repository map executable through cross-language fixtures and shell-level composition tests.
2. Mark the live entry points above as characterization boundaries.
3. Keep Anky's mobile projects in place while internal interfaces are introduced.
4. Add a root workspace only when TOHSENO actually has multiple publishable TypeScript packages; do not add speculative monorepo tooling merely to resemble a framework.
5. Treat `apps/gratitude-lock` as a candidate second ritual after it gains real persistence, because it is different enough to expose writing-only assumptions.
