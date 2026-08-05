# TOHSENO dogfood report — first complete local loop, 2026-07-30/31

One person (an autonomous coding agent acting as a technically capable
first-time builder) arrived with intentions, used the real CLI, Engine,
Studio, Shot folders, harness adapter, terminal handoff, execution records,
feedback surfaces, and Evolution mechanism, and asked one question:

> Can one person arrive with an intention, receive a real app, use it, notice
> what it wants to become, attach that feedback to its current version, and
> evolve the same Shot without understanding TOHSENO's internals?

Answer, in brief: **the spine is real — intention → Shot → execution →
usable app → contact → version-bound feedback → Evolution all happened with
real signed records and a real coding harness — but the ceremony broke at
three load-bearing joints** (first-run identity, first-execution retry, and
feedback-to-evolution continuity), each of which was worked around only with
knowledge no first-time builder has.

## Session overview

- Repository: `/Users/kithkui/code/tohseno.com`, commit `1b2caf4`, clean tree
  at start. No branch was created; no push, deploy, publication, or onchain
  action occurred.
- Host: macOS (Darwin 25.5.0), Xcode 26.3 (17C529), simulator iPhone 17 Pro
  (iOS 26.3), zsh.
- Invocation: release binary built from this checkout
  (`cargo build --release -p tohseno-cli`) plus the Swift
  `tohseno-apple-identity` helper, laid out installed-style
  (`~/tohseno-dogfood-1/bin/`), always run from OUTSIDE the repository.
- Isolation: `TOHSENO_DATA_ROOT=~/tohseno-dogfood-1/data` (plus separate
  probe roots); the user's real `~/.tohseno` and `~/Desktop/Tohseno`
  untouched.
- Harness: Claude Code, `claude-subscription` route, `$0.00` additional cost
  on every completion record. The harness ran as the real interactive TUI in
  a PTY; the only synthetic keystrokes were the ones a human would make
  (accepting the trust dialog, `/exit` after landing).
- Identity: `TOHSENO_IDENTITY_BACKEND=software-test` from the first Shot
  onward — because the default path fails closed (F-001).
- Shots created (`~/tohseno-dogfood-1/data/`):
  - `resist` (Shot A) — ShotID `0xe8b3…0444` (third folder; two earlier
    unlanded folders were consumed by F-002/F-003 and deleted), evolutions
    1–3, executions `4faad4ec…` (v1, landed), `168836a3…` (v3, landed), plus
    `565ed50b…`/`c5031…` (cancelled on earlier folders).
  - `alivetoday` (Shot B) — evolutions 1–1, execution `a5197756…` (landed),
    one reference image.
  - `stone` (Shot C) — evolutions 1–1, execution `091ae976…` (landed).
  - `dailyforge` (Shot D) — **not created**: blocked by the slot wall whose
    only exit, `tohseno retire`, waits forever for a physical iPhone
    (F-011 + F-012). Deferred until after repairs.
- Feedback records (all version-bound, signed, private): resist v1
  (`0x143596aa…`, with screenshot attachment), v2 (`0x7d2b5510…`), v3
  (`0x707a868a…`); alivetoday v1 (`0x85265ee3…`); stone v1 (`0xe30635c0…`).
- Evolution: resist evolved 0002 (surprise auto-record, see F-013) and 0003
  (real Evolution from the selected v2 feedback action; landed and verified).

## Chronological journey

1. **Entrance.** From a fresh shell in `$HOME`: `tohseno --help` reads as
   product language; `doctor` answered "Xcode is ready." in about a second;
   `list` gave a clean empty state; `shot harnesses` honestly showed Codex
   and Claude Code with `$0.00` subscription routes. `tohseno studio` served
   127.0.0.1:8888 immediately.
2. **First-run guide.** Studio's four-step FIRST SHOT guide is honest and
   grounded: real server-side Xcode and Apple-signing checks (both green),
   real harness detection. Step 4 closes with "This Mac is ready. The first
   protocol action will create your Secure Enclave-backed Builder identity."
3. **First wall.** The composer's PREPARE SHOT died: *"cannot create a secure
   BuilderID while contract generation 0.8.0 is inactive"*. The CLI failed
   identically. The onboarding's promise is contradicted by the engine — and
   by Studio's own left rail, which already knew: "No local signing identity
   exists… Studio will not create one." No next action was offered (F-001).
   Continued with the repo's own escape hatch,
   `TOHSENO_IDENTITY_BACKEND=software-test` — which no real user knows.
4. **Second wall.** PREPARE SHOT then created the Shot body and execution,
   but the Terminal handoff's AppleScript timed out in this headless context.
   Good: the error contains the exact manual command. Bad: Studio announced
   "Shot was not prepared" while the durable execution sat ready (F-002), and
   the attempted `create` retry hit "the resist folder already holds work —
   `tohseno evolve resist` records it", where the "work" was TOHSENO's own
   standing orders, and the suggested `evolve` would have failed too
   (F-003). Recovered the way a frustrated user would: deleted the unlanded
   folder and re-created.
5. **Shot A lands.** The real Claude Code session read
   `.tohseno/EVOLUTION_INTENT.md`, obeyed AGENTS.md, built a complete SwiftUI
   app, hit the anatomy gate once, understood the diagnostic, fixed the
   project, ran `tohseno evolve` itself, and landed 74 files as verified
   Evolution 0001 — `$0.00`, exit 0, `landed: true`, conformance evidence in
   the completion record. `tohseno verify resist`: 35 checks CONFORMANT.
6. **Contact.** Selecting the app card in Studio auto-built and streamed it
   into the embedded simulator panel. The app is exactly the intention: a
   count, a caption, one enormous green button. Tapped three times; killed
   it; reopened; the 3 survived. Feedback (with screenshot) was signed and
   bound to version 0001.
7. **Shots B and C.** `alivetoday` (one reference image; landed in 989s) and
   `stone` (landed after teaching itself the `apple.bundle_version` gate)
   both verified CONFORMANT. In use, `alivetoday` mirrors the reference
   mock, enforces one-answer-per-day by removing the writing surface, and
   persists; `stone` is a single stone that silently gets heavier
   (`{"weight":3}` survived relaunch) — the deliberately strange intention
   was NOT normalized into a product.
8. **Slot wall.** The fourth create refused cleanly: "Run `tohseno retire
   alivetoday` to free one iPhone slot." `retire` then polled forever for a
   wired iPhone that this loop never used (F-011+F-012). Shot D deferred.
9. **The evolution trap.** `evolve resist --feedback-action <v1 action>`
   first silently sealed Evolution 0002 ("recorded from the working tree.")
   because the folder no longer matched sealed 0001 — drift caused by the
   engine itself rewriting `TOHSENO/fascia.json`, `embedded-provenance.json`
   and the pbxproj version during sealing — and then rejected the selected
   v1 feedback as "not bound to the current exact expression Version"
   (F-013). The human's version-bound feedback was structurally
   unselectable.
10. **Evolution for real.** Following the current rules: fresh feedback on
    v0002, then `evolve` staged cleanly — `EVOLUTIONARY_INTENT.md` carried
    the current version + VersionID, the selected action commitment, and the
    exact intent text (raw observations / interpretation / intention
    preserved verbatim). A second staging attempt was correctly refused;
    Studio was killed and restarted mid-run and truthfully re-read
    `harness_running` from durable records. Evolution 0003 landed (62
    files), CONFORMANT.
11. **Evolved contact.** Same bundle, CFBundleVersion 3, data migrated. On
    the first open of a new day with a seeded yesterday of 5, a quiet gray
    "yesterday: 5" appeared under the caption, faded on its own, and could
    not be summoned back; today's count stayed the only standing number.
    New feedback was recorded against version 0003, separate from its
    parents'.

## Findings

Severity: P0 blocks the core loop for a real user; P1 breaks a promised path
with no in-product recovery; P2 misleads or strands state; P3 cosmetic.

### F-001 · P0 · Engine + Studio + CLI — fresh-machine first Shot fails closed at identity creation
- Repro: empty data root, no `TOHSENO_IDENTITY_BACKEND`; `tohseno create x
  --prompt-file f --accept-genome` (or Studio PREPARE SHOT).
- Expected: the guide's promise — first protocol action creates a Builder
  identity — or an honest, actionable refusal.
- Actual: "invalid identity configuration: cannot create a secure BuilderID
  while contract generation 0.8.0 is inactive…". Exit 1. No next action.
  Onboarding step 4 promises the opposite; the left rail admits the truth.
- Why it matters: the golden path is dead on arrival for every new machine.
  The repo's own E2E only passes because it exports the undocumented
  `software-test` hatch.
- Smallest credible fix: when no identity exists and the generation is
  inactive, mint the explicitly test-only local identity (with its existing
  disclaimers) — locally scoped, never public authority — OR make doctor and
  onboarding state the truth and print the exact hatch. Local-only lifecycle
  must not require chain activation. Regression test: create on a fresh root
  without the env var must succeed (or fail with the documented hatch in the
  message).

### F-002 · P1 · Terminal handoff + Studio — failed Terminal open reports "Shot was not prepared" though it durably was
- Repro: PREPARE SHOT where AppleScript automation is denied/times out
  (headless TCC; also any user who denies the automation prompt).
- Expected: "Shot prepared; Terminal could not be opened; run this command"
  as state-accurate guidance (the command IS included — good).
- Actual: "intake rejected: Shot was not prepared: Terminal could not be
  prepared…" while `execution.json` (phase Prepared) and the Shot body exist.
- Why it matters: the user's mental model of what exists diverges from disk
  at the exact moment they must recover; combined with F-003 the retry
  collapses.
- Fix: truthful wording + surface the runnable command as a first-class
  Studio affordance (copy button), not only inside an error string.
  Regression test: prepare with failing terminal-opener still yields
  phase=Prepared and the guidance names the existing execution.

### F-003 · P0 · Engine + CLI + Studio — a failed/cancelled FIRST execution cannot be retried by any surface
- Repro: prepare Shot; cancel the execution before landing; try anything.
- Actual: `create` → "the folder already holds work — `tohseno evolve
  <app>` records it" (the "work" is TOHSENO's own AGENTS.md/CLAUDE.md;
  `hash_expression_working_tree` does not exclude them); the suggested
  `evolve` → NothingToSeal (no xcodeproj); `shot run` → "already has a
  terminal outcome"; Studio create → same engine refusal.
- Why it matters: any first-run stumble (terminal denied, harness closed
  early, Mac slept) permanently wedges the Shot; the only exit is `rm -rf`
  and a new ShotID.
- Smallest credible fix: treat a folder containing only engine-written
  standing orders as pristine for `create` (or allow explicit re-preparation
  of the first execution when no version has landed); correct the misleading
  `evolve` hint before a first version. Regression test: create → cancel →
  create again succeeds with the same ShotID.

### F-013 · P0 · Engine — version-bound feedback can never seed the next evolution on the first try
- Repro: land any evolution; attach feedback to it; `evolve <app>
  --prompt-file f --feedback-action <that action>`.
- Actual: evolve first auto-records a surprise version ("recorded from the
  working tree.") because sealing itself rewrites working-tree files
  (`TOHSENO/fascia.json`, `embedded-provenance.json`, pbxproj
  CURRENT_PROJECT_VERSION substitution — confirmed by diffing sealed 0001 vs
  0002), then rejects the action: "not bound to the current exact expression
  Version".
- Why it matters: this severs the product's soul — contact belongs to the
  version the human used, and that binding is exactly what evolve refuses.
  Every real user hits this on their first evolution.
- Smallest credible fixes (any one restores the ceremony):
  1. validate the selected feedback actions BEFORE any recording side
     effect (fail fast, folder untouched);
  2. remove engine-caused drift so a landed folder matches its sealed
     version (write engine substitutions into the working tree before
     snapshot, and/or exclude `TOHSENO/fascia.json` the way
     `embedded-provenance.json` already is — protocol-sensitive);
  3. accept feedback actions bound to any ANCESTOR version of the same
     expression (still exact, matches human time).
- Regression test: land v1 → feedback v1 → evolve with that action succeeds.

### F-012 · P1 · Engine/CLI — `tohseno retire` waits forever for a wired iPhone in a phone-free loop
- Repro: `tohseno retire <app>` with no phone attached.
- Actual: infinite poll ("Plug in your iPhone with a cable."), app lock
  held, no timeout, no local-only path — even for apps never installed on
  any phone. This is the slot wall's only prescribed exit, so F-011→F-012
  chain makes a 4th app impossible.
- Fix: when the app has never been installed on a device (or a `--local`
  flag is passed), mark the ledger retired without requiring hardware.
  Regression test: retire simulator-only app completes offline.

### F-011 · P2 · Engine — 3-app slot wall fires for the phone-free loop
- The wall models the free-Apple-ID sideload limit, but counts simulator-only
  apps and speaks entirely in iPhone terms to a user who never plugged one
  in. Revisit whether simulator-only apps consume "iPhone slots"; at minimum
  the wall should say what the limit is for.
- Resolved for 0.8.5: accepted Shot records no longer participate in slot
  accounting. TOHSENO prefers a usable paid Xcode team. Only a positively
  identified free Personal Team can trigger the wall, immediately before a
  physical install, from the connected iPhone's structured app inventory;
  reinstalling the same bundle remains allowed.

### F-007 · P2 · Protocol/Engine — harness-local state is sealed into the signed source world
- `.claude/settings.json` (and whatever `settings.local.json` a real user's
  "don't ask again" produces) entered `evolutions/0001/src/` and the source
  commitment. `.claude` is not in `EXCLUDED_DIRECTORY_COMPONENTS` (protocol
  tree_hash). Harness config is user-local state, like `xcuserdata` (already
  excluded). Changing the exclusion list changes hashing law — needs a
  deliberate protocol decision; at minimum document it. (This same drift
  class feeds F-013.)

### F-009 · P2 · Engine/CLI — several creation failures strand a partial Shot folder
- Nine images: rejected BEFORE side effects ("no attachment was staged") ✓.
- But: missing image file → bare "No such file or directory (os error 2)"
  (no path named) with the folder already created; unsupported type and
  duplicate images → good messages, stranded folders; unavailable harness →
  good message, full Shot body stranded. Each stranded folder then trips
  F-003 on retry.
- Fix: validate reference sources and harness availability before folder
  creation (create already proves this is possible for the image-count
  gate), and name the offending path in IO errors.

### F-004 · P2 · Studio — composer first paint pairs harness "Codex" with Claude models and route
- The harness select showed Codex while model options (Sonnet/Opus) and the
  route ("Claude subscription · $0.00") belonged to Claude Code; the machine
  default (`config.toml` harness=claude) was not reflected. Coherent after
  manually selecting Claude Code.

### F-008 · P2 · CLI — `doctor` diagnoses less than the product needs
- It checks only Xcode. Studio onboarding checks Xcode + signing + harness;
  `evolve` will silently wait forever on missing signing; `create` will die
  on identity backend state (F-001). Doctor should cover signing, harness,
  and identity-creation viability — before generation, as the product
  promises.

### F-005 · P3 · Events copy — "The user confirmed the prepared Shot in Terminal" is asserted even for manual `shot run` from another shell.
### F-006 · P3 · Terminal handoff — `TERM_PROGRAM=iTerm.app` with iTerm absent yields an AppleScript compile error (-2741) instead of falling back to Terminal.
### F-010 · P3 · Studio copy — Shot panel says "Installed on iPhone" for simulator-only apps; `list` prints "signing profile unavailable" noise for the same; `evolve` closes with "Plug in your iPhone anytime…" in a loop that involves no phone.

## Moments of coherence

- **The Genome proposal.** The deterministic plan preserves the builder's
  exact words inside "Purpose", proposes restrained law, and is presented as
  "PROPOSED · NOT COMMITTED" with revision and digest. It clarifies without
  burying — in Studio and in the CLI's review alike.
- **The gates teach.** Twice, a real coding agent failed a gate
  (anatomy, bundle_version), read the diagnostic, fixed the project, and
  landed. The gate messages are good enough for an agent to learn TOHSENO's
  rules mid-session — the virus model working exactly as designed.
- **One truth, every surface.** CLI `shot follow` from a second shell,
  Studio's `/api/executions`, a mid-run Studio restart, and the completion
  record all read the same durable files and never disagreed.
- **Honest failure everywhere at the execution layer.** result-before-
  completion, cancel-while-running, cancel-after-interrupt (dead-PID
  detection → truthful Cancelled record), double-staging, double-create —
  every probe returned a clear refusal and usually one next action.
- **The apps are the intentions.** Three for three: a count and a button; a
  question and its archive; a stone. Nothing added, nothing explained away.
  STONE in particular proved the factory can carry personal weirdness
  without normalizing it.
- **Landed means landed.** `landed` is computed from canonical acceptance
  plus independent verification, deliberately outranking the wrapper's exit
  code; 35 conformance checks re-derive everything from disk.

## Conceptual fractures

1. **The ceremony contradicts its own bookkeeping (F-013).** Feedback
   belongs to the version the human touched; the engine's own sealing makes
   that version instantly non-current, and evolve refuses non-current
   bindings after silently minting a version the human never intended. The
   product's central sentence — "attach feedback to an exact version, evolve
   from it" — is currently false on first use.
2. **Onboarding promises what the engine forbids (F-001).** One surface
   says "this Mac is ready"; the engine and the left rail say identity
   creation is closed. Two truths, one screen apart.
3. **The Shot's own body counts against it (F-003).** Engine-written
   standing orders make every folder "hold work", so the engine mistakes its
   own scaffolding for a user's unrecorded app.
4. **Phone ceremony leaks into the phone-free loop (F-011/F-012, F-010).**
   Slots, retire, refresh nudges and "Installed on iPhone" copy all assume a
   device this loop never used.
5. **Agent droppings enter the signed world (F-007).** The sealed source is
   supposed to be the app; today it also contains the harness's local
   permission choices.

## What was actually exercised (and what wasn't)

Exercised: CLI from arbitrary directories and spaced paths; Studio
onboarding, composer, plan review, library, per-request /api/apps, execution
API, simulator launch/stream; native terminal handoff mechanics (AppleScript
attempted; manual-command recovery used); real Claude Code generations ×4
(three Shots + one Evolution) on the subscription route; agent-run `tohseno
evolve` sealing with real xcodebuild gates ×4; verify/inspect/list; feedback
with attachment ×5 across three apps and three versions of one app;
evolution staging with signed feedback selection; cancel/interrupt/result/
follow; Studio restart mid-execution; boundary probes (images count/type/
dupes/missing, unavailable harness, double-prepare, double-create).

Not exercised (and therefore not claimed): the curl installer (network
install of the pinned v0.7.1 — current main is ahead of that pin), physical
iPhone install/refresh/retire, export/import bundles, `page build`,
migrate/migrate-legacy, adopt, Bankr surfaces (deliberately untouched), the
share-card Shot D (blocked; deferred), and Phase 9's clean-workspace rerun
(pending, after repairs).

## Immediate repair list (Phase 8 input)

P0: F-001 (identity), F-003 (first-execution retry), F-013 (feedback →
evolution continuity — at minimum fail-fast validation order, ideally also
engine-drift removal).
P1: F-002 (truthful prepare-state wording), F-012 (phone-free retire).
P2 candidates if small and safe: F-008 (doctor coverage), F-009 (validate
before folder creation; name paths in IO errors), F-004 (composer default
coherence), F-010/F-005 (copy truthfulness).
Documentation: the software-test identity hatch, harness-local sealed state
(F-007), and the phone-slot policy for simulator-only apps.

---

## Addendum — after the repairs (Phase 8/9, 2026-07-31)

The P0/P1 list above was implemented in six commits on main (identity
fallback; standing-orders pristine check; feedback fail-fast + seal
mirroring; truthful terminal-failure prepare + `retire --local`; input
preflight + doctor coverage; composer coherence). Three further findings
surfaced while re-exercising the repaired loop:

- **F-014 · P1 · Engine** — a second Feedback for the same Version failed
  with an immutable-material conflict (the version index insisted on
  reading Absent). Fixed: an existing index naming the same exact Version
  is accepted in either status; regression test added.
- **F-016 · P0 (introduced and fixed within this cycle) · Engine** — the
  new seal-mirroring ran `substitute_shot_number` over the living folder,
  whose file walk recursed into `.tohseno` and advanced the pbxproj inside
  the previous SEALED snapshot, breaking its signed source commitment.
  `tohseno verify` caught the mutation — the immutability machinery works.
  The walk now skips `.tohseno`/`.git`; a regression test proves a sealed
  snapshot survives a living-folder substitution byte-for-byte; the one
  damaged byte was restored and independently re-verified CONFORMANT.
- **F-015 · P2 · Studio** — the library's `unrecorded_changes` flag used
  the raw protocol walk (which includes Shot-level surfaces), so it read
  true for every app forever, contradicting the CLI's "nothing new". Fixed
  to use the engine's expression hash.

Second clean-workspace run (`~/tohseno-dogfood-2`, no identity env var):
doctor → Studio → create `exhale` (identity minted by default) →
deliberate cancel → re-create with the SAME ShotID → landed 73 files →
"nothing new" immediately after landing (drift gone) → v1 feedback →
`evolve --feedback-action` staged FIRST TRY → Evolution 2 landed 61 files
→ soft-edged circle with a breathing background, matching the intent →
v2 feedback recorded separately → whole app CONFORMANT.

Final verdict and per-gate evidence: `docs/PRODUCTION_READINESS.md`.
