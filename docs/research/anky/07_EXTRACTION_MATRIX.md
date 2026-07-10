# Extraction matrix and risk register

## Classification rules

- **A — Generic continuity primitive:** reusable across many rituals with minimal semantic change.
- **B — Configurable framework primitive:** reusable only when driven by an app policy/manifest or adapter.
- **C — Anky reference implementation:** a strong example of a writing continuity app.
- **D — Anky-only product logic:** should remain branded/product-specific.
- **E — External infrastructure dependency:** operating system, vendor, chain, hosting, or model service.

A source file can contain several concerns. The matrix classifies the smallest meaningful subsystem rather than declaring an entire folder reusable.

## Current implementation: subsystem matrix

| Group | Subsystem | Evidence | Reasoning and dependencies |
|---|---|---|---|
| A | Exact-byte SHA-256 digest helper | `protocol/implementations/typescript/src`, `AnkyHasher.swift`, `AnkyHasher.kt` | Generic content addressing; depends only on crypto primitives. Existing naming is Anky-specific but behavior is reusable. |
| A | Practice-lifecycle vocabulary discovered in the write flow | `WriteViewModel.swift:217-738`, `WriteViewModel.kt:252-735` | Start/progress/checkpoint/seal/outcome are generic concepts, but no standalone implementation exists yet; this is an extraction target, not a ready package. |
| A | Reflection privacy-capability metadata | `backend/server.ts:1272-1345` | ZDR/logging/training capability checks are generally useful; provider-specific truth remains operational. |
| A | Rebuildable local-index pattern | `SessionIndexStore.swift:187-235`, Android `SessionIndexStore.kt` | Derived indexes that can be rebuilt from canonical events are reusable; current schema/previews are Anky-specific. |
| B | Core-action reducer/policy | `WritingSessionEngine.swift:23-171`, Kotlin equivalent | Timing, completion, interruption, and progress types must be configured. Current engines are coupled to writing/unlock behavior and need isolation. |
| B | Versioned artifact codec registry | `protocol/SPEC.md`, three codec implementations | Codec/digest interface is generic; `.anky` is one configured codec. |
| B | Local continuity repository | `ActiveDraftStore`, `LocalAnkyArchive`, `ReflectionStore`, `SessionIndexStore` on both platforms | Store interfaces, atomicity, migrations, and encryption are reusable; paths/file schemas must be adapters. |
| B | Practice-identity interface and secure-secret-store port | `protocol/identity/SPEC.md`, native identity stores | Identity creation/signing/recovery is reusable when suite/storage are configurable; Base EOA must not be mandatory. |
| B | Signed operation authorization | `AnkyPostSigner`, `verifyAnkyBaseRequest` | Exact-body authorization is reusable after purpose/endpoint binding and versioning. Current fixed `POST /anky` statement is unsafe as a generic contract. |
| B | Encrypted backup envelope and recovery coordinator | `ICloudBackupStore.swift:36-207`, `AndroidEncryptedBackupStore.kt:34-245` | HKDF/AES-GCM envelope is reusable; cloud/local destinations, key source, restore UX, and migration are configurable adapters. |
| B | Export/import pipeline | iOS/Android `Exporter`, `BackupImporter`, `SingleAnkyImporter` | Explicit user-controlled portability is generic; codecs, media, validation, and redaction policies vary. |
| B | Reflection policy/provider/repository contract | `backend/server.ts:1278-1345`, client reflection stores | Provider routing and typed derived results generalize; Anky prompt/tier/entitlement behavior does not. |
| B | Return invitation/reminder | iOS/Android notification schedulers, widget snapshots | A continuity app needs a configurable invitation; notification and widget platforms are adapters. |
| B | Optional accumulation/progression subscriber | level stores and signed ledger routes | An append-only aggregate can generalize, but seconds/kingdoms/painting are Anky policy. Must remain downstream of local persistence. |
| B | Optional entitlement boundary | native subscription gateways and backend entitlement store | Framework can expose a capability port, but payments must never gate the core local action by default. |
| C | `.anky` character-timing codec | `protocol/SPEC.md`, Swift/Kotlin/TS protocol code | Canonical example of a private writing artifact; too text/timing-specific for the universal event format. |
| C | Forward-only writing input adapter | `WriteView.swift:885-1024`, `HiddenTextInput.kt` | Demonstrates a high-friction native ritual. Native IME/composition behavior should not be generalized into all actions. |
| C | Eight-minute/silence writing policy | `AnkyDuration.swift:3-36`, Android write VM | Anky's configured policy and reference test case. |
| C | Writing draft recovery UX | `AppRoot.swift:179-204`, iOS/Android write view models | Useful reference for crash/offline continuity, but exact Resume/Discard and fragment continuation are product decisions. |
| C | Anky reflection client/server vertical slice | `MirrorClient`, `handleAnkyReflection` | Shows signed private request, provider routing, retry, and local result storage; should be decomposed, not published wholesale. |
| C | Browser write-before-X experiment | `apps/browser/content.js:1-203` | A second delivery surface for the Anky writing ritual; it lacks framework identity/artifacts/reflection. |
| C | Gratitude Lock ritual prototype | `apps/gratitude-lock/v0/ios/GratitudeLockByAnky/ContentView.swift:4-54` | A candidate second continuity app proving non-writing assumptions, once persistence/identity are implemented. |
| D | Anky companion, lore, visual voice, haptics | native feature/UI/assets directories | Product identity and ritual texture; generalizing it would erase Anky. |
| D | Kingdom/level/painting journey | native level/painting code and `backend/painting` | Accumulated Anky narrative and generated visual artifacts; may inspire a configurable subscriber but remains product logic. |
| D | Write-before-scroll quick passes and unlock ladder | `UnlockPolicy.swift:91-239`, Android gate packages | Specific behavioral bargain with screen-time systems. Not every continuity app blocks another app. |
| D | Anky onboarding copy/screens | `OnboardingView.swift`, Android onboarding | Explains this ritual and its permissions; framework should scaffold app-specific onboarding after the action contract. |
| D | Anky You/profile statistics and settings | native `Features/You` / `feature/you` | Writing stats, phrase UX, exports, purchase state, and product settings mixed in one screen. Reuse underlying capabilities, not screen. |
| D | RevenueCat `pro` product policy | `backend/server.ts:253-257`, native subscription code | Product SKU/entitlement and what it unlocks belong to Anky. |
| D | Tiered Anky prompts and nudge voice | `backend/reflection.ts:107-204` | Writing-duration tiers and Anky persona are reference policy, not provider framework. |
| D | Landing/gallery/memes/agent-skill distribution | `apps/landing` | Anky marketing/distribution content. |
| D | Public Anky Mirrors NFT/token/payment logic | `smart_contracts/src/ANKY_MIRRORS.sol:49-430` | Optional product/public-chain system outside core continuity. |
| D | Livestream/OBS control | `livestream` | Media operations unrelated to continuity architecture. |
| D | Prompt tester and sprite tools | `apps/prompt-tester`, `scripts/anky-sprites` | Internal Anky creative/development tools; patterns may be reused but not framework runtime. |
| E | Apple Keychain/biometric/iCloud | native identity and backup stores | External secure/recovery platform capabilities. |
| E | Android Keystore/biometric/app-private files | Android identity/storage | External platform capabilities with different uninstall semantics. |
| E | Apple FamilyControls/ManagedSettings/DeviceActivity | iOS gate plus extensions | Entitlement- and OS-governed enforcement. |
| E | Android UsageStats/foreground services/alarms | Android gate runtime | Permission/store/OEM-governed enforcement. |
| E | RevenueCat, Apple/Google billing | native and backend subscription code | External entitlement/payment systems. |
| E | OpenRouter, Bankr, Poiesis, underlying models | backend provider implementations | External private-data processors; policy/availability can change. |
| E | Railway container/volume | `railway.toml`, `backend/Dockerfile` | Deployment and durable storage provider. |
| E | Base/EVM and Foundry | identity suite and `smart_contracts` | Optional cryptographic/public infrastructure. |
| E | Cloudflare/Orbiter/web hosting integrations | landing configuration/source | Anky distribution infrastructure. |

## Interpretation

Only a few rows are “A” because most valuable behavior needs application policy or platform adapters. That is healthy. A framework made entirely of generic primitives would either encode Anky accidentally or become a generic SaaS toolkit.

The central reusable seam is:

```text
configured action policy
  → immutable local event + opaque artifact
  → optional subscribers: reflection, accumulation, proof, sync, invitation
```

Anky-specific screen-time, painting, prompts, and visual character remain subscribers/reference behavior.

## Extraction risks

### Critical

#### C1. Artifact/hash identity is not canonical across lifecycle and platforms

iOS can append a terminal marker and replace the hash before reflection; Android reflects existing bytes. Terminal parsers disagree. Hash is simultaneously filename, reflection key, ledger key, and idempotency key.

Protected invariant:

- every existing `.anky` byte sequence and hash must remain readable and referentially intact;
- no migration may silently rewrite artifacts or orphan reflections/ledger references.

Evidence:

- `apps/ios/Anky/Features/Reveal/RevealViewModel.swift:431-458`.
- `apps/android/app/src/main/java/inc/anky/android/core/protocol/AnkyParser.kt:3-35`.
- `protocol/implementations/typescript/src/parse.ts`.

#### C2. Live iOS/Android behavior diverges despite parallel source trees

Android omits write preferences, gate session, level completion, and unlock callbacks at construction, uses deprecated onboarding, retains fragment drafts, can clear prior-day drafts, and does not call server account deletion.

Protected invariant:

- extraction must preserve each currently shipped behavior until a separately approved parity change;
- composition-root tests, not only unit tests, must describe which behavior is intentional.

Evidence:

- `apps/android/app/src/main/java/inc/anky/android/app/AnkyApp.kt:192-217,608-624`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:684-735,1161-1170`.
- `apps/android/app/src/main/java/inc/anky/android/feature/you/YouViewModel.kt:688-738`.

#### C3. Identity is the ownership key for several independent systems

The same address scopes RevenueCat, SQLite ledgers, quotas, paintings, events, and request signatures. Phrase import does not migrate old content or server ownership. Recovery differs by platform.

Protected invariant:

- never rotate/rederive/rename identity without a dual-identity migration and rollback;
- preserve existing EIP-712/domain/account fixtures;
- do not mix existing local archives into a new account silently.

Evidence:

- `protocol/identity/SPEC.md:3-24`.
- `backend/level/db.ts:64-184`.
- native `WriterIdentityStore` implementations.

#### C4. “Trustworthy continuity event” is not an implemented security property

Current signatures prove key control over client-supplied bytes/claims; neither device nor server independently witnesses physical action duration. Publishing a `continuity-proof` package with stronger language would create a false assurance.

Protected invariant:

- proof types must name the exact attestation (“practice key signed this claim”);
- raw private content must not become public to make a weak claim appear stronger.

Evidence:

- `backend/level/routes.ts:92-166`.
- `backend/level/db.ts:222-274`.
- `apps/ios/Anky/Core/Mirror/AnkyPostSigner.swift:21-75`.

### High

#### H1. Signed authorization is not endpoint-bound

The typed structure includes method/path, but clients and server always reconstruct fixed `POST /anky`, including actual GETs and `DELETE /account`. Replay state is process-local.

Protection:

- version and deploy endpoint-bound statements with dual verification;
- test empty-body cross-route substitution and multi-replica replay before reuse.

Evidence:

- `AnkyPostSigner.swift:63-71`.
- `LevelSyncClient.swift:197-233`.
- `backend/server.ts:924-1028`.

#### H2. Duplicate native domain logic has no comprehensive golden corpus

Unicode segmentation, terminal acceptance, suffix timing, recovery, normalization, backup, and reflection semantics can drift.

Protection:

- one immutable fixture corpus and lifecycle trace suite across TypeScript/Swift/Kotlin;
- record intentional platform differences explicitly.

#### H3. Raw local private data is not application-encrypted

Drafts, archives, indexes, and reflections are plaintext within platform sandboxes. TOHSENO's desired “encrypted app context” is stronger than today's implementation.

Protection:

- state the threat model;
- add encryption through storage adapters and migration, never by changing `.anky` bytes/hashes in place.

Evidence:

- native active/archive/reflection stores.
- encrypted backup stores, which show encryption exists only for backup envelopes.

#### H4. Account deletion and recovery claims exceed implementation

Android does not call the backend. Server row deletion leaves generated painting files. Webhooks may recreate subscription state. Android's encrypted backup is installation-local.

Protection:

- inventory and test every data location;
- make UI copy match demonstrable guarantees;
- treat cloud/local backup deletion separately.

Evidence:

- `backend/account/routes.ts:41-78`.
- `backend/level/db.ts:517-560`.
- `backend/painting/config.ts:84-110`.
- Android `YouViewModel.kt:688-738`.

#### H5. Giant orchestration units mix product, policy, and infrastructure

`AppRoot.swift`, native `WriteViewModel`s, and `backend/server.ts` have many responsibilities and hidden ordering dependencies.

Protection:

- introduce narrow internal facades with characterization tests before moving files;
- keep effects ordered around local event persistence.

#### H6. Reflection privacy and consent differ by platform and provider

Android auto-sends for entitled users; iOS asks. Provider ZDR is declared/configured, not cryptographically verified. Retry can redisclose.

Protection:

- explicit consent policy in manifest;
- store provenance/consent receipt locally;
- cache successful result when promising idempotency.

### Medium

#### M1. Crash consistency and Android direct writes

Archive, index, level queue, reflection, and draft clearing are separate effects; Android stores use non-atomic writes.

Protection:

- atomic write/rename or transactional repository;
- startup reconciliation and idempotent subscribers.

#### M2. Backup/import validation is incomplete

Normalization changes hashes, reflection JSON can be orphaned, and no full encrypted cross-platform golden round trip was found.

Protection:

- signed/digested manifest, strict versioning, referential validation, preservation mode.

#### M3. Backend idempotency does not preserve result identity

A succeeded retry calls providers again. Output and disclosure can differ.

Protection:

- store an encrypted/short-lived response envelope or define a derivation record and non-idempotent semantics.

#### M4. Platform gate dependencies are operationally fragile

Apple distribution entitlement, Android special permissions/store policy, and OEM background killing can change.

Protection:

- never make gate enforcement a core package;
- degrade to an invitation without risking local event data.

#### M5. Documentation and code conflict

READMEs contain older paths/routes/flows; parity notes report prior test state.

Protection:

- generated contract docs from fixtures where possible;
- executable call sites remain authoritative.

#### M6. Secrets and provider configuration span many independent projects

Backend/model/payment/chain/store signing configurations are separate and local secret files exist.

Protection:

- package-specific example variables and secret scanning;
- TOHSENO templates default to no external provider/payment/chain.

### Low

#### L1. No root workspace/build graph

This makes cross-project validation manual but does not prevent initial extraction.

#### L2. Hard-coded Anky terminology is widespread

Renaming prematurely is dangerous, but adapters can hide it over time.

#### L3. Generated/release assets make repository status noisy

Large native assets and historical relocation obscure changes. Documentation-only and extraction changes need narrow path audits.

## What must be protected while extracting

1. Exact existing `.anky` bytes, hashes, reflection links, and backup readability.
2. Offline first-glyph-to-draft persistence and crash recovery.
3. Ability to write without account UI, subscription, network, provider, gate permission, or blockchain.
4. Existing mnemonic derivation and account access.
5. No raw writing in backend logs/SQLite.
6. Explicit user export and recovery access.
7. Anky's ritual texture: forward-only input, threshold, stillness, character, and post-session experience.
8. Store-distributed iOS/Android applications and their migrations.
9. Server compatibility during staggered client upgrades.
10. The distinction between private local artifact, pseudonymous server metadata, and optional public-chain objects.

## TOHSENO implication

The initial package boundary should be drawn around new interfaces plus adapters, not around whole current folders. “Internal” is an important version-one status: artifact/event, identity, and storage APIs should remain internal until one Anky adapter and one materially different app prove them.

## Recommendation

Use the risk register as a release gate. No public extraction should occur until C1–C4 have explicit tests/decisions and H1 has a versioned authorization migration plan. Do not fix unrelated Anky parity issues opportunistically inside a package move; isolate and review them as product changes.
