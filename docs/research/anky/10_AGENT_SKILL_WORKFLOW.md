# TOHSENO agent-skill workflow

## Purpose

The TOHSENO skill should guide an agent from an idea to one tested, deployable continuity loop:

```text
interview
  → manifest
  → human confirmation of one action
  → vertical-slice scaffold
  → implement ritual
  → verify continuity/privacy invariants
  → test on target platforms
  → prepare deployment instructions
```

Its primary function is not code generation. It is protecting the ritual from generic product gravity.

The existing Anky skill content under `apps/landing/public/agent-skills/anky` is app-specific distribution material. It may inform packaging, but it is not a generic TOHSENO workflow.

## Stage 1: interview

The agent asks one compact question at a time, reflecting answers back without prematurely suggesting screens. It must discover:

1. **Target human:** Who reaches for this app, in what moment?
2. **Longing/problem:** What do they want to feel, practice, repair, or become through repetition?
3. **One observable action:** What can they physically or digitally do now?
4. **Start:** What is the first unmistakable sign that the action began?
5. **Completion:** What objective condition says the ritual was completed?
6. **Interruption:** What ends or pauses it before completion, and should partial work persist?
7. **Immediate reflection:** What should be felt/seen/heard immediately after action? Does any private data leave the device?
8. **Accumulation:** What becomes meaningful after days or months—without turning into a dashboard?
9. **Identity and recovery:** When should a practice key exist? How can it be recovered? Should another app/device ever relate to it?
10. **Proof/export:** What must the person be able to take with them or prove, and to whom?
11. **Monetization:** Is payment necessary? What may it unlock without gating action, local record, export, or recovery?
12. **Ritual destroyers:** What would make the experience performative, distracting, extractive, shameful, or generic?
13. **Tone/visual direction:** What should the moment feel like, without prescribing a screen system?
14. **Platforms/deployment:** Which native/web platforms and whether a monorepo-hosted server are actually required?

The agent must press for an observable condition when given abstractions such as “be mindful,” “improve,” or “finish a session.” It must not turn a longing into a feature list.

### Required interview output

Before code, the agent writes:

```text
AppName: <one sentence naming the one action and purpose>

Starts when:
Completes when:
Interrupts when:
Partial action:
Immediate reflection:
Accumulates:
Private boundary:
Identity/recovery:
Export/proof:
Must never become:
```

It identifies assumptions and conflicts. If the person cannot confirm the one-line action, the workflow remains in interview; it does not scaffold.

## Stage 2: produce and confirm the manifest

The agent translates the interview into `continuity.manifest.json` using the schema in `09_CONTINUITY_MANIFEST.md`. It then produces a human-readable contract:

- one action;
- completion and interruption examples;
- what is persisted locally;
- what leaves the device, at which consent moment;
- what the key attests;
- recovery timing;
- optional services/cost;
- five things the scaffold will deliberately omit.

### Confirmation gate

The person explicitly confirms:

1. the one-line action;
2. completion and interruption;
3. whether partial action persists;
4. reflection trigger and disclosure;
5. the “what would ruin it” list.

Changes after this gate update the manifest and generated acceptance tests before implementation.

## Stage 3: threat and continuity model

The skill generates a short table:

| Asset | Local location | Leaves device? | Recipient | Retention/deletion | Recovery |
|---|---|---|---|---|---|
| Practice secret | secure-secret adapter | Never, except consented recovery envelope | selected recovery destination | explicit | manifest policy |
| Active checkpoint | private/application-encrypted repository | No by default | — | until commit/discard | crash recovery |
| Artifact | private repository | only declared export/reflection/sync | named endpoint/provider | named | backup/export |
| Event metadata | local log | optional aggregate/sync | named server | named | rebuild/import |
| Reflection | local repository | provider response originates remotely if configured | named provider | named | backup/export |
| Proof | only if enabled | user-selected | chosen verifier/public target | irreversible if public | export |

The agent must use precise language:

- a digest detects byte changes; it is not authorship proof;
- a practice-key signature proves key control over a statement; it does not prove human action or honest duration;
- platform-private is not the same guarantee as application-encrypted;
- ZDR is a provider/contract configuration, not mathematical deletion;
- blockchain publication is public and linkable.

If the manifest and privacy table conflict, scaffolding stops until corrected.

## Stage 4: scaffold the smallest vertical slice

The generated project initially contains:

```text
app entry
→ one action surface
→ action policy/reducer
→ local checkpoint + event/artifact repository
→ immediate feedback
→ continuation invitation
```

Only declared capabilities are added:

- practice identity if configured;
- reflection endpoint/provider if configured;
- recovery/export after first value;
- synchronization/payment/proof only when manifest enables them.

The first runnable milestone works offline and reaches the action without login.

### Scaffold artifacts

- validated `continuity.manifest.json`;
- one action policy and fixture set;
- event/artifact types;
- platform repository/secure-secret adapters;
- one first-launch/action route;
- crash/resume behavior;
- privacy and deletion inventory;
- tests derived from the manifest;
- server capability only if required;
- deployment/readme instructions.

It must not generate placeholder dashboards, profile tabs, settings matrices, feeds, admin CRUD, or analytics suites.

## Stage 5: implement the ritual

The agent implements in this order:

1. Reach the action from a fresh install.
2. Start and progress through the native input/sensor adapter.
3. Checkpoint locally after the manifest's durability boundary.
4. Detect completion/interruption in the pure policy.
5. Commit immutable event and artifact locally.
6. Recover from process death/background interruption.
7. Render immediate feedback from committed state.
8. Invite continuation.
9. Create identity/recovery UX at the configured moment.
10. Add reflection, export, optional aggregate, then optional commercial adapters.

Why local commit precedes reflection: provider, entitlement, and network failures cannot erase continuity.

For Anky, the reference implementation must preserve forward graphemes and `.anky v0` through an adapter. For Gratitude Lock, the same domain facts may be emitted by text, audio, or photo capture without inheriting Anky's timing/editor.

## Stage 6: verify invariants

The skill generates executable cases from the manifest.

### Universal continuity invariants

- Fresh install reaches the core action without account/profile setup.
- First progress creates a recoverable local checkpoint.
- Network loss does not block action, checkpoint, completion, or local commit.
- Process death during action restores or explicitly offers the partial action.
- Exactly one immutable event is committed for one seal transition.
- Completion and interruption are distinguishable.
- Subscriber failure does not roll back the event.
- Reflection is linked to stable event/artifact identifiers.
- Private artifact bytes are absent from logs/telemetry.
- Export is explicit and round-trips without accidental normalization.
- Recovery never silently changes identity/content ownership.
- Deletion reports each local, cloud, server, provider, and generated-asset location.
- Payment cannot gate action, local commit, recovery, or user-owned export.
- No public disclosure occurs by default.

### Identity/crypto invariants

- deterministic fixture derives the same public identity on supported platforms;
- secure storage absence/key loss produces a recoverable state, not silent content reassignment;
- operation signatures bind scope, audience, operation, body digest, expiry, and nonce;
- server rejects changed body, account, method/operation, expired time, and replay;
- proof copy describes only what verification establishes;
- cross-app bridges require explicit signed consent.

### Anky compatibility invariants

- exact fixture bytes/hash remain unchanged;
- Unicode grapheme/duration/terminal behavior is tested in Swift/Kotlin/TypeScript;
- old archive/reflection/ZIP data remains readable;
- eight minutes marks completion but stillness seals;
- fragment behavior is explicitly characterized per platform until aligned;
- iOS/Android composition roots use intended collaborators.

### Reflection invariants

- consent trigger matches manifest;
- disclosed fields match privacy table;
- provider capabilities satisfy policy;
- successful retry semantics are defined;
- result includes policy/provider provenance;
- fallback/unavailable text is not mistaken for a successful reflection;
- deletion behavior matches written promises.

## Stage 7: resist generic application patterns

The skill includes a “ritual lint” pass. It flags any proposed feature not required by the manifest and asks how it supports `act → record → reflect → continue`.

### Default rejection rules

| Proposed pattern | Agent response |
|---|---|
| Dashboard home | Route to the action; use one small accumulated signal only if it invites continuation |
| Profile/account before value | Create contextual identity invisibly; offer recovery after value |
| Social feed | Reject unless the core action itself is consensual coordination and privacy model explicitly supports it |
| Generic CRUD | Model lifecycle events/artifacts, not arbitrary entities |
| Authentication screens | Add only when external coordination requires them; preserve local first value |
| Generic AI chat | Use bounded reflection tied to an event, or omit AI |
| Broad analytics SDK | Start with none; add minimal pseudonymous events only when justified and listed |
| Gamified streak pressure | Prefer compassionate accumulation and recovery from gaps |
| Public blockchain record | Require a precise proof/ownership/coordination need and explicit publication consent |
| Settings before ritual | Use safe defaults; reveal settings/recovery after first value |
| Feature-rich editor | Preserve the action's intended constraints |
| Push-notification campaign | One return invitation matching the person's chosen rhythm |

The agent may recommend a rejected feature later, but only after stating:

- which continuity transition it serves;
- why an existing surface cannot serve it;
- privacy and interruption cost;
- whether it violates the forbidden-pattern list.

## Stage 8: run tests and inspect the experience

The skill performs:

1. schema/manifest validation;
2. pure policy fixtures;
3. artifact/identity golden fixtures;
4. repository crash/migration tests;
5. backend auth/reflection/deletion tests if present;
6. native unit and lifecycle tests;
7. a fresh-install manual/simulator/device walkthrough;
8. offline/slow/failing-provider cases;
9. accessibility, IME/media permission, background, and low-storage cases appropriate to the action;
10. changed-file, secret, dependency, and generated-file audit.

The visual walkthrough checks that the action is dominant. Passing tests cannot compensate for a profile-first or dashboard-first experience.

## Stage 9: prepare deployment instructions

The skill does not deploy, submit to stores, create paid resources, publish packages, or make chain transactions without explicit authority. It prepares:

- exact local build/test commands;
- environment-variable inventory with no secret values;
- server container/health/migration/backups;
- provider privacy and data-processing checklist;
- Apple/Google permissions, entitlements, privacy labels, and review notes;
- package/bundle IDs and signing prerequisites;
- rollback and data migration plan;
- deletion verification procedure;
- store/test-track steps;
- optional public proof/chain warnings;
- post-deploy smoke tests.

Deployment instructions must include an offline-local-value test and a server-disable rollback path.

## Skill state machine

```text
discovering
  → draft-manifest
  → awaiting-core-confirmation
  → privacy-modeled
  → scaffolded
  → ritual-working-offline
  → optional-services-integrated
  → invariants-green
  → deployment-ready
```

The skill may not advance:

- past `awaiting-core-confirmation` without confirmed action/conditions/disclosure;
- to `optional-services-integrated` before local event commit works offline;
- to `invariants-green` with skipped critical tests unexplained;
- to `deployment-ready` without recovery/deletion/rollback instructions.

## Current implementation evidence that shapes the workflow

- Android contains full-looking source that is not wired by the live shell, so the workflow tests composition roots: `AnkyApp.kt:192-217,608-624`.
- iOS reflection consent and Android automatic reflection differ, so the workflow confirms disclosure: `AppRoot.swift:1966-2025`, `PostSessionSealingScreen.kt:90-94`.
- `.anky` hash can change before reflection, so stable event IDs are tested: `RevealViewModel.swift:431-458`.
- Android deletion does not reach the server and server deletion misses files, so deletion is an inventory: Android `YouViewModel.kt:688-738`, `backend/painting/config.ts:84-110`.
- the gate depends on fragile OS capabilities, so local action is independently testable: iOS Screen Time notes and Android `GateWatcherService.kt`.

## Interpretation

A continuity-app skill is closer to a product/architecture guardian than a broad app generator. Its success criterion is a small, durable ritual—not the number of generated features.

## TOHSENO implication

The skill should be published only after:

- the manifest validator exists;
- Anky's compatibility fixtures pass;
- one non-writing app reaches local commit;
- generated tests detect at least one intentional manifest violation;
- deployment instructions have been exercised without auto-deploying.

## Recommendation

Build the skill from the roadmap's executable contracts, not from prose alone. Its first automated action should validate/interview around the manifest; its first generated code should be a local vertical slice; and its strongest lint should be the question:

> Does this make the one repeated action easier to begin, safer to remember, more meaningful to reflect on, or more natural to continue?

