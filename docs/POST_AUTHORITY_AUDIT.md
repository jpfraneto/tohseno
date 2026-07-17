# POST route authority audit

- Status: **Implemented** routes audited on 2026-07-17 at commit `07cb236`
  (plus uncommitted documentation).
- Purpose: classify every mutating HTTP route by the authority that actually
  admits it today, so the phone-rooted authorization requirement in
  [Phone-to-browser bridge](proposals/PHONE_BROWSER_BRIDGE.md) is applied to
  the right routes and not silently claimed for routes it cannot cover.

## The requirement being tested

The owner asked that every POST to a generated backend be authorized by the
phone-held key, including browser-originated requests. Audited literally, no
current route satisfies that. Audited precisely, some routes can never satisfy
it, because they exist before any phone key does or because a different
principal is the correct authority. The defensible invariant is:

> Every application-authorized mutation after contextual identity exists must
> have a verifiable authorization chain rooted in the phone.

`SignedRequestEnvelopeV1` exists only as a schema, canonical-byte
implementation, validator, and fixtures in `packages/contracts`. It binds
method, path, exact body hash, timestamp, nonce, signer, and signature. No
route verifies it, no production signature suite is selected, and there is no
phone key implementation, verification middleware, durable nonce store, QR
pairing, browser delegation, or revocation system. Nothing below is
phone-rooted today.

## Inventory

All inbound POST routes live in `apps/site/server.ts`. The `fetch` call in
`apps/site/src/email.ts` is an outbound client request to the configured email
provider, not an inbound route, and is out of scope here.

| Route | Authority actually enforced today | Class |
|---|---|---|
| `POST /api/submissions` | None. Public intake, fixed-window rate limit per client key, size and schema validation. | A — pre-identity bootstrap |
| `POST /api/capability/session` | Bearer capability token in the body, exchanged for a submission-scoped `HttpOnly` cookie; token must resolve to the claimed submission. | C — capability-authorized customer mutation |
| `POST /api/checkout` | Bearer capability via submission-scoped cookie, `Authorization` header, or body field; must resolve to the claimed submission. | C — capability-authorized customer mutation |
| `POST /api/webhooks/stripe` | Stripe webhook signature verified over the raw body. | B — provider-verified callback |
| `POST /api/payments/mock/complete` | Environment gate only: mock provider selected and `NODE_ENV !== "production"`. | E — development-only control |
| `POST /api/operator/submissions/:id/transition` | Operator bearer token, constant-time compared, with a failure rate limiter. | D — operator authority |
| `POST /api/operator/submissions/:id/summary` | Operator bearer token. | D — operator authority |
| `POST /api/operator/submissions/:id/message` | Operator bearer token. | D — operator authority |
| `POST /api/operator/submissions/:id/revoke-capability` | Operator bearer token. | D — operator authority |
| `POST /api/operator/submissions/:id/inspect-source` | Operator bearer token. Access is recorded without recording content. | D — operator authority |
| `POST /api/operator/submissions/:id/retry-email` | Operator bearer token. | D — operator authority |

## What each class means for the phone requirement

### A. Pre-identity bootstrap — cannot be phone-rooted

`POST /api/submissions` creates the contextual identity; there is no phone key
bound to anything before it succeeds. Future pairing-challenge and
delegation-claim endpoints share this class. The correct controls are rate
limiting, strict schemas, size limits, single-use challenges, and atomic
consumption — not a phone signature that cannot exist yet.

### B. Provider-verified callbacks — a different root of trust

Stripe signs its own webhook with its own key. Demanding a phone signature
here would be impossible and would weaken the boundary: the provider signature
is the authority, and the route must stay narrow, verified over raw bytes, and
idempotent. Future provider callbacks (email delivery events, chain or
brokerage confirmations) stay in this class.

### C. Capability-authorized customer mutations — in scope for the invariant

These are the routes the invariant governs. Today their authorization chain is
rooted in the capability handoff (fragment URL, cookie, or header), which is
rooted in intake plus email delivery — not in a phone. Once a phone-held key
exists for an application, every route in this class must present a chain that
verifiably terminates at that key: either a phone signature on the request or
a browser-ephemeral-key signature carrying a valid phone-signed delegation, as
decided in open question 16. Adding a new class-C route without that chain
would violate the invariant, so the class list itself is part of review.

### D. Operator authority — a separate principal, deliberately

Operator routes act with Anky's operator authority, not the customer's, and
must never be reachable through a customer phone key: a customer must not be
able to transition their own order or inspect audit state by signing harder.
Strengthening operator authentication (hardware-backed operator keys, signed
operator envelopes) is worthwhile but is a separate track from the phone
invariant and must not be conflated with it.

### E. Development-only controls — must remain unreachable in production

`POST /api/payments/mock/complete` is compiled behind the mock provider and a
non-production environment check. It needs no phone authority; it needs the
existing gate to remain intact, and tests already exercise it. It must never
gain a production code path as a side effect of authorization work.

## Consequences

1. The phone requirement is implemented on class C, prepared for future
   class-A pairing routes as their own protocol, and explicitly rejected for
   classes B, D, and E with the reasons above recorded.
2. The decision between literal phone co-signing and scoped delegation with
   step-up (open question 16) only affects class C.
3. Any new POST route must be classified into A–E (or a new argued class) in
   this document before it ships; an unclassified mutation is a review defect.
