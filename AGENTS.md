# TOHSENO repository guidance

This file applies to the entire repository. A more local `AGENTS.md`, if one is added later, may narrow implementation details but must not weaken the privacy, ownership, or approval boundaries here.

## Mission and current status

TOHSENO is the fastest path from one prompt to an iOS app on a phone. A person runs the oneshot, hands their coding agent one sentence, and gets a working app — a **continuity app**: an app where cryptography replaces accounts. A BIP39 seed phrase instead of a signup form, a local log instead of a cloud profile. No auth screens, no OAuth, no email capture — not for the builder, not for their users.

This repository contains: the public site (hero, docs, privacy, the pinned oneshot bootstrap), the base app in `templates/continuity-app` (a compiling, running iOS writing app that every workspace starts from), the manifest schema and validator, the agent build protocol in `skills/continuity-app/SKILL.md`, and the check gate. The intake/payments product that once lived here is preserved on the `archive/intake-product` branch and is not part of main.

## Product constraints

- **Speed is the product.** Anything that adds a question, a config step, or a ceremony must pay for itself in reliability.
- **The base app is the starting point.** One-shots mutate `templates/continuity-app`, never an empty directory. It must always build from a fresh clone with only a signing-team selection, run in the simulator with zero keys, and survive process death without losing text.
- **The manifest is a reliability mechanism, not a moral one.** If a feature cannot be expressed as a valid manifest field, it is unsupported — say so instead of improvising. The builder decides the mechanics (streaks, paywalls, scores are tools, not sins); private-by-default and account-free are defaults, never refusals.
- **Modules are flags.** `AppConfig.swift` is the single configuration seam; a module integrates by flipping its flag, never by rearchitecting. SessionLink stays declared-only until it ships.
- **Ejectable from birth.** Every app builds and runs without TOHSENO credentials; every landing page ships in the same package as its app.

## Private data rules

Never commit or log owner prompts, contact details, credentials, tokens, message bodies, production data, or encryption keys.

- `MASTER_PROMPT.md` in a workspace is private product input: gitignored, never committed, echoed, or transmitted.
- Key slots hold public identifiers; setup writes key *paths*, never secret values. `.p8`/`.p12`/`.pem` files never enter git.
- Keep logs structured and content-free.
- App-runtime content stays on the person's device. This repository operates no backend for generated apps and must never grow one that receives their users' content.

## Architecture and implementation

- Use Bun for JavaScript and TypeScript, strict TypeScript, `Bun.serve`, raw HTML/CSS, and minimal browser JavaScript.
- The base app is SwiftUI with no third-party dependencies; an SPM dependency is acceptable only if it compiles offline with zero configuration.
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

1. Run focused tests, then `bun run check`.
2. If the base app changed: `xcodegen generate` (when project.yml changed) and the simulator test run must be green.
3. Run `git diff --check` and inspect tracked files for secrets.
4. Report limitations honestly, including exactly what was and was not verified in this environment.

### Release discipline for the oneshot pin

`apps/site/public/oneshot.sh` embeds `TOHSENO_PIN`, the exact rails commit every new workspace is created from. A release that changes the rails must land first; a follow-up commit bumps the pin to it — the pin always trails the serving commit by one. `bun run check` verifies pin ancestry and that the pinned commit contains the required base-app files. Site deploys go out with `railway up`, not by pushing to GitHub — but the pinned commit must be pushed to the public repository or the oneshot cannot fetch it.

## Documentation language

Use these words precisely:

- **Implemented:** exercised by a live code path and testable now.
- **Prepared:** configuration and instructions exist, but no external action occurred.
- **Proposed:** architecture or product behavior that is not implemented.
- **Open:** requires a product, security, or ownership decision.
