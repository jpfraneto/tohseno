# Privacy boundary

TOHSENO has two different kinds of private information. They must not be conflated:

1. **Intake data:** a prospective customer's `MASTER_PROMPT.md`, contact details, order messages, and payment/operating metadata used to provide the TOHSENO service.
2. **Continuity-app user data:** the actions, artifacts, reflections, identities, and recovery material created later by people using a generated application.

The current service handles the first category. Its future control plane is designed not to receive the second category.

## Current intake data flow

```text
browser
  └─ HTTPS form body: Markdown + email + mode
       └─ deterministic validation
            ├─ SHA-256 content hash (integrity only)
            ├─ AES-256-GCM encrypted Markdown
            ├─ AES-256-GCM encrypted contact
            ├─ random bearer capability → SHA-256 token hash in database
            └─ safe order metadata + append-only events

optional provider boundaries
  ├─ Stripe: safe submission ID, price/currency/amount, Checkout state
  └─ Resend: decrypted recipient only at delivery time, required message
```

Markdown is sent in the request body, never in a query string or content-hash URL. TLS is an operational production requirement; at-rest encryption does not protect data in transit.

## What is stored

| Data | Stored form | Purpose |
|---|---|---|
| Source Markdown | Versioned AES-256-GCM envelope | Produce the purchased/reviewed service and capsule |
| Contact address | Separate versioned AES-256-GCM envelope | Transactional status communication |
| Content hash | SHA-256 digest | Integrity and explicit byte correlation; never access control |
| Capability | Only a one-way token hash, plus expiry/revocation timestamps | Private bearer access |
| Capsule release | Monotonic release timestamp, never a bearer credential | Preserve paid/approved ejection access across later operational states |
| Order state/events | Safe identifiers, states, actor, timestamps, size-bounded metadata that rejects sensitive field names | Operations and audit |
| Payments | Provider references, amounts, currency, state, idempotency | Commerce and webhook reconciliation |
| Email outbox | Safe template/status/idempotency metadata; customer-authored bodies use AES-256-GCM | Durable transactional service communication |
| Compiled summary | Explicit operator-supplied safe JSON | Product workflow; must not contain source/contact/secrets |

Encryption uses Web Crypto AES-256-GCM with a unique random nonce for every value and a base64-encoded 32-byte deployment key. Each envelope authenticates a record/field context as additional data, so swapping a valid contact, Markdown, or message envelope into another record fails authentication. Decrypted Markdown is also checked against its stored content hash before operator/capsule release. Authentication or integrity failure is fatal for that value; ciphertext is never treated as plaintext. The envelope is versioned for future algorithm/key migration. The current deployment uses one active data key and requires a deliberate rotation migration; see [Key rotation](KEY_ROTATION.md).

## Capability URLs

A capability URL is a private bearer credential. Anyone who possesses it can exercise the access it grants until expiration or revocation.

The model is deliberately two-part:

- The **content hash** says which bytes were submitted and detects changes.
- The **capability token** says who can retrieve the private status/capsule.

The token contains at least 256 bits of cryptographically secure randomness. The database stores only its SHA-256 hash, so a database reader cannot directly recover the URL. Equality checks operate on hashes. Invalid, expired, and revoked tokens return indistinguishable `404` responses.

Private routes send `Cache-Control: no-store`, a strict referrer policy, search-engine exclusion, content-type protection, and a CSP that prevents framing. The token must not appear in logs, analytics, payment metadata, email subjects, error messages, or outbound referrers. The status/capsule HTML must not load third-party assets.

The same bearer may currently identify the private status and eventual capsule authorization. Possession should therefore be limited to the owner and explicitly chosen agent. If it is disclosed, revoke it through the operator interface and handle any replacement as a new capability—not as a content-hash change.

## Operator access

Routine operator list/detail reads return safe metadata, events, and payment state without decrypting source or contact data. Operators may decrypt a submitted source only to provide the requested service and only through the explicit authenticated `inspect-source` endpoint (used by the CLI `show` command). Raw access records a content-free audit event with the submission, actor type, and time. The audit event never contains document text, contact data, capability, or message body.

Operator APIs use a separate bearer token with constant-time comparison and rate limiting for failed authentication. That token is not a customer capability and must not be placed in browser storage, shared URLs, source control, or logs.

Operator visibility should answer questions such as “is this paid?”, “which state is blocked?”, and “is the application healthy?” without exposing private source or future app-user content by default.

## Payment and email processors

Stripe receives necessary Checkout information and safe submission IDs. It must never receive the submitted Markdown, email address as custom metadata, capability token, content hash as authorization, operator messages, encryption material, or other secrets. A verified webhook over the raw body is the source of payment truth; a success redirect is not.

Resend receives the destination address and transactional message when email is enabled. It does not receive the original Markdown. Console mode logs only delivery metadata and must not print message bodies or decrypted recipient addresses unnecessarily.

Provider retention, access controls, and contractual terms remain external operational concerns. Encryption inside TOHSENO does not erase copies legitimately sent to a configured processor.

## Logs and telemetry

Server logs are structured around request ID, route template, status, duration, and safe submission ID when relevant. They exclude:

- Markdown or derived excerpts;
- decrypted contact details;
- raw capability URLs/tokens;
- encryption keys and operator tokens;
- Stripe/Resend secrets;
- message bodies;
- full unexpected request bodies or stack traces in production.

There are no external analytics, tracking pixels, or marketing cookies. The in-memory limiter covers a single process only; production with multiple replicas would need a privacy-preserving distributed limiter before horizontal scaling.

## Future continuity-app boundary

Generated applications must make the core action, checkpoint, event, and artifact useful locally. The TOHSENO control plane may receive minimal operational metadata only when needed, such as deployment version, migration health, aggregate service availability, or an opaque app identifier. It must not receive raw writings, photos, recordings, reflections, practice secrets, or other private continuity payloads merely to operate the app.

An app-specific reflection or synchronization service, if declared, is a separate data plane with its own manifest, encryption, consent, provider, retention, deletion, and ejection contract. “Operated by TOHSENO” does not permit that data to leak into the order/control-plane database.

## Revocation and deletion

- Capability revocation is immediate for subsequent requests but does not delete the submission.
- Expiration and revocation deliberately produce the same public result as an unknown token.
- A deletion request must be sent to `support@anky.app` and verified without asking the requester to email the private Markdown or capability unnecessarily.
- Deletion work must enumerate encrypted source/contact/messages, order/payment records with legitimate retention constraints, email/payment processor copies, backups, and logs rather than promise a single-row deletion is “everywhere.”
- Deletion of intake data is distinct from deleting future application user data, cloud backups, subscriptions, generated assets, or practice identity.

The public privacy notice is plain-language service copy. This document is the engineering boundary; neither should claim a physical address, regulatory status, or deletion guarantee that has not been established.

## Threat-model limitations

Current at-rest encryption protects database contents and backups from disclosure without the application data key. It does not protect against a compromised running server that holds the key, an authorized operator during explicit access, a compromised browser or endpoint, recipient-side capability sharing, traffic metadata, configured processors, or plaintext deliberately copied into an external system.

The application-level encryption threat model for future generated apps remains open. Platform sandbox encryption and application-managed encryption are different guarantees and must be named accurately.
