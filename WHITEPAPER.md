# TOHSENO

## Persistent identity for coherent human intention

**GENESIS candidate · `0.7.0` · not the canonical release**

> TOHSENO is a protocol for giving coherent human intentions persistent
> computational identity and allowing them to become, remain, and evolve as
> verifiable expressions.

> The first TOHSENO factory turns one coherent intention into a native Apple
> application that its owner can use and evolve.

The protocol is broader than software. The factory is deliberately not.
TOHSENO begins with native Apple software because it is a concrete medium in
which intention can become a complete, useful, inspectable thing.

## 1. The problem

People increasingly describe software in ordinary language, yet generated work
usually remains subordinate to the system that produced it. Its identity is an
account row, its history is a chat transcript, its source is an export, and its
continuity depends on a company, model, server, or repository URL.

That model mistakes the current material for the thing that persists.

A person may recognize that something should exist before an application,
repository, deployment, token, or even a precise specification exists. The
creative act begins with that declaration and the commitment to bring it into
form. The resulting work should be able to change its name, code, location,
platform, expression, and economic relationships without losing its origin or
authorized continuity.

TOHSENO gives that continuity a small, verifiable protocol body.

## 2. Canonical objects

- **Coherent Intention** — the preserved human declaration that something
  should exist.
- **Shot** — the persistent identity created when that intention is committed
  into reality.
- **Commitment** — the signed act that begins the Shot.
- **Genome** — the current accepted operational interpretation of what must
  remain true.
- **Expression** — one concrete manifestation of the Shot.
- **Organ** — a bounded declared capability that lets a software expression
  live.
- **Version** — one immutable identifiable state of one expression.
- **Feedback** — experience attached to the exact state that produced it.
- **Evolutionary Intent** — an authorized proposal for changing an existing
  Shot.
- **Evolution** — a verified transition from one accepted version to another.
- **Lineage** — the signed history connecting origin, ownership, expressions,
  versions, experience, and change.
- **Ownership** — authority to approve the Shot's recognized continuity.
- **Token Association** — an optional economic relationship that does not
  replace Shot identity.

These objects are intentionally few. Repositories, generated files, build
artifacts, deployments, public indexes, chain anchors, and tokens are useful
material or evidence. None is the primary object.

## 3. Coherent intention

A coherent intention is human source material: written, spoken, visual,
technical, emotional, incomplete, or accompanied by references. “Coherent”
does not mean polished. It means a person recognizes one thing that should
exist and commits to making it real.

TOHSENO preserves the original material exactly. Operational planning may
interpret it, but must not silently replace it with cleaned product language.
Private intention bytes remain private by default. A public record may commit
their digest and describe their availability without publishing them.

The protocol does not prove that an intention is philosophically true,
valuable, or good. It preserves a verifiable relationship between a declared
origin and the expressions later accepted under its identity.

## 4. The Shot and its commitment

A Shot receives a random permanent Shot ID when an authorized creator signs its
initial commitment. That identity is not derived from a folder, app name,
bundle ID, Git remote, database row, token address, or blockchain transaction.

The first signed lineage action binds the Shot ID, creator authority and key,
preserved intention commitment, origin time, and native, legacy, or descendant
origin. Genome proposal and acceptance follow as distinct actions; they are not
silently folded into creation. This is the explicit transition from
possibility into recognized protocol history.

A Shot survives changes to:

- its display name and description;
- source code and repository location;
- icon, bundle identifier, platform, or deployment;
- individual expressions and their versions;
- owner-facing tools and model providers;
- public availability and node replication;
- token associations.

The current Apple candidate created a stable Shot ID at folder creation but
signed its first record only when the first application state passed all gates.
The evolved protocol makes the initial commitment explicit while preserving
those existing version signatures through a compatibility adapter.

## 5. Genome

The Shot genome is the current accepted interpretation of what must remain true
for the Shot to remain itself. It may declare:

- purpose and intended people or community;
- essential experience and behavior;
- hard invariants and interaction laws;
- aesthetic, privacy, and ownership principles;
- platform commitments and required capabilities;
- boundaries, non-goals, and forbidden transformations;
- acceptance principles.

The genome is not the raw intention, source tree, or a generic model prompt.
The accepted machine-readable revision is canonical, and `GENOME.md` is its
deterministic human-readable rendering. Verification fails if those bytes
drift. Human edits are inputs to a new proposal; the accepted rendering changes
only after an authorized genome acceptance.

Ordinary implementation evolution does not change the genome. A proposed
mutation is a distinct signed action and becomes current only after explicit
acceptance by present authority. Historical revisions remain part of lineage.

The repository also contains a factory `Genome`: Laws, Structure, Taste,
Listening, Unfolding, Memory, and World. That constitution guides materializing
agents and remains important, but it is not the Shot-specific genome.

## 6. Expressions

An expression is one concrete manifestation of a Shot. It has a stable
expression ID, a medium, its own versions, capability graph, materialization
policy, and availability.

An iPhone app and a Mac app can be two expressions of one Shot. A later website
or protocol service can be another. A painting, song, book, or ritual can also
be represented at the protocol level without pretending that the current
factory knows how to produce it.

The first factory remains narrow: complete native Apple software built from an
owner's coherent intention.

## 7. Organs and the Apple Fascia

An organ is a bounded capability declaration, not merely a source directory.
It can state:

- what it provides;
- what state it owns;
- permissions and dependencies;
- events it emits and consumes;
- genome constraints it satisfies;
- acceptance tests;
- supported platforms.

The existing TOHSENO Apple Fascia is the first mature capability substrate. It
defines a finite connective anatomy for expressive applications:

- an app-specific InstallationKey;
- local-first persistence;
- private-by-default boundaries;
- narrowly scoped continuity envelopes;
- embedded provenance;
- distribution facts;
- declared network and Apple capabilities;
- deterministic conformance receipts.

Generated applications use native Apple frameworks and carry no third-party
runtime dependencies by default. They open to useful behavior without a
mandatory account. A deterministic verifier, never an LLM, judges conformance.

Neutral organ records adapt and explain the Fascia; they do not replace its
normative sources or turn the Apple factory into a vague universal generator.

## 8. Versions

A version is one immutable state of one expression. It binds:

- Shot and expression IDs;
- expression version ID and sequence;
- accepted genome revision and digest;
- source state or digest;
- materialization provenance;
- capability lock;
- build identity and artifact digests where practical;
- acceptance and verification results;
- actor and time;
- known incompleteness.

The existing candidate already seals complete numbered worlds containing
source, record, signature, Fascia, conformance, build log, Simulator artifact,
and best-effort preview. A crash-safe marker is the commit point. Failed
attempts are archived and do not consume the next canonical number.

Those worlds remain the material body of accepted Apple versions. Existing
`tohseno.shot/1` records remain byte-for-byte valid and are interpreted as v1
Apple-expression version commitments.

## 9. Feedback

Feedback belongs to the exact state that produced an experience. A feedback
record therefore binds the Shot ID, expression ID, version ID, build identity
when available, author identity when known, timestamp, privacy, observation,
and attachment descriptors.

Text, screenshots, files, and structured observations are supported. Private
is the default. Attachments can remain local while their digests and
availability are recorded. No feedback is a valid and honest state; absence is
not synthesized into approval.

Feedback never floats vaguely at the Shot level. An observation about version
`0001` remains about `0001` after later evolution.

## 10. Evolutionary intent and evolution

An evolutionary intent is a coherent instruction for changing an existing
Shot. It may arise from direct human desire, selected feedback, failures,
references, environmental change, or new technical possibilities.

It distinguishes:

- expression behavior or experience changes;
- implementation changes;
- capability or organ changes;
- proposed genome mutations.

A generated evolutionary intent is a proposal until accepted by an authorized
actor.

An evolution is the verified transition from one accepted version to another.
It preserves unchanged hard invariants, names accepted genome mutations, binds
materialization provenance, runs acceptance gates, records incompleteness, and
appends lineage only after success. It never overwrites the prior version or
the original intention.

## 11. Lineage

New canonical lineage is a sequence of signed, append-only actions encoded with
closed JSON, RFC 8785 canonicalization, SHA-256 commitments, and low-s P-256
signatures. Actions include their protocol and schema versions, Shot ID, actor,
time, previous action, payload and payload digest, and availability.

Lineage can contain:

- origin and commitment;
- intention descriptors;
- genome proposals and acceptances;
- expression and organ declarations;
- accepted versions and verification;
- feedback and evolutionary intents;
- evolutions and publications;
- ownership changes;
- token associations;
- artifact availability;
- forks and descendants;
- optional public anchors.

Derived snapshots and indexes are useful but reproducible. They cannot
supersede canonical actions.

Lineage need not be globally complete to be valid. The protocol distinguishes
absent, unknown, intentionally private, locally available, publicly committed,
replicated, cryptographically verified, and on-chain anchored. A participant
must not upgrade one state into another by implication.

## 12. Ownership and identity

Ownership is authority to approve continuity-changing actions. It does not
claim authorship of every generated line.

The candidate's existing identity system remains authoritative:

- a P-256 DeviceKey signs protocol commitments;
- the pinned counterfactual BuilderAccount on Robinhood Chain determines a
  durable chain-scoped Builder ID;
- private keys remain in Keychain/Secure Enclave and never enter a Shot;
- Apple ID governs Apple signing and distribution, not TOHSENO ownership.

The on-chain BuilderAccount supports device administration and optional
recovery. The current CLI and offline verifier deliberately accept only the
initial DeviceKey that reproduces the Builder ID. Replacement, transfer, and
recovery cannot be claimed as fully offline-verifiable until a canonical
authority proof chain and evidence-backed nonce source are implemented.

A visibly test-only software key may sign local private records where hardware
storage is unavailable. It cannot authorize public actions.

## 13. Nodes

A TOHSENO node preserves, validates, serves, and optionally replicates public
protocol records. The current candidate node stores action records, not
referenced artifact bytes. A node has its own identity and storage and can:

- validate schemas, commitments, signatures, and available lineage segments;
- store content-addressed signed actions;
- rebuild derived Shot indexes;
- serve the public records it possesses;
- report missing or private artifacts honestly;
- synchronize valid public records with configured peers;
- inspect storage integrity and supported schema versions;
- index optional chain anchors.

Validity is layered. A schema-valid, signature-valid segment whose predecessor
is unavailable can be retained as an unresolved public segment; it is not
promoted to authority-verified state until the complete available branch
reduces under the candidate authority policy. A later parent can resolve or
reject it.

Nodes do not need a shared mutable database or distributed consensus. They
agree on deterministic byte, signature, segment, and authority results when the
required context is available. They need not agree that an intention is
subjectively coherent, possess every artifact, choose one global current head
amid unresolved authorized branches, or materialize code.

One surviving node can still preserve and serve the public lineage it holds.
A malicious node can withhold or lie about availability, but cannot forge an
authorized action or alter content without failing verification.

Local creation, use, evolution, and verification do not require a node.

## 14. Public and private data

Private by default:

- raw intentions and references;
- unpublished source and artifacts;
- private feedback and attachments;
- application data and usage;
- transient plans and agent transcripts;
- recovery material and private keys;
- local continuity relationships.

Suitable for optional public commitment or replication:

- Shot and expression identifiers;
- signed public lineage actions;
- content digests and declared availability;
- accepted public genome projections;
- public verification receipts;
- owner-authorized artifacts;
- public chain anchors and token relationships.

Portable export and public page generation are projections, not recursive
copies of a working directory. A generated repository excludes private working
material from publication by default.

## 15. Verification and threat model

Cryptography proves that an authorized key signed exact bytes. Schemas and
reducers give those bytes deterministic meaning. Acceptance receipts prove
which checks ran and what they observed. None proves creative or metaphysical
truth.

Verification defends against:

- forged or replayed lineage actions;
- signature and payload tampering;
- schema downgrade and unknown-field attacks;
- unauthorized genome mutation;
- divergent derived state;
- incomplete or malicious replication;
- tampered portable bundles;
- chain/domain mismatch;
- spoofed token associations;
- private-data publication;
- malicious references and unsafe paths;
- untrusted templates, organs, and dependency substitution;
- arbitrary code execution disguised as inspection.

Materialization remains a code-execution boundary. Templates, dependencies, and
agents are not trusted merely because a record is signed. The Apple factory
uses strict project anatomy, source scans, fixed Fascia sources, bounded
dependencies, compilation, artifact checks, and deterministic conformance.

## 16. Smart-contract boundary

A private Shot exists before and independently of a chain.

The candidate contracts are optional public witnesses:

- `BuilderAccountFactory` predicts and deploys controller accounts;
- `BuilderAccount` validates authorized actions;
- `ShotRegistry` records controller, accepted head, sequence, nonce, public
  state, and an optional public checkpoint commitment;
- `ShotRelations` records optional handles, token coordinates, and App Store
  attestations.

They do not store repositories, intentions, genomes, feedback, transcripts, or
private references. A chain anchor can prove that a commitment existed and was
authorized; it cannot contain the total creative object or make subjective
intention objectively true.

The contracts are non-upgradeable, administrator-free, unaudited, planned, and
undeployed in this candidate. Planned addresses are not production contracts.

## 17. Token association

A Shot may have zero or more historical token associations. One current
association can identify a chain and token contract, with prior relationships
preserved in lineage or contract event history.

A token is not:

- the Shot ID;
- the owner or controller;
- an expression or repository;
- application ownership;
- a requirement for creation, use, verification, or evolution.

The existing v1 contract ABI names this relationship an Appcoin and accepts any
nonzero target chain ID, including Base `8453`. Protocol-facing language calls
it a Token Association while retaining ABI compatibility.

Anky can therefore be its own Shot, have one or more software expressions, and
associate `$ANKY` on Base. That relationship does not make the token contract
the Shot, merge Anky with TOHSENO, conflate `$ANKY` with any `$TOHSENO`
association, or constrain either project's ownership.

## 18. Portability: downloaded coherent intentions

“The open-source factory of downloaded coherent intentions” has a technically
honest meaning:

A Shot can be projected into a portable, verifiable bundle that another person
or machine can receive, inspect, follow, instantiate, express, fork, or evolve
according to its authority and available artifacts.

Such a bundle contains or resolves:

- Shot identity and origin;
- genome and ownership facts;
- expression definitions and accepted versions;
- signed lineage;
- explicit artifact availability and omissions;
- verification results;
- optional token associations;
- enough metadata for safe inspection or supported materialization.

It is not merely a zip of generated code. Import verifies before trust.
Receiving records is not adopting ownership. Cloning source is not creating
lineage. Attaching private local material does not publish it.

The current candidate bundle is deliberately narrower than the full portable
model: it carries a canonical file inventory and verified lineage projection,
omits expression source and retained build artifacts, and declares that it is
not ready for materialization. Its manifest proves the included bytes relative
to that manifest; signed lineage authenticates canonical actions. A trusted
transport or separately communicated manifest digest is still required to
authenticate the bundle as a whole. Resolving source and safely materializing
a received Shot remain future work.

## 19. Forks and descendants

An evolution preserves Shot identity. A descendant creates a new Shot ID and a
signed relationship to its parent. A copy without that relationship is simply
a copy.

This rule allows inspiration, independent ownership, and divergent genomes
without corrupting the parent's continuity or pretending every derivative is
the same creative identity.

## 20. The first factory: native Apple software

The concrete loop is:

```text
human intention
→ signed Shot commitment
→ accepted Shot genome
→ native Apple expression plan
→ resolved capabilities and Fascia
→ materialized source and Simulator artifact
→ deterministic acceptance
→ immutable version
→ exact-version feedback
→ authorized evolutionary intent
→ next accepted version
```

The visible folder remains the builder's direct working surface. It carries its
own identity, human-readable intention and genome, concise evolutionary-intent
surface, immutable version worlds, exact-version feedback, verification, and
private working area. Any compatible coding agent or editor can work in it.
Editing is ordinary filesystem activity; accepting an evolution is a signed
protocol event.

An accepted version completes on the Mac. A phone is a destination, not a birth
requirement. `tohseno refresh` can install later when a device is present.

Every generated application embeds Shot ID, expression ID, version ID, genome
revision and digest, protocol version, and practical source/build identity.
That metadata lets the app and tooling bind exported feedback or logs to the
exact experienced state. It does not claim that iOS permits arbitrary
inspection of other installed applications.

## Explicit non-goals

TOHSENO does not:

- prove the metaphysical coherence or worth of an intention;
- make an application, repository, token, or chain record the Shot itself;
- turn the first factory into a universal creative generator;
- require a server, blockchain, token, account, or Apple ID for local
  continuity;
- put private intentions, feedback, app data, or transcripts on-chain;
- create a universal human or installation identifier;
- permit one app to inspect arbitrary other installed iOS apps;
- make an LLM the verifier;
- trust unverified imported code or templates;
- invent distributed consensus where deterministic signed records suffice;
- claim undeployed or unaudited contracts are production infrastructure;
- fabricate missing history during migration.

## Release and protocol status

TOHSENO `0.7.1` is the stable local product release. Its installer, CLI,
Studio, Apple identity helper, and deterministic protocol materials are
released together as one checksummed artifact set.

The GENESIS protocol carried by that product remains explicitly pre-1.0 and
noncanonical. Its contracts are undeployed and unaudited, the public protocol
page is staging material, and there is no mainnet or permanent Arweave
publication claim. Product stability does not canonize the protocol or turn
candidate infrastructure into production fact.
