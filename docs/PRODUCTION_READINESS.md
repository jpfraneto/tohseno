# TOHSENO production readiness — controlled local alpha gate

> Historical note: this report records the 2026-07-31 local-alpha session.
> Current shipped and inactive state is in `docs/STATE.md`. The encrypted
> web-to-local handoff added on 2026-08-03 was activated later that day through
> the ordered record in `release/WEB_INTENTION_HANDOFF_ACTIVATION.md`.

Assessed 2026-07-31 at the end of the first complete dogfooding cycle
(`docs/DOGFOOD_REPORT.md`), after the Phase-8 repairs, against a second
clean workspace. Every claim below was verified on this machine in this
session; nothing is carried over from documentation or memory.

## Verdict

**READY FOR A CONTROLLED LOCAL ALPHA.**

Controlled means: a handful of technically comfortable macOS users, invited
individually, running a freshly pinned release with the fixes below, with
someone watching their first session. It does not mean public availability,
and one manual confirmation (the Terminal automation prompt, below) should
happen on a normal desktop before the first invite.

## Evidence, gate by gate

1. **Start from outside the repository.** Every command in both dogfood
   workspaces ran from `$HOME`, `/tmp`, or the Shot folders — never inside
   the checkout. Installed-style layout: `bin/tohseno`,
   `bin/tohseno-apple-identity`, `fascia/apple` beside `bin`.
2. **Environment failures are diagnosed before generation.** `tohseno
   doctor` now reports Xcode, Apple Development signing, usable harnesses,
   and identity state in ~1s. Reference images are proven stageable before
   any folder or identity exists; nine images, missing files, unsupported
   types, and duplicates all refuse with named paths and zero side effects;
   the CLI resolves the harness before intake.
3. **No hidden working-directory assumptions.** Fresh shells, arbitrary
   cwd, and a data root containing spaces all completed create → land.
4. **Studio and CLI operate over the same Engine truth.** The same
   `execution.json`/`events.jsonl`/`completion.json` observed via `shot
   follow` (second shell), `GET /api/executions`, and after killing and
   restarting Studio mid-execution. The library's `unrecorded_changes` flag
   now uses the same expression hash as `tohseno evolve` (it used to be
   permanently true).
5. **A Shot is preparable through the visible product flow.** Studio:
   guide → composer → plan review ("PROPOSED · NOT COMMITTED") → PREPARE
   SHOT. CLI: `create` with the same deterministic proposal. Five Shots
   prepared this way.
6. **Terminal handoff is understandable and recoverable.** The prepared
   execution is durable; when the Terminal window cannot open, prepare now
   succeeds with "SHOT PREPARED · … Run this command manually: …" and that
   command works from any shell. *Caveat:* in this automated session macOS
   never displayed the Terminal-automation consent prompt (headless TCC),
   so the happy path — window opens, command preloaded, user presses
   Enter — was verified only up to the AppleScript boundary. One human
   confirmation on a desktop is the remaining step.
7. **A real harness completes generations.** Five landings through real
   interactive Claude Code sessions on the `claude-subscription` route
   ($0.00 actual on every completion record): resist ×2 (v1, v3),
   alivetoday, stone, dailyforge, exhale ×2 (v1, v2).
8. **A successful result is a real buildable app.** All five apps built,
   installed, launched, and were used in the simulator; each matched its
   intention (a tap counter that persists and resets by local day; a
   one-answer-a-day journal; a stone that silently gets heavier; a daily
   constraint with a restrained share card; a breathing circle whose room
   learned to breathe in v2).
9. **Execution records survive restarts.** Files under
   `.tohseno/executions/` outlived Studio kills/restarts and binary swaps;
   a killed wrapper left phase+PID on disk and `shot cancel` detected the
   dead process and wrote a truthful Cancelled completion.
10. **Cancellation and failure remain truthful.** result-before-completion,
    cancel-while-running, cancel-after-interrupt, double-prepare,
    double-create, unknown feedback action, drifted-folder evolve — every
    probe refused honestly, almost always with one next action.
11. **The previous Evolution remains immutable.** Version records for every
    ordinal stayed inspectable and byte-verified. Strongest evidence: when a
    Phase-8 fix of mine accidentally rewrote one byte inside a sealed
    snapshot, `tohseno verify` caught it (`source.commitment` mismatch,
    `conformance.truth` false). The walk now skips `.tohseno`/`.git`, a
    regression test pins it, and the restored snapshot re-verified.
12. **Feedback belongs to a specific version.** Stored under
    `feedback/versions/<ExpressionID>/<ordinal>/<id>/` with signed action
    commitments and digest-named attachments; nine feedback records across
    seven versions of five Shots; a version now accepts more than one
    feedback (that was broken until this cycle).
13. **Feedback becomes a coherent evolutionary intent.** `evolve
    --feedback-action <commitment>` binds the exact current Version and the
    selected signed observations into `EVOLUTIONARY_INTENT.md`, validated
    BEFORE any side effect. Proven first-try in both workspaces after the
    drift repair; the protocol reducer independently enforces the binding.
14. **The same Shot evolves without losing identity.** One ExpressionID
    across resist 1→2→3 and exhale 1→2; the same ShotID survived a
    cancelled first execution and a re-create.
15. **Local lineage without public anchoring.** Contract generation 0.8.0
    stayed inactive throughout; every verification was offline; nothing was
    deployed, broadcast, published, or anchored.
16. **A second clean-workspace run succeeds.** `~/tohseno-dogfood-2`, empty
    data root, NO `TOHSENO_IDENTITY_BACKEND`: doctor → Studio → create
    (identity minted by default) → deliberate cancel → re-create (same
    ShotID) → land → launch → v1 feedback → evolution staged first-try →
    evolution landed → v2 feedback → all CONFORMANT.
17. **No unrelated repository work damaged.** The session added commits on
    main (report, fixes, docs); no unrelated file was reverted or deleted;
    the tree is otherwise clean.
18. **No production deployment occurred.** No push, no release, no site
    change, no App Store, no chain.

Automated suites at the final commit: cargo workspace 267 tests, Studio
static 10, Swift apple-identity 7 + fascia 9, forge 80 — all green.

## Remaining known risks

- **The Terminal automation prompt** (gate 6) has not been human-confirmed
  in this cycle. If macOS denies or times out, the product now degrades
  honestly to a copy-paste command, but the golden path deserves one real
  confirmation.
- **Agent discipline after sealing.** If a harness edits files after its
  final `tohseno evolve`, the folder drifts and the next evolution
  fail-fasts with guidance (record first, re-attach feedback). The standing
  orders now forbid post-seal edits; a misbehaving agent costs a junk
  version but can no longer orphan feedback silently.
- **Open P2/P3 findings** (documented in the report): harness-local
  configuration (`.claude/`) is sealed into signed source (needs a
  deliberate protocol decision); phone-centric copy and the 3-slot wall in
  a phone-free loop; the `EVOLUTIONARY_PROMPT.md` /
  `EVOLUTIONARY_INTENT.md` / `EVOLUTION_INTENT.md` naming trio; `list`'s
  "signing profile unavailable" noise; stranded-folder messages point to
  `evolve` before a first version exists.
- **The public installer still pins v0.7.1**, which predates every fix in
  this cycle. An alpha invite requires cutting and pinning a new release
  archive (release-operator ritual: build, checksum, update install.sh pin).
- The dogfood harness sessions granted the coding agent broad local
  permissions inside isolated Shot folders; alpha users' own permission
  choices will differ and shape how autonomous generation feels.

## Manual steps still required

1. One human run of PREPARE SHOT on a normal desktop, accepting the macOS
   automation prompt, pressing Enter in the opened Terminal.
2. Cut, checksum, and pin a new release (the existing release ritual) so
   the one-line installer serves these fixes.
3. The v0.7 retirement notice still needs to be added to the external
   release notes by a release operator (pre-existing item from STATE.md).

## Repeating the local alpha validation

```sh
cargo build --release
swift build --package-path apple-identity -c release
mkdir -p ~/tohseno-alpha/bin ~/tohseno-alpha/data ~/tohseno-alpha/fascia
cp target/release/tohseno apple-identity/.build/release/tohseno-apple-identity ~/tohseno-alpha/bin/
cp -R dist/genesis/fascia/apple ~/tohseno-alpha/fascia/apple
export PATH="$HOME/tohseno-alpha/bin:$PATH" TOHSENO_DATA_ROOT="$HOME/tohseno-alpha/data"
tohseno doctor
tohseno studio            # or: printf 'your intention\n' > /tmp/i.md
tohseno create myapp --prompt-file /tmp/i.md --accept-genome
# press Enter in the opened Terminal (or run the printed command manually)
tohseno verify myapp
tohseno --json feedback myapp --version 1 --text "what you noticed"
tohseno evolve myapp --prompt-file /tmp/evo.md --feedback-action 0x…
```

## The single most important next action

Cut and pin the new release from this commit and put it in front of one
invited human — watching their first Shot end to end, especially the
moment the Terminal opens. Everything else this product needs next will
come out of that contact, through its own feedback ceremony.
