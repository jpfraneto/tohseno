# System architecture

The first TOHSENO vertical slice is intentionally one small Bun process and one SQLite database. The public surface should feel like one button; the internal behavior is a set of explicit, deterministic boundaries.

## Context

```text
customer browser                         operator CLI
      |                                      |
      | public/private HTTP                  | bearer-authenticated HTTP
      v                                      v
┌─────────────────────────────────────────────────────────────┐
│ apps/site: one Bun.serve process                            │
│                                                             │
│ static pages → intake → crypto → submissions/state machine │
│                               ├─ capability/status/capsule  │
│                               ├─ payment provider           │
│                               ├─ email provider             │
│                               └─ narrow operator API        │
└──────────────────────┬───────────────┬──────────────────────┘
                       |               |
               bun:sqlite/WAL       HTTPS adapters
                       |          Stripe / Resend (optional)
               persistent volume
```

There is no separate frontend build, framework runtime, admin dashboard, queue service, cache, ORM, cloud SDK, analytics service, or generic authentication system. Transactional email uses a small durable outbox in the same SQLite database.

## Repository layers

### `apps/site`

The deployment unit serves static assets and all HTTP routes. Its internal modules have narrow roles:

- configuration validates environment and centralizes product copy/prices;
- security supplies headers, request IDs, bearer comparison, body limits, and the single-process rate limiter;
- crypto owns versioned AES-256-GCM envelopes and SHA-256 helpers;
- database applies raw migrations, enables foreign keys/WAL, and exposes transaction boundaries;
- submissions validates and persists private intake;
- capabilities resolves bearer hashes and renders status/capsule views;
- state machine defines mode-specific transitions;
- payments composes disabled, mock, or Stripe behavior;
- email composes disabled, console, or Resend behavior;
- operator exposes authenticated, explicit inspection and mutation operations.

The server entry point should remain routing/composition code, not a second domain layer.

### `packages/manifest`

The manifest schema is the boundary between human meaning and deterministic implementation. It is deliberately smaller than a universal application schema. Validation distinguishes runtime properties, coding-agent guidance, and operator/deployment metadata. Anky and Daily Observation prove different action/media/completion shapes, but neither example proves that the schema is final.

### `packages/contracts`

Language-neutral schemas and golden fixtures begin the compatibility harness for:

- `ContinuityEvent` — stable lifecycle identity and artifact references;
- `ContinuityArtifact` — immutable sealed bytes/reference and digest;
- `ContinuityReflection` — a separate, independently deletable derivation;
- `ContinuityProof` — a narrowly worded attestation with minimal disclosure;
- `SignedRequestEnvelopeV1` — actual method/path, exact body hash, time, nonce, signer, and signature.

These contracts are early and non-final. No generic production identity or proof verifier is implied.

### Examples, template, and skill

Examples demonstrate contracts without claiming shipped applications. The continuity-app template is a handoff/runbook seed, not generated native code. The skill guides a coding agent from interview through manifest, invariants, deployment preparation, and ejection while refusing unsupported scope.

## HTTP surface

| Boundary | Routes | Response type |
|---|---|---|
| Public pages | `GET /`, `GET /privacy`, `GET /healthz` | HTML/text or health JSON |
| Intake/status | `POST /api/submissions`, `POST /api/capability/session`, `GET /status/<submission-id>` | JSON/form fragment handoff; bounded scoped browser exchange; private HTML |
| Payment | `POST /api/checkout`, `GET /checkout/success`, `GET /checkout/cancel`, `POST /api/webhooks/stripe` | JSON/same-origin continuation page/webhook acknowledgement |
| Capsule | `GET /c/<submission-id>`, `GET /c/<submission-id>/MASTER_PROMPT.md` | Private HTML/Markdown via matching scoped cookie or bearer header |
| Operator | `/api/operator/submissions...` list/detail/inspect-source/transition/summary/message/retry-email/revoke | JSON behind bearer authentication |

Capability tokens never enter a request path or query string. Private paths contain only the safe submission ID. Browser handoffs use a fragment, then a bounded same-origin bootstrap verifies the token-to-ID binding and installs a separate HttpOnly strict cookie for that submission; coding agents use an Authorization header. The fragment is absent from HTTP and platform access logs, and the application records only route templates. Every private read and Checkout requires the resolved credential's submission to equal the safe path/body ID. Capability routes are no-store and produce `404` for unknown, expired, mismatched, or revoked credentials. Raw Markdown is available only on its explicit scoped sub-route, never embedded in a log or public page.

## Submission transaction

```text
bounded body
→ validate content type, UTF-8-like Markdown, size, useful length, email, mode
→ compute SHA-256 content hash
→ encrypt Markdown and contact independently with fresh nonces
→ create random capability; hash it for storage
→ transaction:
     insert submission
     append creation/submission event(s)
     advance to READY_FOR_PAYMENT or ANKY_REVIEW
→ set the strict HttpOnly capability cookie
→ return private status handoff with the raw capability only in its URL fragment
→ attempt configured transactional email outside private logs
```

The raw bearer must be returned because it cannot be recovered from the database. It is never placed in the request path or query. Every private fragment load reconciles the bearer and safe submission ID through a bounded JSON body while hiding any stale content; the browser reloads only when the scoped cookie changed or the page was a bootstrap shell. Per-submission cookie names prevent handoffs in separate tabs from replacing one another. Non-browser agents send the bearer in an Authorization header with the same safe-ID-scoped route. A failure after commit must not cause the server to print it. Retry behavior must avoid duplicate payments and state corruption.

## Persistence model

SQLite is normalized around six record types:

- `submissions`: encrypted intake, integrity/capability hashes, monotonic capsule release, mode, status, manifest version, optional safe summary, timestamps;
- `order_events`: append-only, sequence-ordered transition and access audit history;
- `payments`: provider/session references, checkout attempt, amount/currency/status, idempotency;
- `payment_events`: deduplicated provider event IDs and safe outcomes;
- `messages`: durable notification intent/status, encrypted customer-authored bodies, template/idempotency metadata, and provider references.
- `schema_migrations`: applied raw migration versions.

`schema_migrations` records raw SQL migration application. Foreign keys are enabled. WAL is appropriate for the single-server volume. Order transitions and payment webhook handling run in transactions so the current status and append-only history cannot diverge under a normal failure.

SQLite triggers reject updates and deletes on `order_events`, and tests exercise both failures. A database superuser can still alter or remove schema/triggers, so file access must be restricted and backed up.

## Order state machine

The complete vocabulary is shared across modes:

```text
DRAFT, SUBMITTED, PREFLIGHTING, NEEDS_CLARIFICATION, REFUSED,
READY_FOR_PAYMENT, PAYMENT_PENDING, PAID, MANIFEST_LOCKED, GENERATING,
NEEDS_CREDENTIALS, QA, FAILED, READY, SUBMITTED_TO_STORES, LIVE,
EJECTED, CANCELLED, REFUNDED, ANKY_REVIEW, ANKY_ACCEPTED, ANKY_DECLINED
```

Each mode receives only its declared edges. Illegal edges fail without mutating the submission or appending an event. The initial automated paths are:

```text
self-hosted:
SUBMITTED → READY_FOR_PAYMENT → PAYMENT_PENDING → PAID
→ MANIFEST_LOCKED → GENERATING → READY

client-owned:
SUBMITTED → READY_FOR_PAYMENT → PAYMENT_PENDING → PAID
→ MANIFEST_LOCKED → NEEDS_CREDENTIALS

anky-operated:
SUBMITTED → ANKY_REVIEW → ANKY_ACCEPTED | ANKY_DECLINED
```

Later QA, store, live, refund, cancellation, failure, and ejection edges remain explicit operator/provider actions where legal; no mode is allowed to wander through another mode's commercial path.

For self-hosted and client-owned orders, every paid/delivery state has a contextual invariant: a `paid` payment record must already exist. The transition graph alone cannot be used to route an unpaid failure into generation or capsule release. Anky-operated review uses its separate acceptance gate.

Capsule release is monotonic once the applicable verified-payment or Anky-acceptance state is reached. A later `FAILED` state describes operations; it does not silently take an already released source/ejection contract away. Expiry and explicit capability revocation remain the authorization controls.

## Payment boundary

All payment modes implement the same small interface.

- `disabled` renders an honest unavailable state and never advances payment.
- `mock` deterministically exercises the local flow and refuses to start under `NODE_ENV=production`.
- `stripe` creates Checkout sessions from configured Price IDs and treats only a verified, idempotent webhook as payment evidence.

Self-hosted Checkout is one-time. Client-owned Checkout includes one setup line and one licensed monthly recurring line. Before returning a Checkout URL, the Stripe adapter reconciles expanded Price objects, quantities, amount, currency, billing scheme, recurrence, mode, and submission reference against centralized product configuration. Anky-operated never calls a payment provider. Safe submission IDs may appear in metadata; private source, contact, capabilities, messages, and secrets may not.

Webhook verification requires the exact raw request bytes. JSON parsing before verification would invalidate the security boundary. Provider event/session references and database idempotency prevent duplicate delivery from duplicating transitions.

## Email boundary

Email is a side effect of committed order state, not a state authority. The intent is committed transactionally, then atomically claimed from the SQLite outbox; delivery failure records `failed` without rolling back a valid submission or payment transition. Caught background drains run after capability/webhook responses are constructed, so provider latency cannot prevent the one-time capability handoff or delay a Stripe acknowledgement. Startup scheduling and the authenticated operator retry path drain pending/failed rows. Providers are disabled, console metadata-only, or paced Resend delivery through direct `fetch` with stable idempotency keys. Message bodies and recipients remain outside console logs.

## Security boundary

All responses receive a content security policy, content-type protection, referrer policy, permissions policy, cross-origin opener policy, and CSP `frame-ancestors`. Scripts are same-origin external files; no inline-script exception is needed for capability bootstrap. Private routes additionally disable caching and indexing. Static files are selected through an allowlist/safe path resolution rather than arbitrary filesystem joining.

Submission creation and failed operator authentication have simple in-memory limits. This is suitable for the intended single process; it is not a distributed defense. Proxy-provided client addresses are ignored unless `TRUST_PROXY` is explicitly configured for the deployment topology.

Production errors are safe summaries with request IDs, never stack traces or request bodies. Structured logs include route templates and safe IDs only.

## Control plane versus data plane

```text
TOHSENO control plane                    generated app data plane
---------------------                    ------------------------
order state                              user actions/checkpoints
deployment health                        private artifacts
safe app/version IDs                     reflections
payment/operating metadata               practice identity secrets
credential readiness                     recovery material/backups

               no implicit content flow ←──────────────┘
```

An optional app-specific service can process content only when its manifest, consent, encryption, retention, deletion, ownership, and ejection contract says so. It is not permitted to reuse the intake SQLite database as a convenient content warehouse.

## Deployment topology

The supported first production topology is one container, one process, and one persistent SQLite volume at `/data`. Horizontal replicas sharing or copying a SQLite file are outside this slice. Scale-up should first preserve a single writer and add measured operational safeguards; a distributed database/rate limiter is a future architecture decision, not a hidden default.

Health checks prove that the process/config/database are usable without disclosing order counts or private fields. Backups must include the database and the exact matching data-key custody procedure; a database backup without the key is unreadable, and a key without the database does not recreate data.
