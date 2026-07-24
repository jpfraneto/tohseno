# Things that are not clear for TOHSENO yet

The asymptote of this file is empty. Every entry is a decision that is not yet
made, or a promise that is not yet implemented. Delete entries as they resolve;
do not soften them.

---

## 1. The promise and the implementation do not meet

**Unclear:** README and positioning promise intent → production. `WHAT_TOHSENO_IS.md`
lists TestFlight submission, production deployment, and TokenMint as *Proposed*.
Nothing in `packages/cli/src/` owns the path from a built app to a live App Store
listing.

**Why it blocks:** this is the entire product claim. Everything else is polish on
a flow that currently stops at Simulator.

**Open:** which steps of the Apple path are automatable at all, and which are
irreducibly human (Developer Program enrollment, identity verification,
screenshots, privacy nutrition label, review). We do not yet have that map.

---

## 2. Owner steps do not exist as a concept

**Unclear:** `ShotPlan` (`packages/cli/src/planning.ts:32`) emits `definitionOfDone`,
which describes what the *app* must satisfy. There is no structure describing what
the *human* must do. That list is the one that actually blocks shipping.

**Open:**
- Schema for an owner step: id, version, digest, preconditions, ordering, one-line
  instruction, detector.
- Whether owner steps are selected by the planner from a pinned catalog (same
  discipline as app skills) or generated freeform. Freeform is unverifiable and
  rots — assume catalog until argued otherwise.
- Which steps have machine detectors (cert in keychain, bundle ID resolves,
  archive built) and which are manual checkboxes.
- How the catalog stays current when Apple changes its flow, independently of the
  CLI release. A six-month-old install giving confident wrong instructions is
  worse than no instructions.

---

## 3. Is TOHSENO a process, a command, or a skill

**Unclear:** today `tohseno` runs the creation flow and exits; only `tohseno studio`
starts the loopback server. The stated intent is that TOHSENO *is* a long-running
local process and every command is a client of it.

**Open:**
- Do we refactor so the server is the single holder of state and CLI/Studio are
  both clients? (This is what makes "one ladder, two projections" true rather than
  two implementations that drift.)
- Fixed port or ephemeral? (7015340 is not a valid port; max is 65535.)
- Lifecycle: who starts it, who stops it, what happens on a second invocation.

---

## 4. "Skill" means three different things

**Unclear:** the word is overloaded and will make the repo ungreppable:
1. `skills/*/SKILL.md` — app capability units composed into a generated app.
2. `packages/cli/factory/AGENTS.md` / `CLAUDE.md` — instructions to the coding
   agent working inside a shot.
3. TOHSENO itself as a skill installed into the user's own agent.

**Open:** distinct names for all three, decided before (3) is built. Also: is (3)
the primary distribution surface, replacing "install a CLI"?

---

## 5. Continuity is special by exception, not by design

**Unclear:** `skills/continuity-app/` has a `SKILL.md` but no `skill.json` — the only
one of five missing it. It is therefore not a resolvable composition unit; it is a
document. Meanwhile continuity is claimed as the most important idea in the system.

**Open:** specify it properly, or state plainly that it is documentation and not a
composable skill.

---

## 6. Creator identity: designed, not specified

**Decided:** identity is a keypair the creator holds (BIP39, as continuity-v1 already
had). Apps ship the public key and attest common authorship by signature. No
account, no TOHSENO registry, ejection intact.

**Open:**
- One key per creator, or a derivation path per app under one seed?
- Where does the seed live on the Mac, and what is the recovery story?
- Is creator attestation opt-in in the manifest (defaulting to none, alongside the
  existing `identity.strategy`), or assumed for every shot?
- What exactly does one app do with the knowledge that another app shares its
  author? The network property is asserted; the behavior is not designed.

---

## 7. Chain layer: boundaries not drawn

**Decided:** the key is the identity. A contract (TOHSENO.sol) is an *optional*
discovery index. Every app must work fully with zero chain access. Chains are
indices, never truth — which is what makes "works on every chain" hold.

**Open:**
- What the contract actually stores (creator pubkey → app records?) and who pays gas.
- RPC access is someone's server. Which one, and what happens when it is down.
- App Store review friction for crypto-touching iOS apps. Real, survivable, but it
  is an owner step with teeth and belongs early in the ladder, not at submission.
- Whether any of this ships before intent → App Store works end to end. Default: no.

---

## 8. Token

**Unclear:** the token is for the TOHSENO app, ticker tohseno.com, and should be
launched through TOHSENO itself. TokenMint is *Proposed*. A token launch is the most
irreversible external action the system could take, and doctrine forbids irreversible
external actions without explicit approval.

**Open:** sequencing. Assumed: after intent → App Store works end to end, through the
same approval rail as every other external action. Not before.

---

## 9. The model layer

**Unclear:** "formal wrapper around OpenRouter, sell credits, charge by intent, by cost,
by model" versus today's implementation, which shells out to exactly two pinned
providers (`codex`, `claude`) in `planning.ts:64`.

**Open:**
- Does the plan boundary become provider-agnostic, keeping composition/locks/verifier
  intact behind it? (Assumed yes — the process opinion is the moat, not the model.)
- Billing, credits, key custody, and what happens to the "no TOHSENO account" promise
  the moment we sell anything.
- What "unopinionated" means concretely: unopinionated about *what app you build*, or
  also about *how it gets built*. These are different claims and only the first is safe.

---

## 10. One-line UX: unspecified below the happy path

**Decided:** never more than one line. Banner once. Left/right binary choice.

**Open:**
- `io.ts` is 28 lines of readline `question()`. The raw-mode keypress primitive
  (`ask(line) → left | right`) does not exist yet.
- Failure output. Codesigning and Xcode failures produce hundreds of lines. Assumed
  rule: one line naming what broke, one copy-pasteable next step, full detail written
  to a path. Unproven.
- No ASCII art or banner exists anywhere in the repo. The typography is undecided.

---

## 11. Mascot

**Unclear:** a character carries the tone that one-line output cannot. Proposed rule:
the character never gets its own line — it is the voice of the line already being
printed, never an addition.

**Open:**
- Who owns the character. `TRADEMARKS.md` exists for a reason; the face of the product
  should not be rented.
- Form in a terminal: a few-character expression rendered from the existing state
  machine (`machine.ts`, `progress.ts`), not an illustration.

---

## 12. Names not claimed

**Unclear:** `tohseno` and the entire `@tohseno` npm scope are unregistered as of
2026-07-24. `packages/cli/package.json` declares `@tohseno/cli@0.4.0`, never published;
install.sh does all distribution today.

**Open:** claim both. Placeholder publish is fine. Names go exactly once.
