# ADR 0003 — The folder-shaped Shot

**Status:** accepted; terminal-launch behavior superseded by ADR 0005
**Date:** 2026-07-29

## Context

The candidate engine owned its apps: worlds lived at
`~/.tohseno-genesis/apps/<app>/shots/NNNN/src`, hidden, numbered, reachable
only through tohseno's own verbs. Field runs (`CLAUDE_FEEDBACK_1..3.md`)
showed what that costs. People who build with AI tools work *in a folder*:
they open it with Claude Code, the Claude or Codex desktop apps, Xcode, an
editor — and iterate. Against the old layout that natural motion was
impossible (the folder was hidden) and fatal (one out-of-band edit broke
`source.commitment` and the lineage refused to evolve — verified
empirically). The engine treated the builder's own hands as corruption.

The protocol never required any of that. An Evolution is a signed
commitment to a *state* — a complete source world and its parent — and says
nothing about who or what produced the state. The coupling of "evolution"
to "tohseno drove a sandboxed agent" was an engine choice, not law.

## Decision

Learn from git: the repository carries itself, and the tool is a verb you
run where you stand.

### Layout

```text
~/Desktop/Tohseno/                    ← the family home (TOHSENO_HOME)
└── emilia/                           ← the app IS a normal folder: the working tree
    ├── emilia.xcodeproj/ …           ← visible, editable by any tool
    ├── TohsenoFascia/  TOHSENO/  …
    └── .tohseno/                     ← the Shot carries its own ledger
        ├── app.toml                  ← ShotID, BuilderID, bundle id, latest seal
        ├── TASK.md  intent.md  fascia/   ← the briefing (private; never hashed)
        └── evolutions/
            ├── 0001/                 ← sealed complete world (src/, TOHSENO/ records,
            └── 0002/                    signature, conformance, artifact, logs)
```

```text
~/.tohseno-genesis/                   ← the machine home: what is truly machine-scoped
├── identity/                         ← BuilderID + DeviceKey (like ~/.gitconfig’s user)
├── config.toml  walls/  locks/
```

The machine home is no longer where apps live; it is an identity card and
preferences. `TOHSENO_DATA_ROOT` still points both homes at one directory
for isolation and tests. Apps are recognized by one marker: a directory
containing `.tohseno/app.toml` is a Shot.

### The seal is the event

`tohseno shot [<app>]` seals the working tree — however it got there — as
the next Evolution: snapshot (minus `.tohseno` and private files) →
anatomy gates → device-SDK compile → Simulator artifact → signed record →
conformance → immutable `evolutions/NNNN/`. Editing is not a tohseno
operation; *sealing* is. A person who forgets `tohseno evolve` and simply
runs their agent in the folder loses nothing: their end state seals as a
valid Evolution. Records remain complete worlds, never diffs — diffs are
derivable from two worlds; a world is not derivable from fragile diffs.

`tohseno evolve` (agent-driven) remains, and now refuses to run over
unsealed changes ("run `tohseno shot` first") so it can never destroy work
it did not make. After any successful seal or driven evolution, the working
tree is checked out to match the sealed world exactly, so cleanliness is
checkable by comparing the tree hash against the latest record.

### Conducted creation

Interactive `tohseno create <app>` composes the briefing into `.tohseno/`
and opens the builder's *own* detected agent in a new terminal window at
the folder — their login, their session, their normal flow. tohseno speaks
one line and steps back; `tohseno shot` completes the birth. The sandboxed
driven mode survives behind `--harness` (a known agent id or an absolute
path) for headless and automated use; ADR 0002's credential broker now
matters only there.

### Borrowed from git, deliberately

- **Directory-local invocation:** every verb walks up from the current
  directory to find `.tohseno`; app names become optional inside a folder.
- **No daemon, no registry:** each `tohseno` process spawns, reads the
  folder, does one job, exits. The family home is a convention, not a
  database — moving a folder moves the app.
- **Working tree vs. object store:** the folder root is the mutable
  present; `evolutions/` is the immutable past; `app.toml` is HEAD.
- **Portability as a property:** the folder carries identity, lineage,
  signatures, and worlds. Zip it, move Macs, hand it to another factory —
  "any compatible machine can continue a Shot" becomes physically true.

### Exclusion law

`.tohseno` joins the normative source-tree exclusions (with `.git`,
`.DS_Store`, `prompt.md`, `TASK.md`, build products). One shared predicate
governs hashing, snapshotting, and anatomy walks, so the working tree and
its sealed snapshot always agree byte-for-byte. Because sealed snapshots
carry a concrete `CURRENT_PROJECT_VERSION`, sealing rewrites that value to
the new sequence before the compile gate.

## Consequences

- The engine's `Ledger` maps to the new layout; `Shot` paths become
  `<app>/.tohseno/evolutions/NNNN` and all existing gates, records, and
  verification run unchanged on top.
- Out-of-band editing changes meaning: from corruption to *unsealed work*.
  `verify` reports an honest dirty state instead of a broken one.
- No automatic migration from pre-folder candidate ledgers; they were
  isolated candidates and can be re-created.
- Studio gains a truthful model for "unsealed changes" and can offer the
  seal as its primary action.

## Amendment — one Shot, many Evolutions; the agent records (same day)

Owner review corrected the ontology and collapsed the surface:

- **A Shot is the enduring intent behind an app.** One Shot per folder,
  permanent ShotID; every recorded state is an Evolution *of that same
  Shot*. All voices now say "evolution N of emilia", never "shot N".
- **The `shot` verb and "seal" vocabulary are retired.** Recording an
  Evolution is what `tohseno evolve` means: with nothing else, it records
  the folder's current state (recording is to worlds what `git commit` is
  to files); with a prompt it first records any out-of-band work, then
  drives a headless agent — no refusal states, no "unsealed changes".
- **The virus model.** The engine writes `AGENTS.md` (+ a `CLAUDE.md`
  pointer) into every world — the standing file agents auto-read. It
  carries the constitution and the closing habit: *record it yourself:
  `tohseno evolve`*. TOHSENO never drives the builder's agent; the ontology
  permeates whatever enters the folder.
- **MEMORY is the sixth organ.** `MEMORY.md`, inside the signed world, is
  the Shot's memory of its own becoming and current state; it absorbs the
  earlier `INTERPRETATION.md`. Agents read it first and update it last.
- Internal type names (`Shot` as the evolution-directory handle,
  `reserve_shot`, …) still predate this ontology; renaming them is
  mechanical follow-up work and changes no behavior.

## Second amendment — the minimal expression that just works (same day)

The historical launch details in this amendment are superseded by ADR 0005:
current preparation never executes a harness or adds permission-bypass flags.

- **The driven/sandboxed harness mode is deleted.** Every mode that is not
  essential is noise. TOHSENO finds the builder's agent, opens it in the
  builder's own session on the folder and steps back. This historical version
  used permission-bypass flags; ADR 0005 removes them and inserts an explicit
  human Enter boundary. The repair loop, the Seatbelt boundary, and the
  credential broker (ADR 0002) all vanish with the mode they served.
- **The CLI is minimal:** `create <name>` (the intake box also swallows a
  dropped prompt file and reference images), `evolve [--note]`, `adopt`
  (the current folder becomes a Shot), `refresh`, `list`, `verify`,
  `inspect`, `studio`, plus the protocol surface. No `--harness`, no
  `--prompt-file`, no modes.
- **References live privately and content-addressed at
  `.tohseno/references/<sha256>`;** dropped text files become the intention
  verbatim; exact reference descriptors and selected Feedback share the
  prompt-bound pending evolution state until the next recording carries them
  into signed history.
- **WORLD is the seventh organ** (story, design system, asset prompts in
  the signed world; `page build` renders it) and **every Evolution gets
  eyes**: a Simulator `preview.png` captured at recording time, shown by
  Studio and the static page.
- **The internal rename landed:** the engine and CLI now say `Evolution`
  everywhere (`reserve_evolution`, `latest_evolution`, …); only the
  protocol crate keeps its canonical `Shot*` schema names, which are
  correct — they name the Shot, not its evolutions.

### Notes from the second adversarial review

- Adoption pre-flights before any side effect (`<name>.xcodeproj` and
  `TohsenoFascia/` must exist) and re-roots a latest evolution that no
  longer verifies as an honest `LegacyAdoption` origin — which also gives
  worlds stranded by a fascia repin a way forward.
- The static page now names its own excerpts: WORLD.md's story and the
  recorded preview are the only world content it shows, and it says so.
- Accepted as candidate behavior: the fascia repin invalidates
  previously recorded evolutions (re-root via `adopt`); `--json verify`
  tag values follow the Evolution rename; the Simulator preview is a
  best-effort screenshot, not a deterministic artifact.
