# TOHSENO

## Personal software with an identity and a history

**GENESIS candidate · `1.0.0-rc.1` · not the canonical release**

> A Shot is an Apple app with a permanent identity, a known anatomy, and a
> history controlled by its builder.

> TOHSENO is the open protocol that lets any compatible machine create,
> understand, verify, and continue a Shot.

## The problem

Software has traditionally arrived as somebody else's finished product. Even
when a person can now describe software in ordinary language, the output often
remains trapped inside the tool that generated it: identity belongs to an
account, history belongs to a database, and trust belongs to a hosted service.

Personal software needs a different center. The app should remain usable,
understandable, and evolvable when the first generator, website, company, or
server disappears.

## The Shot

A Shot begins with one coherent intention and becomes one complete Apple app.
It receives a random permanent ShotID. Every later Evolution keeps that ID and
produces another complete source world rather than a patch.

The integer Evolution number is also the Xcode `CFBundleVersion`. Each
Evolution commits to its source, its connective anatomy, and the preceding
Evolution. The builder signs the canonical record. Old Evolutions remain
immutable.

The result is an app with a locally verifiable lineage:

```text
intention → Evolution 1 → use → Evolution 2 → use → …
```

## BuilderID

The BuilderID is the durable controller of a Shot. In the GENESIS candidate it
is a deterministic smart-account address on Robinhood Chain.

The BuilderID is not an Apple ID, username, company account, computer, phone,
or permanent private key. Replaceable P-256 DeviceKeys authorize actions for
it. A compromised device can be revoked without changing the BuilderID or
Shot ownership. Optional recovery uses an established Ethereum recovery
authority derived from BIP-39 and BIP-44; recovery material never enters a
generated app.

Those replacement and recovery transitions are protocol design targets, not a
completed GENESIS implementation. The candidate signer and offline verifier
accept only the initial DeviceKey that reproduces the BuilderID. Its encrypted
recovery vault is a local backup only; it does not activate account recovery.
The CLI exposes no authorize, revoke, rotate, or recover command until a
canonical proof chain and evidence-backed nonce source exist.

On a Mac where hardware-backed key storage is unavailable, a visibly
test-only software DeviceKey may still sign local private records — its
conformance receipt says so in plain text — but it can never authorize a
public action. Private creation degrades gracefully; publication never does.

Apple ID remains relevant to Apple's code-signing and distribution systems. It
does not establish TOHSENO ownership.

## The Apple Fascia

The purpose of a generated app may be infinitely expressive. Its connective
tissue is finite.

The TOHSENO Apple Fascia declares the small shared anatomy that another machine
can inspect:

- an app-specific InstallationKey;
- local-first storage behavior;
- private-by-default data boundaries;
- narrowly scoped continuity envelopes;
- embedded provenance;
- distribution metadata;
- declared network and Apple capabilities;
- deterministic conformance receipts.

The Genome guides the intelligence that creates an app. It has four organs:
Laws, Structure, Taste, and Listening. Listening governs how intelligence
reads an intention — the builder's language is the app's language, a thin
intention is distilled into one complete gesture, a thick intention is a
contract, and every decision made on the builder's behalf, including any
law-forced deviation from an explicit request, is written into the world as
`INTERPRETATION.md`.

The Fascia is checked by ordinary deterministic code. An LLM is never the
conformance judge.

Generated applications use native Apple frameworks and carry no third-party
runtime dependencies by default. They open to useful behavior without a
mandatory account.

## Local first

Creating, building, signing, installing, using, evolving, and verifying a
private Shot require no TOHSENO server.

The Mac is enough for a complete Shot. A Shot finishes — signed record,
conformance receipt, retained Simulator artifact, verifiable lineage — when
its world builds and verifies on the Mac. The iPhone is a destination, not a
birth requirement: installation happens immediately when a phone is present
and resumes later through `tohseno refresh` when it is not. An unfinished
attempt never poisons the lineage, and a finished Shot can always evolve.

Prompts, reference images, unpublished source, app data, usage, recovery
material, and continuity relationships stay local by default. Publication is a
separate signed act. The builder chooses which commitment and public artifact,
if any, to expose.

## Consentful continuity

Each installation creates its own P-256 InstallationKey. Apps do not receive a
universal person identifier and cannot correlate a user merely because both
were made with TOHSENO.

When a person wants continuity, one installation can issue a signed, expiring,
nonced envelope for a specific audience, originating Shot, and narrow set of
claims. No username, password, Apple ID, shared private key, or Anky identity
server is necessary.

Continuity begins with consent. Unlinkability is the default.

## Signed Evolutions

TOHSENO uses RFC 8785 canonical JSON, SHA-256 commitments, and low-`s` P-256
signatures. These are established technologies.

Cryptography proves that a key signed bytes. The protocol schema gives those
bytes exact meaning. The BuilderID determines whether the key has authority.
A contract enforces the consequence of a public action.

> TOHSENO uses established cryptography to create a portable ownership and
> continuity layer for personal software.

It does not claim to have invented the underlying cryptography.

## A neutral public witness

A private Shot already exists before it reaches a chain.

The registry records only the minimal facts needed to witness a public lineage:
controller, current Evolution commitment, sequence, public state, and an
optional public content commitment. Anyone may relay a valid action. The
messenger does not become the owner.

The candidate contracts are non-upgradeable and have no privileged official
client, company key, relayer, or node. Human-readable handles, appcoin
associations, and App Store attestations remain outside the neutral core.

The registry is a public judge of signed transitions. It is not the author of
the protocol language and it is not the creator of a Shot.

## Independent factories and nodes

The first TOHSENO CLI and Studio are one doorway. They are not a required
doorway. The same is true one level down: the coding agent is not canonical
either — `tohseno create <app> --harness /path/to/agent` accepts any
executable that fulfills the TASK.md contract inside the same sandbox.

The normative schemas, byte rules, vectors, and conformance tests allow another
factory to read a Shot, verify it, and create the next complete Evolution. A
node may relay signed actions, index contract events, or serve public artifacts,
but indexing does not confer ownership.

One canonical rulebook. No canonical doorway.

## TOHSENO as Shot 1

The first TOHSENO companion app is created through the same factory path as any
other app. It can display Builder identity and devices, approve pairing,
inspect records, and show provenance. It has no username/password system and
needs no Anky server for local operation.

It is Shot 1 historically, not constitutionally. It receives no protocol
privilege. The `tohseno` handle must be claimed through the same signed action
available to any Shot.

## Relationship to `$TOHSENO`

An appcoin is an optional relation selected by a Shot controller. A token is
not required for Shot creation, ownership, verification, publication,
evolution, transfer, or use.

The GENESIS candidate never guesses a token address and does not deploy or
modify `$TOHSENO`. An association may be recorded only when exact coordinates
and explicit authorization are supplied.

## Conformance

Compatibility is a testable claim. A conforming implementation reproduces the
canonical record bytes and commitments, verifies signatures and ordered
lineage, checks the Fascia and Apple project, detects undeclared sensitive
capabilities, and emits evidence for every judgment.

Human output can remain simple. Machine output includes each check, expected
and observed values, and its evidence path. Failure returns a non-zero status.

## Explicit non-goals

TOHSENO does not make Apple ID a protocol identity, put private app data
on-chain, require a token, standardize every Apple API, create a universal user
profile, make one server canonical, or promise that unaudited contracts are
safe merely because tests pass.

The GENESIS release candidate is deliberately not the final canonical
protocol. Its job is to run the whole design against reality, expose failures,
and leave exact evidence for the cleanup that precedes canonical shipment.
