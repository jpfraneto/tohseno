# Product contract

This document defines what TOHSENO accepts, produces, refuses, and promises in its first vertical slice.

## Long-term product

The intended input is one private file:

```text
MASTER_PROMPT.md
```

The intended long-term output is a private-by-default native application, reflection service where declared, landing page, infrastructure, tests, store materials, deployment path, and ejection package.

That complete compiler is future work.

## Current product

The current repository implements:

1. Private Markdown intake and deterministic preflight validation.
2. Explicit self-hosted, client-owned, and Anky-operated order paths.
3. Encrypted source/contact persistence and private bearer capabilities.
4. Mode-specific payment and approval gates.
5. An append-only operator state machine and narrow operator interface.
6. A private agent capsule and source contract when the order becomes eligible.
7. A small manifest validator, examples, early continuity contracts, and a coding-agent skill.
8. Deployment preparation for one Bun server and persistent SQLite database.

It does not semantically compile a prompt into a native app. Deterministic intake acceptance means only that the source is valid UTF-8-like Markdown within limits, meaningfully nonempty, paired with a structurally valid email address, and assigned a supported mode.

## Intake contract

The source may be pasted or loaded from a `.md` file in the browser. Browser file loading only fills the same textarea; the server remains authoritative.

The server enforces:

- maximum UTF-8 size: 256 KiB;
- a minimum useful length;
- rejection of obvious binary/control-byte input;
- a supported operating mode;
- conservative structural email validation;
- accepted form or JSON content type and bounded request size.

The server must not echo the document into errors or logs. It calculates a SHA-256 integrity hash, encrypts the Markdown and contact, creates an unguessable capability, stores only the capability hash, appends order events, and returns a private status link. The raw capability appears only in that link's `#capability=` URL fragment; it is never placed in an HTTP path or query string and is not sent as a referrer.

For browser access, a small first-party bootstrap exchanges the fragment and its safe submission ID through a size-limited JSON `POST` body and receives an expiry-bounded, submission-specific cookie with `Secure`, `HttpOnly`, `SameSite=Strict`, and host-only scope. Human status, Checkout, and capsule requests then use safe-ID-scoped routes. Coding agents extract the fragment locally and send it as `Authorization: Bearer` to the matching safe-ID-scoped capsule route. The server binds every cookie, header, session exchange, and Checkout request back to that named submission. A capability must never be copied from the fragment into a request path, query, application log, platform path log, or payment request.

## Delivery contract by state

State names describe order operations, not application-runtime completion.

- `SUBMITTED` means intake was durably accepted.
- `READY_FOR_PAYMENT` means an eligible paid mode may request Checkout if configured.
- `PAID` means a verified provider event was processed, never merely a success redirect.
- `MANIFEST_LOCKED` means the source contract for this order is fixed at its current version for the next workflow step; it does not claim semantic compilation.
- `READY` in the self-hosted first slice means the private coding-agent capsule and source contract can be retrieved.
- `NEEDS_CREDENTIALS` in client-owned mode means production preparation is waiting for scoped customer-owned access.
- `ANKY_REVIEW` means Anky, Inc. is considering first-party adoption; no capsule or publication is promised.
- `LIVE` is reserved for an application that an operator has verified as publicly or privately available at its intended production endpoint/store. The current automated payment path does not jump to it.
- `EJECTED` means the applicable ownership handoff has been recorded; it does not erase customer data automatically.

Every successful transition is mode-legal and appended with previous state, next state, actor type, safe metadata, and timestamp. Transition metadata may never contain Markdown, email, bearer capabilities, credentials, secrets, or message bodies.

## Capsule contract

An eligible capsule contains:

- submission ID and content hash;
- operating mode and current order state;
- TOHSENO repository and skill reference;
- exact owner/agent instructions;
- the original `MASTER_PROMPT.md`;
- an explicit warning to ask before creating paid resources, changing DNS, rotating production credentials, or submitting to stores.

The human capsule view uses the browser's private session cookie. A coding agent retrieves the raw Markdown capsule from its stable route with the capability in the `Authorization` header. Both transports authorize through the same stored capability hash and preserve identical expiry and revocation behavior.

Self-hosted capsules ask the owner's agent to scaffold, validate, and prepare/deploy only with required approvals, preserve the manifest, and return repositories, credentials, URLs, ownership details, and ejection instructions.

Client-owned instructions additionally preserve customer ownership of bundle IDs, package IDs, source, domains, developer accounts, and infrastructure. They require readiness checks for Apple Developer, Google Play Console, DNS, and the desired infrastructure account, using scoped access rather than shared passwords.

Anky-operated submissions do not expose an automatic production capsule while in review.

## Manifest boundary

The manifest separates three concerns:

1. **Runtime-enforced properties:** action, completion, interruption, privacy, reflection consent, identity/recovery, exports, and optional capabilities.
2. **Coding-agent guidance:** target human, tone, visual direction, and implementation cautions.
3. **Operator/deployment metadata:** ownership mode, environment, stores, infrastructure, and handoff information.

A change is supported only if it can become a valid manifest diff and corresponding invariant tests. The current schema is intentionally small; unsupported concepts are not accepted by adding arbitrary JSON fields or bespoke code outside the contract.

## Commercial boundary

TOHSENO sells a repeatable product flow, not unlimited custom development.

- Prices and customer-facing copy come from one configuration module.
- Payment unlocks an operating step, not a false claim of generated software.
- When Checkout is disabled, the landing page must say so before a person submits private Markdown; the status page must repeat the unavailable state and no payment may be implied.
- Stripe test mode is an internal verification boundary, not a public founding-product purchase path, and must be labeled as such whenever it is reachable.
- Checkout metadata contains safe submission identifiers only.
- No store approval timeline is guaranteed; Apple and Google control review.
- Paid infrastructure, DNS changes, store submissions, and credential rotation always require explicit owner approval.

The current client-owned expectation is production preparation within eight hours after all required account access and credentials are ready. Public App Store and Google Play availability follows platform review and cannot be guaranteed within that window.

## Refusal contract

TOHSENO should refuse or defer:

- multiple unrelated core actions in one manifest;
- generic dashboards, profiles, feeds, CRUD, or chat added for completeness;
- private-content analytics or a centralized continuity-content warehouse;
- payment that gates the core local action, local record, recovery, or owner export;
- undocumented provider disclosure;
- claims that a digest proves authorship or that a key signature proves human behavior;
- cross-app identity linkage without explicit, selective consent;
- custom features that cannot be represented and tested through the manifest.

Refusal is not abandonment. The operator or agent should name the unsupported requirement, the invariant it conflicts with, and the smallest manifest-compatible alternative if one exists.

## Definition of a complete current order handoff

A current vertical-slice handoff is complete only when the customer can retrieve the authorized capsule/status, the order history is auditable, private fields remain encrypted, payment status came from a trusted provider path, ownership/approval instructions are explicit, and the limitations above remain visible. It is not complete merely because a row says `READY`.
