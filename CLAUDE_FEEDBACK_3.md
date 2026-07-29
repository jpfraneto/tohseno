# CLAUDE_FEEDBACK_3 — Rosa, three weeks postpartum

**Persona.** Rosa is home with Emilia, three weeks old. She sleeps in
90-minute fragments. She has a Mac from her old job and about ten minutes of
attention at a time, usually with one hand. She is not going to read anything.

**Description level.** Medium — four run-on sentences typed fast, lowercase,
in Spanish, with her daughter's name in them:

> necesito algo super simple porfa. son las 3am y nunca me acuerdo de que lado
> le di pecho la ultima vez ni a que hora. un boton gigante izquierda y
> derecha, que me diga cual toca ahora y hace cuanto fue la ultima. nada mas.
> que se vea de noche sin quemarme los ojos. mi bebe se llama emilia

**Command.** `tohseno create emilia` — she named the app after her daughter.
Nobody designs for that, and everybody does it.

---

## The run

### The walls don't care that she has ten minutes

Rosa's stock attempt died exactly like the others: identity helper first
(`secure_enclave_unavailable`), then the Claude harness
(`EEXIST … /tmp/claude-501`, and structurally, no credentials inside the
sandbox). Maya might try again tomorrow; Ilan greps the source. Rosa closes
the lid. **Wall tolerance is a persona property, and this persona has zero.**
Every failure mode earlier documented is, for her, simply the end.

### The machine, when it runs, is nearly instant

With the generation step subtracted (bring-your-own agent), the entire
machinery — intake → identity → genome → fascia validation → device-SDK
compile → protocol record — took **about ten seconds**. This matters: the
pipeline is not the slow part. The slow parts are the walls, and the walls
are all at the edges (identity, agent auth, phone). Fix the edges and this is
genuinely a 3am-compatible tool.

### The genome is silent about her language

The prompt is Spanish. TASK.md wraps it in English law and never says a word
about language. As the agent I built the app in Spanish ("ahora toca
izquierda", "deshacer última") because anything else would be absurd — but
nothing in the genome asked me to, and a less attentive agent would have
shipped her an English app. **The builder's language is part of the builder's
intent.** The genome should say so.

### Her daughter's name went into the machine

"mi bebe se llama emilia" — a name typed in trust. The system's actual
behavior is exemplary: the prompt is private by default, stays in the local
ledger, never leaves the machine, and publication would be a separate signed
choice. But the system never tells her that, and this is the persona who
deserves the sentence most. One line at intake — "what you write here stays
on this Mac" — converts silent virtue into felt trust.

### The MASTER_PROMPT.md landmine

Running `create` from a directory that happens to contain a file named
`MASTER_PROMPT.md` triggers:

```text
Press y to use MASTER_PROMPT.md or n to type this shot.
```

Interactively, Rosa meets a question she cannot parse (what is
MASTER_PROMPT.md? why would tohseno feed a random file to her app?). With
piped input it is worse: the confirm consumed the first line of the actual
intention as a y/n answer, silently corrupting the prompt. A convenience
built for the repo's own genesis workflow leaks into everyone's kitchen. It
should be an explicit flag (`--prompt-file`), not ambient filesystem magic.

### The cliff has a second cliff behind it

Suppose Rosa reaches `Plug in your iPhone with a cable.` and actually plugs
it in. What the code then requires from a first-time phone (verified in
`gates/device.rs`): Trust prompt → Developer Mode toggle → **phone restart**.
A phone restart, at 3am, for the parent whose phone is also the baby monitor,
white-noise machine, and only clock. None of this ceremony was announced
before it started; each step is revealed only when the previous one is
performed. A first-run sentence — "the first install needs your phone, a
cable, and one restart; after that it's automatic" — lets her choose *when*
to pay that cost instead of discovering it mid-commitment.

And after the ceremony: a free Apple ID signature **expires every 7 days**.
Her 3am lifeline dies weekly unless she reopens the Mac and runs
`tohseno refresh`. `refresh` is the right verb with the right behavior — but
the expiry future is disclosed once, as an upsell line, not as the honest
shape of her coming month. For this persona, silent weekly death of the app
is the single worst property of the entire system.

### She can never evolve what never finished

A week later Rosa wants diapers counted too:

```text
tohseno evolve emilia
→ tohseno: emilia has no complete shot to evolve
```

Because shot 1 never met a phone, the app is frozen at zero forever. The
incomplete state poisons the future: no evolve, no verify, no inspect, no
registry, no page. One missed gate at the end of night one turns the whole
lineage into a dead branch — with no path back except starting over.

### The app itself

True-black background, two enormous warm-toned buttons, "ahora toca
izquierda", "última: izquierda · recién", "hoy: 1 toma", an underlined
"deshacer última" for one-handed misstaps. SwiftData for the feed log (the
genome's storage law routed this correctly), no seconds anywhere (nobody
needs seconds at 3am), Reduce Motion irrelevant because nothing moves.
Verified working in the simulator — which is exactly where Rosa will never be
allowed to see it, because the simulator library only opens for *completed*
shots, and completion needs the phone.

## What I would change (carried into the evolution pass)

1. **The first sentence of the product should be the price list.** Before
   anything runs: this machine needs Xcode, a signed-in Apple ID, your
   iPhone with a cable, one Developer-Mode restart, and (on a free Apple ID)
   a weekly refresh. Say it once, up front, in one breath — then never again.
2. **Let the simulator count as a birth.** A shot that builds and runs in
   the simulator should be *complete-on-Mac* — verifiable, inspectable,
   evolvable — with the phone install as a later, resumable step
   (`refresh` already is that verb). The current model makes the phone a
   constitutional organ when it is really a destination.
3. **Genome: honor the builder's language** — the app speaks the language
   the intention was written in, unless asked otherwise.
4. **Say the privacy sentence at intake.** "This stays on your Mac" costs
   five words and is worth more to Rosa than the entire whitepaper.
5. **Kill the MASTER_PROMPT.md ambient magic.**
6. **Weekly expiry deserves a plan, not a footnote** — at minimum, `list`
   and Studio should show "dies in N days" per app (the data already
   exists: `days_until_expiry`), and `refresh` should be suggested at the
   moment it matters.

## Scorecard

| Moment | Felt like |
|---|---|
| Typing four sentences at 3am | The product's one true magic — if it worked |
| Identity/harness walls | The lid closes; there is no second attempt |
| Pipeline speed (walls removed) | Ten seconds; genuinely 3am-compatible |
| Spanish in, Spanish out | Luck, not law — the genome never asked |
| Her daughter's name in the prompt | Kept private by architecture, unannounced by product |
| Device ceremony + weekly expiry | The hidden invoice, delivered in installments |
| `evolve` refusal | The dead branch: one missed night, frozen forever |
| The app | Exactly "nada más" — and she'll never see it |

---

## Addendum — what this run already changed

- The price list is now the first sentence of the product: a one-time
  "first shot:" status names Xcode, the Apple ID, the cable, the Developer
  Mode restart, and the weekly refresh before anything runs
  (`engine/src/machine.rs`).
- The privacy sentence is spoken at intake: "what you write here stays on
  this Mac; publishing is a separate signed choice." (`cli/src/intake.rs`).
- The MASTER_PROMPT.md ambient magic is gone; `--prompt-file` is the only
  file path into an intention.
- **The simulator counts as a birth.** Rosa's exact run now ends in
  `shot 1 of emilia is complete and verified on this Mac.` — 22 seconds —
  followed by a calm, non-blocking `Plug in your iPhone anytime and run
  \`tohseno refresh emilia\`…`. Her week-two evolution
  (`pañales`) completed as a verified Evolution 2 whose running app still
  remembered her shot-1 feed log. The dead branch is gone.
- The genome now honors the builder's language as law
  (`genome/LISTENING.md`).
