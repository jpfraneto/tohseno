# TOHSENO repository guidance

This file applies to the entire repository. A more local `AGENTS.md`, if one is added later, may narrow implementation details but must not weaken the privacy, ownership, or approval boundaries here.

## Mission and current status

TOHSENO is the fastest path from one intention to an independently owned iOS
app on a phone, then to the next shot. It is a local intention compiler and
open app factory: the selected coding agent interprets private intent into a
sanitized plan, the deterministic factory composes a neutral kernel, template,
and app skills, and the result is verified before it is presented as ready.
Account-free and private-by-default are starting choices, not a universal app
category or a refusal to implement declared mechanics.

This repository contains the reusable local-first CLI in `packages/cli`, the
shared composition engine in `packages/skills`, the public site, the neutral
iOS kernel in `templates/ios-kernel`, starting templates in `templates/blank`
and `templates/daily-game`, bundled app skills under `skills/`, generic and
legacy manifests, the local Studio, pinned machine rails, and the check gate.
`templates/continuity-app` and `skills/continuity-app` remain supported legacy
continuity-v1 inputs; they are not the default or the universal product model.

## Product constraints

- **Speed is the product.** Anything that adds a question, a config step, or a ceremony must pay for itself in reliability.
- **Composition is the starting point.** New shots deterministically compose
  an immutable neutral kernel, one template, and an ordered dependency-closed
  skill set. Blank is a real template, not an empty directory. Every bundled
  composition must build from a fresh release with zero credentials and run in
  Simulator without hidden TOHSENO services.
- **The manifest is a reliability mechanism, not a moral one.** If a feature cannot be expressed as a valid manifest field, it is unsupported — say so instead of improvising. The builder decides the mechanics (streaks, paywalls, scores are tools, not sins); private-by-default and account-free are defaults, never refusals.
- **Skills are deterministic capabilities.** A skill has a descriptor,
  dependencies, conflicts, owned files, acceptance checks, and a digest. The
  lock is authoritative; coding-agent prose cannot silently substitute for an
  installed capability. Legacy continuity modules retain their historical
  `AppConfig.swift` flag contract.
- **Ejectable from birth.** Every app builds and runs without TOHSENO credentials; every landing page ships in the same package as its app.

## Brand contract

- **“Take another one.”** is the brand line. TOHSENO makes ideas cheap to try,
  not disposable: every shot remains independently owned, and taking the next
  one stays easy.
- The mirrored `ONE SHOT` wordmark is a discoverable visual reversal. Do not
  explain the name in the landing-page hero.
- Put the builder and their idea in the spotlight. Prefer direct verbs such as
  take, make, run, ship, and continue over claims about TOHSENO itself.
- Be candid that most shots miss and that the prototype is the payoff. Never
  promise wealth or make financial mechanics, tokens, urgency, or speculation
  the reason to build. Describe only mechanics that are implemented now.
- Public voice is casual, generous, direct, and self-aware. Never use
  “revolutionary,” “unleash,” or “empower.”
- Public visual language is a darkroom: near-black, silver-halide grey, one hot
  signal color, mirrored type, repetition, and contact-sheet frames. Keep it
  raw and useful rather than decorative.
- The founder origin story does not belong on the landing page. Internal brand
  notes do not enter tracked public files.
- Never fabricate a shots-taken number. A local count must say what it counts;
  any communal counter remains unimplemented until it has a privacy-preserving,
  truthful source.

## Private data rules

Never commit or log owner prompts, contact details, credentials, tokens, message bodies, production data, or encryption keys.

- `MASTER_PROMPT.md` and `MASTER_EVOLUTIONARY_PROMPT.md` in a workspace are
  private product input: gitignored, never committed, echoed, or transmitted.
- Key slots hold public identifiers; setup writes key *paths*, never secret values. `.p8`/`.p12`/`.pem` files never enter git.
- A prototype provider secret in a legacy continuity-v1 shot may use only its `DEV_SECRET` seam in
  gitignored `Config/Local.xcconfig`, declared in the manifest's development
  secrets as the canonical `dev-secret` slot. It is for an owner-controlled
  Debug device only, is forced empty in simulator and Release builds, and must
  become short-lived TokenMint credentials before distribution.
- Keep logs structured and content-free.
- App-runtime content stays on the person's device. This repository operates no backend for generated apps and must never grow one that receives their users' content.

## Architecture and implementation

- Use Bun for JavaScript and TypeScript, strict TypeScript, `Bun.serve`, raw HTML/CSS, and minimal browser JavaScript.
- The neutral kernel and bundled templates are SwiftUI with no third-party
  dependencies; an SPM dependency is acceptable only if it compiles offline
  with zero configuration.
- Keep runtime dependencies and indirection small. Do not add a framework, ORM, component system, analytics SDK, or build system without a demonstrated requirement.
- Keep runtime-enforced manifest properties separate from coding-agent guidance and operator/deployment metadata.
- Prefer deterministic behavior at runtime. AI interpretation belongs between human intent and the manifest, not in storage, identity, or persistence invariants.
- Do not copy production code from any external application (including Anky or Auramaxxing repositories) into this repository or into generated apps. Documented contracts may be referenced; implementations are original.

## External actions

Do not create paid infrastructure, spend money, alter DNS, submit to an application store, rotate production credentials, deploy production, or publish packages without explicit owner approval. Preparing commands, configuration, runbooks, and dry-run validation is in scope. The fastlane `beta` lane is always prepared and printed, never executed unprompted.

## Change discipline

Before changing code:

1. Read this file and any nearer repository guidance.
2. Inspect the working tree and preserve unrelated work.
3. State which manifest property or product contract the change serves.
4. Check whether the change expands disclosure, ownership, cost, or external authority.

Before handing off:

1. Run focused tests, then `bun run check`. A changed manifest is validated
   with `bun run validate <path>` (the CLI gate; importing `validate.ts` or
   running it directly validates nothing).
2. If the base app changed: run `xcodegen generate` after changing
   `project.yml` or adding/removing/moving Swift files (the project is
   generated, not file-system-synced), and the simulator test run must be green.
3. Run `git diff --check` and inspect tracked files for secrets.
4. Report limitations honestly, including exactly what was and was not verified in this environment.

### Release discipline for the oneshot pin

`apps/site/public/oneshot.sh` is a thin compatibility delegator and must never
create a second kind of workspace or install unpublished CLI code. Release
ordering is part of the trust boundary:

1. land and push the frozen CLI source commit while the canonical installer
   still points at the previous release;
2. build the deterministic archive from that exact clean commit and publish
   the versioned GitHub release;
3. only then land and push a follow-up canonical-installer commit containing
   the published archive and internal-tree hashes;
4. only then land the serving commit that pins `oneshot.sh` to that installer
   commit and its exact installer-file hash.

The oneshot pin therefore trails its serving commit by one. Unpublished source
commits may sit between serving commits without changing the pin. `bun run
check` verifies the thin boundary, serving-commit parentage when the delegator
changes, pin ancestry, installer integrity, and public reachability. Site
deploys go out with `railway up`, not by pushing to GitHub, and no deploy occurs
without explicit owner approval.

## Documentation language

Use these words precisely:

- **Implemented:** exercised by a live code path and testable now.
- **Prepared:** configuration and instructions exist, but no external action occurred.
- **Proposed:** architecture or product behavior that is not implemented.
- **Open:** requires a product, security, or ownership decision.
