# Tohseno wireless-first and network-trust evolution report

Date: 2026-09-01

Status: implemented and locally verified source; not a production activation or
physical acceptance record

## Outcome

This pass makes one narrow, useful slice of the evolutionary direction real:

```text
Builder chooses a stable app slug
  -> Companion approves the exact Ship
  -> Registry publishes the exact release and source
  -> Builder signs a root-alias request for that exact app
  -> operator revalidates and approves the immutable request
  -> tohseno.com/app-slug explains the exact recipient path
  -> recipient Claims the exact release
  -> recipient Mac verifies, builds, signs, and retains the artifact
  -> the one safely resolved paired iPhone receives it over an available
     Xcode-supported transport
```

The same pass defines, but does not pretend to implement, the next two trust
objects: content-addressed machine Verification Reports and DeviceKey-signed
human Release Attestations. The Registry page now makes the honest current
state legible: signed provenance and bounded build facts exist; human release
attestations do not.

No frozen protocol byte, generation-0.8 ABI, Claims ABI, contract activation,
release artifact, production service, relayer, Claim, alias, or physical
installation changed merely because this source changed.

## What the repository actually contained

### Authority and public identity

`protocol/` remains normative. Generation 0.8 is the active public Shot witness.
The off-chain catalog binds a DeviceKey-signed release to its ShotID, BuilderID,
release digest, public checkpoint, deterministic source archive, Xcode build
recipe, and declared permissions. The Builder DeviceKey on Companion remains
the human publication authority.

ADR 0035 already separated four facts that must stay separate:

- Ship is the one birth of a public Shot;
- later public releases are Updates;
- Claim is a public, non-transferable encounter with an exact release; and
- Install is private physical evidence on a recipient's own Apple devices.

The additive Claims contract and activation are live for verified reads, while
the constrained Claims and Registry write paths remain dark outside the
owner-attended acceptance window.

### Delivery

The implementation was further ahead than its product language. CoreDevice
inventory already accepted an active paired `localNetwork` tunnel as reachable,
as well as USB/wired/direct transports. The factory already retained a verified
build when no usable phone was present and retried installation without running
the coding harness again. A canonical Claim could already queue exact-release
preparation while the recipient Mac was offline.

The safety limitation was equally clear. Initial setup persisted a one-way
digest of the exact CoreDevice that received and launched Companion, but later
app delivery ignored it. Multiple phones failed closed; one different phone
could nevertheless be selected when it was the only reachable device and the
intended Companion phone was absent.

Apple's current [Device Hub documentation](https://developer.apple.com/documentation/xcode/pairing-your-devices-with-your-mac)
supports network pairing on current systems and running a cable-paired device
through Xcode over Wi-Fi, while still requiring a cable for older first-pairing
environments. The architectural fact
is therefore **target identity first, transport second**, not “the cable is the
target.”

### Registry and human links

The signed catalog already had an optional `app_slug`, and Registry code already
understood Builder-local app routes. In practice:

- `tohseno deploy` always signed `app_slug: null`;
- Companion could sign a global alias claim but silently chose the first owned
  app;
- the server persisted the claim only as `pending_policy_review`;
- no bounded approval operation existed; and
- the website server never dispatched a root path such as `/anky` to the
  Registry renderer, so even a manually created pointer would not render.

The canonical `/s/<ShotID>` page exposed source and Claim actions but did not
give a friend a small, comprehensible Mac → Companion → Claim → local build →
iPhone path.

### Trust evidence

The Registry already possessed strong provenance facts and a deliberately
narrow build classification. It did not possess signed human review evidence,
external identity proof, Farcaster relationships, a deterministic public
Verification Report, or a Release Attestation. The signed Builder profile
reserved external attestations but correctly refused non-empty unverifiable
claims.

That was the right fail-closed starting point. Claim could not be stretched to
mean review, endorsement, installation, ownership, or safety.

## Architectural decisions

Two additive accepted decisions now govern the direction:

- [ADR 0036](docs/adr/0036-destination-driven-apple-delivery.md) defines a
  private Install Target as the intended iPhone and treats USB and local network
  as observed transports. It specifies safe bootstrap, ambiguity, replacement,
  privacy, migration, and physical acceptance requirements.
- [ADR 0037](docs/adr/0037-network-mediated-release-trust.md) defines exact-
  release Identity Bindings, Verification Reports, Release Attestations, and
  private social context. It forbids opaque safety scores, inherited reviews,
  cosmetic identity checks, and external services becoming Builder authority.

These ADRs describe additive local/catalog evidence. They do not amend protocol
encodings or authorize production activation.

## What changed

### Protocol and domain model

- No file under `protocol/` changed.
- No contract, ABI, EIP-712 action, Claim encoding, Claim mark, public
  checkpoint, or generation coordinate changed.
- Local `ProjectPublication` records gained an optional `app_slug`. Old records
  decode with `None`; completed new publications retain the verified catalog
  slug.
- Internal device inventory renamed the absence state from `CableMissing` to
  `DeviceUnreachable`. The advanced `tohseno doctor/1` projection now emits
  `device_unreachable`; current consumers of the old diagnostic value must
  migrate.
- The Registry gained append-only local index records for pending alias claims
  and approvals. They are website storage schemas, not normative protocol
  schemas or on-chain ownership.

### Builder deployment and slug stability

`tohseno deploy` now accepts:

```sh
tohseno deploy --app-slug your-app
```

The slug is constrained to 2–64 lowercase ASCII letters, numbers, and single
hyphens. Website-reserved roots are rejected. If omitted, the CLI derives a
bounded slug from the app display name, falling back to an opaque Shot prefix.
Once a publication completes, later Updates must retain the same slug.

The slug enters the exact catalog release before DeviceKey approval, so a server
cannot substitute it after Companion signs. Companion's Ship sheet derives and
shows the release slug from the already validated canonical catalog JSON, while
making clear that a root alias is a separate later request.

After canonical completion, `tohseno status` now shows the exact public release,
stable slug, prospective friend route, and the Companion Profile action needed
to request review. It no longer tells a Builder whose release just completed to
run another deploy without explaining the alias step. It calls the route live
only after the public alias record binds the expected slug and Shot, the
Registry's current Shot projection binds the exact locally published release
digest and canonical route, and a HEAD request confirms the human page renders.
A later or otherwise different release becomes a **do not send** conflict rather
than an optimistic link; the Builder must reconcile and rerun status.

### Companion authority

The Profile alias surface now:

- lists only installable releases controlled by the active BuilderID;
- requires the Builder to select the exact app/Shot;
- defaults an empty alias field to that release's signed `app_slug`;
- signs the existing bounded alias claim with the DeviceKey;
- validates the server's pending receipt; and
- exposes the immutable request ID as selectable text for operator review.

Companion still does not grant the alias itself. The Registry operator cannot
invent a Builder request, and neither party can overwrite a different approved
owner.

### Registry and website

The site has a new optional `REGISTRY_ALIAS_REVIEW_TOKEN_SHA256` configuration.
Malformed configuration or enabling review without the Registry aborts startup.
The raw token is never configured or stored—only its lowercase SHA-256 verifier.

The authenticated approval operation accepts exactly one action:

```text
POST /api/registry/v1/alias-reviews/<request-id>
{"decision":"approve"}
```

The same authenticated review route supports a read-only `GET` that revalidates
the stored request and returns its alias, Builder, Shot, signer key, request
digest, current exact release, and pending/approved state. The operator can
therefore inspect the actual object before invoking the one mutation.

Before writing anything, it rechecks:

1. the immutable stored request and its DeviceKey signature;
2. request ID, digest, and signer-key consistency;
3. that the server originally received it inside the signed deadline window;
4. current on-chain Builder DeviceKey authority;
5. current Builder control of an installable Shot; and
6. absence of a conflicting approval or alias pointer.

Approval and pointer records are create-only and serialized. Repeating the same
approval is idempotent; a different owner or target is a conflict. Root aliases
are now dispatched by the website while reserved application routes remain
unshadowed.

The exact app page now presents:

- one exact release digest and checkpoint;
- current Claim Edition state and an exact-release Companion deep link;
- signed Builder, source, checkpoint, permissions, build profile, dependency,
  and Apple-signing facts;
- a conspicuous statement that bounded machine observations are not “safe”;
- a conspicuous statement that human Release Attestations are unavailable; and
- four small recipient steps, including Mac/Xcode prerequisites, pairing,
  exact Claim, recipient-local signing, retained artifacts, and Wi-Fi/USB
  reachability.

The Claim action continues to use `tohseno://claim/<ShotID>?release=<digest>`.
A generic Universal Link was not fabricated. Apple's
[Associated Domains model](https://developer.apple.com/documentation/xcode/supporting-associated-domains)
requires a matching app identifier in the website association file and a
matching entitlement in the app. Recipient-built Companion copies may have
different Apple team identifiers, so the explicit web button and custom scheme
remain the honest compatible handoff.

### Mac, delivery, and CoreDevice

Normal user-facing states now say **Make your iPhone reachable**, **Ready for
your iPhone**, and **Wi-Fi or USB**. The cable remains explicit where Apple may
require first pairing, Trust, or retained cable-genesis compatibility. Engine,
CLI, native Mac UI, readiness, install retry, removal recovery, and comments now
use destination/reachability language consistently.

This is supported by existing implementation, not merely copy: `devicectl`
inventory already recognizes a connected local-network tunnel, exact builds are
retained across absence/restart, and initial Companion setup persists a one-way
digest of its physical CoreDevice. Delivery now consumes that digest for both
claimed apps and living-project evolutions. It selects the exact target across
USB/Wi-Fi, other visible phones, and inventory order; when the target is absent,
another phone is never substituted. Older records without the digest retain the
exactly-one-reachable-device fallback.

ADR 0036's complete versioned, multi-target, Companion-ID-bound association is
still not implemented. The current source is a single bootstrap target with no
attended replacement/reset operation, and it has no physical-device acceptance
evidence yet.

### Relay

No relay envelope, capability, mailbox, push payload, or plaintext boundary
changed. The relay transports private durable intent and wake-ups; it never
selects a physical phone, approves a public release, grants an alias, interprets
source, or becomes Builder authority.

### Identity, Farcaster, GitHub, Base, verification, and attestations

ADR 0037 defines these boundaries, but this pass intentionally adds no cosmetic
provider badges or self-asserted “verified” identities:

- DeviceKey remains Builder/reviewer authority.
- Farcaster is designed as optional stable-FID social context, not security
  delegation.
- GitHub is designed as technical identity/provenance context, not a score.
- Base/EVM is designed as economic identity, not truth or authority.
- Verification Reports are designed as content-addressed exact-release machine
  evidence with direct observations separated from interpretation.
- Release Attestations are designed as bounded DeviceKey-signed human statements
  over one exact release and optional report.

The Registry explicitly renders their current absence. That is a working trust
improvement: recipients can distinguish what the network proved from what it
has not yet observed.

## Invariants preserved

- The non-exportable Companion DeviceKey remains sovereign Builder authority.
- External identities, OAuth, social follows, wallets, tokens, and operators do
  not gain Ship, Update, review, Claim, or installation authority.
- A Shot ships once; later public releases are Updates.
- One first-Ship Claim Edition remains immutable.
- Claim remains a public exact encounter, not review, endorsement, ownership,
  installation, transfer, or a safety certificate.
- Exact ShotID, release digest, checkpoint, source digest, and signatures remain
  independently verifiable.
- The recipient's Mac remains the private factory and signs with the recipient's
  Apple identity.
- Installation details and device identifiers remain private.
- The person whose device will run the software retains final authority.
- Frozen protocol history and deployed generation-0.8/Claims contracts remain
  unchanged.
- No release or production write gate is bypassed.

## What is genuinely working

| Evidence layer | Result in this pass |
| --- | --- |
| Rust formatting and lint | `cargo fmt` and workspace Clippy passed with warnings denied. |
| Rust unit/integration | Full locked workspace suite passed across all targets/features. |
| Website | TypeScript passed; 134 Bun tests passed, including signed alias request, authenticated pre-approval inspection, unauthorized rejection, approval, idempotence, conflict safety, immutable public pointer, and an exact Shot-plus-release Companion Claim URI on the root page. |
| Companion | 32 Swift tests passed after exact-app/alias receipt changes. |
| Mac app | 31 Swift tests passed after the final command-copy adjustment. |
| CompanionKit | 24 Swift tests passed with the validated catalog-slug projection and matching global-alias bound. |
| Fascia | 9 Swift tests passed. |
| Contracts | Forge build and 100 tests passed; no contract source changed. |
| Studio deletion/static boundary | 21 Node tests passed. |
| Network integration | `test-network-e2e.sh` passed, including exact Claim/offline queue, signed alias/root-page exact Claim handoff, Registry/Claims, Swift, Solidity, and Simulator-build slices. |
| Friend-page visual/deep link | Browser automation was unavailable in this session. HTML, responsive rules, routing, and exact-link bytes are tested; real Safari/Messages/social-app rendering and custom-scheme opening are not claimed. |
| Apple identity / local-service lifecycle | Apple identity passed 8 tests. The ontology, local-Companion, and macOS-service lifecycle scripts all passed using exact temporary Keychains and unique verification LaunchAgent labels; the login Keychain and real LaunchAgent were untouched. |
| Physical iPhone | Not run; no physical installation evidence claimed. |
| Production relayers and root alias | Not enabled or changed. |
| Owner-attended / clean-Mac | Existing RC6 evidence only; this new source has none. |

The live read-only production surfaces were also checked on 2026-09-01. Health
was ready; the Mac download remained the recorded `v1.2.0-rc.6` candidate and
digest; generation-0.8 Registry reads were available with an empty timeline and
its relayer disabled; Claims activation, runtime code, and indexer agreed while
its funded relayer remained disabled. That is the correct dark-write baseline,
not evidence that these new client/server bytes are released.

Verification mode is now an explicit test-only boundary. It accepts only an
absolute regular non-symlink temporary Keychain, scopes Apple Security add and
lookup operations to that Keychain, refuses Secure Enclave creation there, and
uses a validated non-production service-label namespace. Production still uses
the ordinary Keychain, Secure Enclave Builder DeviceKey, and production
LaunchAgent path; there is no automatic downgrade.

## Designed but not implemented

- durable versioned Install Target records;
- explicit association IDs bound to each install intention;
- attended association replacement/reset and multi-target management;
- physical multi-phone, replacement, and cross-restart acceptance evidence;
- background installation guarantees while the Mac sleeps or Apple withdraws
  reachability;
- generic Universal Links for independently re-signed Companion builds;
- provider-verified Identity Bindings;
- Farcaster FID proof and follow context;
- GitHub account/repository proof;
- Base/economic identity binding;
- deterministic public Verification Reports;
- Companion review and DeviceKey-signed Release Attestations;
- withdrawal, dispute, and compromised-key projections for attestations;
- personalized scoped trust, reviewer history, or reputation projections;
- $TOHSENO verification incentives; and
- the new signed/notarized client, website deployment, production alias,
  canonical first Ship, second-person Claim, and recipient physical install.

## Remaining risks

- **Apple pairing restrictions:** pairing, Trust, Developer Mode, Personal Team
  capacity, provisioning, and OS/Xcode policy remain Apple-controlled.
- **Wireless reliability:** local network discovery and tunnels may disappear;
  the Mac must stay available and the artifact must remain retryable.
- **Device association correctness:** the bootstrap selector uses a stable
  one-way CoreDevice digest and refuses substitution, but it still needs
  physical multi-phone evidence, explicit replacement, Companion-ID binding,
  and recovery from a stale identifier.
- **Farcaster availability:** future social context needs bounded caches and an
  honest unavailable state; it cannot be a hard security dependency.
- **GitHub availability:** provenance proof must retain stable IDs and immutable
  evidence instead of trusting mutable usernames or a live API forever.
- **Identity proof weakness:** provider compromise or cosmetic OAuth must not
  grant DeviceKey authority.
- **Sybil reviewers and collusion:** review count, follows, money, and stake are
  not substitutes for scope, evidence, history, and recipient judgment.
- **Malicious Builders and updates:** every Update begins without inherited
  attestations; prior reputation cannot bless new bytes.
- **Machine-analysis limitations:** absence of findings is not safety, and build
  observations must remain reproducible and tool-versioned.
- **Prompt injection:** untrusted source and metadata are verifier data, never
  agent instructions; future model analysis needs a hostile-input boundary.
- **Privacy:** device association, follows, Apple identity, installations, and
  private trust preferences must not leak into public Registry data or logs.
- **Economic incentives:** payment and $TOHSENO may fund useful work but cannot
  determine truth, authority, rank, or permission to install.
- **Operational authority:** the alias review token and relayer keys are narrow
  production capabilities; leakage or overly long activation windows increase
  risk even though they cannot sign for a Builder.

## Smallest next milestone

Do not add another trust subsystem next. Release these exact source changes as a
new signed candidate, deploy the matching website configuration, and perform
one owner-attended end-to-end proof:

```text
real Builder DeviceKey
  -> one real Ship with signed app slug and Claim Edition
  -> one reviewed root alias
  -> friend opens tohseno.com/app-slug
  -> second identity Claims the exact release while its Mac is offline
  -> Mac later prepares the exact release
  -> cable removed after any required first pairing
  -> one intended iPhone becomes reachable over Wi-Fi
  -> recipient-local signed install and bundle-inventory proof
```

That single physical trace would make more of both north-star statements true
than adding broad unproven identity or reputation features.
