# ADR 0037: Network trust is exact-release evidence

Status: accepted

Date: 2026-09-01

This decision is additive to ADR 0034's person-to-person source network and ADR
0035's Claim semantics. It changes no frozen protocol encoding, generation-0.8
ABI, Claims ABI, public checkpoint, Claim Edition, DeviceKey authority, Ship,
Update, or installation truth.

## Context

The Registry can already show who signed one exact catalog release, its current
chain-backed checkpoint, content-addressed source, and declared build and
permission facts. It cannot yet answer which machine observations were made
about that exact release, which people reviewed which bounded scopes, or which
of those people are socially relevant to the recipient.

Claim cannot carry those meanings. A Claim says that one Tohseno identity
encountered and claimed one Shot at the exact release/checkpoint recorded by
the Claim. It is not review, endorsement, safety certification, installation,
ownership, or trust delegation.

External identities also cannot replace Builder authority. Farcaster, GitHub,
Base, X, OAuth, wallets, tokens, follower counts, or server accounts do not
hold the Builder DeviceKey and cannot Ship, Update, sign a human review, or
authorize an installation as the Builder.

## Decision

Tohseno's trust model is evidence for a recipient's decision, not a centralized
safe/unsafe verdict. It has four distinct additive layers.

### Identity Binding

An `IdentityBinding` is a DeviceKey-authorized statement that one BuilderID is
related to one stable external identifier. The first supported classes may be
Farcaster FID, GitHub numeric account ID, Base/EVM address, and optional X
account ID. Mutable username, display name, avatar, and bio are cached display
metadata, not binding identity.

Every binding states its schema/policy version, BuilderID, provider, stable
external identifier, proof class and digest, creation and optional expiry time,
nonce, DeviceKey identifier, and signature. Revocation or supersession is an
append-only signed statement; history is not silently erased. Provider proof
is independently verified before the Registry labels a binding verified.
Possession of the provider account without the active Builder DeviceKey grants
no Builder authority.

### Verification Report

A `VerificationReport` is content-addressed machine evidence about one exact
immutable catalog release. It binds at least the ShotID, release digest,
checkpoint digest, source-tree commitment, tool/policy versions, observation
time, observation set, findings, and report digest.

Direct observations—such as a declared entitlement, dependency version,
source digest, or network destination—remain distinguishable from model or
human interpretation. Model-assisted analysis is labeled as machine output.
Untrusted source is data, never instructions to the verifier; prompt injection,
build scripts, dependency metadata poisoning, and private-source disclosure are
explicit threats. A report does not say that a human reviewed anything and
does not mean the release is safe.

### Release Attestation

A `ReleaseAttestation` is a DeviceKey-signed, versioned, bounded human statement
about one exact release. It binds the ShotID, release digest, checkpoint digest,
reviewer BuilderID and DeviceKey ID, review-policy version, selected scopes,
outcome, finding references, optional Verification Report digest, creation
time, nonce, and signature.

The first scope vocabulary is deliberately small: source, dependencies,
entitlements, permissions, networking, privacy, payments, and reproducibility.
The UI must show the canonical meaning before the Companion signs. Withdrawal,
supersession, dispute, and DeviceKey revocation are append-only historical
facts. An attestation never attaches to a later release automatically.

### Personal social context

Farcaster follows may answer **which reviewers are people you already follow**.
They are a private/social prior, not a security delegation. Native scoped trust
preferences, if later added, remain private and explicit. Tohseno stores or
caches only the minimum social graph data needed for the stated experience and
never creates public follower counts, popularity scores, or leaderboards.

GitHub supplies technical identity and source provenance context. Base supplies
an economic identity around future useful work. Neither stars, followers,
contribution volume, wallet balance, token stake, nor payments become a global
reputation score. Reputation remains an inspectable projection over behavior:
releases, scoped attestations, findings, withdrawals, disputes, and
reproducibility history.

## Registry language and threat boundary

The Registry may render only facts backed by verified current evidence, for
example **Source digest matched**, **3 network destinations observed**, or **2
people you follow attested to this exact release**. It must not render **Safe**,
**Verified app**, an opaque trust score, or a social follow as approval.

The malicious-update case is mandatory: a new release begins with zero
attestations for that release. Builder and reviewer history may remain visible,
but no previous review is inherited. Claim and installation continue to bind
the exact release the person saw, even if a newer Update appears.

The relay and catalog operator transport and index signed evidence. They do not
sign for Builders/reviewers, rewrite scopes, swap digests, or decide what a
recipient may run. The recipient retains final authority.

## Rollout and acceptance

The current signed Builder profile reserves external attestations but correctly
rejects unverifiable provider claims. That behavior remains until a provider
proof verifier and migration-tested closed schema exist.

The first accepted vertical slice requires:

1. one deterministic content-addressed Verification Report for one exact public
   release;
2. one Companion-reviewed and DeviceKey-signed Release Attestation referencing
   that exact report/release;
3. Registry signature/digest verification and bounded display;
4. a second release that starts with no attestations;
5. invalid signature, replay, digest substitution, withdrawal, compromised old
   DeviceKey, and prompt-injection fixtures; and
6. honest separation of unit, integration, physical-Companion, and production
   evidence.

Farcaster, GitHub, Base, economic incentives, autonomous verification workers,
and personalized trust can ship in later additive slices. Their absence must
remain visible; placeholder checkmarks and self-asserted bindings are forbidden.

## Consequences

Tohseno becomes able to accumulate accountable intelligence around software
without replacing human judgment or turning popularity and money into truth.
The Mac is the intelligence workbench, Companion is the sovereign approval
surface, Registry is the public evidence view, and final installation authority
remains with the person whose device will execute the software.

