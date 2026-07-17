---
name: continuity-app
description: Distill, scaffold, validate, and prepare an ejectable continuity application from a private TOHSENO capsule or MASTER_PROMPT.md. Use when a coding agent must turn one repeated human action into a local-first act → record → reflect → continue loop while enforcing the TOHSENO manifest, privacy, refusal, deployment-approval, and ownership boundaries.
---

# Build a continuity application

Protect one durable ritual from generic product gravity. Treat the checked-in
`continuity.manifest.json` as the source contract and `MASTER_PROMPT.md` as
private product input. Produce the smallest tested vertical slice that lets a
person act, records locally, optionally reflects, and makes return natural.

## Start with the honest boundary

State this before work when expectations could be ambiguous:

- The current TOHSENO repository provides the product shell, private intake,
  manifest, contract harness, operator workflow, templates, and this agent
  workflow.
- It does not yet provide a complete native continuity-app compiler.
- A TOHSENO order in `READY` means its private agent capsule and source contract
  are available; it does not mean a native application has already been
  generated, deployed, or approved by a store.
- AI may help interpret human meaning into a manifest. Runtime privacy, storage,
  identity, signing, completion, exports, and deployment behavior must remain
  deterministic and testable. Continuity must survive without AI.

Never copy Anky production implementation code into a generated app. Use its
documented contracts and compatibility findings as evidence. Preserve `.anky`
or Anky identity behavior only behind explicitly named compatibility adapters.

## Respect capsule and repository privacy

Treat a capability URL as a private bearer secret. Fetch it only for the task,
never echo it into logs, commits, issue bodies, build output, analytics, or
payment metadata, and redact it from reports. Keep the original
`MASTER_PROMPT.md`, contact details, credentials, private messages, and
production data out of Git unless the owner explicitly designates a safe
private repository and file policy.

Do not use a content hash as authorization. A hash identifies exact content
integrity; an independent capability grants access. Do not confuse a digest,
practice-key signature, or server receipt with stronger proof than its explicit
statement supports.

## Follow the workflow in order

### 1. Inspect and inventory

Read applicable repository instructions, the private prompt or capsule, the
manifest schema at `packages/manifest/continuity.manifest.schema.json`, existing
manifest and runbook files, and the contract schemas in
`packages/contracts/schemas/`. Inspect the working tree and preserve owner
changes. Inventory target platforms, existing code, ownership mode, data
locations, services, and known open decisions.

For an existing application, record the evolution boundary before changing
source. Preserve the owner's exact request only in an owner-approved private
location. Create or update a safe `EVOLUTION.md` index using
`templates/continuity-app/EVOLUTION.md`, with a stable revision ID, parent,
sanitized intent summary, current manifest/release references, open or
unsupported requirements, approval needs, and later implementation evidence.
Never paste the raw request, a capability, a secret-bearing private reference,
or model chain-of-thought into that index. Do not publish a private prompt digest
by default; a digest identifies bytes but does not authorize access and may
create correlation or guessing risk.

Distill an evolution request into a proposed manifest diff before code. Preserve
the current manifest and release as ancestors; rollback creates a new revision
rather than erasing history. Obtain owner confirmation before a revision changes
the core ritual, privacy, disclosure, identity, recovery, ownership, cost, or
external authority. Keep owner/update authority distinct from runtime practice
identity unless an accepted contract explicitly relates them.

If the capsule is Anky-operated but not approved, stop at an application-status
and review package. Do not infer production authorization.

### 2. Interview for one observable action

Ask compact questions, one at a time when a human is available. Do not suggest
screens before the action is clear. Determine:

1. Who reaches for the app, and in what moment?
2. What do they long to practice, repair, notice, or become through return?
3. What one physical or digital action can they perform now?
4. What observable event starts it?
5. What deterministic condition completes it?
6. What interrupts it, and should partial work persist or resume?
7. What immediate feedback belongs after completion or interruption?
8. Is reflection absent, local, or remote; opt-in or automatic; and what leaves
   the device at what consent moment?
9. What becomes meaningful after days or months without becoming a dashboard?
10. When does contextual practice identity appear, and how are identity and
    content recovered separately?
11. Which artifacts, reflections, or narrowly worded proofs can the owner
    export, and to whom?
12. What may payment unlock without gating action, local record, recovery, or
    owner export?
13. What would make the ritual distracting, performative, extractive, generic,
    or shameful?
14. Which platforms, server capabilities, ownership mode, and deployment
    accounts are actually required?

Press for observable conditions when answers say only “be mindful,” “improve,”
or “finish.” Do not turn a longing into a feature list.

Produce this short confirmation record before code:

```text
AppName: <one sentence naming the one action and its purpose>

Starts when:
Completes when:
Interrupts when:
Partial action:
Immediate feedback:
Reflection and consent:
Accumulates:
Return invitation:
Private boundary:
Identity recovery:
Content recovery:
Exports and proofs:
Must never become:
Assumptions and open decisions:
```

Do not scaffold until the owner or authoritative capsule confirms the core
action, completion, interruption, partial-action behavior, reflection
disclosure, and ritual-destroyer list. When no human is available, preserve
uncertainty explicitly and stop before a choice that would materially change
the product.

### 3. Produce and validate the manifest

Write `continuity.manifest.json` using schema version `0.1.0`. Keep the three
boundaries distinct:

- `runtime` contains behavior and privacy properties code must enforce.
- `guidance` constrains coding-agent judgment, visuals, tone, and refusal.
- `operations` records ownership, deployment targets, ejection, and approval
  boundaries; it cannot weaken runtime guarantees.

Require one application identity, target human, one-sentence core action, input
kind, completion, interruption, checkpoint behavior, feedback, accumulated
continuity, return invitation, privacy, identity, recovery, artifacts, and
forbidden patterns. Add reflection, proofs, synchronization, and payments only
when explicitly chosen.

Validate with the repository validator and tests. Resolve every error. Surface
warnings rather than hiding them. Confirm that:

- core action, checkpoint, local commit, and return work offline;
- action precedes account and profile creation;
- sealed artifacts are immutable and event IDs do not derive from hashes;
- reflections are separate, consent-attributed, independently deletable records;
- recovery of identity is separate from content, subscription, ownership,
  backup, and reflection-history restoration;
- the control plane receives operational metadata, not end-user continuity
  content;
- payment never gates the core action, local record, recovery, or owner export;
- every application is ejectable from birth.

If a requested change cannot be represented as a valid manifest diff, classify it
as unsupported rather than silently turning it into custom agency work.

For an unsupported request, name the requested behavior, the missing or violated
manifest field, the continuity/privacy invariant at risk, and the smallest
supported alternative. Do not promise bespoke work outside the product
contract.

### 4. Model privacy and continuity

Before scaffolding, create an implementation inventory with these columns:

| Asset | Canonical local location | Leaves device? | Recipient and purpose | Retention/deletion | Recovery |
|---|---|---|---|---|---|
| Practice secret | Secure-secret adapter | Only in explicit recovery envelope | Owner-selected destination | Explicit | Manifest identity policy |
| Active checkpoint | Private local repository | No by default | — | Commit or explicit discard | Crash recovery |
| Event and artifact | Private local log/blob store | Only declared export/reflection/sync | Named recipient | Enumerated | Content policy |
| Reflection | Separate local record | Input only as declared | Named provider | Independently deletable | Explicit export/backup |
| Proof | Only if enabled | Owner-selected | Chosen verifier | Public disclosure may be irreversible | Export |
| Operational metadata | Control plane, if required | Yes | Named operator | Minimal retention | Not content recovery |

Use precise language:

- A digest detects byte changes; it is not authorship proof.
- A practice-key signature proves key control over a scoped statement; it does
  not prove human action or honest elapsed time.
- Platform-private storage is not application-level encryption.
- Provider zero-retention configuration is not mathematical deletion.
- Public or blockchain publication is durable, linkable disclosure.
- Source parity between native platforms does not prove runtime parity.

If this inventory conflicts with the manifest, update and reconfirm the
manifest before code. Do not silently resolve open questions such as fragment
reflection, generic identity cryptography, application-encryption threat model,
cross-app identity, Android off-device recovery, optional chain/payment/gate
modules, or generated-app licensing.

### 5. Refuse generic product gravity

Run a ritual-lint pass on every proposed feature. Require it to support the one
action or one transition in `act → record → reflect → continue`.

Reject by default:

- a generic dashboard home instead of the action;
- profiles, account creation, authentication, or recovery before first value;
- social feeds, followers, public-by-default records, or leaderboards;
- generic CRUD for events and artifacts;
- unrelated AI chat or an open-ended assistant tab;
- analytics that centralize private content or broad third-party SDKs;
- manipulative streaks, shame, loss aversion, or notification campaigns;
- settings matrices, admin panels, and feature-rich editors before the ritual;
- payments, blockchain, paintings, gates, or subscriptions unrelated to the
  manifest;
- cloud synchronization added merely because storage exists.

Do not accept a rejected pattern because it is conventional. If it is genuinely
the core action or a required coordination surface, encode its lifecycle,
privacy, interruption, ownership, and consent effects as a manifest diff and
obtain confirmation first.

### 6. Scaffold the smallest offline vertical slice

Build in this order:

1. Reach the native or web action from a fresh install without login.
2. Start and progress through the declared input adapter.
3. Checkpoint locally at the manifest durability boundary.
4. Detect completion and interruption through deterministic policy code.
5. Commit exactly one immutable event and its sealed artifact or references.
6. Recover after backgrounding, process death, and partial writes.
7. Render immediate feedback only from committed local state.
8. Invite continuation at the declared rhythm.
9. Offer identity recovery and content recovery at their separate declared
   moments.
10. Add export, then optional reflection, proof, synchronization, payment, and
    remote services in that order only when declared.

Subscriber, network, entitlement, provider, and AI failures must not roll back a
committed event. Use stable event IDs for idempotency. Never mutate a sealed
artifact to prepare reflection; create a derived artifact with an explicit
relation if transformation is unavoidable.

Do not generate placeholder dashboards, profiles, feeds, CRUD panels, analytics
suites, or fake integrations. Keep platform storage, lifecycle, secure-secret,
and composition-root behavior explicit; do not hide native differences behind
an unproven TypeScript abstraction.

### 7. Test manifest invariants

Generate executable acceptance cases from the manifest and contract fixtures.
At minimum verify:

- fresh install reaches the action before identity/account UI;
- offline action, checkpoint, completion, local commit, export, and return;
- process death restores or explicitly handles partial action;
- one seal transition creates exactly one stable event;
- completion and interruption remain distinct;
- sealed artifact bytes, byte length, and digest cannot mutate;
- Unicode and canonical bytes round-trip without accidental normalization;
- reflection links to event and optional artifact with consent/provider
  provenance and can be deleted independently;
- no private artifact appears in logs, telemetry, crash reports, URLs, payment
  metadata, or control-plane records;
- recovery never silently reassigns identity or content ownership;
- payment/provider/subscriber failure leaves continuity intact;
- method, actual path, exact body hash, timestamp, nonce, and signer are bound by
  versioned request envelopes when signing is present;
- deletion reports device, backup, server, provider, and generated-asset
  locations;
- every target platform's live composition root uses the intended adapters;
- a clean export and ejection build work without TOHSENO credentials.

Run schema validation, unit and lifecycle tests, native builds, offline/failure
walkthroughs, accessibility and permission checks, migration/rollback tests,
secret scans, dependency audits, and changed-file review appropriate to the
targets. Explain any skipped critical test; do not call the product ready while
critical invariants are unverified.

### 8. Prepare deployment without taking ownership

Never create paid infrastructure, spend money, alter DNS, submit to application
stores, or rotate production credentials without explicit owner approval.

Prepare exact build/test/migration commands, environment-variable names without
values, data/backup topology, health checks, provider privacy inventory,
permissions and entitlements, store privacy labels and review notes, bundle and
package identifiers, signing prerequisites, rollback, deletion verification,
and post-deploy smoke tests. Include an offline-local-value test and a path that
disables every optional server without breaking continuity.

For client-owned mode, verify Apple Developer organization and Google Play
Console readiness, request scoped access instead of passwords, confirm domain
and DNS ownership, confirm the intended infrastructure account, and preserve
the client's bundle IDs, package IDs, source, domains, signing identities,
infrastructure, and data plane. Stop for owner approval before cost or store
submission.

For self-hosted mode, return a reproducible owner-run deployment and do not
retain required credentials. For Anky-operated mode, require recorded adoption
approval before production work; selective review is not automatic publishing.

### 9. Produce the ejection package

Return:

- every source repository and exact commit;
- original prompt, owner-authorized evolution history, locked/current manifest
  history, generated acceptance tests, and runbook;
- ownership map for source, bundle/package IDs, domains, stores, services,
  databases, signing identities, and data;
- build, test, migration, backup/restore, deployment, rollback, and deletion
  instructions;
- production URLs and environment-variable inventory without secret values;
- scoped credential handoff through the owner's approved secret manager;
- provider/export formats, data-location inventory, and known limitations;
- proof that a clean owner-controlled environment can build, operate, update,
  export, delete, and redeploy without TOHSENO credentials.

Customers stay because the system works, not because leaving is impossible.

## Completion gate

Call the application deployment-ready only when the manifest is confirmed,
privacy inventory agrees with code, the local ritual works offline, invariants
are green on each claimed target, ownership is recorded, and deployment,
rollback, deletion, recovery, and ejection instructions are executable. For an
evolved application, require a safe trace from owner intent through manifest
revision to verified release evidence. Report
store review and external-provider behavior as outside TOHSENO's control.

End with an honest list of what works, what remains unimplemented or unverified,
which approvals are still required, and the single next action for the owner.
