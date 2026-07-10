# TOHSENO

TOHSENO compiles one Markdown document into a continuity app.

A continuity app is built around one meaningful action that a person returns to over time:

```text
act → record → reflect → continue
```

Input:
`MASTER_PROMPT.md`

Long-term output:
a private-by-default native application, reflection service, landing page, infrastructure, tests, store materials, deployment path, and ejection package.

Current repository status:
the first product shell, manifest contract, private intake, operator workflow, agent capsule, and architectural contract harness.

The full compiler does not exist yet. Today, TOHSENO provides an honest doorway and operating spine: it accepts a private source document, records an explicit ownership mode and order lifecycle, takes payment where configured, and releases a private coding-agent capsule when the applicable gate is satisfied. In self-hosted mode, `READY` means that capsule and source contract are available; it does not mean a native application has already been generated.

## What works now

- A raw, responsive landing page for pasted Markdown or a browser-loaded `.md` file.
- Deterministic intake validation with a 256 KiB maximum, UTF-8 and binary-like checks, a minimum useful length, conservative email validation, and explicit operating modes.
- AES-256-GCM encryption at rest for Markdown, contact details, and operator messages.
- SHA-256 content integrity plus an independent, revocable 256-bit bearer capability transported outside request URLs.
- A normalized SQLite store, raw migrations, append-only order events, and mode-specific legal transitions.
- Disabled, local mock, and Stripe Checkout payment providers with verified, idempotent webhooks.
- Disabled, metadata-only console, and Resend email providers.
- Private status and capsule routes, with the production capsule withheld until the applicable payment or approval boundary.
- Upfront payment-availability disclosure, canonical-host handling, and bearer-free infrastructure log paths scoped by safe submission IDs.
- A narrow bearer-authenticated operator API and CLI instead of an admin dashboard.
- A small continuity manifest schema, two materially different examples, and language-neutral contract fixtures.
- A coding-agent skill that protects the one action, tests invariants, prepares deployment, and produces an ejection package.
- One boring Bun server, a Docker image, and Railway deployment configuration for a persistent SQLite volume.

## What does not exist yet

This repository does not yet generate complete native iOS or Android applications, create infrastructure, submit to stores, operate a reflection provider for generated apps, synchronize end-user continuity data, or provide a production cryptographic practice-identity package. Proof and sync contracts are early, explicitly non-final interoperability work. No production deployment is performed by repository setup.

## User flow

1. A person pastes `MASTER_PROMPT.md` or loads a local `.md` file, enters an email address, and chooses an operating mode.
2. The server validates only deterministic intake properties. It does not claim to understand or compile the idea semantically.
3. The server hashes and encrypts the document and contact details, stores only a hash of a random capability token, appends the initial order events, sets a strict HttpOnly capability cookie, and returns a private status handoff whose bearer exists only in the URL fragment.
4. Self-hosted and client-owned orders are offered Checkout only when their payment provider and prices are configured. Anky-operated applications enter selective review and never create Checkout automatically.
5. Verified payment events advance the state machine. A browser success redirect is never proof of payment.
6. A paid self-hosted order reaches `READY`, exposing its private capsule. A paid client-owned order reaches `NEEDS_CREDENTIALS`, with customer-ownership and scoped-access instructions. An Anky-operated order exposes only review status until accepted.
7. A coding agent follows the capsule, `skills/continuity-app/SKILL.md`, the manifest, and the operator runbook. It must ask before paid resources, DNS changes, credential rotation, or store submission.

## Operating modes

| Mode | Price boundary | Ownership and present outcome |
|---|---:|---|
| Self-hosted | $88 once | The owner receives the private capsule, source contract, and runbook and operates everything. |
| Client-owned | Founding price: $888 setup + $88/month | The customer owns source, developer accounts, identifiers, domain, infrastructure, and data plane; TOHSENO operates through scoped access. Payment moves the order into credential preparation. |
| Anky-operated | Selective | Anky, Inc. may adopt the application as a first-party product. Submission starts review, not payment or automatic publication. |

The complete ownership and approval matrix is in [Operating modes](docs/OPERATING_MODES.md).

## Repository architecture

```text
apps/site                 one Bun HTTP server, static site, encrypted intake,
                          payments, email, capabilities, operator boundary
packages/manifest         small public manifest schema, types, validator, examples
packages/contracts        draft language-neutral event/artifact/reflection/proof/
                          signed-envelope schemas and golden fixtures
examples                  Anky and daily-observation source/manifest examples
templates/continuity-app  honest handoff and operator/ejection starter contract
skills/continuity-app     coding-agent workflow and refusal rules
docs                      doctrine, privacy, architecture, ADRs, operations, research
scripts                   checks, migrations/backups/secrets, operator CLI
```

The runtime path is intentionally short:

```text
raw HTML form → Bun.serve → deterministic validation → Web Crypto → SQLite
                                         ├─ payment provider
                                         ├─ email provider
                                         └─ narrow operator API
```

See [System architecture](docs/SYSTEM_ARCHITECTURE.md) and the accepted [architecture decisions](docs/adr/README.md).

## Privacy model

The source document and contact details are encrypted at rest with a deployment-supplied data key. The content hash is an integrity identifier, never a public address or credential. Access uses an independent random bearer capability; only its one-way hash is stored. Browser handoffs use safe submission-ID paths such as `/status/<submission-id>#capability=…`. Every private fragment is reconciled through a bounded same-origin POST with that safe ID, then stored in a separate HttpOnly `SameSite=Strict` cookie for that submission. Multiple owner handoffs can therefore coexist without one cookie authorizing or overwriting another. The fragment remains the owner's private coding-agent handoff and is never transmitted in an HTTP request. Coding agents use the safe-ID-scoped raw route with an `Authorization: Bearer` header. Bearers never belong in request paths or query strings. Private responses use no-store caching, restrictive referrer and search directives, and invalid, expired, mismatched, or revoked capabilities return the same `404` response.

Submitted source documents are intake data, not the private continuity data produced by future users of an app. The future control plane is designed to receive operational health metadata without receiving that end-user content. Payment providers receive only safe order identifiers and necessary commerce fields—not Markdown, contact details, or capabilities.

Read [Privacy boundary](docs/PRIVACY_BOUNDARY.md) and the public `/privacy` page before operating the service.

## Local development

Requires [Bun](https://bun.sh/) and no external service account for the local test path.

```sh
bun install
cp .env.example .env
bun run generate-secrets
# Put the printed TOHSENO_DATA_KEY and TOHSENO_OPERATOR_TOKEN in .env.
bun run migrate
bun run dev
```

Use `PAYMENTS_MODE=mock` only outside production and `EMAIL_MODE=console` or `disabled` locally. Then open `http://localhost:3000` (or the configured port). Full setup and safe test commands are in [Local development](docs/LOCAL_DEVELOPMENT.md).

## Validation

```sh
bun run test
bun run typecheck
bun run check
```

`bun run check` is the before-commit gate. Tests use temporary databases and fake providers; they do not require network access.

## Production preparation

Build the included Docker image and run one server instance with a persistent volume. On Railway, mount the volume at `/data` and set:

```text
DATABASE_PATH=/data/tohseno.sqlite
```

Configure a public HTTPS `BASE_URL`, strong data/operator secrets, production Stripe prices and webhook secret, and optionally Resend. Do not use mock payments in production. Follow [Deployment](docs/DEPLOYMENT.md), [Stripe](docs/STRIPE.md), [Email](docs/EMAIL.md), and [Key rotation](docs/KEY_ROTATION.md). The repository prepares deployment; it does not deploy or change DNS.

## Product and contribution contract

Start with [Doctrine](docs/DOCTRINE.md), [Product contract](docs/PRODUCT_CONTRACT.md), and [Open questions](docs/OPEN_QUESTIONS.md). Any feature must be expressible as a valid manifest diff. Otherwise it is unsupported, not an invitation to quietly become a custom software agency.

The Anky architectural study is preserved verbatim under [docs/research/anky](docs/research/anky/README.md). No Anky production implementation code was copied.

## License and names

Source in this repository is licensed under [Apache License 2.0](LICENSE). That license does not grant trademark rights to the TOHSENO or Anky names or logos; see [TRADEMARKS.md](TRADEMARKS.md).
