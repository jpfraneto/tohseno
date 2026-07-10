# Extraction roadmap

## Governing rule

Anky stays working throughout. Each phase introduces a reversible boundary and proves it before the next phase. Existing `.anky` bytes, hashes, identity derivation, backups, and deployed server contracts receive compatibility treatment; framework cleanliness is never a reason to rewrite user data silently.

This roadmap describes future work. In the current study phase, only `docs/tohseno` is changed.

## Current implementation

No TOHSENO runtime package, event envelope, generic manifest validator, sync layer, generic proof, scaffold, or agent skill is implemented in this repository today. Anky's current compatibility anchors are `.anky v0`, `anky.base.eoa.v1`, the two native application shells, and the deployed Bun API contract. The phases below are recommendations, not descriptions of changes already made.

Evidence:

- `protocol/SPEC.md:1-57`.
- `protocol/identity/SPEC.md:3-24`.
- `apps/ios/Anky/AnkyApp.swift:5-13`.
- `apps/android/app/src/main/java/inc/anky/android/MainActivity.kt:17-54`.
- `backend/server.ts:324-509`.

## Phase 0 — freeze architecture and product decisions

**Goal**

Make current behavior and unresolved policy choices visible. Establish one authoritative inventory of live entry points, privacy boundaries, data locations, external processors, and platform divergence.

**Files affected**

- `docs/tohseno/*` only during this study.
- In a later decision pass, possibly a new decision-record subdirectory beneath `docs/tohseno`.

**Prerequisites**

- Repository/entry-point audit.
- Product and engineering owners review iOS/Android differences.

**Risks**

- Treating stale READMEs as executable truth.
- Mistaking a recommendation for current behavior.
- Resolving product questions implicitly during implementation.

**Validation criteria**

- Every current claim cites an existing path/type/function/route.
- One-line Anky action is confirmed.
- Owners decide or explicitly defer stable event ID, terminal/hash semantics, fragment reflection, consent, proof meaning, identity timing, and deletion scope.
- No production files change.

**Rollback**

- Documentation is additive; revert the documentation directory if rejected. No runtime/data rollback is needed.

**Explicitly not yet**

- No new package, schema migration, file move, auth version, Android parity fix, or app scaffold.

## Phase 1 — build the continuity contract harness

**Goal**

Turn the implicit current contracts into executable characterization tests before moving code.

**Files affected**

- `protocol/fixtures/` and `protocol/expected/`.
- `protocol/implementations/typescript/test/`.
- `apps/ios/Anky/Tests/` and Swift package tests.
- `apps/android/app/src/test/` and selected `androidTest/`.
- `backend/test/`.
- New top-level `fixtures/lifecycle`, `fixtures/identity`, and `fixtures/backup` only after naming is agreed.

**Required fixtures**

- exact complete/fragment bytes and hashes;
- terminal values 1,000–8,000 and invalid boundaries;
- spaces/newline rejection, complex Unicode graphemes, normalization;
- frozen resume and suffix replacement;
- eight-minute threshold versus silence seal;
- fragment continuation/retention behavior;
- pre/post-reflection artifact/hash;
- EIP-712 derivation/signature across Swift/Kotlin/TypeScript;
- signed actual operation/method/path negative cases;
- archive/reflection/ZIP import, tamper, orphan, cross-platform encrypted backup;
- first-launch/onboarding/composition wiring;
- deletion location inventory.

**Prerequisites**

- Phase 0 current/desired behavior distinctions.
- Access to supported simulators/emulators and any existing test keys/config that do not contact production.

**Risks**

- A characterization test can freeze an accidental bug.
- Native crypto/Unicode test vectors may expose existing divergence.
- UI instrumentation may be flaky.

**Validation criteria**

- One corpus runs in all three language implementations.
- Expected divergences are labeled by platform and linked to a product decision.
- Live composition tests fail if Android's intended collaborators are omitted.
- No fixture contains real user writing or secrets.
- Existing application behavior remains unchanged.

**Rollback**

- Tests/fixtures are additive. Remove only a disputed fixture; retain a written record of why it was not a contract.

**Explicitly not yet**

- No shared runtime package, `.anky v1`, identity rotation, endpoint-auth production switch, storage encryption migration, or UI redesign.

## Phase 2 — define compatibility decisions and internal ports

**Goal**

Introduce small interfaces beside existing implementations, with no directory move and no user-visible behavior change.

**Files affected**

- iOS internal protocols near:
  - `apps/ios/Anky/Core/WriteBeforeScroll/WritingSessionEngine.swift`;
  - `Core/Storage/*`;
  - `Core/Identity/*`;
  - `Core/Mirror/*`.
- Android interfaces near equivalent `core/protocol`, `core/storage`, `core/identity`, and `core/mirror`.
- Backend interfaces extracted locally from `backend/server.ts` into narrowly named modules.
- `protocol` version/fixture documentation.

**Interfaces introduced**

- pure action policy/reducer;
- clock and lifecycle commands;
- artifact codec/digest;
- event/checkpoint repository;
- practice identity and scoped operation signer;
- reflection policy/provider/repository;
- post-commit subscriber.

**Prerequisites**

- Phase 1 green characterization harness.
- Decisions on stable event IDs and old `.anky` treatment.
- Endpoint-binding migration design, including old-client window.

**Risks**

- Facades can become thin aliases that preserve coupling.
- Effect ordering can change while “only” introducing interfaces.
- A generalized name can hide Anky-specific semantics.

**Validation criteria**

- Existing view models/routes use adapters satisfying the new internal ports.
- Characterization outputs remain byte-for-byte identical.
- Dependency tests prevent domain interfaces from importing UI, RevenueCat, providers, gates, or file APIs.
- Old clients still authenticate and reflect.

**Rollback**

- Keep old implementations behind an alternate adapter/feature flag during conversion.
- Repoint composition roots to old concrete types without data migration.

**Explicitly not yet**

- No public package release, source relocation, event-log data migration, second app, or removal of legacy auth/codec.

## Phase 3 — add a sidecar event log and internal reusable packages

**Goal**

Create the first internal TOHSENO packages and record stable event envelopes alongside unchanged Anky artifacts.

**Files affected**

- New internal package roots corresponding to:
  - `continuity-domain`;
  - `continuity-artifact`;
  - `continuity-log`;
  - `practice-identity`;
  - `reflection`;
  - `anky-compat`.
- Native Swift/Kotlin adapter modules.
- Existing mobile composition roots only to inject adapters.
- Storage migration/reconciliation code and tests.

**Migration strategy**

1. Read legacy archive/index as today.
2. Derive sidecar event envelopes without changing `.anky` bytes.
3. Write new events and old-compatible artifacts together.
4. Shadow-build old/new projections and compare.
5. Rebuild sidecars safely if deleted.
6. Do not remove legacy index/read paths yet.

**Prerequisites**

- Phase 2 ports stable under Anky.
- Event schema and migration decision record.
- Atomic/reconciliation behavior tested on both platforms.

**Risks**

- Dual-write partial failure.
- Event duplication during rebuild.
- Identity/content ownership ambiguity on imported phrases.
- App size/build graph complexity.

**Validation criteria**

- Fresh and legacy installs show identical Anky UI/content.
- Event IDs remain stable before/after reflection.
- Killing the app at each dual-write step recovers deterministically.
- Sidecar removal/rebuild does not alter `.anky` hashes/reflections.
- Package dependency boundaries are enforced.

**Rollback**

- Legacy files remain source-readable.
- Disable sidecar reads/writes with a local migration flag; retain artifacts.
- Sidecar schema is additive and can be rebuilt.

**Explicitly not yet**

- No deletion of legacy stores, public npm/Swift/Maven packages, sync, proof publication, or forced content re-encryption.

## Phase 4 — align Anky adapters and deploy versioned authorization

**Goal**

Resolve intentional platform differences and security gaps as explicit Anky releases, using the internal contracts.

**Files affected**

- `apps/android/.../app/AnkyApp.kt` composition wiring.
- Android onboarding, fragment/draft, preferences, deletion, and recovery paths.
- iOS/Android reflection-trigger adapters.
- `AnkyPostSigner`/signed-operation clients.
- `protocol/implementations/typescript/src/identity.ts`.
- `backend/server.ts` verification and route middleware, with new version modules/tests.
- account/painting deletion code.

**Prerequisites**

- Product decisions on:
  - opt-in versus automatic reflection;
  - fragment persistence/continuation;
  - terminal/hash canonical behavior;
  - Android recovery;
  - deletion promise.
- Dual-version auth support deployed server-first.
- Store-release rollback plan.

**Risks**

- Old clients can be locked out.
- Auth semantics can introduce replay/substitution bugs.
- Fixing fragment behavior can expose duplicate archives.
- Recovery/deletion changes can be irreversible.

**Validation criteria**

- Server accepts old and new auth only for the declared compatibility window.
- New signatures bind operation/audience/scope/body/expiry/nonce; cross-route negative tests pass.
- Android composition-root tests prove dependencies are live.
- Reflection consent matches manifest on both platforms.
- Account deletion test verifies rows, painting files, local stores, and documented external limitations.
- Staged mobile rollout metrics contain no raw private data.

**Rollback**

- Server retains old verifier behind a time-bounded compatibility flag.
- Mobile adapters can return to old behavior without rolling back data schema.
- Deletion migrations are additive until verified; never restore deleted user data.

**Explicitly not yet**

- No public framework promise, generic template, sync, public proof, or Anky UI rewrite.

## Phase 5 — implement a materially different second app

**Goal**

Use the internal kernel to build Gratitude Lock (or another confirmed non-text ritual) through local commit. Prove the interfaces are not merely renamed Anky abstractions.

**Files affected**

- `apps/gratitude-lock` or a new explicitly approved example path.
- Internal framework packages and adapters only when a real requirement exposes a gap.
- Its `continuity.manifest.json`, fixtures, privacy/deletion docs, and tests.

**Prerequisites**

- Stable Phase 3 packages.
- Confirmed second-app one-line action and manifest.
- Media storage/encryption threat model if using voice/photo.

**Risks**

- Copying Anky's eight-minute/text/level assumptions.
- Expanding the universal schema for one app.
- Handling large media as inline event payloads.
- Building a polished second product before testing the framework seam.

**Validation criteria**

- Starts/completes/interruption differ materially from Anky.
- Works offline and commits one encrypted local event.
- Uses local reflection or no provider.
- Does not import Anky prompts, `.anky`, gate, RevenueCat, Base, painting, or companion code unless specifically justified.
- Both apps pass shared lifecycle repository tests.
- Package changes simplify both apps rather than adding app-name conditionals.

**Rollback**

- Example app is isolated; framework changes remain behind versions.
- Revert a generalized API that only serves the second app and keep an app adapter.

**Explicitly not yet**

- No claim of universal action support, broad template marketplace, social features, or production deployment requirement.

## Phase 6 — publish the TOHSENO scaffold and reference server

**Goal**

Stabilize the smallest proven public API, manifest validator, native template, and self-hostable reflection service.

**Files affected**

- `README.md`, licenses/governance/security policy.
- Public package metadata and release workflows.
- `templates/native-continuity-app`.
- `services/reflection-server`.
- public fixtures/examples/docs.
- generated API/manifest documentation.

**Prerequisites**

- Anky and second app pass shared contracts.
- Security review of identity/auth/encryption/deletion.
- Versioning/deprecation policy.
- Reproducible clean checkout builds.

**Risks**

- Publishing unstable storage/identity APIs.
- Accidental vendor or EVM lock-in.
- Templates embedding secrets, paid services, or Anky visuals.
- Support burden across native toolchains.

**Validation criteria**

- Clean-room scaffold reaches offline action → record → local reflect → continue.
- No login, provider key, payment, chain, or hosted service is required by default.
- Manifest validator rejects known contradictions.
- Published packages have API compatibility and migration tests.
- Reference server runs with a local/no-network provider.
- deployment instructions include deletion, backup, rollback, and privacy inventory.

**Rollback**

- Pre-1.0 semver with explicit experimental labels.
- Keep internal packages unpublished until ready.
- Deprecate rather than yank any version capable of reading user data.

**Explicitly not yet**

- No one-line remote installer, automatic store submission, hosted multi-tenant SaaS, default sync, or public proofs.

## Phase 7 — publish the agent skill and installer

**Goal**

Package the workflow in `10_AGENT_SKILL_WORKFLOW.md` as a tested coding-agent skill. Add a one-line installer only after signed/versioned releases and a non-destructive preview exist.

**Files affected**

- `skills/continuity-app/SKILL.md`.
- narrowly required references/scripts/templates.
- skill tests/evaluations.
- installer/checksum/release metadata.
- public documentation.

**Prerequisites**

- Phase 6 scaffold is stable.
- Skill can validate manifests and generate fixtures without network.
- Example projects prove upgrade paths.
- Installer security/ownership/release process is defined.

**Risks**

- Agent generates generic apps despite doctrine.
- Installer executes unreviewed remote code or overwrites a dirty project.
- Skill promises deployment/security capabilities not in packages.
- Prompt instructions drift from schemas/tests.

**Validation criteria**

- Interview refuses to scaffold before core-action confirmation.
- Ritual lint catches dashboard/profile/feed/auth/AI-chat fixtures.
- Generated app passes offline, crash, privacy, identity, and deletion tests.
- Installer defaults to dry-run, verifies release checksums/signatures, refuses destructive overwrite, and pins a version.
- Deployment remains an instruction/explicit-approval step.

**Rollback**

- Version the skill independently.
- Pin known-good scaffold releases.
- Installer changes no project until preview/confirmation; removal affects only its created files.

**Explicitly not yet**

- No unattended deployment, credential creation, store submission, paid-resource provisioning, blockchain transaction, or background data import.

## Cross-phase release gates

| Gate | Must be true before proceeding |
|---|---|
| Data | Existing `.anky`, reflection, backup, and identity fixtures still read |
| Offline | Action/checkpoint/commit work without server |
| Privacy | Raw content absent from logs; disclosures and deletions enumerated |
| Identity | Recovery/rotation does not silently reassign content |
| Compatibility | Server supports staggered client versions |
| Platform | Live composition tests pass on iOS and Android |
| Genericity | Second app uses packages without Anky conditionals |
| Public API | Security/migration behavior documented and tested |
| Skill | Manifest and invariant tests drive generation |

## Interpretation

The safest extraction begins with executable knowledge, then internal seams, then additive event metadata. Package moves and publishing occur late. The second app is not marketing garnish; it is the proof that a boundary is genuinely reusable.

## TOHSENO implication

The framework's release sequence is part of its architecture: compatibility and privacy guarantees must be proven while packages are internal. A public scaffold comes only after Anky remains stable under adapters and a second app demonstrates the same kernel without inheriting Anky's ritual.

## Recommendation

Begin Phase 1 with the fixture/characterization harness. The single most valuable first test should capture:

> Given the same Unicode/timing trace and identity fixture, Swift, Kotlin, and TypeScript agree on exact artifact bytes, digest, completeness, and scoped request statement—and each live mobile shell commits exactly one recoverable local event.
