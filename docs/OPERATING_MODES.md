# Operating modes

The operating mode decides who owns production assets, who performs operations, when payment is offered, and what the current vertical slice releases. It does not change the privacy rule that future end-user continuity content stays outside the TOHSENO control plane.

## At a glance

| Concern | Self-hosted | Client-owned | Anky-operated |
|---|---|---|---|
| Price | $88 once | Founding price: $888 setup + $88/month | Selective; no automatic Checkout |
| Source and generated work | Customer | Customer | Determined only after explicit acceptance and agreement |
| Developer/store accounts | Customer/operator chosen by customer | Customer organization | Anky, Inc. if adopted, subject to acceptance |
| Bundle/package IDs | Customer | Customer | Anky, Inc. if adopted |
| Domain and DNS | Customer | Customer | Anky, Inc. if adopted |
| Infrastructure/data plane | Customer | Customer | Anky, Inc. if adopted |
| TOHSENO role | Supplies capsule, contracts, runbook | Operates through scoped customer access | Reviews for genuine first-party adoption |
| Initial submitted path | `READY_FOR_PAYMENT` | `READY_FOR_PAYMENT` | `ANKY_REVIEW` |
| Verified-payment outcome now | Private capsule becomes `READY` | Order becomes `NEEDS_CREDENTIALS` | Not applicable |
| Automatic publication | Never | Never | Never |

Prices are current product configuration, not architecture constants. They must be changed through the centralized site configuration so page copy and payment behavior remain consistent.

## Self-hosted

**Offer:** $88 once.

The customer receives a private capsule, source contract, and operator instructions. Their chosen coding agent or team owns the implementation and operation. TOHSENO does not silently create accounts, infrastructure, domains, or store submissions.

The first-slice payment lifecycle is:

```text
SUBMITTED
→ READY_FOR_PAYMENT
→ PAYMENT_PENDING
→ PAID
→ MANIFEST_LOCKED
→ GENERATING
→ READY
```

`GENERATING` is the deterministic assembly of the current private capsule/source contract. `READY` means that capsule is available. It does not mean native source, binaries, hosted infrastructure, or store listings already exist.

Capsule release is monotonic: after verified payment reaches `READY`, a later operational `FAILED` state does not withdraw the owner’s source/ejection path. Capability expiry or explicit revocation can still end bearer access.

The capsule instruction tells the owner's agent to preserve the manifest, follow the TOHSENO continuity-app skill and runbook, ask before paid or externally consequential actions, and return all repositories, credentials, URLs, ownership details, and ejection instructions to the owner.

Self-hosting transfers operational responsibility. The owner must configure secrets, backups, TLS, provider accounts, incident response, deletion handling, and any future app data plane.

## Client-owned

**Offer:** founding price of $888 setup plus $88/month.

The customer owns:

- Apple Developer and Google Play Console organizations;
- bundle IDs, package IDs, application identities, and signing relationships;
- source repositories and generated work;
- domain and DNS;
- infrastructure and billing accounts;
- runtime database, storage, backups, and future user data plane.

TOHSENO operates through explicit, revocable, least-privilege access. Prefer organization invitations, repository roles, API tokens scoped to required operations, and audited infrastructure roles. Do not ask customers to send account passwords, recovery phrases, or unrestricted root credentials.

The first-slice payment lifecycle is:

```text
SUBMITTED
→ READY_FOR_PAYMENT
→ PAYMENT_PENDING
→ PAID
→ MANIFEST_LOCKED
→ NEEDS_CREDENTIALS
```

At `NEEDS_CREDENTIALS`, the customer-facing instruction requires checks for:

1. Apple Developer organization readiness.
2. Google Play Console readiness.
3. Scoped access rather than passwords.
4. Domain and DNS ownership.
5. The intended customer-owned infrastructure account.
6. Human approval before spending money or submitting to stores.

The service expectation begins only after all required access and credentials are ready:

> Your source contract, infrastructure plan, store materials, and production candidate are expected within eight hours after all required account access and credentials are ready.

Public App Store and Google Play availability follows platform review and cannot be guaranteed within that window. TOHSENO does not control platform approval.

Reaching `NEEDS_CREDENTIALS` releases the client-owned capsule once. Later production/QA failures do not remove it; access remains governed by the capability’s expiry and revocation state.

## Anky-operated

**Offer:** selective.

Anky, Inc. may adopt, publish, support, and operate an application as a genuine first-party product. This is an application review, not an automatic publishing service, marketplace upload, or promise of acceptance.

The initial lifecycle is:

```text
SUBMITTED → ANKY_REVIEW
              ├─ ANKY_ACCEPTED
              └─ ANKY_DECLINED
```

Anky-operated submission never creates a Checkout session automatically. While review is pending, the status page shows application state without exposing a production capsule. `ANKY_ACCEPTED` records the approval gate and releases the capsule, but does not authorize paid infrastructure, DNS changes, store submission, or credential changes by itself; those still require the owner/operator approvals applicable to the adoption agreement.

If declined, the source remains private and the capability remains subject to normal expiration/revocation/deletion handling. Decline is not permission to publish the source or contact details.

## Approval boundary

In all modes, coding agents and operators must stop for explicit owner approval before:

- creating a paid resource or accepting a paid plan;
- changing DNS or transferring a domain;
- creating, rotating, or revoking production credentials;
- inviting external organizations into customer accounts beyond agreed scope;
- submitting an application or update to Apple, Google, or another store;
- publishing a package, repository, artifact, proof, or private source document;
- initiating an irreversible data migration or deletion not already requested.

Preparing a dry-run, access checklist, store material, infrastructure plan, or exact command is not the same as taking the action.

## Changing modes

Mode changes conflate ownership, payment, credentials, and data responsibility, so they are not an ordinary state transition. The first slice does not silently convert modes. A future mode-change workflow must create a reviewed ownership migration that accounts for source, identifiers, provider contracts, infrastructure, backups, subscriptions, and capability access before changing the stored mode.
