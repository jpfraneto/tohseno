# Open product questions

The decisions below are intentionally unresolved. Implementations and product copy must not choose an answer implicitly or promise a guarantee that depends on one.

## Required open questions

### 1. May incomplete fragments be reflected?

The event model can preserve interrupted artifacts, but that does not decide whether reflection is appropriate. Anky's current platforms and backend behavior differ. A decision must define eligibility, consent, provider disclosure, copy, and whether a partial artifact receives a different policy.

### 2. Is reflection always opt-in?

Explicit opt-in is the privacy-safe default at a network disclosure boundary, while some local deterministic feedback can reasonably be automatic. The product must decide whether any remote reflection may run automatically and how consent is recorded and revoked.

### 3. What is the application-level encryption threat model?

“Platform-private” storage, application-managed content encryption, end-to-end encrypted synchronization, and encrypted backup protect against different adversaries. The model must name protection while unlocked, key custody, lost-device behavior, operator access, attachment handling, search/indexing, recovery, and deletion before generated apps promise application encryption.

### 4. What is the first generic practice-identity cryptographic suite?

Anky's Base EOA suite is a compatibility adapter, not an automatic framework default. A generic suite decision must cover app scoping, signing purpose, platform libraries, secure storage, recovery, rotation, revocation, multiple devices, export, and long-term algorithm agility.

### 5. How should cross-device and cross-app identity bridging work?

The default is separate contextual identities. A bridge would need explicit selective consent and a narrowly worded signed relation without silently exposing a universal person identifier. Content migration and account/subscription ownership must remain separate.

### 6. What are complete Android off-device recovery and deletion semantics?

Current Anky cryptography can be manually portable, but Android storage/backup and server deletion are not equivalent to iOS. A product decision needs a real off-device destination, key-loss behavior, archive restore, server and provider deletion, generated files, backups, subscription identifiers, and truthful user-facing wording.

### 7. What is the second proof application after Anky?

Daily Observation is a useful non-writing manifest example, not a shipped proof. The second implementation must materially differ in action, media, completion, interruption, accumulation, and recovery while exercising the same kernel. Candidate selection should be driven by what boundary it tests, not marketing convenience.

### 8. Which payments, blockchain, paintings, gates, and subscription modules remain optional?

The core local action and user-owned record cannot depend on them by default. Each module needs a manifest representation, platform/privacy/ownership contract, failure degradation, export/ejection behavior, and evidence from more than one app before becoming a reusable module.

### 9. What is the long-term open-source license strategy for generated applications?

This repository is Apache-2.0. That does not settle the license of generated customer applications, generated templates, bundled assets, app-specific server code, or commercial modules. The strategy must preserve ejection, third-party license compliance, customer expectations, and TOHSENO/Anky trademark boundaries.

### 10. What is the private application-evolution provenance contract?

The current system stores one encrypted source prompt and a current manifest
version, not an append-only history from owner request through manifest and
release. A decision must define stable revision identity, parentage and branches,
exact prompt encryption and deletion, owner/update authority, approval,
manifest diffs, release evidence, rollback, retention, and ejection. Raw prompts
must not become public Git, logs, order metadata, or chain data by default.

### 11. How does a phone delegate authority to a browser?

A QR bridge needs a production identity suite, secure storage, origin and
audience binding, app and operation scopes, ephemeral browser keys, expiry,
single-use challenges, replay defense, step-up approval, multiple-device
behavior, rotation, revocation, and recovery. Recovery material must never enter
the QR, browser, extension, server, URL, or log. Existing signed-request V1
fixtures do not decide this delegation protocol.

### 12. What are the authorization and operation audit contracts?

A browser-visible history can improve transparency, but it is not a continuity
event or independent proof. A decision must define fixed event kinds, allowed
safe fields, local storage, retention, export, deletion, failure codes, and the
boundary between authorization decisions and operation outcomes. Request bodies,
private artifacts, capabilities, keys, signatures, and recovery material remain
excluded.

### 13. What is the first external action and its key/authority model?

Base, Solana, Hyperliquid, and Robinhood expose materially different chain,
exchange, brokerage, custody, disclosure, fee, and rollback boundaries. Before
creating adapters or smart contracts, choose one exact manifest-declared
operation, environment, signer role, preview/step-up policy, limits, ownership,
failure degradation, and ejection path. Practice identity, browser delegation,
and financial execution authority must not silently reuse one key.

### 14. What is the free/community initializer contract?

A one-line local initializer could ask which coding agent will build the app,
but it needs pinned and verifiable distribution, dirty-tree/overwrite safety,
private capability input outside shell history, deterministic output, rollback,
and agent data-handling disclosure. “Free” is a new product path alongside the
current paid modes; it requires an explicit source, support, ownership, abuse,
and compatibility contract before public copy or package publication.

### 15. How can evolution history become public teaching material?

Tutorials can show intent, manifest diffs, implementation, tests, and live
verification, but exact owner prompts and production evidence may be private.
A publication policy must define owner approval, redaction, provenance,
third-party rights, deletion limits, and the separation between canonical
private records and public educational derivatives.

### 16. Does the phone co-sign every browser request, or sign a scoped delegation with step-up?

Both models satisfy the accepted invariant that every application-authorized
mutation after contextual identity exists must carry a verifiable
authorization chain rooted in the phone. Literal co-signing requires the phone
online and interactive for every mutation and risks habituated approvals;
scoped delegation binds a phone-signed, short-lived grant to the application,
origin, browser ephemeral key, operation allowlist, and expiry, with fresh
phone step-up for publishing, financial, account-authority, and code-evolution
operations. The current recommendation is delegation plus step-up, but the
choice affects availability, approval fatigue, revocation semantics, and the
operation registry, so it is an owner decision. Verification middleware must
not be implemented before it is made. See the
[POST route authority audit](POST_AUTHORITY_AUDIT.md) and
[Phone-to-browser bridge](proposals/PHONE_BROWSER_BRIDGE.md).

### 17. What multi-tenancy trust tier does the deployment cell serve?

One container per application with per-cell user, network, volume, and
resource limits is sufficient isolation for one owner's applications on one
VPS, and arguably for reviewed TOHSENO-generated code. It is not a sufficient
boundary for untrusted or customer-modified code, or for cells holding
financial or external-action keys — those need a hardware-virtualized boundary
(separate VM/VPS or microVM) before co-residency is offered. The tier choice,
the shared-ingress implementation, backup encryption key custody, and default
resource bounds must be decided explicitly before any multi-tenant hosting is
sold. See [Deployment cell](proposals/DEPLOYMENT_CELL.md).

## Decision process

Resolve an open question through an ADR only when the decision includes:

1. the exact claim or behavior being chosen;
2. affected manifest fields and contracts;
3. privacy, recovery, ownership, and ejection consequences;
4. compatibility and migration treatment for existing applications;
5. executable acceptance and negative tests;
6. a rollback or staged-release plan where state or external systems change.

Until then, choose the least-disclosing and least-coupled behavior that preserves local continuity, label it as a temporary default, and keep the stronger capability disabled.

## Accepted decisions that are not open

The architecture has already accepted separate stable event IDs, immutable sealed artifacts, separate reflections, actual-route signed envelopes, distinct identity/content recovery, independent content hashes and capabilities, ejection from birth, metadata-only control plane behavior, and deterministic runtime boundaries around AI. See the [ADR index](adr/README.md). Do not reopen those through an implementation shortcut; supersede them with explicit evidence and a new ADR if necessary.
