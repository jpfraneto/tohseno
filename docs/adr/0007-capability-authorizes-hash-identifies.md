# ADR 0007: Capabilities authorize; content hashes identify integrity

- Status: Accepted
- Date: 2026-07-10

## Context

Content addressing is useful but frequently confused with access control. A SHA-256 digest may be copied into ledgers, fixtures, payments, or logs and can sometimes be recomputed or guessed from known input. Giving it both meanings makes privacy revocation impossible.

## Decision

TOHSENO treats a capability token as access control and a content hash as an integrity identifier. Authorization code accepts only the capability validation result. Integrity code verifies hashes against exact bytes. No fallback permits a submission ID, content hash, email, payment reference, or status redirect to substitute for the capability.

## Consequences

- Access checks remain revocable and time-bounded without changing content identity.
- Safe references may use a submission ID/content digest only where disclosure policy allows, but they grant no private access.
- Tests explicitly demonstrate that a valid content hash without a capability is unauthorized.
- UI/copy calls capability URLs private bearer links, not shareable content URLs.

## Non-goals

This ADR does not make every content hash public or safe to log. Digests can still be stable/linkable metadata and are disclosed only for a defined operational purpose.
