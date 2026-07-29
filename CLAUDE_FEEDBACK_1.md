# CLAUDE_FEEDBACK_1 — Maya, breathwork facilitator

**Persona.** Maya guides breathing circles in a rented studio. She is not technical.
A friend told her "you describe an app and it appears on your phone." She has a
MacBook, an iPhone somewhere in her bag, and twenty minutes between sessions.

**Description level.** The minimum viable intention — one breath of text:

> an app that breathes with me. a circle that grows and shrinks so my clients
> can follow inhale, hold, exhale, hold.

**Command.** `tohseno create tide` (Claude Code selected as the agent).

---

## What actually happened, in order

This run was executed for real against the working tree (debug build of the
GENESIS candidate CLI, fresh data root). Every wall below is a verbatim
transcript, not a thought experiment.

### Wall 1 — identity fails before anything begins (0:03)

```text
using Claude Code.
preparing your TOHSENO identity…
tohseno: Apple identity helper failed (secure_enclave_unavailable):
Secure Enclave P-256 is unavailable; use software-test only for CI or testing
```

The very first promise — "your identity is already there" — broke in three
seconds. The helper is an ad-hoc, linker-signed CLI binary
(`flags=0x20002(adhoc,linker-signed)`), and macOS will not grant such a binary
Secure Enclave / data-protection keychain access. This will reproduce on any
source-built machine, and probably on the packaged release unless the helper
ships Developer-ID-signed with proper entitlements.

Maya reads the words "Secure Enclave", "P-256", "CI" and closes the terminal.
The sentence speaks the *builder's* language, not hers, and offers her no door.

### Wall 2 — `create` was broken for every brand-new app (0:04)

With the software-test backend, the next attempt died with:

```text
preparing your TOHSENO identity…
tohseno: No such file or directory (os error 2)
```

Root cause: `Ledger::load_app` required the app directory to exist before it
could report `AppMissing`; a brand-new app therefore surfaced a raw io error,
and `Engine::create` only tolerates `AppMissing`. **`tohseno create` could not
create any new app at this source revision.** Fixed in this pass
(`engine/src/ledger.rs`, regression test added). The deeper lesson: nothing in
CI ever ran the one command the product is named after.

### Wall 3 — the signing gate rejects a fully signed-in Mac (0:05)

```text
Open Xcode → Settings → Accounts and sign in with your Apple ID.
```

…printed forever, on a Mac that *is* signed in, with a valid
"Apple Development" certificate. `development_team()` parsed the certificate
CN's parenthetical suffix (`9VZLT45T68`) as a Team ID — it is a certificate
label, not a team; the real team lives in the subject OU (`84V63LKV45`). It
also filtered Xcode's known teams to free personal teams only, locking out
anyone whose only membership is a company team. Both fixed in this pass
(`engine/src/gates/sign.rs`): team IDs now come from certificate OUs and any
Xcode-known team qualifies.

Felt experience: an instruction that is *wrong* and *repeats forever* is the
most corrosive kind of wall — Maya would obediently sign in again, see the same
line, and conclude she is the problem.

### Wall 4 — the frozen Claude invocation has bit-rotted (0:07)

```text
writing shot 1 of tide…
  Error: Invalid MCP configuration:
  mcpServers: Invalid input: expected record, received undefined
tohseno: harness exited unsuccessfully (Some(1))
```

The engine passes `--mcp-config {}`; current Claude Code requires
`{"mcpServers":{}}`. Fixed in this pass (`engine/src/harness.rs`). But the
category matters more than the instance: the harness contract is a frozen
argument vector aimed at a fast-moving third-party CLI. It will rot again.

### Wall 5 — the sandbox guarantees no agent can ever work (0:09)

Next attempt: sandboxed Claude died on `EEXIST: file already exists, mkdir
'/tmp/claude-501'` (a host-owned scratch dir it cannot use inside Seatbelt).
But behind that incidental failure is a structural one, stated plainly in
`engine/src/harness.rs` itself:

> Provider authentication therefore deliberately remains unavailable until it
> can be supplied by a narrow credential broker; host credentials must never be
> staged into this sandbox.

Fresh HOME, cleared environment, Keychain denied. Claude, Codex, Grok,
OpenCode, Hermes — none can authenticate inside this boundary. **The GENESIS
candidate cannot generate an app for any user, with any agent, on any Mac.**
The printing press has no ink. Everything downstream of this line is
unreachable product.

To keep exploring the machine honestly, I used the harness's own generic mode:
I authored the app myself as a TASK.md-compatible agent (the CLI previously
made a custom agent unreachable — `choose_harness` silently overrode any
custom configuration with the first installed known agent; `--harness` now
accepts an absolute path, added in this pass).

### The machine between the walls is genuinely good (0:15)

```text
using your own coding agent at …/tide-agent.sh.
preparing your TOHSENO identity…
preparing shot 1…
writing shot 1 of tide…
  reading TASK.md and writing the complete tide world...
  complete project is in src/
building shot 1…
committing shot 1…
Plug in your iPhone with a cable.
```

Once a world exists in `src/`, the pipeline is quietly excellent: fascia
anatomy verified byte-for-byte, the closed `TohsenoFascia/` inventory enforced,
device-SDK compile gate passed, protocol record written. The four voices are
calm and honest. This stretch feels like the product the whitepaper describes.

### Wall 6 — the phone is a cliff, not a gate (0:16 → forever)

`Plug in your iPhone with a cable.` blocks eternally. There is no:

- simulator path ("see it on your Mac while you find your cable");
- deferral ("your world is safe; run `tohseno refresh tide` when the phone is
  here");
- graceful exit (Ctrl-C leaves an `incomplete/` attempt that `tohseno list`
  reports as "tide · no complete shots" — five attempts of Maya's evening,
  summarized as nothing).

The Studio has a simulator library — but it can only display *completed*
shots, and completion requires the phone. On a phone-less Mac the simulator
feature can never show anything. The one machine that can already run her app
(this Mac) is forbidden from showing it.

## What the app turned out to be

I built what the one-line intention asked (screenshots in the run archive):
a single calm screen, a teal circle that grows over the inhale and settles on
the exhale, phase word in the center, a quiet pace menu (Box 4·4·4·4,
Deep 4·7·8, Gentle 5·5), Reduce Motion honored, pattern stored in
`@AppStorage`, `InstallationIdentity.shared.prepare()` on first launch. It
builds and breathes in the simulator.

One authentic defect survived every gate: the phase word crossfades over the
*whole four-second breath*, so "exhale" and "hold" overlap illegibly
mid-transition. No gate can see it — the loop builds the app but never looks
at it. **The pipeline has hands and no eyes.**

## What the genome felt like from the agent's seat

- LAWS/STRUCTURE/TASTE compose into a clear, honest TASK.md. As an agent I
  never wondered what the *plumbing* wanted.
- But the genome says nothing about what to do with an intention this thin.
  Maya gave one sentence; every product decision (presets? countdown? sound?
  session log?) was mine alone. The genome governs the body and is silent
  about the soul: no guidance to honor the *register* of the prompt (her words
  were "breathes with me" — the app should feel like an exhale, not a tool).
- TASTE is eleven lines of "don'ts" plus fonts. It never says: find the one
  gesture that *is* the app and make everything else disappear.
- Nothing tells the agent to leave a trace of its interpretation ("I read your
  sentence as X; I chose Y") — the builder gets a finished world with no
  account of the choices inside it, which will matter at evolve time.

## What I would change (carried into the evolution pass)

1. **Ink before press.** A credential broker so real agents can work inside
   the sandbox (localhost proxy holding provider auth outside the boundary is
   the narrowest honest design), or an explicit, visible "trusted harness"
   mode. Until then the candidate should say plainly at `create` that it
   cannot generate yet — not fail five ways in sequence.
2. **Mac-first materialization.** Let a shot reach a *seen, verified* state in
   the simulator without a phone; make the cable a deferrable step
   (`tohseno refresh` already exists and is the natural resume verb).
3. **Speak the builder's language at walls.** Every handoff line should name
   the world, not the cryptosystem ("tide is safe; I couldn't reach your
   phone" instead of eternal silence under an imperative).
4. **Give the loop eyes.** A screenshot gate after build — even just archived
   in the shot for the builder — turns invisible experiential defects into
   evidence.
5. **Genome: govern interpretation, not just anatomy.** A fourth genome file
   about listening — how to read a thin intention, what to preserve of the
   builder's words, what to record about choices made on their behalf.

## Verbatim scorecard

| Moment | Felt like |
|---|---|
| `tohseno create tide` one-liner | The right promise — one breath in, one app out |
| Identity preparation | Cryptography error in a meditation app's face |
| Brand-new app creation | Broken at this revision (now fixed) |
| Signing gate | Wrong instruction, repeated forever (now fixed) |
| Claude harness | Bit-rotted flags, then structurally credential-less |
| Fascia/build/commit gates | Quiet, exact, trustworthy — the best of the system |
| iPhone gate | A cliff with no rope back |
| `tohseno list` afterwards | "no complete shots" — my evening, erased |

---

## Addendum — what this run already changed

Landed in this repository during the same mission:

- `tohseno create` works for brand-new apps again (`engine/src/ledger.rs`, with a regression test).
- The Apple signing gate reads certificate OUs and accepts company teams (`engine/src/gates/sign.rs`); this Mac now passes it.
- The Claude invocation's bit-rotted `--mcp-config {}` is fixed (`engine/src/harness.rs`).
- `--harness /absolute/path` brings your own TASK.md-compatible agent (`cli/src/intake.rs`, `engine/src/machine.rs`).
- **The Mac is enough:** completion no longer requires the phone. A Shot finalizes after a Simulator-artifact build, and the device gate became a non-blocking offer resumed by `tohseno refresh` (`engine/src/machine.rs`, `engine/src/gates/build.rs`).
- The genome grew a fourth organ, `genome/LISTENING.md`, and TASTE now warns about text riding long animations — the exact defect this run's app shipped with.
- The credential broker that would let stock agents print again is designed in `docs/adr/0002-harness-credential-broker.md`; until it exists the harness failure explains itself honestly.

Proof: a fresh persona run after these changes reached
`shot 1 … is complete and verified on this Mac.` in 22 seconds, verified
CONFORMANT, and evolved to a verified Evolution 2 — all without a phone.
