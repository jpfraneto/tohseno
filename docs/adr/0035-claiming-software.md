# ADR 0035: Claiming software

Status: accepted

Date: 2026-08-30

Supersedes:

- ADR 0034 only where it restricts BuilderAccount deployment to a first public
  Builder action, treats Install as the Registry's primary acquisition action,
  or leaves Registry as a catalog rather than a timeline of software events;
- ADR 0026 only where Registry contains a quick New Shot composer; and
- prior product copy only where it calls every public release a Ship.

It retains ADR 0034's person-to-person source network, active generation-0.8
ShotRegistry, BuilderAccount authority, exact catalog and receipt verification,
recipient-local Xcode signing, public-source honesty, fork permission, private
installation identity, and physical proof requirement. It changes no frozen
protocol encoding and no deployed generation-0.8 ABI.

## Context

ADR 0034 made native software capable of traveling from one person to another.
It established a trustworthy separation: the Mac is the factory, Companion is
the human authority, and generation-0.8 Registry state plus a signed catalog is
the public witness. The network still lacks a durable act by which another
Tohseno identity can say that it encountered a particular app at a particular
point in that app's life.

Installation is too temporary for that role. It depends on a Mac, Xcode,
Apple signing, provisioning, a cable, a reachable phone, and capabilities the
recipient may not have. It also remains private by design. Payment and generic
wallet ownership are wrong abstractions: Tohseno is moving software, not
creating inventory or a marketplace.

The product also needs a truthful sense of time. A Shot currently enters the
public network and may later publish more releases, but describing every
release as another shipment erases the distinction between birth and change.
The Registry can become a living place only when those facts have stable names.

## Decision

Tohseno adds one public primitive: **Claim**.

A Claim says:

> This Tohseno identity encountered and claimed this Shot at this exact point
> in its life.

A Claim is public, intentional, signed through the claiming Tohseno smart
account's Companion DeviceKey authority, gas-sponsored by Tohseno, recorded on
Robinhood Chain, and represented by one non-transferable ERC-721 receipt. It is
bound to one Shot, the exact release and public checkpoint encountered, and one
expressive Claim-mark commitment. One Tohseno account may Claim a Shot at most
once. The relationship survives updates, signing refreshes, reinstalls, and
replacement Macs or phones so long as the same account authority is recovered.

A Claim is not payment, purchase, license enforcement, Apple installation
proof, proof of unique humanity, generic wallet activity, transferable
inventory, Shot ownership, or Builder authority. It does not make public source
scarce and does not override install safety or fork permission.

### Birth, change, and lineage

A Shot is shipped exactly once. The first accepted transition from a private or
local candidate to a discoverable network Shot is **Ship** and produces exactly
one `shot.shipped` event. Its immutable `shipped_at` derives from canonical
first-registration evidence rather than application-server time.

Every later accepted public release of that Shot is an **Update** and produces
one `shot.updated` event. An update never produces another Ship. The developer
command remains `tohseno deploy`, but an unshipped candidate ends with
`Shipped.` and an already shipped Shot ends with `Updated.`

A Fork fixes the exact parent release and creates a different random ShotID.
The child remains private until explicitly deployed. It is then shipped once,
opens its own Claim Edition, and may produce one derived `shot.forked` event in
addition to—not instead of—its one birth event. Nothing about the parent is
republished.

### One immutable Claim Edition per Shot

Every newly shipped public Shot opens exactly one Claim Edition. The Builder
selects its policy as part of the exact first-Ship Companion approval. The
policy belongs to the Shot, not a release, and is immutable after opening.
Updates, profile and alias changes, or controller transfer cannot reopen,
replace, reset, extend, or otherwise alter it.

This release supports exactly four policies:

1. Open: `maxClaims = 0`, `closesAt = 0`.
2. Limited: `maxClaims > 0`, `closesAt = 0`.
3. Timed: `maxClaims = 0`, `closesAt` is a future timestamp.
4. Limited and timed: both values are nonzero; the edition closes at the first
   exhausted boundary.

Zero supply means unlimited and zero closing time means never. Scarcity is per
Tohseno identity, not physical device or proven person. There are no prices,
quantities, auctions, tiers, allowlists, per-release editions, paid mints, or
edition administration.

### Additive Claims contract

Generation-0.8 `ShotRegistry`, `BuilderAccountFactory`, `BuilderAccount`, and
`P256Verifier` remain byte-for-byte unchanged. Claims are implemented by one
additive, non-upgradeable `TohsenoClaimsV1` contract referencing the exact
active ShotRegistry. It is not a Registry generation and has separate signed
activation evidence.

The contract has no owner mint, policy editor, supply override, pause,
confiscation, arbitrary URI replacement, or upgrade path. A defect requires a
truthful successor or abandonment. It stores one immutable edition per Shot,
one immutable Claim record per token, account-and-Shot uniqueness, per-Shot
sequential claim numbers, global token IDs beginning at one, and independent
nonces for edition and Claim actions.

An edition may open only for an existing Shot. The submitted
`OpenClaimEdition` EIP-712 action binds chain, Claims contract domain, trusted
ShotRegistry, ShotID, immutable policy, exact current controller, nonce, and
deadline. The contract reads the current controller and verifies the action
through that controller's ERC-1271 authority. Any address may relay it.

A `ClaimSoftware` EIP-712 action binds chain and contract domain, trusted
ShotRegistry, ShotID, claimant Tohseno account, exact release digest, exact
current public checkpoint digest, gesture commitment, claimant nonce, and
deadline. The claimant must be a deployed contract account whose ERC-1271 path
accepts the exact signature. The Claims contract verifies that the supplied
checkpoint remains the current ShotRegistry head at execution time. A changed
head, duplicate Claim, closed edition, exhausted supply, stale nonce, expired
deadline, wrong contract or chain, or invalid account signature reverts.

Claim number and token ID are allocated only by canonical contract execution.
Limited-edition races use chain ordering; there is no reservation or predicted
number. Minting performs no arbitrary recipient callback. ERC-721 transfer and
approval paths revert so the semantic receipt can never become inventory.

The contract emits `ClaimEditionOpened` and `SoftwareClaimed` with enough exact
data to reconstruct Claims without trusting catalog metadata. It stores no app
name, description, icon URL, handle, source, raw gesture geometry, device, or
installation information.

### Activation and gas sponsorship

Clients trust Claims only through a separately versioned signed activation
record binding chain ID, contract address, runtime code hash, exact expected
ShotRegistry, source version/generation, deployment transaction and block, and
an ordered activation digest. A copied configuration value is never authority.
Clients fail closed when activation, runtime code, constructor state, or the
active Registry witness disagree.

Normal users see no gas, fee, RPC, chain selector, seed import, wallet-connect,
or balance ceremony. The existing constrained relayer may add only exact
allowlisted BuilderAccount bootstrap, edition-open, and Claim calls. It stores
jobs before submission, retries idempotently, rate-limits abuse, and never
holds a DeviceKey. Claims are not shown as confirmed until canonical chain
evidence exists.

The existing deterministic BuilderAccount is the one Tohseno smart account for
both shipping and claiming. It is deployed lazily for the first public Tohseno
action—Ship or Claim, whichever comes first. Technical surfaces retain
`BuilderAccount` and `BuilderID`; product surfaces may say **Tohseno address**
until the account controls a Shot. No unrelated EOA or second identity is
created.

### Claim privacy

Claiming deliberately makes the account-to-Shot relationship public. Before a
first Claim, Companion gives one compact disclosure that the Tohseno address
will be associated with the app on Robinhood Chain. That disclosure is not a
claim of unique humanity.

Installation remains private. A Claim never publishes the physical iPhone,
InstallationKey, Apple team, Mac or pairing identity, IP address, source path,
private intention, prompt, device name, or install evidence. Public profile
pages do not automatically aggregate everything an account has claimed even
though the chain facts and direct receipts are queryable.

### The Claim mark ritual

Companion is the primary Claim surface. The fixed ritual is **Draw a circle
around the app**: the artifact is centered on a quiet field and the person draws
one continuous, forgiving loop. Acceptance checks intentional distance,
substantial enclosure of the artifact center, and endpoint return near the
start. It does not score beauty, circularity, handwriting, or identity.

During the stroke only canvas-local x/y positions needed for presentation are
held. On completion they are normalized to a unit canvas, resampled by arc
length to exactly 64 points, quantized to a closed fixed-width
`tohseno.claim-mark/1` encoding, and SHA-256 committed. Touch timestamps,
velocity, acceleration, force, radius, azimuth, altitude, device motion, and
behavioral inference are neither persisted nor transmitted. Raw points are
discarded after canonical geometry exists. The gesture is expression, not
entropy or a signing key; DeviceKey authorization remains cryptographic proof.

The accepted path freezes and settles the visible mark, produces one restrained
haptic, says `Claiming…`, signs and relays the exact Claim, and waits. Only a
canonical mint changes the state to `Claimed` with the on-chain number. The
actual normalized mark remains part of the receipt.

When drawing is not reasonably available, accessibility interaction offers one
intentional **Hold to close the circle** alternative. It produces a canonical
accessibility Claim-mark representation rather than fabricated hand geometry.
VoiceOver names the product action, not EIP-712 or NFT mechanics.

### Claim, then prepare the exact encounter

Claim and Install remain separate truths. Normal Registry Install and Fork
experiences begin from a confirmed Claim, while permitted public source may
still be inspected without one. A Claim can succeed even when Xcode, signing,
entitlements, Trust, Developer Mode, cable, or hardware prevent installation;
that fact never undoes the Claim.

Canonical confirmation automatically persists the existing network-install
intention for the exact release recorded by the Claim. It does not ask whether
to send the app to the Mac and does not create a second downloader or build
system. The Companion durable outbox may wait while the Mac sleeps or is
offline. When the Mac returns, it independently verifies that exact release,
downloads and prepares it, and reports the existing truthful build/install
states. If a newer update exists, the claimed release is still prepared first
and `Update available` is a separate fact.

Human states are bounded to Claim, Drawing, Claiming, Claimed/Waiting for your
Mac, Claimed/Preparing on your Mac, Claimed/Ready for your iPhone,
Claimed/Installed, Claimed/Update available, and Closed. No button press or
pending transaction is reported as Claimed.

### Registry becomes the living world

Your Apps remains the workshop: source, Build/App/Source, simulator, iPhone,
evolution, and Ship/Update. Registry removes its quick New Shot composer and
local-app grid. Its primary modes are exactly **Discover**, **Following**, and
**Updates**, plus Search.

Discover is a deterministic public timeline of software facts, not human
posts or an App Store grid. The closed event set is `shot.shipped`,
`shot.updated`, `shot.forked`, and `claim.edition_closed`. Events reference
their catalog and chain evidence, order by canonical block/receipt facts, and
are reorg-aware and idempotent. Individual Claims do not flood Discover;
current counts and edition closure summarize that activity.

Following filters bounded public events locally by privately held exact
BuilderIDs. Follow and Unfollow are idempotent preference operations in the
existing encrypted Mac/Companion relationship. They feel immediate on either
device, queue durably while the peer is offline, reconcile in both directions,
and survive handle changes. There is no public follow server, graph, count,
leaderboard, popularity score, token, or notification-per-update behavior.

Updates is a durable, private, high-signal inbox for facts where the person's
own relationship or action matters: a claimed app changed, preparation became
ready, a fork of their Shot shipped, their finite edition closed, an alias
changed, publication needs approval, or evolution completed. Stable IDs,
idempotent insertion, read state, and paired-device reconciliation prevent
restart spam. Generic Discover events and individual Claims do not enter it.

Every Shot page gains a timeline containing exactly one birth and subsequent
updates, fork relations, Claim history/count, and edition closure. Builder
profiles show identity, attestations, shipped software, activity, and private
Follow state—never follower counts. A person's own Companion Profile becomes a
cabinet of claimed software with the exact encounter, mark, Builder, release,
and expandable chain details.

### Website and service

`/registry` becomes the public Discover timeline. The canonical Shot page gives
the artifact room and changes its primary action from Install to Claim. It
renders exact chain-backed edition state and safely deep-links iPhone to
Companion; Mac asks the paired iPhone to Claim. Without Tohseno it offers the
product, never wallet connect. Builder pages deep-link a bounded exact
BuilderID for private Follow. A canonical direct Claim-receipt route and stable
token-metadata API render immutable encounter facts and normalized mark without
prices, trading, transfer, rarity, or marketplace chrome.

The Registry service adds closed read models for Claims activation, editions,
account-and-Shot Claim state, token receipts and metadata, paginated Shot
Claims, public timeline, and Shot timeline. Write orchestration prepares and
submits only exact edition-open and Claim authorizations. Prepare responses are
structured facts sufficient for Companion to independently validate and
recompute the canonical action; the server never asks Companion to sign an
opaque digest and never signs for a claimant or Builder.

The canonical indexer records edition and Claim events with transaction, block,
log ordering and reorg handling. Pending transactions do not increment counts.
Timed closure derives from immutable policy and canonical chain time without a
fictional close transaction. Existing internal alias-claim types and routes are
made explicit (`AliasClaimRequest`) wherever ambiguity with `SoftwareClaim`
would otherwise exist.

### Release and operational truth

This is the 1.2.0 product line while 1.1.0 release evidence remains frozen to
its recorded source candidate. Backwards-compatible Claims reads and indexing
deploy dark first. The additive contract is deployed only after complete
contract and cross-language verification from a clean committed source. Its
activation, relayer, native clients, and physical Claim are proven before any
write path or Claim advertising becomes live.

Acceptance requires a real production Ship with one edition, a second identity
Claim through the physical Companion ritual, canonical NFT evidence, a Claim
while the Mac is offline followed by automatic exact-release preparation, a
physical recipient-signed install, one later Update that preserves the Claim
and edition, private Follow reconciliation, live website/API receipt paths, and
one-Ship timeline proof. Source code, local tests, simulator UI, a pending
transaction, or operator database rows cannot substitute for those facts.

## Consequences

Tohseno gains a durable encounter without turning software into a financial
asset. The Registry can express birth, change, lineage, encounter, and time
while Your Apps stays a practical machine. People can notice Builders without
being turned into audience metrics, and an app can matter before Xcode and a
cable are available.

The public account-to-Claim relationship creates real privacy and Sybil
limits. One-per-Tohseno-identity is not proof-of-personhood or Sybil resistance.
Gas sponsorship needs constrained operational funding and abuse controls.
Immutable contracts and editions make mistakes expensive, so activation must
remain fail-closed and separately governed.

Most importantly, the product's verbs stop collapsing into one another:
software is shipped once, updated forever, claimed once per identity, prepared
on a Mac, and installed only when a physical device proves it.
