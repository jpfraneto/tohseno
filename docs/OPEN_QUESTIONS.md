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
