# Proposal: phone-to-browser bridge

- Status: **Proposed**
- Open: production identity suite, delegation protocol, secure storage,
  recovery, multiple-device behavior, first operation catalog, and the
  co-signing-versus-delegation decision (open question 16)
- Does not implement: QR pairing, browser extension, native signing, or a
  production verifier

## Product shape

The phone can be the contextual authority that connects a native continuity app
to a browser session. The browser should receive a narrow, short-lived ability
to perform named operations; it should never receive the phone's seed, recovery
material, or long-lived private key.

This bridge appears after first local value. Authentication must not become a
profile ceremony or prerequisite for the core action.

## The authorization invariant

The owner's requirement is that every POST to the backend be authorized by the
phone-held key, including browser-originated requests. Stated literally, the
requirement is unsatisfiable at two edges: the public bootstrap and pairing
requests that exist before any phone key is bound, and provider webhooks that
are authorized by the provider's own signature. The
[POST route authority audit](../POST_AUTHORITY_AUDIT.md) classifies every
current route against those edges. The invariant this proposal defends is:

> Every application-authorized mutation after contextual identity exists must
> have a verifiable authorization chain rooted in the phone.

For a browser-originated request, two models can satisfy it:

1. **Literal phone co-signing.** The phone signs every browser POST. The
   chain is one link, but the phone must be online, reachable, and
   interactive for every mutation; a dropped connection stalls the browser
   entirely, and constant approval taps train the owner to approve without
   reading — a security cost, not only a convenience cost.
2. **Scoped delegation plus step-up (recommended).** The phone signs a
   short-lived delegation bound to the application, origin, browser ephemeral
   public key, exact operation allowlist, and expiry. Each browser POST is
   signed by the ephemeral key and carries the delegation; the server
   verifies both signatures, the binding between them, expiry, revocation,
   and replay. The chain has two links, both verifiable, still rooted in the
   phone. Sensitive operations — publishing private content, spending,
   trading, transferring assets, changing account authority, or evolving
   application code — require a fresh phone step-up approval regardless of an
   active delegation.

Model 2 is the recommendation because it keeps every-request verification
while reserving deliberate phone interaction for the operations where an
inattentive approval is expensive. The choice between the models is an owner
decision recorded as open question 16; implementation of verification
middleware must not begin before it is made. Under either model, operator
routes remain a separate principal and provider webhooks remain
provider-verified — a phone key never substitutes for those authorities.

## Keep key roles separate

One mnemonic is recovery material from which keys may be derived; it is not “the
private key.” The following roles must not silently reuse one key:

| Role | Purpose |
|---|---|
| Practice identity | App-scoped continuity identity or attestations |
| Owner/update authority | Approval of application evolution |
| Browser delegate | Short-lived authorization for one browser session |
| Release signer | Software/build provenance |
| External-action signer | Chain, exchange, brokerage, or other external action |

If a shared recovery root is ever supported, derivation must be versioned,
hardened, and domain-separated by app, environment, network, and purpose. That
choice is opt-in and does not replace the default of unlinkable contextual
identities.

## Proposed pairing sequence

```text
browser creates ephemeral key and same-origin session
        ↓
service creates a single-use pairing challenge
        ↓
browser shows a non-authorizing QR payload
        ↓
phone shows verified origin, app, scopes, and expiry
        ↓
owner approves on phone
        ↓
phone signs a delegation to the browser public key
        ↓
service atomically consumes the challenge
        ↓
browser signs only registered operations until expiry/revocation
```

The QR contains a challenge reference and public context only. It is not a bearer
credential. The phone and browser should display a matching confirmation code
so a substituted session is visible. A screenshot of the QR must be useless
without the browser's ephemeral private key and phone approval.

The signed delegation needs to bind at least:

- protocol version;
- application and intended origin/audience;
- browser session and ephemeral public key;
- exact operation scopes;
- challenge nonce;
- issued-at and expiry times;
- phone signer suite and app-scoped key ID.

`SignedRequestEnvelopeV1` already binds actual method, path, exact body hash,
timestamp, nonce, and signer. It does not bind origin, audience, application,
delegation, or named operation. Browser use therefore needs a new envelope
version or an explicitly verified outer delegation contract; V1 fields must not
be silently reinterpreted.

## Finite operation registry

The hardcoded nature of requests is a strength when it is made explicit. Each
supported operation should declare:

- stable operation ID and version;
- exact HTTP method and path template;
- closed body schema and maximum size;
- required delegation scope and step-up policy;
- idempotency and replay behavior;
- private fields and disclosure destination;
- safe response and error vocabulary;
- content-free audit event;
- offline/failure fallback and revocation behavior.

Unknown operations, arbitrary URLs, additional body fields, and scope widening
fail closed. A browser extension exposes this typed registry, not generic fetch
or signing access. Its worker keeps the ephemeral key, checks sender origin, and
uses the narrowest host permissions.

## Browser-visible audit journal

Logging every request means a local, typed transparency journal, not console
logging of network traffic. Keep two schemas separate:

- `AuthorizationAuditEvent`: pairing requested, approved, rejected, expired,
  or revoked;
- `OperationAuditEvent`: registered operation prepared, sent, accepted, or
  rejected.

Safe fields can include event ID, operation ID, policy/release version, time,
request ID, outcome, and a fixed error code. Do not record request or response
bodies, reply text, exact artifacts, seed material, private keys, capabilities,
cookies, signatures, recovery material, or correlatable content hashes.

The journal is useful user-facing history, not independent proof. Its local
storage, retention, export, and deletion behavior must be declared.

## Step-up and failure behavior

A short-lived delegation may cover low-risk operations. Publishing sensitive
content, spending, trading, transferring assets, changing account authority, or
evolving application code requires an exact phone step-up approval unless a
separate manifest and threat model explicitly justify otherwise.

Bridge, extension, server, or network failure may not roll back a committed
continuity event. The core action, local checkpoint, local record, recovery, and
owner export remain useful without the bridge.

## Acceptance evidence

- altered origin, app, delegate key, scope, operation, method, path, body,
  timestamp, expiry, and nonce all fail;
- expired/future challenge, replay after restart, concurrent double claim,
  wrong session, revoked delegate, and rotated phone key all fail;
- a captured QR cannot pair a different browser;
- additional JSON fields and unknown operations fail closed;
- durable nonce and idempotency semantics are tested separately;
- canary secrets never enter URLs, logs, errors, plaintext database pages, WAL,
  analytics, or crash output;
- extension origin/permission negatives pass;
- pairing and optional services can be disabled without breaking continuity.

## Reference-app question

Replyguy can test this boundary if its manifest preserves local first value. One
candidate is an offline reply-composition/practice action followed by an
optional, explicitly authorized publish operation. If the only primary action
requires a remote social platform, it cannot currently claim the generic
offline continuity invariant.
