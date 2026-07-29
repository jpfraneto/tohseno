# CLAUDE_FEEDBACK_2 — Dr. Ilan Weiss, theoretical physicist

**Persona.** Ilan does lattice QCD by day and distrusts every calculator they
did not derive themselves. They read `--help` before running anything, write
specs like referee reports, and their affection is earned in significant
figures.

**Description level.** Maximal — a 60-line referee-grade spec
(`--prompt-file`), with exact CODATA constants, a dimensional-analysis model
(everything reduces to powers of energy under ħ = c = k_B = 1), five numeric
acceptance checks, and explicit non-negotiables including "unit tests are the
point."

**Command.** `tohseno create planck --prompt-file planck-prompt.md`

---

## The run

### The identity wall does not care who you are (0:03)

Same first failure as the breathwork facilitator, verbatim:
`secure_enclave_unavailable`. The difference is what happens next: Ilan reads
the message, greps the repo, finds `TOHSENO_IDENTITY_BACKEND=software-test` in
the engine source, and continues — into a trap documented below. The variable
appears in no `--help`, no README, no error text. The persona most able to
climb this wall still had to read the vendor's source code to do it.

### The spec meets the genome, and the genome wins silently

The prompt's non-negotiable — "include unit tests for every conversion path;
the tests are the point" — collides with STRUCTURE's law: *one* iOS
application target. A test target is a second target. As the coding agent I
obeyed the genome and dropped the tests.

Nothing surfaced this. The builder's highest-priority requirement was
discarded without a word in any voice, log, or record. The genome needs a rule
for this exact moment: **when law overrides intention, the deviation must be
written down where the builder will read it.** (I validated the five
acceptance checks out-of-band — all pass, chain shown below — but Maya
couldn't have, and future-Ilan won't remember.)

```text
PASS 300 K → eV      0.02585199979
PASS 1 GeV → fm      0.1973269805   (reciprocal correspondence)
PASS 1 GeV⁻² → mb    0.3893793722
PASS m_e → MeV       0.5109989507
PASS m_e → kg        9.109383714e-31
PASS 1 eV → THz      241.7989242
```

A second, subtler point: the pipeline verifies *anatomy* (fascia, project
shape) and *buildability*, and verifies neither *behavior* nor *numbers*. For
this persona the entire product is the numbers. The acceptance checks sat
machine-readable in `prompt.md`; no gate will ever run them. Intent-level
acceptance is the missing organ: the genome could ask the agent to encode the
builder's checks in a deterministic, runnable form, and a gate could execute
them. Today "CONFORMANT" can be numerically wrong in every digit.

### The repair loop is real and it works (0:12)

I shipped a deliberate, realistic flaw (a wrong-case constant reference —
`hBarCInGevMeter`). The engine's response was the best moment of the run:

```text
building shot 1…
repairing shot 1 · pass 1 of 8…
  reading the build failure in TASK.md and repairing the project...
building shot 1…
committing shot 1…
```

Honest voice, bounded passes, the failure appended to TASK.md so the agent
sees exactly what the machine saw. Two refinements: the appended context is
the **raw 328-line xcodebuild log** (invocation preamble, build settings,
environment dump) when perhaps a dozen lines carry the error — noisy for the
agent and expensive for a metered one; and the instruction "fix only the
project, preserve the user's intent" is exactly right and deserves to be a
genome law rather than an inline string.

### Verification is all-or-nothing, and that erodes trust (0:15)

The shot passed generation, fascia anatomy, compile, and had its protocol
record prepared. The phone gate then blocked completion, and after that:

```text
tohseno inspect planck      → tohseno: app has no complete Shot
tohseno --json verify planck → tohseno: app has no complete Shot
tohseno registry show planck → tohseno: app has no complete Shot
```

There is a `shot.json` on disk with real commitments in it. The tooling that
exists to build confidence refuses to discuss it. For the trust-motivated
persona this reads as: *the system will not show its work* — the precise
failure the app itself was specified to avoid. `verify` on an incomplete shot
should verify everything that exists and state exactly which gates remain.

### What the honest commands get right

`network status` is the best surface in the CLI: per-check ✓/×/– with reasons,
a live P256VERIFY probe, and the sentence "the candidate is not ready or
remains undeployed." No inflation. `protocol info` and `identity devices` are
equally plain — the device listing even prints `software_test · TEST ONLY ·
replacement and revocation unavailable`, which is admirably blunt.

But blunt honesty without causal explanation creates a cliff of its own: the
candidate *forced* software-test on this Mac (wall 1), and software-test keys
"cannot authorize protocol records or public actions" — so signed history,
the protocol's central promise, is structurally unreachable here, and no
command ever says so in one sentence. The chain "ad-hoc-signed helper → no
Secure Enclave → test-only key → no signable Shots → no publishable lineage"
exists only in my notes. The system should be able to tell its own tragedy.

### Small true things

- `--prompt-file` is verbatim, exactly as promised. Respected.
- The fascia's own reference source emits Swift-6 concurrency warnings
  (`InstallationIdentity.prepare()` actor isolation) into every generated
  project. Normative sources should be warning-free — they are the one piece
  of code every world inherits.
- The device wall archives failed attempts under `incomplete/` — good — but
  each retry re-burns the *same* shot number's directory while `list` shows
  nothing about any of it.
- CFBundleVersion = Evolution number survived every gate; the token
  substitution (`__TOHSENO_SHOT__`) is clean.

## What I would change (carried into the evolution pass)

1. **A deviation record.** When the genome forces the agent to override the
   builder's stated intention, the genome must require a written note in the
   shot (visible in `inspect`) naming what was dropped and why.
2. **Behavioral acceptance as a gate.** Let intentions carry machine-checkable
   claims; have the genome instruct the agent to encode them, and a
   deterministic gate run them. Anatomy conformance is necessary, not
   sufficient.
3. **Partial verification.** `verify`/`inspect` must speak about incomplete
   shots — per-gate ✓/×/pending exactly like `network status` does.
4. **Trim repair context.** Extract the error lines (xcodebuild makes them
   greppable) with the full log referenced by path, not inlined.
5. **Causal honesty.** One sentence connecting identity backend → signing
   capability → protocol reachability, shown at `create` time when the
   backend is degraded.
6. **Warning-free fascia sources**, verified in CI alongside the byte-identity
   checks.

## Scorecard

| Moment | Felt like |
|---|---|
| Writing a real spec into `--prompt-file` | Being taken literally, which is all I ask |
| Identity wall | Solvable only by reading their source — a locked side door |
| Genome vs. my non-negotiables | Overruled without a hearing |
| Repair loop | The machine at its best: bounded, honest, effective |
| `verify` on my almost-shot | "No complete Shot" — show me *something* |
| `network status` | The one command I'd cite in a paper |
| The app itself | Ten significant figures and shown work — correct, verified by hand |

---

## Addendum — what this run already changed

- `verify` and `inspect` now speak about unfinished apps: a per-stage
  ✓/– progress report of the newest attempt replaces the bare
  "app has no complete Shot" (`cli/src/protocol_commands.rs`).
- Repair context is distilled to the error lines with the full log referenced
  by path (`engine/src/genome.rs::distill_failure`), and the repair
  instruction is now genome law (`genome/LAWS.md`).
- The deviation record exists: `genome/LISTENING.md` requires
  `src/INTERPRETATION.md` with a deviations section — "a deviation that is
  not written down did not happen honestly."
- Test-only DeviceKeys may now sign *local* records (marked "TEST ONLY …
  never publishable" in the conformance receipt) and still can never
  authorize public actions (`engine/src/builder_identity.rs`), so the Mac
  that forced software-test is no longer excluded from the protocol's local
  half.
- Behavioral acceptance as a deterministic gate remains open — it needs a
  fascia-level design (a second target is still unlawful), and is recorded as
  future work rather than half-shipped.
