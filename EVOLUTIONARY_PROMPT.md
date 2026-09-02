# TOHSENO ADR 0035 — CLAIMING SOFTWARE
# THE REGISTRY BECOMES A PLACE
#
# EXECUTION PROMPT
#
# OPERATING MODE:
# UNDERSTAND → DECIDE → IMPLEMENT → VERIFY → COMMIT → PUSH →
# DEPLOY DARK → DEPLOY CONTRACT → PROVE → RELEASE ONLY WHEN TRUE.
#
# DO NOT RETURN A PRODUCT PLAN.
# DO NOT STOP AT AN ADR.
# DO NOT BUILD A MOCK REGISTRY.
# DO NOT ADD SOCIAL-MEDIA FEATURES.
# DO NOT REWRITE TOHSENO 1.1 RELEASE EVIDENCE.
# DO NOT MODIFY THE DEPLOYED GENERATION-0.8 SHOTREGISTRY ABI.
#
# THIS EVOLUTION HAS FIVE LAWS:
#
#     A SHOT IS SHIPPED ONCE.
#
#     A SHOT MAY UPDATE FOREVER.
#
#     A TOHSENO IDENTITY MAY CLAIM A SHOT ONCE.
#
#     A BUILDER MAY BE FOLLOWED.
#
#     THE REGISTRY IS THE LIVING WORLD CREATED BY THESE FACTS.
#
# The product state after this work must feel categorically different:
#
#     YOUR APPS = THE WORKSHOP
#     REGISTRY  = THE WORLD
#     APP PAGE  = THE ALTAR
#     COMPANION = THE HUMAN HAND + AUTHORITY
#     PROFILE   = THE MEMORY OF ENCOUNTERS
#
# ============================================================
# 0. PRESERVE THE CURRENT 1.1 TRUTH BEFORE TOUCHING ANYTHING
# ============================================================

You are entering a repository whose 1.1 person-to-person network implementation
already exists in source and whose stable release evidence may still be incomplete.

DO NOT accidentally convert unfinished 1.1 acceptance into 1.2 evidence.

Before editing:

    pwd
    git status --short
    git branch --show-current
    git remote -v
    git log -10 --oneline

Then inspect:

    release/
    docs/STATE.md
    docs/adr/0034-person-to-person-native-software.md
    current version declarations
    current 1.1 readiness record
    current protected tag/ruleset evidence
    current production deployment state

Determine the exact immutable source commit intended for 1.1.0.

If 1.1.0 is not yet released:

1. preserve that exact source commit in the existing readiness evidence;
2. do not rewrite its acceptance requirements;
3. do not claim ADR 0035 behavior belongs to 1.1.0;
4. ensure 1.1 can still be built/released from its recorded exact commit;
5. target this evolution at 1.2.0, or the next correct minor version if
   repository state has advanced.

Do not create `v1.1.0` merely to clear the path for this work.

Do not invalidate physical acceptance already gathered for 1.1.

Do not mix 1.1 and ADR 0035 release evidence.

This is a NEW product evolution.

Expected target:

    Tohseno 1.2.0

if repository version state still matches the current documented state.

# ============================================================
# 1. WORKING TREE SAFETY
# ============================================================

Record the exact initial dirty-path set.

Rules:

- never `git reset --hard`;
- never `git clean`;
- never discard unrelated work;
- never silently stash;
- never overwrite an untracked owner file;
- preserve pre-existing dirty edits in files you must touch;
- stage only work belonging to this evolution;
- inspect `git diff` and `git diff --cached` before every commit;
- never commit secrets, provisioning data, wallet private material, relay keys,
  notarization credentials, RPC secrets, OAuth secrets, or `.env` values.

The same principle applies to product source:

Tohseno may evolve an app only through an explicit human request.
Claiming, following, Registry browsing, installation, publication, or updating
must never implicitly mutate unrelated source or Git state.

# ============================================================
# 2. READ THE CURRENT SYSTEM AS AUTHORITY
# ============================================================

Before designing any implementation, read the actual current versions of at least:

- AGENTS.md
- README.md
- docs/STATE.md
- docs/ARCHITECTURE.md
- docs/LIVING_CONNECTION.md
- current threat model
- ADR 0034
- ADR 0033
- ADR 0032
- ADR 0026
- ADR 0025
- protocol/SPECIFICATION.md
- protocol/CONFORMANCE.md
- active generation-0.8 vectors and schemas
- contracts/README.md
- contracts active generation source
- contract activation records
- current BuilderAccountFactory implementation
- current BuilderAccount implementation
- current ShotRegistry implementation
- current P256 verifier
- current Foundry configuration
- network/
- cli/src/network_commands.rs
- CLI init/deploy/install/fork implementations
- native installation and refresh code
- application/
- engine/
- macos/Tohseno/
- companion/apple/TohsenoCompanion/
- sdk/apple/TohsenoCompanionKit/
- Companion DeviceKey implementation
- Companion relay schemas and command admission
- website/apps/site/
- current Registry service implementation
- current catalog schema
- current signed Builder profile schema
- current alias-claim implementation
- current blob/staging implementation
- current Robinhood relayer
- current chain indexer
- current website app pages
- current Registry Mac UI
- current Registry Companion UI
- current Profile Mac/Companion UI
- existing release workflows
- existing production runbooks

Protocol bytes remain governed by `protocol/`.

Historical ADRs remain historical.

Do not edit ADR 0034 to make history look cleaner.

Supersede current product behavior through ADR 0035.

# ============================================================
# 3. WRITE ADR 0035 FIRST
# ============================================================

Create the next correct ADR, expected to be:

    docs/adr/0035-claiming-software.md

Title:

    ADR 0035: Claiming software

Status:

    accepted

Date:

    current date

ADR 0035 MUST establish the following product evolution.

ADR 0034 made native software capable of traveling person-to-person.

ADR 0035 gives a human a durable act of encounter with that software.

The central primitive is:

    CLAIM

A Claim says:

    "This Tohseno identity encountered and claimed this Shot
     at this exact point in its life."

Claim is NOT:

- payment;
- purchase;
- license enforcement;
- Apple installation proof;
- a generic wallet transaction;
- a transferable speculative asset;
- a replacement for Shot ownership;
- a replacement for Builder authority;
- proof of unique humanity.

Claim IS:

- public;
- intentional;
- signed by the claiming Tohseno identity;
- gas-sponsored by Tohseno;
- recorded on Robinhood Chain;
- represented by one non-transferable NFT;
- bound to one Shot;
- bound to the exact release/checkpoint encountered;
- bound to one expressive gesture commitment;
- singular per Tohseno identity per Shot;
- durable across devices, Macs, reinstalls, updates and signing refreshes.

ADR 0035 MUST also establish:

    A Shot is shipped exactly once.

    Every later network release of that Shot is an Update.

    A Fork is a different Shot.
    When that child enters the network, the child is shipped once.

    Every public Shot opens exactly one Claim Edition when it is shipped.

    The Claim Edition belongs to the Shot, not to a release.

    Updates do not reopen, replace, reset or alter the Claim Edition.

    Claim supply and time horizon are fixed at shipment.

    The Registry is a timeline of software events, not an App Store grid.

    Following Builders is private preference state, not public social capital.

    No follower counts exist.

# ============================================================
# 4. PRODUCT AXIOMS
# ============================================================

Carry these exact axioms through code, language and UI.

## A Shot is born once

The first successful transition from private/local candidate to publicly
discoverable network Shot is:

    SHIP

There may never be a second `shot.shipped` event for the same ShotID.

## A Shot changes through updates

After shipping:

    Update

is the only publication vocabulary.

Never write:

    shipped v2
    shipped v3
    shipped another release
    re-shipped

Use:

    updated
    published an update
    update available

The CLI command may remain:

    tohseno deploy

for developer ergonomics.

Its semantics depend on public state:

    unshipped Shot + deploy = Ship
    shipped Shot   + deploy = Update

Its output MUST reflect that distinction.

First:

    Shipped.

Later:

    Updated.

## A human claims once

For one Shot:

    one Tohseno account → zero or one Claim NFT

Never multiple units.

Never quantity selection.

Never “buy 10.”

## A claim persists

An update does not invalidate the Claim.

A signing refresh does not alter the Claim.

A phone replacement does not alter the Claim.

A Mac replacement does not alter the Claim.

A reinstall does not create another Claim.

## A claim remembers encounter

A Claim records which exact network release/checkpoint the human encountered
when claiming.

If the app updates tomorrow, the receipt still says:

    claimed at release X

## Claim is not installation

Claim:

    durable public relationship

Install:

    executable currently materialized and signed on a device

These states must remain distinct.

# ============================================================
# 5. THE NEW NETWORK LIFECYCLE
# ============================================================

Implement this conceptual lifecycle.

BUILDER:

    existing Xcode project
          ↓
    tohseno init
          ↓
    local candidate Shot
          ↓
    tohseno deploy
          ↓
    source snapshot + catalog release + Registry action
          ↓
    choose immutable Claim Edition
          ↓
    approve exact Ship on Companion
          ↓
    ShotRegistry registration
          ↓
    Claim Edition opens
          ↓
    signed catalog becomes discoverable
          ↓

        SHIPPED ONCE

          ↓
    tohseno.com/<app>


HUMAN:

    encounters app
          ↓
        CLAIM
          ↓
    Companion opens
          ↓
    draw one circle around artifact
          ↓
    Companion recomputes exact Claim payload
          ↓
    Companion DeviceKey authorizes
          ↓
    Tohseno relays
          ↓
    Robinhood Chain confirms NFT mint
          ↓

        CLAIMED

          ↓
    exact claim number + mark receipt
          ↓
    installation intention automatically enters durable Companion outbox
          ↓
    Mac may be offline
          ↓
    Mac eventually receives request
          ↓
    verifies exact claimed release
          ↓
    downloads source
          ↓
    prepares build
          ↓
    Ready for your iPhone
          ↓
    cable
          ↓
    recipient-local Xcode signing
          ↓
    devicectl install
          ↓
    physical verification
          ↓

        INSTALLED


BUILDER LATER:

    source changes
          ↓
    tohseno deploy
          ↓
    Companion approves exact update
          ↓
    AppendCheckpoint
          ↓
    new immutable catalog release
          ↓

        UPDATED

          ↓
    existing claimants may see:
        Update available


FORK:

    claimed Shot
          ↓
    Fork
          ↓
    exact parent release
          ↓
    new local source
          ↓
    NEW ShotID
          ↓
    evolve
          ↓
    deploy
          ↓

        CHILD IS SHIPPED ONCE

          ↓
    child receives its OWN Claim Edition

# ============================================================
# 6. CLAIM EDITIONS
# ============================================================

Every publicly shipped Shot MUST have exactly one Claim Edition.

The Builder chooses the edition when the Shot is first shipped.

Support exactly four policies in this release.

## Open

    maxClaims = unlimited
    closesAt  = never

Human presentation:

    OPEN EDITION · ∞

## Limited

Example:

    maxClaims = 888
    closesAt  = never

Human presentation:

    431 / 888 CLAIMED

## Timed

    maxClaims = unlimited
    closesAt  = exact timestamp

Human presentation:

    OPEN UNTIL SEP 8
    3h 41m REMAINING

## Limited + Timed

Example:

    maxClaims = 888
    closesAt  = exact timestamp

The edition closes at whichever occurs first:

    supply exhausted
    OR
    closesAt reached

No other policy in v1.

Do not add:

- pricing;
- auctions;
- allowlists;
- rarity;
- tiers;
- staking;
- paid mints;
- Dutch auctions;
- bonding curves;
- per-release editions;
- multiple editions per Shot.

# ============================================================
# 7. EDITION POLICY IS IMMUTABLE
# ============================================================

This is a hard invariant.

If a Builder ships:

    888 Edition

they may not later change it to:

    8,888

If they ship:

    Open Edition

they may not later convert it into a scarce edition.

If they ship:

    closes September 8

they may not extend the deadline after people acted on that fact.

The claim horizon is part of the Shot's birth.

Updates cannot change it.

Alias changes cannot change it.

Builder transfer cannot change it.

Profile changes cannot change it.

The contract must enforce this.

The server must not simulate mutability.

# ============================================================
# 8. SAY "IDENTITIES", NOT "HUMANS" OR "IPHONES"
# ============================================================

Scarcity is enforced per Tohseno identity/account.

It is NOT proof-of-personhood.

Therefore product copy may say:

    888 Tohseno identities can claim this.

Do NOT claim:

    888 unique humans

unless the system has independent proof-of-personhood, which it does not.

Do NOT bind scarcity to physical phone hardware.

Phones are replaceable.

The account is the durable owner.

# ============================================================
# 9. ADD A SEPARATE CLAIMS CONTRACT
# ============================================================

The existing active generation-0.8 contracts remain immutable.

DO NOT:

- edit ShotRegistry;
- edit BuilderAccountFactory;
- edit BuilderAccount;
- edit generation-0.8 source;
- deploy a replacement ShotRegistry;
- call this a registry generation upgrade;
- resurrect ShotRelations;
- add handles to ShotRegistry.

Introduce ONE additive product contract:

    TohsenoClaimsV1

on Robinhood Chain.

It references the already trusted active ShotRegistry.

This contract is attached to Shots but is not part of frozen Registry generation
0.8 protocol semantics.

Create explicit activation evidence for Claims separately from generation-0.8
Registry activation.

Do not make clients trust a copied address merely because it appears in source.

Use the repository's existing signed activation philosophy.

The new claims activation must bind at minimum:

- chain ID;
- contract address;
- exact runtime code hash;
- expected ShotRegistry address;
- exact source generation/version;
- deployment transaction;
- deployment block;
- activation sequence/digest according to the new additive activation model.

# ============================================================
# 10. USE ONE NON-TRANSFERABLE ERC-721 CONTRACT
# ============================================================

Implement Claim receipts as ERC-721-compatible NFTs.

Use ONE Tohseno Claims contract for all Shots.

DO NOT deploy one NFT contract per app.

DO NOT use ERC-1155 for v1.

Reason:

Every Claim is individually meaningful.

Each Claim has:

- one exact claimant;
- one sequential claim number inside a Shot;
- one exact release encountered;
- one exact Registry checkpoint encountered;
- one gesture commitment;
- one timestamp;
- one unique receipt.

That is a unique NFT receipt.

Conceptually:

    TohsenoClaimsV1
        token #184 → Prayer Lock
                     claim #184 of 888
                     claimant 0x...
                     release X
                     checkpoint Y
                     gesture commitment Z

# ============================================================
# 11. CLAIM TOKENS ARE NON-TRANSFERABLE
# ============================================================

A Claim means:

    this identity claimed this software

Therefore it may not become inventory.

Token transfer MUST revert.

Approval-for-transfer MUST NOT create a useful transfer path.

The holder may not sell it to make the semantic statement false.

Do not add a marketplace.

Do not add prices.

Do not add royalties.

Do not add burn-and-remint migration as a shortcut.

Recovery must preserve the SAME Tohseno account and therefore the same NFT
ownership rather than moving Claim NFTs to a new identity.

If ERC-721 interface conformance requires standard transfer functions, implement
them as explicit reverts under a documented non-transferability rule.

# ============================================================
# 12. CLAIM CONTRACT STATE
# ============================================================

Implement the smallest exact state model.

Conceptually:

    struct ClaimEdition {
        bool opened;
        uint64 maxClaims;      // 0 means unlimited
        uint64 totalClaims;
        uint64 openedAt;
        uint64 closesAt;       // 0 means never
    }

    struct ClaimRecord {
        bytes32 shotId;
        uint64 claimNumber;
        address claimant;
        bytes32 releaseDigest;
        bytes32 checkpointDigest;
        bytes32 gestureCommitment;
    }

Required mappings conceptually:

    ShotID → ClaimEdition

    ShotID + Tohseno account → tokenId

    tokenId → ClaimRecord

Token IDs may be monotonically allocated globally beginning at 1.

Do not derive identity from claim number.

Do not overload ShotID into a token ID if doing so destroys unique per-claim
receipts.

# ============================================================
# 13. CLAIM EDITION OPENING
# ============================================================

An edition may be opened only after the Shot exists in the active ShotRegistry.

Only the current Shot controller may authorize opening.

The transaction itself may be relayed by anyone.

The authorization MUST be an exact EIP-712 typed action signed through the
current BuilderAccount authority.

Define a closed action such as:

    OpenClaimEdition

binding at minimum:

- chainId;
- TohsenoClaimsV1 address;
- active ShotRegistry address;
- ShotID;
- maxClaims;
- closesAt;
- exact controller;
- nonce;
- deadline.

The contract MUST:

1. confirm the Shot is registered;
2. read the exact current controller from ShotRegistry;
3. verify authorization against that controller using the current account's
   ERC-1271 behavior;
4. reject if edition already exists;
5. reject invalid limits/deadlines;
6. persist immutable policy;
7. emit one exact event.

Conceptual event:

    ClaimEditionOpened(
        shotId,
        controller,
        maxClaims,
        opensAt,
        closesAt
    )

No admin may later rewrite it.

# ============================================================
# 14. CLAIM ACTION
# ============================================================

Define an exact EIP-712 action:

    ClaimSoftware

It MUST bind at minimum:

- chainId;
- TohsenoClaimsV1 address;
- ShotRegistry address;
- ShotID;
- claimant Tohseno account;
- exact catalog release digest;
- exact public checkpoint digest;
- gesture commitment;
- claimant nonce;
- deadline.

The claimant's Companion DeviceKey is the human authorization source.

The final on-chain verification should use the claimant smart account's exact
supported authority path.

Do not accept an opaque arbitrary EOA signature as a substitute.

Do not use an InstallationKey.

Do not use pairing keys.

Do not use the Apple signing identity.

# ============================================================
# 15. BIND CLAIM TO THE CURRENT SHOT STATE
# ============================================================

A human must never draw a circle around release X and silently receive release Y.

At claim preparation:

1. resolve exact Shot;
2. resolve exact current discoverable Release;
3. verify catalog manifest;
4. verify Builder signature;
5. verify current Registry head;
6. bind release digest + public checkpoint digest into ClaimSoftware.

At claim execution:

The contract MUST verify that the supplied checkpoint digest still equals the
current relevant Registry head if the active ShotRegistry ABI permits exact
verification.

If the Shot updated between review and mint:

    revert

The UI then says:

    This app changed while you were claiming it.
    Review the update and claim again.

Do NOT silently rebase a Claim onto newer software.

# ============================================================
# 16. ONE CLAIM PER IDENTITY
# ============================================================

Contract invariant:

    claimTokenOf[shotId][claimant] == none

before mint.

After mint:

    claimTokenOf[shotId][claimant] == exact tokenId

forever.

A second Claim against the same Shot MUST revert deterministically.

This remains true after:

- app updates;
- alias change;
- builder profile change;
- phone replacement;
- Mac replacement;
- signing refresh;
- Builder transfer.

# ============================================================
# 17. LIMITED EDITION RACES
# ============================================================

Do not invent reservation semantics in v1.

If only one slot remains and two valid identities submit:

- chain ordering decides;
- first valid canonical claim succeeds;
- second reverts because supply is exhausted.

The losing UI says:

    This edition closed before your claim confirmed.

Do not say Claimed before canonical confirmation.

Do not fake a reserved number while transaction is pending.

# ============================================================
# 18. GASLESS IS A PRODUCT INVARIANT
# ============================================================

Normal users NEVER see:

- gas estimates;
- ETH balances;
- RPC selection;
- chain selection;
- "connect wallet";
- seed import before claiming;
- transaction fee approval.

The Companion signs.

Tohseno relays.

Robinhood Chain executes.

The existing constrained relayer may be extended only with exact allowlisted
Claims operations.

Do not expose an arbitrary transaction relay.

The claims relayer may submit only the precise supported actions needed for:

- Tohseno account bootstrap when required;
- ClaimEdition opening;
- ClaimSoftware.

Rate limit it.

Persist jobs before submission.

Make retries idempotent.

Never hold a user's DeviceKey.

# ============================================================
# 19. TOHSENO ADDRESS / WALLET IDENTITY
# ============================================================

A person installing Tohseno already has a Companion DeviceKey.

Do not add a separate wallet onboarding ceremony.

Do not create a new unrelated EOA.

Reuse the existing account architecture truthfully.

The existing BuilderAccount implementation may serve as the on-chain Tohseno
smart account controlled by that DeviceKey.

Technical protocol vocabulary remains unchanged:

    BuilderAccount
    BuilderID

But product UI for a person who has not shipped anything may simply say:

    Tohseno address

A person does not become conceptually a Builder merely by claiming an app.

When that same account controls a Shot, it is the BuilderID for Registry law.

ADR 0035 supersedes ADR 0034 only where account deployment was restricted to the
first public Builder action.

The new rule is:

    The Tohseno smart account is deployed lazily on the person's
    first PUBLIC Tohseno action.

A Claim is a public action.

A Ship is a public action.

Whichever comes first may bootstrap the same deterministic account.

Do not create two identities.

# ============================================================
# 20. CLAIM PRIVACY MUST BE HONEST
# ============================================================

Claiming is public.

The account address and relationship to the claimed Shot are observable on-chain.

This is a deliberate change from the prior default where end-user installation
identity remained private.

Preserve the old privacy law for installation itself.

DO NOT publish:

- physical iPhone identifier;
- InstallationKey;
- Apple team;
- Mac identity;
- pairing identity;
- IP address;
- source path;
- install date unless separately implied by private local state;
- evolution prompts;
- device name.

The public fact is:

    Tohseno account X claimed Shot Y.

No more.

Before the person's first Claim, show one compact disclosure:

    Claims are public on Robinhood Chain.
    Your Tohseno address will be associated with this app.

Do not show this every time after they understand it.

Do not call it proof that they are a unique human.

# ============================================================
# 21. CONTRACT EVENTS
# ============================================================

Emit enough exact evidence to reconstruct Claims without trusting the catalog.

At minimum:

    ClaimEditionOpened(...)

and:

    SoftwareClaimed(
        bytes32 indexed shotId,
        address indexed claimant,
        uint256 indexed tokenId,
        uint64 claimNumber,
        bytes32 releaseDigest,
        bytes32 checkpointDigest,
        bytes32 gestureCommitment
    )

Block time is canonical claim time.

Do not duplicate mutable app metadata on-chain.

Do not store:

- app name;
- description;
- icon URL;
- Builder handle;
- source;
- gesture raw touch stream.

# ============================================================
# 22. CLAIM METADATA
# ============================================================

The NFT must be useful in Tohseno and intelligible to standard wallet/indexer
surfaces where Robinhood Chain support permits.

Implement a stable token metadata route.

Conceptually:

    /api/claims/v1/token/<tokenId>

Each token metadata object should resolve immutable encounter information:

- app name as of claimed release;
- ShotID;
- Claim number;
- edition policy;
- claimed at;
- exact release;
- Builder;
- normalized Claim mark rendering;
- external URL to the Claim receipt.

Do not let future app updates rewrite the historical meaning of an old Claim.

If mutable current app metadata is shown, separate it explicitly from:

    metadata at encounter

The cryptographic receipt remains immutable.

# ============================================================
# 23. CLAIM RECEIPT SCHEMA
# ============================================================

Create one closed, versioned off-chain object.

Expected conceptual name:

    tohseno.claim-receipt/1

It MUST bind:

- schema;
- active chain;
- claims contract address;
- ShotRegistry address;
- ShotID;
- tokenId;
- claimNumber;
- claimant address;
- exact release ID/digest;
- exact public checkpoint digest;
- exact gesture commitment;
- normalized gesture geometry;
- canonical transaction hash;
- canonical block number/hash;
- canonical block timestamp;
- edition policy;
- exact source/icon digest needed to render historical receipt.

Do not include private device or pairing data.

The receipt may be reconstructed from chain + immutable catalog facts, but if
persisted by the service it must remain independently verifiable.

# ============================================================
# 24. THE CLAIM GESTURE
# ============================================================

The primary gesture is fixed:

    DRAW A CIRCLE AROUND THE APP.

Do not brainstorm alternatives.

Implement this ritual.

Visual composition:

- app icon centered;
- generous black/neutral field;
- faint incomplete Tohseno-like ring or spatial cue;
- minimal instruction:

      Draw a circle around it.

The person places one finger down and draws one continuous loop around the
artifact.

The loop does NOT need to be geometrically perfect.

It may wobble.

It may be narrow or wide.

It may be expressive.

The app should recognize intentional enclosure, not handwriting quality.

When the loop closes:

1. freeze the exact visible path;
2. make the loop visually settle/close;
3. produce one restrained native haptic;
4. transition to:

       Claiming…

5. build the exact claim authorization;
6. sign;
7. relay;
8. wait for canonical on-chain confirmation.

Only after confirmed NFT mint:

       Claimed
       #312 of 888

or:

       Claimed
       #312 · Open Edition

The mark remains visible around the artifact.

# ============================================================
# 25. THE GESTURE IS EXPRESSION, NOT BIOMETRICS
# ============================================================

This is a hard privacy boundary.

DO NOT persist or transmit:

- touch timestamps;
- velocity;
- acceleration;
- pressure;
- force;
- radius;
- azimuth;
- altitude;
- device motion;
- hand/finger inference;
- behavioral signature;
- raw UIKit touch-event metadata.

Use only final 2D geometry.

As points arrive, capture only canvas-local x/y coordinates necessary to render
the current path.

After stroke completion:

1. map positions into normalized unit-canvas coordinates;
2. resample by arc length to exactly 64 points;
3. quantize each x and y to a fixed-width integer representation;
4. serialize through a closed `tohseno.claim-mark/1` encoding;
5. SHA-256 the canonical bytes;
6. use that digest as `gestureCommitment`.

Discard the raw point stream after canonical normalized geometry exists.

Create shared test vectors so Swift, Rust/server and any receipt renderer agree
on the exact commitment.

The gesture is NOT cryptographic entropy.

The gesture is NOT the signing key.

The DeviceKey signature remains authorization.

# ============================================================
# 26. CIRCLE ACCEPTANCE
# ============================================================

Do not make the ritual frustrating.

The stroke is acceptable when all of the following are true:

- one continuous stroke;
- enough spatial distance to represent an intentional loop;
- the path substantially encloses the app icon center;
- endpoint returns within a forgiving threshold of the start region;
- path is not merely a tap or short line.

Do NOT score circularity.

Do NOT reject because the loop is ugly.

Do NOT turn it into a CAPTCHA.

Do NOT expose a percentage score.

The semantic condition is:

    the human drew a boundary around the artifact.

# ============================================================
# 27. ACCESSIBILITY
# ============================================================

Claiming must remain possible for people who cannot perform the drawing gesture.

Preserve the circle as the primary product ritual.

Under accessibility interaction where drawing is not reasonably available,
provide one intentional alternative:

    Hold to close the circle

The same visual ring closes while the control is held for a bounded duration.

This alternative produces a canonical accessibility Claim mark representation,
not fabricated hand geometry.

The cryptographic Claim semantics remain identical.

VoiceOver labels must explain:

    Claim this app

not raw NFT/EIP-712 details.

# ============================================================
# 28. CLAIM MUST PRECEDE NORMAL NETWORK INSTALL
# ============================================================

ADR 0035 changes the normal Registry relationship.

Current primary action:

    Install

becomes:

    Claim

The normal Tohseno Registry path is now:

    Claim
      ↓
    confirmed
      ↓
    prepare/install

A person may still inspect public source without claiming where the source is
public under ADR 0034.

Do not pretend Claim scarcity is DRM.

But the normal one-tap Tohseno Install and Fork experiences begin from a
confirmed Claim.

# ============================================================
# 29. CLAIM AUTOMATICALLY SENDS THE APP TO THE MAC
# ============================================================

After on-chain confirmation, DO NOT ask:

    Would you like to send this to your Mac?

That is redundant.

Claim already expresses the intention.

Immediately create the exact existing durable network-install intention against
the release bound into the Claim.

Reuse the current verified Install receive pipeline.

Do not write a second downloader/build system.

Flow:

    Claim confirmed
        ↓
    durable install intention
        ↓
    paired Mac mailbox
        ↓
    exact release verification
        ↓
    source download
        ↓
    build safety
        ↓
    local source/build preparation

If Mac is offline:

    Claimed
    Waiting for your Mac

That state may persist for hours.

No error.

When Mac later comes online, it resumes automatically.

# ============================================================
# 30. CLAIM BINDS INSTALL TO THE ENCOUNTERED RELEASE
# ============================================================

The automatic post-Claim preparation MUST use the exact release recorded in the
Claim.

If an app updates while the Mac is offline:

Do NOT silently prepare the newer release.

Prepare what the person claimed.

Afterward the user may separately see:

    Update available

This preserves encounter truth.

# ============================================================
# 31. CLAIM STATES
# ============================================================

Use a finite human state model.

For an unclaimed open edition:

    Claim

During human gesture:

    Drawing

During chain work:

    Claiming…

Confirmed, Mac unknown/offline:

    Claimed
    Waiting for your Mac

Mac downloading/verifying:

    Claimed
    Preparing on your Mac

Mac buildable:

    Claimed
    Ready for your iPhone

Installed:

    Claimed
    Installed

New network release:

    Claimed
    Update available

Limited edition exhausted before this identity claimed:

    Closed
    888 / 888 claimed

Timed edition expired:

    Closed

Never show Claimed before canonical mint confirmation.

# ============================================================
# 32. UPDATE DOES NOT MINT AGAIN
# ============================================================

When a claimed app updates:

- no second NFT;
- no new Claim number;
- no new gesture;
- no edition reset;
- no supply change;
- no claim transaction.

The identity already has a durable relationship with the Shot.

Update is software evolution, not another encounter.

# ============================================================
# 33. SHIPPING IS ONE EVENT
# ============================================================

Refactor product semantics so this is mechanically enforced.

For every public Shot:

    shipped_at

is immutable and derived from the first accepted public registration/catalog
transition.

There may be exactly one:

    shot.shipped

event.

Every subsequent accepted catalog release produces:

    shot.updated

Never another ship.

If existing code calls both first registration and append "publication", internal
generic publication vocabulary may remain where mechanically useful.

Human-facing and Registry event semantics MUST distinguish birth from update.

# ============================================================
# 34. CLI SHIPPING LANGUAGE
# ============================================================

Keep:

    tohseno deploy

because it is a good builder command.

On a candidate/local Shot:

    tohseno deploy

means Ship.

First-deploy output should become approximately:

    Preparing Prayer Lock…

    ✓ Source snapshot
    ✓ Build profile
    ✓ Registry release
    ✓ Ready to ship

    Claim edition:
    Open Edition · one per Tohseno identity

    Waiting for approval on your iPhone…

After all truths exist:

    ✓ Approved
    ✓ Registered on Robinhood Chain
    ✓ Claim edition opened
    ✓ Source published
    ✓ Registry updated

    Shipped.

    https://tohseno.com/...

On an already shipped Shot:

    tohseno deploy

means Update.

Final output:

    ✓ Approved
    ✓ Registry checkpoint appended
    ✓ Source published
    ✓ Registry updated

    Updated.

    https://tohseno.com/...

Never say:

    Shipped v2

# ============================================================
# 35. FIRST SHIP MUST CHOOSE CLAIM POLICY
# ============================================================

The Claim policy belongs to birth.

At first deploy, include policy in the exact Companion approval.

Companion publication sheet must allow:

    Open
    Limited
    Until date
    Limited until date

Open should be the visually simplest/default option.

Example:

    How can people claim Prayer Lock?

    ● Open Edition
      Anyone can claim once.

    ○ Limited
      First ___ Tohseno identities.

    ○ Until date

    ○ Limited until date

The selected policy becomes part of the signed exact Ship authorization.

Once signed and opened on-chain:

    immutable.

# ============================================================
# 36. NONINTERACTIVE CLI CLAIM-POLICY FLAGS
# ============================================================

Support exact bounded flags for automation where needed.

Choose clear final names consistent with current CLI style.

Conceptually:

    tohseno deploy --claim-edition open

    tohseno deploy \
      --claim-edition limited \
      --max-claims 888

    tohseno deploy \
      --claim-edition timed \
      --closes-at 2026-09-08T18:00:00Z

    tohseno deploy \
      --claim-edition limited \
      --max-claims 888 \
      --closes-at ...

Do not accept conflicting combinations.

For an already shipped Shot, supplying Claim Edition flags MUST fail:

    This app already shipped.
    Its Claim Edition is permanent.

# ============================================================
# 37. SHIP TRANSACTION ORDER
# ============================================================

The first Shot must not appear publicly claimable until all birth facts agree.

Persist a resumable state machine.

Conceptually:

    source staged
        ↓
    catalog release prepared
        ↓
    Registry registration prepared
        ↓
    Claim Edition policy prepared
        ↓
    ONE Companion human approval
        ↓
    exact bounded signatures returned
        ↓
    ensure Tohseno account
        ↓
    commitShot
        ↓
    wait required generation-0.8 delay
        ↓
    RegisterShot
        ↓
    verify canonical ShotRegistry state
        ↓
    open Claim Edition
        ↓
    verify Claim contract state
        ↓
    promote immutable source
        ↓
    promote catalog release
        ↓
    emit/project ONE shot.shipped event
        ↓
    return public link

If Registry registration succeeds but Claim Edition transaction temporarily
fails:

- do NOT register another Shot;
- do NOT create another Ship;
- persist partial state;
- retry exact remaining action idempotently;
- keep app undiscoverable until Claim Edition exists.

# ============================================================
# 38. UPDATE TRANSACTION ORDER
# ============================================================

For an existing Shot:

    snapshot
      ↓
    signed catalog update
      ↓
    Companion authorization
      ↓
    AppendCheckpoint
      ↓
    canonical confirmation
      ↓
    promote immutable source/catalog
      ↓
    project shot.updated

Claims contract is untouched.

Do not reopen edition.

# ============================================================
# 39. FORKS
# ============================================================

Keep ADR 0034's exact parent-release fork relation.

A Fork remains private/local until intentionally shipped.

When shipped:

- NEW ShotID;
- NEW Builder/controller;
- one `shot.shipped` event for child;
- own immutable Claim Edition;
- exact parent Shot/release relation preserved;
- parent unchanged.

Registry timeline may project:

    Radio for Birds entered Tohseno
    Forked from Tiny Radio

This is both:

    child shot.shipped

and a derived fork relation.

Do not create a second ship event merely to represent the fork.

# ============================================================
# 40. FOLLOW BUILDERS
# ============================================================

Add:

    Follow

to Builder identity.

A person follows a BuilderID, not a mutable handle.

Handle changes do not break following.

Following is NOT:

- on-chain;
- a token;
- a public social graph;
- a follower count;
- a popularity score;
- a recommendation signal;
- part of Builder profile authority.

Following is private personal preference state.

No public endpoint may answer:

    how many followers does Builder X have?

Do not implement that number.

# ============================================================
# 41. FOLLOW STATE
# ============================================================

Store following state privately within the existing personal Mac/Companion
relationship.

Use the existing encrypted durable command/projection infrastructure.

Required behavior:

From Companion:

    Follow

must feel immediate even if Mac is offline.

Companion:

1. updates optimistic local presentation;
2. persists an idempotent private follow operation;
3. queues it through existing encrypted outbox;
4. Mac durably reconciles;
5. projection returns;
6. both surfaces converge.

From Mac:

    Follow

updates local durable state and projects to Companion.

Use exact BuilderID.

Unfollow is idempotent.

Do not introduce a public follow server merely to synchronize two paired devices.

# ============================================================
# 42. NO FOLLOWER COUNTS
# ============================================================

This is deliberate.

Do NOT add:

    12.4K followers

Do NOT rank Builders by follows.

Do NOT create follow leaderboards.

Do NOT expose a public follow graph.

The reason to follow someone is:

    I want to notice when their software changes.

Nothing more.

# ============================================================
# 43. THE REGISTRY IS NOT A MARKETPLACE GRID
# ============================================================

The current Registry surface must evolve.

The Registry is the world outside the workshop.

Your Apps remains:

    source
    build
    app
    simulator
    iPhone
    evolve
    ship/update

Registry becomes:

    discover
    follow
    claim
    timelines
    forks
    editions
    updates

REMOVE the quick:

    New Shot

composer from Registry.

Creation already has a proper location.

Do not place a factory control inside the world.

# ============================================================
# 44. REGISTRY INFORMATION ARCHITECTURE
# ============================================================

Use exactly three primary Registry modes:

    Discover
    Following
    Updates

and Search.

No more primary tabs in this evolution.

## Discover

The public living network.

## Following

The same type of network events, filtered to Builders the user follows.

## Updates

A private, high-signal inbox about the person's existing relationship to
software and their own actions.

# ============================================================
# 45. DISCOVER IS A TIMELINE OF SOFTWARE
# ============================================================

Do not turn Discover into human posts.

There are:

- no text posts;
- no replies;
- no comments;
- no likes;
- no reposts;
- no quote posts;
- no engagement score.

Software itself creates the events.

Examples:

    PRAYER LOCK
    entered Tohseno
    311 / 888 claimed
    by @maubaron

    ANKY
    updated
    Open Edition · 4,821 claimed
    by @jpfraneto

    RADIO FOR BIRDS
    entered Tohseno
    forked from Tiny Radio
    by @alice

    EIGHT
    8 / 8 claimed
    edition closed

This is network activity.

Not social media.

# ============================================================
# 46. PUBLIC TIMELINE EVENT MODEL
# ============================================================

Create a deterministic public timeline projection from existing authoritative
facts.

Do NOT make the timeline itself a new authority.

Every event must reference its evidence.

Create a closed event schema if useful, conceptually:

    tohseno.timeline-event/1

Canonical public event types for this release:

    shot.shipped
    shot.updated
    shot.forked
    claim.edition_closed

Do not add arbitrary event types.

`shot.shipped`:

    exactly once per ShotID

`shot.updated`:

    once per accepted post-birth catalog release

`shot.forked`:

    derived when a newly shipped child has a verified parent relation

`claim.edition_closed`:

    derived when finite supply fills or timed horizon expires

Do not emit every Claim into the global Discover feed.

# ============================================================
# 47. CLAIM ACTIVITY MUST NOT FLOOD THE WORLD
# ============================================================

Raw Claim events exist and are public on-chain.

That does NOT mean Discover should render one row per mint.

Do NOT produce:

    Alice claimed X
    Bob claimed X
    Carol claimed X
    Dave claimed X

forever.

Instead:

- show current claim count on app/event cards;
- show edition close as an event;
- allow app Timeline/detail to inspect claim history or recent claims;
- paginate raw claims where shown;
- keep Discover quiet enough that software events remain legible.

# ============================================================
# 48. EVENT ORDERING
# ============================================================

Use canonical evidence ordering.

For on-chain-backed events prefer:

    blockNumber
    transactionIndex
    logIndex

and canonical block timestamp.

For catalog update events bind them to the corresponding canonical Registry
receipt rather than `Date.now()` from an application server.

Reorg-aware indexing is required.

No duplicate `shot.shipped` after reindex/restart.

# ============================================================
# 49. FOLLOWING
# ============================================================

Following is a local/private filtered view of the public timeline.

Do not make the server learn the full follow list just to render it in v1.

The network is initially small enough to:

1. fetch the bounded public event window;
2. filter by locally held BuilderIDs.

If pagination requires additional pages to fill the view, continue bounded
fetching.

Optimize later if actual scale requires it.

Privacy first.

# ============================================================
# 50. UPDATES IS NOT DISCOVER
# ============================================================

Updates is a private software inbox.

It should contain only events where this person's action or relationship matters.

Eligible examples:

    Prayer Lock updated
    You claimed #184
    [Update]

    Weird Camera is ready
    Your Mac finished preparing it.
    [Connect iPhone]

    Someone shipped a fork of Anky
    [View fork]

    Your 888 Edition is complete
    888 / 888 claimed

    Your alias request was approved

    Prayer Lock publication needs your approval
    [Open Companion]

    Your evolution finished
    [Open App]

Do not put generic Discover events in Updates.

Following a Builder does NOT automatically generate an Updates notification for
every update.

Those events belong in Following.

Future explicit notification preferences may change that.

Not now.

# ============================================================
# 51. NOTIFICATION PHILOSOPHY
# ============================================================

Do NOT create another red-badge attention machine.

No notification per Claim.

No:

    someone claimed your app
    someone claimed your app
    someone claimed your app

A Builder can always see claim count.

High-signal edition completion is worthy:

    888 / 888 claimed

If daily aggregate claim summaries already fit naturally into existing durable
state, one bounded aggregate may be acceptable.

Do not build a scheduler solely for this release.

Registry sidebar may show one restrained unread Updates count/dot.

Use Tohseno's existing accent language.

Do not add engagement-red notification styling.

# ============================================================
# 52. UPDATE INBOX DURABILITY
# ============================================================

Private Updates need:

- stable IDs;
- idempotent insertion;
- read/unread state;
- durable local storage;
- paired-device reconciliation where current Companion architecture permits;
- no server-visible private history unless already within encrypted relay
  transport.

Restarting Mac or Companion must not duplicate every Update.

# ============================================================
# 53. APP TIMELINE
# ============================================================

Every public Shot page gains:

    Timeline

This is the app's life.

Example:

    NOW
    v4 / current update

    AUG 30
    431 / 888 claimed

    AUG 29
    Updated

    AUG 27
    Radio for Birds shipped as a fork

    AUG 25
    Claim #184

    AUG 20
    Updated

    AUG 14
    Shipped
    888 Edition opened

The exact presentation may be more restrained.

The semantic structure must be correct.

There is one birth.

Everything follows from it.

# ============================================================
# 54. BUILDER PROFILE
# ============================================================

Builder profile becomes meaningful as a place in the Registry.

Display:

- avatar if available;
- display name;
- handle;
- verified external attestations;
- shortened BuilderID with technical detail disclosure;
- Follow / Following;
- software they shipped;
- software timeline/activity.

Do NOT add:

- follower count;
- likes;
- bio engagement metrics;
- post composer;
- direct messages;
- social reputation score.

Builder reputation comes from software and its lineage.

# ============================================================
# 55. MAC REGISTRY UI
# ============================================================

Keep the existing workshop UI intact for Your Apps.

Do not make Build/App/Source mystical.

That UI is a machine.

When user selects Registry, the center should clearly become the world.

Required layout:

    Registry

    Discover   Following   Updates                Search

Then living event cards/content.

Remove:

    quick New Shot

from Registry.

Do not show:

    Apps on this Mac

as if they are Registry content.

Local apps remain in the normal sidebar/workshop.

Registry event/app cards should support:

- app icon;
- app name;
- Builder;
- Follow state where useful;
- Ship/Update/Fork event context;
- Claim Edition state;
- current Claim count;
- current user's Claim state;
- Claim CTA;
- open app detail/timeline.

# ============================================================
# 56. COMPANION REGISTRY UI
# ============================================================

Companion receives the same conceptual Registry:

    Discover
    Following
    Updates

adapted naturally for iPhone.

The phone is the primary Claim surface.

App detail is intentionally quieter than Mac workshop UI.

No build internals.

No source inventory.

No transaction hexadecimal wall.

The primary app encounter should feel like:

    artifact
    builder
    small description
    edition status
    Claim

with timeline/details below.

# ============================================================
# 57. THE APP PAGE IS AN ALTAR FOR THE ARTIFACT
# ============================================================

Do not literally label it "altar."

Translate the idea into restraint.

The app page should give the artifact space.

Example composition:

                [ icon ]

             PRAYER LOCK

              @maubaron

      A small ritual for beginning
               the day.

           311 / 888 CLAIMED


                CLAIM

Do not surround the Claim button with:

- charts;
- token prices;
- gas;
- wallet connect;
- trading UI;
- engagement metrics.

The central question is:

    Do I want this software in my world?

# ============================================================
# 58. AFTER CLAIM
# ============================================================

Once confirmed, replace primary claim state with something like:

                CLAIMED
               #312 / 888

                 ◯

       Waiting for your Mac

or:

                CLAIMED
               #312 / 888

            Ready for iPhone

The user's actual normalized mark should remain available as the visual Claim
receipt.

# ============================================================
# 59. PROFILE BECOMES A CABINET OF ENCOUNTERS
# ============================================================

Companion Profile should not lead with generic crypto wallet vocabulary.

Show identity first.

Then:

    Claimed

Each Claim card:

    Tohseno
    Open Edition · #31

    Prayer Lock
    888 Edition · #184

    Weird Camera
    Open Edition · #902

Tap one:

    Prayer Lock

    Claim #184 of 888
    Claimed Aug 30, 2026

    Release at encounter
    ...

    Your mark
    [render normalized loop]

    Builder
    @maubaron

    [Open]
    [Timeline]
    [Receipt]

Technical details may reveal:

    Tohseno address
    chain
    token ID
    transaction
    ShotID

Do not make those the emotional center.

# ============================================================
# 60. DO NOT AUTOMATICALLY TURN CLAIMS INTO PUBLIC PROFILE CONTENT
# ============================================================

The chain fact is public.

That does not require Tohseno to aggressively surface someone's complete claim
history on their public Builder webpage.

For v1:

- user's own Profile shows their Claims;
- direct Claim receipt links work;
- on-chain data remains queryable;
- do NOT automatically add "everything this Builder claimed" to public profile.

Preserve contextual privacy even around public facts.

# ============================================================
# 61. WEBSITE REGISTRY
# ============================================================

Evolve:

    https://tohseno.com/registry

into the public Discover view.

No authenticated Following view is required on the web.

No wallet connection.

Render real public timeline facts.

Search continues to resolve:

- app;
- Builder;
- handle;
- ShotID.

# ============================================================
# 62. WEBSITE APP PAGE
# ============================================================

Change the primary action from:

    Install

to:

    Claim

Show exact edition state.

Examples:

    OPEN EDITION · ∞
    4,821 claimed

    311 / 888 claimed

    Closed · 888 / 888

    Open for 3h 41m

Claim button on iPhone:

    Open in Tohseno Companion

using the existing safe deep-link strategy.

On Mac:

    Claim on your iPhone

and open native Tohseno/route the request to paired Companion if the local
product supports that path.

Without Tohseno:

    Get Tohseno

No wallet connect.

# ============================================================
# 63. WEBSITE BUILDER PAGE
# ============================================================

Public Builder pages gain:

    Follow

If opened on a device with Tohseno Companion, use a bounded deep link containing
only the exact BuilderID and safe routing metadata.

If not installed, explain that Following lives in Tohseno.

Do not create browser accounts merely for Follow.

Do not create cookie-based follower identity as a second identity system.

# ============================================================
# 64. WEBSITE CLAIM RECEIPT
# ============================================================

Add one canonical receipt route.

Conceptually:

    https://tohseno.com/c/<token-id>

or another clear non-conflicting route.

Display:

- artifact;
- claim number;
- edition;
- normalized mark;
- claimed timestamp;
- Builder;
- Shot link;
- chain verification detail.

The receipt is not a speculative NFT marketplace page.

No floor price.

No "list."

No transfer button.

# ============================================================
# 65. EXISTING ALIAS "CLAIM" TERMINOLOGY
# ============================================================

The current service already uses language equivalent to alias claims.

Do not create ambiguous code where:

    claim

could mean either:

    global alias request
    software NFT Claim

Rename internal APIs/types where necessary.

Use explicit vocabulary such as:

    AliasClaimRequest
    SoftwareClaim
    ClaimEdition

Routes should also be unambiguous.

Do not break existing alias semantics.

# ============================================================
# 66. REGISTRY API — CLAIMS
# ============================================================

Extend the existing versioned Registry service.

Add exact read capabilities for:

- claims contract status/activation;
- Shot Claim Edition;
- Claim state for an exact Tohseno account + Shot;
- Claim receipt;
- paginated Shot claims;
- token metadata;
- public timeline;
- Shot timeline.

Add exact write/orchestration capabilities for:

- prepare Claim Edition opening;
- submit authorized Claim Edition opening;
- prepare Software Claim;
- submit Software Claim authorization.

Do not let the server sign on behalf of claimant or Builder.

# ============================================================
# 67. CLAIM PREPARATION API
# ============================================================

A prepare endpoint must return structured facts, not an opaque digest.

Companion must receive enough information to independently validate:

- active chain;
- claims contract;
- registry;
- Shot;
- Builder;
- exact release;
- exact checkpoint;
- claimant account;
- edition policy;
- nonce;
- deadline.

Companion constructs/recomputes canonical EIP-712 digest itself.

Mac/server cannot ask:

    sign 0xdeadbeef

without context.

# ============================================================
# 68. CLAIM INDEXER
# ============================================================

Extend production indexing to TohsenoClaimsV1.

Index:

- edition openings;
- Claims;
- token IDs;
- claim number;
- claimant;
- release digest;
- checkpoint digest;
- gesture commitment;
- canonical block;
- canonical transaction.

Handle reorg.

Do not count a pending transaction as a Claim.

Claim count shown to users must derive from canonical contract state/indexed
canonical evidence.

For high-stakes checks such as final slot availability, read fresh chain state.

# ============================================================
# 69. TIMED EDITION CLOSURE
# ============================================================

A timed edition does not need an operator transaction to become closed.

Contract claim eligibility derives directly from block timestamp.

Registry may project:

    claim.edition_closed

when the canonical chain time has crossed the immutable deadline.

The projection is derived.

Do not pretend an on-chain "close" transaction occurred if none did.

# ============================================================
# 70. CLAIM NUMBER
# ============================================================

Claim number is assigned by canonical contract execution.

Never preallocate it in UI.

Never display:

    You will be #312

before transaction confirms.

After confirmation:

    #312

is immutable.

# ============================================================
# 71. SOURCE ACCESS IS NOT DRM
# ============================================================

ADR 0034 deliberately publishes buildable source.

A limited Claim Edition does not magically make those bytes inaccessible.

Be truthful.

A finite edition means:

    only N Tohseno identities can receive the canonical Tohseno Claim receipt.

The normal Tohseno Install/Fork ritual requires Claim.

But public source availability remains governed by the release's source and
license declarations.

Do not market finite claims as technical exclusion if source is public.

# ============================================================
# 72. FORK PERMISSION
# ============================================================

Retain `fork_allowed`.

A Claim does not override Builder-declared fork permission.

If:

    fork_allowed = false

the claimed user can Install but Tohseno does not offer Fork.

If true:

    Claim
      ↓
    Fork

is available.

No legal permission is inferred from NFT ownership.

# ============================================================
# 73. INSTALL PERMISSION
# ============================================================

Retain existing install compatibility and safety rules.

Claim confirmation does not override:

- unsafe build hooks;
- unsupported entitlements;
- incompatible iOS;
- missing Xcode;
- missing Apple signing;
- device Trust;
- Developer Mode;
- locked phone;
- physical verification.

Claim may succeed while install cannot.

Present that truth separately.

Example:

    Claimed #184

    This app requires a capability Tohseno cannot currently re-sign
    with your Apple team.

Do not undo the Claim.

# ============================================================
# 74. CLAIM DOES NOT REQUIRE THE MAC ONLINE
# ============================================================

This is a flagship experience.

Test it explicitly.

Scenario:

1. Mac offline/asleep.
2. User encounters a Shot on iPhone.
3. User Claims.
4. Companion signs.
5. Tohseno relays on-chain transaction.
6. Claim confirms.
7. UI says:

       Claimed #71
       Waiting for your Mac

8. Installation intention stays durable.
9. Hours later Mac reconnects.
10. Mac receives exact request.
11. Mac verifies exact claimed release.
12. Mac downloads and prepares.
13. Companion updates:

       Ready for your iPhone

No user re-submission.

# ============================================================
# 75. DO NOT TURN CLAIM INTO PAYMENT
# ============================================================

No price.

No Stripe.

No token.

No USDC.

No $TOHSENO requirement.

No wallet balance prerequisite.

No "free mint" marketing language in the primary interface.

The product word is:

    Claim

The technical detail may say:

    Gas sponsored by Tohseno

inside receipt/details.

# ============================================================
# 76. DO NOT TURN REGISTRY INTO NFT CULTURE
# ============================================================

Do not add:

- floor prices;
- volume;
- rarity ranks;
- traits for speculation;
- OpenSea links as primary UI;
- mint countdown hype copy;
- flipping;
- offers;
- holder leaderboards.

The on-chain object is infrastructure for durable encounter.

The product is software.

# ============================================================
# 77. UPDATE AVAILABILITY
# ============================================================

When a claimed Shot updates:

derive:

    Update available

for that user's installed/claimed relationship.

If not installed:

    New version available

but preserve the original Claim receipt.

If local source has been forked/evolved:

do NOT silently overwrite it.

Reuse ADR 0034's upstream-change honesty.

# ============================================================
# 78. BUILDER FOLLOWING + UPDATES
# ============================================================

A person following @alice should naturally see:

    Alice shipped Camera Thing

and later:

    Camera Thing updated

inside Following.

They should NOT automatically receive OS notifications for both.

Following creates awareness.

It does not seize attention.

# ============================================================
# 79. MAC SIDEBAR
# ============================================================

Keep current sidebar hierarchy restrained.

Registry may display a subtle unread Updates indicator when useful.

Do not create separate sidebar items for:

- Discover;
- Following;
- Claims;
- NFTs;
- Notifications.

They live inside Registry.

Your Apps remain visually primary workshop objects.

Profile remains identity/memory.

# ============================================================
# 80. COMPANION NAVIGATION
# ============================================================

Preserve the current small navigation model.

Do not add a Wallet tab.

Do not add a Notifications tab.

Do not add an NFT tab.

Claimed software lives in Profile.

Discover/Following/Updates live in Registry.

Apps remain the user's current working/installed relationship.

Create remains available where currently governed.

# ============================================================
# 81. PRODUCT LANGUAGE
# ============================================================

Use exact vocabulary.

First network entry:

    Ship
    Shipped

Later source release:

    Update
    Updated

Human acquisition:

    Claim
    Claimed

Physical software state:

    Prepare
    Ready for iPhone
    Installing
    Installed

Derivative creation:

    Fork
    Forked from

Builder preference:

    Follow
    Following

Do not interchange:

    Ship
    Update
    Claim
    Install

They mean different things.

# ============================================================
# 82. THREAT MODEL — CLAIMS
# ============================================================

Extend threat model.

At minimum address:

- malicious relayer;
- replayed Claim signature;
- replayed Edition-open signature;
- wrong chain;
- wrong Claims contract;
- wrong ShotRegistry;
- stale Registry head;
- release changed during Claim;
- forged gesture commitment;
- server substitution of receipt geometry;
- duplicate Claim;
- Claim cap race;
- timed expiry race;
- unregistered Shot;
- wrong Shot controller opening edition;
- transferred Shot controller after edition open;
- claimant smart-account bootstrap;
- forged claimant account;
- contract reentrancy;
- unsafe ERC-721 receiver callbacks;
- transfer attempts;
- metadata server failure;
- indexer reorg;
- relayer gas abuse;
- limited-edition Sybil behavior;
- privacy leakage between Claim account and installation device.

Document explicitly:

    One-per-Tohseno-identity is not Sybil resistance.

# ============================================================
# 83. ERC-721 MINT SAFETY
# ============================================================

Because the recipient may be a smart account and Claims are non-transferable,
do not introduce arbitrary recipient callback execution during mint merely for
conventional safe-transfer behavior.

Inspect the exact current BuilderAccount capabilities.

Use the smallest safe mint path consistent with ERC-721 ownership semantics and
the non-transferable design.

No arbitrary external callback should be needed to create a Claim.

# ============================================================
# 84. CONTRACT ADMINISTRATION
# ============================================================

Prefer no mutable owner/admin state.

The Claims contract should not need:

- owner mint;
- pause;
- edition edit;
- arbitrary URI replacement;
- supply override;
- emergency confiscation.

If any administrative ability is genuinely required, justify it in ADR 0035 and
threat model before implementing it.

Default is immutable mechanics.

A contract bug requires successor/abandonment rather than pretending immutable
claims can be patched invisibly.

# ============================================================
# 85. CLAIM CONTRACT TESTS
# ============================================================

Add exhaustive Foundry tests.

At minimum:

- open edition for registered Shot;
- reject unregistered Shot;
- exact current controller authorization;
- reject wrong controller;
- ERC-1271 verification;
- nonce replay rejection;
- deadline rejection;
- wrong chain/domain;
- second edition open rejected;
- open policy immutable;
- open edition unlimited claims;
- finite supply exactly N;
- N+1 rejected;
- timed claim before close;
- timed claim at/after close rejected according to exact boundary;
- limited+timed behavior;
- one account one Claim;
- second Claim rejected;
- unique token IDs;
- exact per-Shot claim numbers;
- release digest recorded;
- checkpoint digest recorded;
- gesture commitment recorded;
- head mismatch rejected;
- transfer rejected;
- approval/transfer path cannot bypass non-transferability;
- claim survives Shot update;
- edition survives Shot controller transfer;
- fork child gets independent edition;
- no admin mint;
- no arbitrary relayer authority;
- event fields exact;
- gas snapshot.

# ============================================================
# 86. CROSS-LANGUAGE SIGNATURE VECTORS
# ============================================================

Create frozen test vectors for:

    OpenClaimEdition
    ClaimSoftware

Verify exact agreement across:

- Solidity;
- Rust;
- Swift.

Use canonical P-256 / BuilderAccount signing law already present.

Do not double-hash.

Enforce low-s as current protocol requires.

# ============================================================
# 87. GESTURE VECTORS
# ============================================================

Create a shared fixture, conceptually:

    fixtures/claim-mark-v1.json

Include:

- ordinary clockwise loop;
- counterclockwise loop;
- irregular loop;
- wide loop;
- narrow loop;
- failed line;
- failed tap;
- failed non-enclosing path;
- quantized bytes;
- expected SHA-256 commitment.

Swift generation and server/Rust validation must agree exactly.

# ============================================================
# 88. SHIPPING INVARIANT TESTS
# ============================================================

Add tests proving:

- first public release → exactly one `shot.shipped`;
- second catalog release → `shot.updated`;
- tenth catalog release → still only one Ship;
- service restart/reindex does not duplicate Ship;
- retry after partially completed first birth does not duplicate Ship;
- alias mutation does not create Ship;
- Builder profile mutation does not create Ship;
- refresh does not create Ship;
- installation does not create Ship;
- fork child creates its own one Ship;
- parent receives no new Ship event.

# ============================================================
# 89. FOLLOW TESTS
# ============================================================

Test:

- Follow by exact BuilderID;
- handle rename preserves follow;
- duplicate Follow idempotent;
- Unfollow idempotent;
- Companion offline Follow queues;
- Mac reconnect reconciles;
- Mac-origin Follow projects to Companion;
- no public Registry record;
- no on-chain record;
- no follower-count endpoint;
- Following feed includes matching Builder events;
- Following feed excludes non-followed Builders.

# ============================================================
# 90. UPDATES INBOX TESTS
# ============================================================

Test:

- claimed app update creates one relevant item;
- repeated polling creates no duplicate;
- Ready for iPhone creates actionable item;
- fork of user's Shot creates one item;
- edition close creates one item;
- alias state transition creates one item;
- individual Claims do NOT each create notifications;
- reading persists;
- reconnect does not resurrect read items;
- public Discover events do not automatically enter Updates.

# ============================================================
# 91. TIMELINE TESTS
# ============================================================

Test:

- deterministic ordering;
- chain reorg handling;
- canonical timestamp;
- no `Date.now()` authority;
- one Ship;
- N Updates;
- child fork relation;
- edition close;
- claim count current;
- pagination;
- no global feed claim flood.

# ============================================================
# 92. CLAIM → OFFLINE MAC E2E
# ============================================================

Add an automated production-shaped E2E around the existing network harness.

Prove:

    claimant account
       ↓
    exact published Shot
       ↓
    gesture commitment
       ↓
    ClaimSoftware authorization
       ↓
    NFT mint
       ↓
    confirmed Claim receipt
       ↓
    durable install intention while Mac unavailable
       ↓
    Mac returns
       ↓
    exact claimed release downloaded
       ↓
    existing install pipeline receives it

No production fake success branch.

# ============================================================
# 93. LIMITED EDITION E2E
# ============================================================

Use a tiny fixture edition:

    maxClaims = 2

Prove:

- identity A Claims #1;
- identity B Claims #2;
- edition is closed;
- identity C cannot Claim;
- UI/API report 2 / 2;
- identity A cannot Claim again;
- update does not reopen edition.

# ============================================================
# 94. OPEN EDITION E2E
# ============================================================

Prove:

- Open Edition has no cap;
- multiple distinct test identities Claim;
- each receives sequential Claim number;
- each holds one NFT;
- no quantity exists;
- app Update changes neither edition nor old Claims.

# ============================================================
# 95. PHYSICAL ACCEPTANCE — PRIMARY RITUAL
# ============================================================

The product is not accepted until the actual ritual works on a physical iPhone.

Use a real public Shot.

On iPhone:

1. open live app URL;
2. tap Claim;
3. Companion opens correct Shot/release;
4. app icon appears centered;
5. draw a natural imperfect circle;
6. circle is accepted;
7. haptic occurs;
8. UI says Claiming…;
9. actual DeviceKey signs;
10. production relayer submits;
11. Robinhood confirms;
12. token exists;
13. UI changes to Claimed ONLY now;
14. exact claim number appears;
15. actual loop renders as receipt;
16. Profile contains Claim.

Record:

- ShotID;
- account;
- token ID;
- claim number;
- tx hash;
- block;
- gesture commitment;
- release digest;
- checkpoint digest.

Never record private key.

# ============================================================
# 96. PHYSICAL ACCEPTANCE — MAC OFFLINE
# ============================================================

Repeat with Mac intentionally offline.

Claim must complete.

Then:

    Claimed
    Waiting for your Mac

Bring Mac online later.

Verify:

- durable request arrives;
- exact claimed release resolves;
- source verifies;
- source prepares;
- Companion changes to Ready for your iPhone.

Then plug phone in.

Verify physical install.

This is a REQUIRED acceptance test.

# ============================================================
# 97. PHYSICAL ACCEPTANCE — ONE SHIP, ONE UPDATE
# ============================================================

Builder:

1. create/adopt small app;
2. first `tohseno deploy`;
3. choose finite or Open edition;
4. approve;
5. observe:

       Shipped.

6. another person Claims;
7. builder changes visible behavior;
8. runs `tohseno deploy` again;
9. approves;
10. terminal MUST say:

       Updated.

11. Registry Timeline contains:

       one Ship
       one Update

12. Claim Edition remains exactly unchanged.
13. claimant sees Update available.
14. claimant installs update.
15. no new Claim transaction exists.

# ============================================================
# 98. PHYSICAL ACCEPTANCE — FOLLOW
# ============================================================

On recipient Companion:

1. open Builder;
2. Follow;
3. take Mac offline if useful;
4. Builder ships a second distinct Shot;
5. recipient's Following view later shows the new Ship;
6. Builder updates existing Shot;
7. Following shows Update;
8. no follower count appears anywhere;
9. no public follow record exists.

# ============================================================
# 99. WEBSITE ACCEPTANCE
# ============================================================

On live production origin verify:

    /registry
    canonical Shot route
    Builder route
    Claim receipt route
    Claim token metadata route
    claims contract status endpoint

Verify:

- finite supply real;
- open supply real;
- Claim CTA routes correctly;
- closed edition cannot Claim;
- Builder Follow deep link correct;
- app Timeline distinguishes Ship/Update;
- no "shipped v2";
- no wallet connect;
- no gas;
- no NFT marketplace language;
- canonical claim receipt resolves.

# ============================================================
# 100. CONTRACT DEPLOYMENT
# ============================================================

This prompt AUTHORIZES deployment of the new additive TohsenoClaimsV1 contract
to Robinhood Chain only after:

- complete Foundry suite passes;
- exact source is committed;
- runtime hash is known;
- constructor references exact active trusted ShotRegistry;
- deployment credential is the correct governed credential;
- deployment evidence is prepared;
- no existing generation-0.8 contract is modified.

Do not deploy a new Registry generation.

Record exact:

- chain ID;
- contract address;
- deployment tx;
- deployment block/hash;
- runtime code hash;
- constructor/reference state;
- source commit.

Create signed activation evidence using the repository's established trust model
adapted cleanly for this additive Claims contract.

Clients fail closed unless activation verifies.

# ============================================================
# 101. RELAYER DEPLOYMENT
# ============================================================

Extend the current constrained production relayer.

Do not make a new general wallet service.

Allow only exact claims-related calls.

Update production configuration dark first.

Health/status must distinguish:

    Claims contract configured
    Claims activation verified
    Claim relayer funded
    Claim relay enabled

Do not say Ready when any necessary boundary is absent.

# ============================================================
# 102. WEBSITE RELEASE ORDER
# ============================================================

Avoid a broken advertising window.

Safe sequence:

1. merge/deploy backwards-compatible Claims read/index support dark;
2. deploy additive TohsenoClaimsV1;
3. record/activate exact Claims contract;
4. enable chain indexer;
5. deploy relayer support dark;
6. deploy Mac/Companion source implementation;
7. run local + CI matrices;
8. produce signed/notarized target release when governed gates permit;
9. physically test real Claim;
10. physically test offline-Mac handoff;
11. physically test install;
12. test one Update;
13. verify one Ship invariant;
14. activate Claim write path;
15. enable new Registry/website Claim UI;
16. verify public origin from outside localhost.

Do not advertise Claim before actual mint works.

# ============================================================
# 103. 1.1 AND 1.2 RELEASE DISCIPLINE
# ============================================================

If 1.1.0 remains unreleased:

DO NOT quietly replace its source candidate with ADR 0035.

Retain its exact release commit and readiness evidence.

ADR 0035 should target the next correct minor line.

If current release architecture requires shipping 1.1 before 1.2 can become
public, obey that order.

Do not manufacture old acceptance.

If 1.1 completes during this session and owner evidence is available, finish its
governed release exactly.

Otherwise leave 1.1 evidence truthful and keep 1.2 Claims UI dark until an
appropriate released client exists.

# ============================================================
# 104. UPDATE CURRENT DOCS
# ============================================================

Reconcile current source truth in the same change.

At minimum update:

- README.md
- AGENTS.md
- docs/STATE.md
- docs/ARCHITECTURE.md
- docs/LIVING_CONNECTION.md
- threat model
- current CLI docs
- current Registry docs
- Companion docs
- website docs
- privacy docs
- release runbooks
- contract docs
- new ADR 0035

Pay special attention to stale text that still says Companion is not an identity
or wallet if ADR 0034/0035 now makes DeviceKey + smart account a public
authorization boundary.

Preserve distinctions:

    Companion pairing identity
    Companion Builder DeviceKey
    Tohseno smart-account address
    BuilderID when controlling a Shot
    InstallationKey
    Apple signing identity
    recovery authority

Do not merge them in prose.

# ============================================================
# 105. WEBSITE POSITIONING
# ============================================================

Do not replace Tohseno's core shipping mission with NFT marketing.

The main website remains about native software moving person-to-person.

Claims enrich the network.

Suitable language:

    Ship iPhone apps.
    Person to person.

and, in Registry/app context:

    Claim software you want in your world.

Do not lead with:

    Mint NFTs for apps

That is infrastructure language, not the product.

# ============================================================
# 106. REGISTRY EMOTIONAL TEST
# ============================================================

Open the current macOS app.

Select one of Your Apps.

It should still feel like:

    workshop
    machine
    source
    build
    phone

Then select Registry.

It must feel like walking through a door.

You should see:

    software entering the network;
    software changing;
    forks becoming new software;
    editions filling and closing;
    Builders worth following;
    things you can Claim.

If Registry feels like:

    another settings screen
    another app grid
    another create form

the product is wrong.

# ============================================================
# 107. CLAIM EMOTIONAL TEST
# ============================================================

A user should never need to understand:

    ERC-721
    EIP-712
    ERC-1271
    relayer
    gas
    Robinhood RPC
    BuilderAccountFactory

to experience the main ritual.

They encounter:

    Prayer Lock

They press:

    Claim

They draw:

    one circle

The system says:

    Claiming…

Reality happens.

Then:

    Claimed
    #184 of 888

And their Mac quietly begins preparing the software.

That is the experience.

# ============================================================
# 108. NO FAKE SUCCESS
# ============================================================

Retain Tohseno's epistemology.

"Claimed" means:

    canonical NFT exists.

"Shipped" means:

    first Registry state + first catalog source + immutable Claim Edition
    are all verifiably live.

"Updated" means:

    new Registry checkpoint + catalog release are canonically live.

"Ready for iPhone" means:

    exact claimed/selected release has been verified and built enough to
    require only the named device action.

"Installed" means:

    physical device inventory verified exact bundle.

"Following" means:

    durable private follow state exists.

Never infer any of these from button press alone.

# ============================================================
# 109. REQUIRED LOCAL VERIFICATION
# ============================================================

Run the complete current AGENTS.md verification matrix.

At minimum include all currently relevant:

    cargo fmt --all -- --check

    cargo clippy --locked --workspace --all-targets --all-features -- -D warnings

    cargo test --locked --workspace --all-targets --all-features

    Swift package builds/tests across:
      apple-identity
      fascia
      CompanionKit
      Companion
      macOS Tohseno

    exact Xcode build fixtures

    forge fmt --check
    forge build
    forge test -vvv

    website typecheck
    website tests

    Studio tests if still governed

    installer tests

    activation tests

    network E2E

    ontology/lifecycle scripts

    Companion E2E

    macOS service lifecycle

plus every new Claims/timeline/follow suite.

Warnings remain errors where repository policy requires it.

# ============================================================
# 110. CI
# ============================================================

Add Claims coverage to authoritative CI.

CI must prove:

- Solidity;
- cross-language signatures;
- claim mark vectors;
- Registry service;
- timeline;
- one-Ship invariant;
- Follow;
- Updates;
- website;
- native clients.

Do not make live Robinhood writes from ordinary CI.

Read-only live activation/code checks are acceptable under current policy.

# ============================================================
# 111. COMMIT AND PUSH
# ============================================================

After implementation:

1. inspect complete diff;
2. confirm no secrets;
3. confirm old 1.1 evidence remains truthful;
4. confirm unrelated dirty work untouched;
5. create cohesive commits;
6. push through authoritative repository rules;
7. do not force push;
8. do not bypass tag protection.

This prompt authorizes the implementation to reach `origin/main` when repository
policy permits.

# ============================================================
# 112. PRODUCTION DEPLOYMENT
# ============================================================

Deploy all backward-compatible production support using the repository's current
hosting/deployment system.

Do not migrate providers.

Deploy as needed:

- Registry API;
- claim indexer;
- timeline;
- token metadata/receipt service;
- constrained relayer;
- website;
- relay changes only if actually required.

Use real durable storage.

Run public-origin health checks.

Claims write/UI remains dark until contract activation + physical proof passes.

# ============================================================
# 113. RELEASE
# ============================================================

Choose the next correct version from repository state.

Expected:

    1.2.0

Do NOT reuse immutable 1.1 or 1.0.2 evidence.

When release gates are available:

- clean exact source commit;
- universal build under current policy;
- Developer ID signing;
- hardened runtime;
- notarization;
- stapling;
- Gatekeeper;
- mounted DMG;
- exact manifest;
- exact SHA-256;
- origin round trip;
- clean Mac install;
- Companion install/pair;
- Claim physical test;
- offline Mac test;
- physical install;
- one Update;
- Follow test.

Only then activate stable release.

# ============================================================
# 114. PRODUCTION SMOKE SHOT
# ============================================================

Use a real small compatible Shot.

Do not insert fake rows.

Its first ship must use the normal path:

    tohseno init
    tohseno deploy
    Companion chooses Claim Edition
    Companion approves
    Registry registers
    Claim Edition opens

Then a second Tohseno identity must Claim through normal Companion flow.

Record exact real evidence.

For first production Claims testing, a tiny app is preferred over a complex
entitlement-heavy app.

The purpose is proving the network.

# ============================================================
# 115. FINAL ACCEPTANCE CHECKLIST
# ============================================================

Do not call ADR 0035 complete until each applicable item is YES.

SHIP

[ ] Is each Shot shipped exactly once?
[ ] Does first deploy say Shipped?
[ ] Does later deploy say Updated?
[ ] Can retries never create another Ship?
[ ] Does fork child get its own one Ship?

CLAIM EDITION

[ ] Does every new public Shot get exactly one edition?
[ ] Is Open Edition possible?
[ ] Is finite N possible?
[ ] Is timed possible?
[ ] Is limited+timed possible?
[ ] Is edition policy immutable?
[ ] Do updates leave it unchanged?

CLAIM

[ ] Is one account limited to one Claim per Shot?
[ ] Is every Claim a real non-transferable ERC-721 NFT?
[ ] Is Claim gasless to the user?
[ ] Does Companion DeviceKey authorize it?
[ ] Is exact release bound?
[ ] Is exact checkpoint bound?
[ ] Is gesture commitment bound?
[ ] Is claim number assigned only on-chain?
[ ] Does UI wait for canonical confirmation?

GESTURE

[ ] Does user draw one continuous circle?
[ ] Is an imperfect expressive loop accepted?
[ ] Is only final x/y geometry retained?
[ ] Are timing/pressure/velocity discarded?
[ ] Does canonical 64-point representation verify cross-language?
[ ] Does user's mark appear in Claim receipt?
[ ] Does accessibility path exist?

OFFLINE

[ ] Can Claim complete with Mac asleep?
[ ] Does installation intention survive?
[ ] Does Mac later receive it?
[ ] Does Mac prepare exact claimed release?
[ ] Does Companion eventually say Ready for your iPhone?

INSTALL

[ ] Does claimed release locally build?
[ ] Does recipient use their Apple signing identity?
[ ] Does cable handoff remain truthful?
[ ] Does Installed require physical evidence?

REGISTRY

[ ] Has New Shot composer been removed from Registry?
[ ] Does Discover feel like network activity?
[ ] Does Following filter by private Builder follows?
[ ] Does Updates contain only high-signal personal items?
[ ] Are individual Claims absent from global-feed spam?
[ ] Does app Timeline contain exactly one Ship?

FOLLOW

[ ] Can Builder be followed?
[ ] Does handle change preserve follow?
[ ] Is follow private?
[ ] Are there zero follower counts?
[ ] Is there no public follow graph?

PROFILE

[ ] Does own Profile show Claimed software?
[ ] Can Claim receipt render exact mark?
[ ] Is Tohseno address available in technical detail?
[ ] Is there no wallet-connect ceremony?

WEBSITE

[ ] Does app page say Claim rather than Install?
[ ] Does edition state render from real chain data?
[ ] Does Claim deep-link to Companion?
[ ] Does Builder page allow Follow?
[ ] Does receipt route work?
[ ] Is there no NFT marketplace chrome?

CHAIN

[ ] Is generation-0.8 ShotRegistry unchanged?
[ ] Is TohsenoClaimsV1 separately deployed?
[ ] Is exact runtime activation verified?
[ ] Does relayer remain constrained?
[ ] Is no user private key server-side?

RELEASE

[ ] Is code committed?
[ ] Is it pushed?
[ ] Is CI green?
[ ] Are production support services deployed?
[ ] Is Claim contract deployed/activated?
[ ] Has a real production Claim succeeded?
[ ] Has offline-Mac flow succeeded?
[ ] Has physical install succeeded?
[ ] Has one Update been proven?
[ ] Is stable release advertised only after those truths?

# ============================================================
# 116. FINAL REPORT
# ============================================================

Return a precise engineering report containing:

1. ADR 0035 path;
2. architecture implemented;
3. exact release target;
4. exact commit SHA(s);
5. pushed branch;
6. CI run;
7. TohsenoClaimsV1 address;
8. deployment transaction;
9. runtime code hash;
10. activation evidence;
11. active generation-0.8 Registry coordinates independently verified;
12. production service deployment IDs;
13. production smoke ShotID;
14. Claim Edition type;
15. claimant Tohseno address;
16. Claim token ID;
17. Claim number;
18. Claim transaction hash;
19. gesture commitment;
20. exact claimed release;
21. offline-Mac test result;
22. physical install result;
23. one-Update result;
24. Follow result;
25. website/app/receipt URLs;
26. release DMG URL + SHA-256 if legitimately released;
27. any exact remaining owner-attended boundary.

Do not include private keys, recovery words or secrets.

# ============================================================
# 117. THE FINAL PRODUCT TEST
# ============================================================

Step away from implementation.

Open Tohseno.

On the left are Your Apps.

Choose one.

It feels like a workshop.

Source.
Build.
Simulator.
Phone.
Evolve.
Update.

Now click Registry.

The room changes.

Software is alive here.

A new app entered Tohseno this morning.

Another changed an hour ago.

A fork became its own thing.

An edition reached 888 / 888 and closed.

A Builder you follow shipped something new.

You encounter an artifact.

You open it.

There is space around it.

You see:

    431 / 888 claimed

You press:

    CLAIM

Your iPhone shows the artifact.

You draw one imperfect circle around it.

The circle closes.

A haptic.

    Claiming…

The Mac may be asleep.

That does not matter.

Robinhood Chain confirms.

Only then:

    CLAIMED
    #432 / 888

Your mark is there.

It is yours.

The app now exists in your Profile as something you encountered.

Your Mac wakes three hours later.

Without another request it receives the exact intention.

It verifies the exact software you claimed.

It downloads it.

It prepares it.

Your phone eventually says:

    Ready for your iPhone.

You plug in the cable.

The native app materializes.

Days later the Builder changes it.

The Registry does NOT say they shipped it again.

It says:

    Updated.

Because a thing can only enter the world once.

It can change forever after that.

You follow that Builder because you want to notice what they make next.

There are no follower counts.

There are no posts.

There are no likes.

There is software.

There are people.

There are encounters.

There is lineage.

There is time.

If the system left behind by this prompt does not materially produce that
experience using real production state, continue working.

Tohseno 1.1 made software travel.

ADR 0035 makes encountering software mean something.

Build the place.
