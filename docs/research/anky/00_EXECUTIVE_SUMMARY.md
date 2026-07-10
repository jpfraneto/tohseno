# TOHSENO architectural study: executive summary

## Scope and evidence status

This study describes the repository as it exists in the working tree on 2026-07-10. It does not refactor, extract, rename, or modify production code. The working tree already contained a large, in-progress relocation from `apps/write-8-minutes/{ios,android}` to `apps/{ios,android}` and unrelated backend/protocol edits before this documentation was created. Therefore, all mobile evidence below uses the live `apps/ios` and `apps/android` paths and treats Git history only as context, not as the runtime source of truth.

No applicable `AGENTS.md` file exists in the repository or its parent path. Local secret files were not opened. The final validation record is in `12_OPEN_QUESTIONS.md`.

## Anky in one line

> **Anky: write forward continuously until stillness seals the session, then optionally receive a private reflection and return to write again.**

The product's marketed eight-minute ritual is not, in code, an automatic stop at eight minutes. Eight minutes changes the session from a fragment to a complete Anky; the artifact is actually sealed after the configured inactivity period, or at a platform lifecycle boundary in a limited quick-pass path.

Evidence:

- `apps/ios/Anky/Core/Protocol/AnkyDuration.swift:3-36` — `AnkyDuration.isComplete` and terminal-silence policy.
- `apps/ios/Anky/Features/Write/WriteViewModel.swift:564-738` — silence scheduling and sealing.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:252-330` — Android's hard-coded silence timer.
- `protocol/SPEC.md:1-57` — canonical text artifact description.

## Current implementation

Anky is not one shared cross-platform application. It is two native implementations, a Bun/Hono backend, a TypeScript reference protocol implementation, and several adjacent products and tools:

```text
iOS SwiftUI ─┐                           ┌─ OpenRouter / Bankr / Poiesis
             ├─ signed HTTPS ─ Bun/Hono ─┼─ SQLite + painting files on Railway
Android      ┘                           └─ RevenueCat webhooks

        Swift codec ─┐
        Kotlin codec ├─ intended to implement protocol/SPEC.md
     TypeScript codec┘
```

The continuity loop exists today:

| Step | What exists | Important qualification |
|---|---|---|
| act | Forward-only native writing surfaces | Input semantics differ by platform; Android does not wire all stored preferences into its live composition root. |
| record | A draft is written after accepted glyphs | iOS writes atomically; Android uses direct file writes and can delete a prior-UTC-day draft during load. |
| complete/interrupted | Silence seals both fragments and complete sessions | Eight minutes classifies completeness; it does not itself end the session. |
| persist | Exact `.anky` bytes are named by SHA-256 | There is no version header, event envelope, identity, outcome, or stored signature. |
| reflect | The raw reconstructed writing is sent to `POST /anky` | iOS requests only after a user chooses “Read”; Android auto-starts for entitled users after sealing. |
| continue | Archive, journey/painting, reminders, and write-before-scroll invite return | Progression and gate behavior are Anky product logic, not generic continuity infrastructure. |
| recover/export | Phrase display/import and ZIP/Markdown export exist | Recovery and deletion guarantees diverge sharply between platforms. |

Evidence:

- `apps/ios/Anky/AppRoot.swift:1778-2025` — iOS post-session choice and explicit reflection request.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/PostSessionSealingScreen.kt:90-94` — Android immediate reflection request.
- `apps/ios/Anky/Core/Storage/LocalAnkyArchive.swift:32-102` and `apps/android/app/src/main/java/inc/anky/android/core/storage/LocalAnkyArchive.kt:12-29` — content-addressed archives.
- `backend/server.ts:2002-2324` — reflection route lifecycle.

## Five most important findings

### 1. The strongest reusable seed is a set of contracts, not a directory to move

The repository has useful primitives: canonical UTF-8 artifact bytes, SHA-256 addressing, BIP-39/BIP-44 identity derivation, EIP-712 request signing, local-first storage, encrypted backup envelopes, a reflection-provider router, and cross-language fixtures. They are embedded in Anky names and assumptions, duplicated across Swift/Kotlin/TypeScript, and sometimes behaviorally divergent. Extracting files first would freeze accidental inconsistencies into a framework.

Evidence:

- `protocol/implementations/typescript/src/identity.ts:5-170` — derivation, typed-data signing, recovery.
- `apps/ios/Anky/Core/Identity/WriterIdentity.swift:18-108` and `apps/android/app/src/main/java/inc/anky/android/core/identity/WriterIdentity.kt:9-73` — native duplicates.
- `backend/server.ts:1278-1345` — existing provider interface and router.
- `protocol/fixtures/` and `protocol/expected/` — language-neutral protocol fixtures.

### 2. A `.anky` is a private artifact, not yet a complete continuity event or proof

The format records a start time and per-glyph timing deltas. Its digest detects byte changes if recomputed, but it has no schema version, app/policy identifier, outcome, signer, signature, stable event identifier, or proof semantics. The server's signed request proves control of a practice key over one HTTP body; it does not prove that the claimed timing was physically observed, and that signature is not persisted with the artifact.

Evidence:

- `protocol/SPEC.md:5-45` — file grammar and terminal marker.
- `apps/ios/Anky/Core/Protocol/AnkyHasher.swift` and `apps/android/app/src/main/java/inc/anky/android/core/protocol/AnkyHasher.kt` — SHA-256 digest.
- `apps/ios/Anky/Core/Mirror/AnkyPostSigner.swift:21-75` — request-scoped signature.
- `backend/level/routes.ts:92-166` and `backend/level/db.ts:222-274` — signed, client-asserted ledger fields.

### 3. Practice identity is portable cryptographically, but recovery is not operationally equivalent

All three implementations derive a Base/EVM secp256k1 key at `m/44'/60'/0'/0/0` from a 12-word BIP-39 phrase. iOS stores the phrase in a device-only Keychain item and can opt into a synchronizable Keychain backup plus encrypted iCloud document backup. Android wraps the phrase with an Android Keystore AES key inside app-private storage, disables Android backup, and offers only manual phrase/ZIP workflows. Importing another identity does not bind or migrate the existing local archive.

Evidence:

- `protocol/identity/SPEC.md:3-24` — identity law.
- `apps/ios/Anky/Core/Identity/WriterIdentityStore.swift:18-130` and `apps/ios/Anky/Core/Identity/KeychainClient.swift:27-43`.
- `apps/android/app/src/main/java/inc/anky/android/core/identity/WriterIdentityStore.kt:12-90`.
- `apps/android/app/src/main/AndroidManifest.xml:35-43` and `apps/android/app/src/main/res/xml/backup_rules.xml`.

### 4. Source parity is not runtime parity

Android contains ports of many iOS types, but `AnkyApp` constructs `WriteViewModel` without the writing-preference store, gate session, level-completion callback, or unlock callbacks. Those constructor arguments have no-op/default values. Android also invokes a deprecated onboarding wrapper that auto-advances rather than the complete flow present in the same file. This is an architectural warning: class-level parity and unit coverage cannot establish composition-root parity.

Evidence:

- `apps/android/app/src/main/java/inc/anky/android/app/AnkyApp.kt:192-217` — live construction.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/WriteViewModel.kt:148-185` — default/no-op collaborators.
- `apps/android/app/src/main/java/inc/anky/android/app/AnkyApp.kt:608-624` — onboarding call site.
- `apps/android/app/src/main/java/inc/anky/android/feature/onboarding/OnboardingScreen.kt:115-205,1415-1447` — full flow versus deprecated wrapper.

### 5. Reflection privacy is local-first but not local-only

The backend validates and reconstructs the full writing, then sends it to an external model provider under a configured zero-data-retention policy. The server does not store raw reflection input or output in SQLite, but it stores the full account address and artifact hash in ledgers/idempotency state. The separate dynamic-painting path also sends accumulated writing for distillation and persists generated per-account assets; account deletion clears database rows but does not delete those painting files.

Evidence:

- `backend/server.ts:206-258,1348-1403,2002-2324` — provider configuration and route.
- `backend/reflection.ts:107-204` — raw writing inserted into the prompt.
- `backend/level/db.ts:64-184,517-560` — durable records and account-row deletion.
- `backend/painting/config.ts:84-110` and `backend/painting/pipeline.ts:142-177` — per-account file persistence.

## Interpretation

Anky demonstrates the continuity pattern convincingly, but its reusable boundary is still conceptual:

- The **ritual** is excellent reference-application material.
- The **domain vocabulary** should be made explicit before extraction.
- The **artifact digest** should remain distinct from event identity and proof.
- The **practice identity** should become an adapter-backed capability, not an EVM requirement for every app.
- The **reflection contract** can be generalized, while Anky's tiers, prompts, character voice, paywall, and paintings remain product code.
- Native OS controls, purchases, hosted models, and blockchain features are optional adapters, never framework prerequisites.

## Three greatest extraction risks

1. **Critical — canonical data and hash drift.** iOS may append a terminal marker before reflection and thereby change the hash; Android reflects the archived bytes as-is. Parsers disagree about acceptable terminal duration. A premature shared package could invalidate links among artifacts, reflections, ledgers, and backups.
2. **Critical — composition and lifecycle drift.** Duplicated native logic has different draft deletion, fragment continuation, post-seal reflection, onboarding, deletion, and gate wiring behavior. Moving common-looking code without characterizing the live shells would silently change Anky.
3. **Critical — identity and ownership migration.** The address is the RevenueCat app-user identifier and backend account key. Phrase replacement, uninstall, cross-device restore, server deletion, and generated-asset deletion do not form one transactional recovery model.

## TOHSENO implication

TOHSENO version one should be a small domain kernel plus adapters:

- a pure continuity-session reducer and policy contract;
- a versioned opaque artifact/digest contract;
- an append-only local event repository;
- a practice-identity interface with the existing Base EOA suite as one internal adapter;
- a reflection request/result contract with privacy capabilities;
- native secure/private storage adapters;
- Anky kept intact as the first reference implementation.

Proofs, synchronization, payments, screen-time gates, blockchain minting, public identity bridges, and generic templates should not be mandatory version-one packages.

## Recommended first extraction step

Do not move implementation code yet. Build a **continuity contract harness** around today's behavior:

1. Create shared golden fixtures for exact artifact bytes, Unicode glyph segmentation, hash, continuation, terminal markers, and EIP-712 signatures.
2. Add composition-root tests that exercise actual iOS and Android first-launch, resume, seal, reflection opt-in, deletion, and recovery wiring.
3. Decide the canonical stable event identifier and whether reflection may ever rewrite artifact bytes.
4. Only then isolate a pure reducer behind the existing native view models.

This preserves Anky while turning implicit behavior into an executable extraction boundary.

## Recommendation

Treat Anky as a reference implementation that TOHSENO must continue to support, not as a codebase that must immediately conform to a clean-room framework design. The safe path is:

> document → characterize → decide compatibility rules → isolate pure boundaries → prove them with a second ritual → publish.

