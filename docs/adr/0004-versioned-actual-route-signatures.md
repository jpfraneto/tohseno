# ADR 0004: Version signed request envelopes and bind the actual route

- Status: Accepted
- Date: 2026-07-10

## Context

Anky request types include method and path, but some clients reconstruct fixed `POST /anky` values even when authorizing different routes. A generic signature that does not bind the operation permits cross-route ambiguity and cannot support safe evolution.

## Decision

Signed network operations use an explicitly versioned envelope. `SignedRequestEnvelopeV1` signs the actual normalized HTTP method, actual request path, exact body hash, timestamp, nonce, signer identity/suite, and protocol version. The verifier receives the real method/path from the request and rejects any mismatch.

Replay/freshness and operation idempotency are enforced deliberately. A new signing interpretation requires a new protocol version and staged compatibility plan; fields are not silently reinterpreted.

## Consequences

- Golden fixtures include changed body, method substitution, path substitution, expiration, nonce replay, Unicode/canonical-byte, and empty-body cases.
- Server changes deploy before clients when supporting a staggered migration.
- TLS remains required; a signature is authorization, not confidentiality.
- A valid signature establishes control of the signing capability over the statement, not honest human action or trustworthy timing.

## Non-goals

This ADR does not choose the first generic identity suite or a distributed nonce store. No production generic signing package is claimed by the schema alone.
