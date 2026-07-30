# ADR 0005: Authentic local Shot execution

- Status: Accepted
- Date: 2026-07-30
- Extends: ADR 0003's visible folder and ADR 0004's coherent intention lineage

## Context

The engine already owns the visible Shot folder, private intention materials,
signed lineage, immutable accepted Evolutions, validation, and verification.
The CLI already owns terminal and loopback HTTP concerns, while Studio is a
view over those same engine APIs. The former harness handoff was insufficient:
it selected a binary, added permission-bypass flags, and asked Terminal to
execute it immediately. Its process-local prose events could not observe work
started from a separate Terminal process.

TOHSENO must orchestrate a person's installed coding harness without replacing
its native interface or weakening protocol acceptance boundaries.

## Decision

1. `engine::harness` is the adapter boundary. An adapter describes discovery,
   authentication when safely detectable, model choices, attachment behavior,
   inference/payment routes, cost-estimation capability, argument construction,
   and process-exit completion detection. Adapter commands never add
   permission-bypass or non-interactive flags.
2. Each preparation writes `.tohseno/EVOLUTION_INTENT.md` and deterministic
   `.tohseno/references/image_N.<ext>` aliases. Existing content-addressed
   private objects remain authoritative for digest verification and lineage.
   Input order assigns labels; original filenames never influence local paths.
3. `.tohseno/executions/<execution-id>/execution.json`, `events.jsonl`, and
   `completion.json` are private local orchestration records, not canonical
   lineage and not node-ingestible public actions.
4. Preparation records a Git tree object through an isolated temporary index.
   This creates no commit or ref and does not disturb the user's index. A
   repository without its own `.git` directory receives a private local Git
   repository, but no automatic commit.
5. The CLI opens Terminal (or a detected iTerm session) through a small zsh
   bootstrap. The bootstrap changes to the Shot repository and places
   `tohseno shot run --app … --execution …` in the editable line buffer.
   It does not execute that line. The person's Enter starts the execution.
6. `shot run` starts the selected harness as an ordinary interactive child
   with inherited stdin, stdout, and stderr. Studio never proxies the
   conversation and TOHSENO never scrapes arbitrary terminal output as its
   event protocol.
7. The wrapper records evidence-backed phases, independently compares final
   Git tree state with the prepared tree, and verifies an accepted Evolution
   through the existing verifier. Process exit alone never means “landed.”
8. Studio polls the same durable local events and completion record used by
   `tohseno shot follow` and `tohseno shot result`. The existing loopback,
   Host, origin, and mutation-header defenses remain unchanged.

## Consequences

- Claude Code and Codex keep their authentic permission, planning, question,
  and tool-use interfaces.
- A local subscription route can honestly show `$0.00` additional cost. API
  routes with no reliable preflight estimator are labeled usage-based rather
  than receiving an invented estimate.
- Pre-existing uncommitted files are part of the prepared Git tree and are
  never attributed to the execution diff.
- Harness summaries and actual billed cost remain unavailable unless a future
  adapter can obtain them through a structured, secret-safe native mechanism.
- Only one unfinished execution may own the mutable top-level intention package
  for a Shot at a time.
