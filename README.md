# TOHSENO

Open rails for **continuity apps**: software built around one meaningful action that a person returns to over time.

```text
act → record → reflect → continue
```

TOHSENO is the framework — the bones. It does not generate your app; your coding agent does. This repository gives that agent the contract, guardrails, templates, and refusal rules to build a continuity app that is private by default and ejectable from birth.

## Start here

One command, from any machine with git and a coding agent:

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
```

It clones these rails at an exact pinned commit, asks whether you want to start blank or from a shipped working example (`anky` or `daily-observation`), creates a fresh app workspace, and prints the one command to hand your coding agent. It accepts no secrets, sends no telemetry, creates no accounts, and deploys nothing. Read it first if you prefer: `curl -fsSL https://tohseno.com/oneshot.sh | less`.

Then the loop is:

1. **Hand off.** Start your coding agent in the workspace: *read `AGENTS.md` and begin.*
2. **Distill.** If you started blank, the agent does not build the placeholder prompt — it interviews you (one question at a time) about the one repeated action your app protects, then writes `MASTER_PROMPT.md` with you and gets your confirmation. If you started from an example, `MASTER_PROMPT.md` and the manifest are already complete and the agent builds immediately.
3. **Build inside the rails.** The skill walks the agent through interview, manifest, privacy inventory, smallest offline vertical slice, invariant tests, deployment preparation, and the ejection package. The manifest schema in `packages/manifest` is the boundary: a feature that cannot be expressed as a valid manifest diff is unsupported, not quietly improvised.

## If you are a coding agent reading this

You are the compiler. Read, in order:

1. [`AGENTS.md`](AGENTS.md) — the repository contract: product constraints, private-data rules, approval boundaries.
2. [`skills/continuity-app/SKILL.md`](skills/continuity-app/SKILL.md) — the build protocol you follow step by step.
3. [`packages/manifest/continuity.manifest.schema.json`](packages/manifest/continuity.manifest.schema.json) and its validator — every supported feature is a property here.
4. [`templates/continuity-app/`](templates/continuity-app/) — the workspace you start from; [`examples/`](examples/) shows two materially different filled-in manifests.

Never create paid infrastructure, alter DNS, submit to stores, rotate credentials, or deploy production without the owner's explicit approval. Never commit or transmit the owner's `MASTER_PROMPT.md`, credentials, or capabilities.

## What a continuity app is

One primary action with observable start, completion, and interruption. Progress recorded locally, useful without a network or model provider. Reflection that helps the person recognize what happened. A respectful return path. No dashboards-as-homepage, no feeds, no manipulative streaks, no account ceremony before first value. The full rules are in [Doctrine](docs/DOCTRINE.md).

## The paths

| Path | Price | What it is |
|---|---:|---|
| Rails (this repo) | Free, Apache-2.0 | The one-liner above. Your agent, your machine, your app, your keys. |
| Self-hosted intake | $88 once | The managed doorway at [tohseno.com](https://tohseno.com/intake): private encrypted intake, order lifecycle, and a released agent capsule. |
| Client-owned | $888 setup + $88/month | You own source, accounts, domain, and data plane; TOHSENO operates through scoped access. |
| Anky-operated | Selective | Anky, Inc. may adopt the app as a first-party product after review. |

The paid paths point at where the managed product is heading: everything from code generation to cloud deployment, operated for you. Today they honestly provide intake, ownership contracts, and the agent capsule — see [Operating modes](docs/OPERATING_MODES.md) and the boundary below.

## What works now / what does not

**Implemented:** the hero landing and `/intake` form, deterministic intake validation, AES-256-GCM encryption at rest, hash-plus-capability access, SQLite store with append-only order events, disabled/mock/Stripe payments with verified idempotent webhooks, email providers, private status and capsule routes, a bearer-authenticated operator API and CLI, the manifest schema and contract fixtures, the continuity-app skill, the pinned oneshot bootstrap, and one Bun server with Docker and Railway deployment preparation.

**Not yet:** generating complete native iOS/Android applications, creating infrastructure, store submission, a reflection provider for generated apps, end-user continuity sync, or a production practice-identity package. `READY` on a self-hosted order means the capsule and source contract are available — not that an application has been generated. Proof and sync contracts are early, explicitly non-final work.

## Repository architecture

```text
apps/site                 one Bun HTTP server: hero landing, /intake, encrypted
                          submissions, payments, email, capabilities, operator API
packages/manifest         public manifest schema, types, validator
packages/contracts        draft event/artifact/reflection/proof/envelope schemas
examples                  two filled-in source/manifest examples
templates/continuity-app  the workspace the oneshot creates apps from
skills/continuity-app     the coding-agent build protocol and refusal rules
docs                      doctrine, roadmap, ADRs, proposals, privacy, operations
scripts                   check gate, migrations, backups, secrets, operator CLI
```

Runtime path: `raw HTML → Bun.serve → deterministic validation → Web Crypto → SQLite`, with payment, email, and operator providers at the edges. See [System architecture](docs/SYSTEM_ARCHITECTURE.md) and the [ADRs](docs/adr/README.md).

## Privacy model

Submitted documents and contact details are encrypted at rest with a deployment-supplied key. Content hashes identify; independent revocable bearer capabilities authorize, and only their one-way hashes are stored. Bearers travel in URL fragments or `Authorization` headers, never in request paths, query strings, or payment metadata. Invalid, expired, or revoked capabilities all return the same `404`. Submitted documents are intake data — the control plane is designed never to receive the private continuity data of a generated app's end users. Details: [Privacy boundary](docs/PRIVACY_BOUNDARY.md) and the public `/privacy` page.

## Local development

Requires [Bun](https://bun.sh/); no external accounts for the local path.

```sh
bun install
cp .env.example .env
bun run generate-secrets   # put the two printed secrets in .env
bun run migrate
bun run dev
```

Use `PAYMENTS_MODE=mock` and `EMAIL_MODE=console` only outside production. Full setup: [Local development](docs/LOCAL_DEVELOPMENT.md).

## Validation

```sh
bun run check   # the before-commit gate: typecheck, tests, contract corpus,
                # static surface, oneshot pin ancestry, secret hygiene
```

Tests use temporary databases and fake providers; no network access needed.

## Production preparation

Build the Docker image, run one instance with a persistent volume (`DATABASE_PATH=/data/tohseno.sqlite` on Railway), configure an HTTPS `BASE_URL`, strong secrets, and real Stripe prices. The repository prepares deployment; it does not deploy or change DNS. See [Deployment](docs/DEPLOYMENT.md), [Stripe](docs/STRIPE.md), [Email](docs/EMAIL.md), [Key rotation](docs/KEY_ROTATION.md).

### Release discipline for the oneshot pin

`apps/site/public/oneshot.sh` embeds `TOHSENO_PIN`, the exact rails commit every new workspace is created from. A release that changes the rails must land first, then a follow-up commit bumps the pin to it — so the pin always trails the serving commit by one. `bun run check` verifies the pin is a real ancestor commit containing the required template files.

## Product and contribution contract

Start with [Doctrine](docs/DOCTRINE.md), [Product contract](docs/PRODUCT_CONTRACT.md), [Roadmap](docs/PRODUCT_ROADMAP.md), and [Open questions](docs/OPEN_QUESTIONS.md). New protocol work stays in [Proposals](docs/proposals/README.md) until its decisions and tests support an ADR. Sanitized [intent distillations](docs/intent/README.md) record the intended evolution without committing raw owner prompts. The Anky architectural study is preserved verbatim under [docs/research/anky](docs/research/anky/README.md); no Anky production code was copied.

## License and names

Apache License 2.0 ([LICENSE](LICENSE)). The license grants no trademark rights to the TOHSENO or Anky names; see [TRADEMARKS.md](TRADEMARKS.md).
