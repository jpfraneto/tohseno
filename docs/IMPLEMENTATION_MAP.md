# TOHSENO implementation map

This map describes the repository at `2ba008f` before the coherent-intention
protocol work began on 2026-07-29. It is the compatibility baseline for the
changes that follow. Existing code is authoritative where this document and
older prose differ. The stable `v0.7.1` patch release supersedes the
historical release-status statements in this snapshot.

> This is deliberately a pre-change map, not current protocol law. Statements
> below about publishing intention digests, `ShotRelations`, the absence of a
> peer node, or v0.7 predicted addresses are superseded by ADR 0006,
> `docs/PRIVACY.md`, and the current specification. They remain here only to
> explain the migration baseline.

## Repository lineage

The current `main` contains nineteen commits from 2026-07-29, beginning with
`e501d7a` and ending at `2ba008f`. They carried forward the Genesis candidate,
added the protocol and contracts, made a Mac sufficient for a complete
materialization, introduced the reusable Apple Fascia, moved Shots into visible
self-ledger folders, made one Shot contain many immutable Evolutions, simplified
the CLI around builder-owned agents, updated Studio, and provisioned the
hardware-backed Apple identity helper.

At this historical snapshot, the candidate was `0.7.0`, had not been tagged or
released, and the live installer remained `v0.6.0`. The candidate protocol page
was not deployed, and
the planned BuilderAccountFactory, ShotRegistry, and ShotRelations addresses
have no code. The checked-in Genesis candidate report predates the late-day
folder and CLI changes and is historical evidence, not a report for current
HEAD.

## Current canonical model

### Protocol

`tohseno-protocol` is a pure Rust crate. It owns closed JSON types, RFC 8785
canonicalization, SHA-256 commitments, fixed-width encodings, low-s P-256
signatures, deterministic tree hashing, EIP-712 actions, and interoperability
vectors.

- `ShotId` is a random 32-byte identity. It is already independent of the app
  name, folder, Git remote, bundle ID, token, database key, and controller.
- `ShotRecord` (`tohseno.shot/1`) is named for the Shot but represents one
  immutable state of the initial Apple expression. Its sequence is currently
  both the Evolution number and Apple build number.
- `SignatureSidecar` signs the canonical `ShotRecord`.
- `verify_lineage` reconstructs the contiguous signed record chain and freezes
  Shot ID, Builder ID, bundle ID, and Apple Fascia across that v1 chain.
- `FasciaManifest` declares the Apple expression's concrete capabilities,
  storage, network, privacy, and distribution constraints.
- `AppMetadata` is the public provenance resource embedded in the app.
- `ConformanceReport` records deterministic acceptance observations.
- `PublicAction`, `DeviceAction`, `PairingRequest`, and
  `ContinuityEnvelope` cover public registry actions, device authority,
  pairing, and installation continuity.
- `ShotOrigin::LegacyAdoption` is the existing honest migration boundary. It
  creates an N+1 signed root without inventing signatures for old history.

There is no neutral canonical record yet for the source intention, a
Shot-specific genome, an expression, a distinct version, feedback,
evolutionary intent, an evolution transition, artifact availability, a
descendant relationship, or a general append-only lineage action.

### Local engine and generated folders

The engine owns state and orchestration. A generated Shot is a visible app
folder containing `.tohseno/app.toml`, a private briefing, references, the
factory genome, the Apple Fascia, and immutable completed directories at
`.tohseno/evolutions/NNNN`.

`tohseno create` captures raw text and references, resolves the existing
Builder identity, creates the stable Shot ID, writes the local binding and
briefing, and starts one detached unattended runner. The runner performs
intelligent conception, validates and internally accepts the app-specific
proposal, materializes and repairs the Release candidate, and asks the engine
to record and deliver it. Preparation alone does not sign the initial
commitment or create version `0001`.

The recording engine snapshots the living tree, checks the Apple project and
fixed Fascia, builds a Release artifact, attempts a preview, runs conformance,
birth experience checks, and offline verification, signs the Version record,
writes the crash-safe `.complete` marker, and advances derived state. Its
low-level `record` API retains best-effort connected-device installation after
acceptance. The unattended Create/Evolve runner instead uses
`record_and_deliver`: it waits for a paired iPhone and installs and launches
the exact verified candidate before signing Version acceptance. Failed
attempts are archived under `.tohseno/incomplete` and never consume a
canonical sequence.

The static engine `Genome` is the factory constitution: Laws, Structure,
Taste, Listening, Unfolding, Memory, and World. It is not the accepted,
Shot-specific operational genome introduced by this work. The existing term
and implementation remain valid in that factory role.

The Apple Fascia and its finite capability declarations are the first concrete
capability substrate. They are not replaced by generic source folders called
organs; they are adapted into richer neutral capability declarations while the
Apple factory remains specific.

### Identity and ownership

The existing identity system is retained:

- a P-256 DeviceKey signs local protocol commitments;
- its pinned counterfactual BuilderAccount on chain `4663` determines the
  chain-scoped Builder ID;
- private keys stay in Keychain/Secure Enclave and never enter a Shot folder;
- software-test identity is local-only;
- BuilderAccount supports stronger device/recovery semantics on-chain, while
  the current offline verifier accepts only the initial key.

Ownership transfer exists in the registry contract, but the v1 off-chain
record chain freezes `builder_id`. Neutral ownership actions must reconcile
that boundary without rewriting old records or adding a second identity
system.

### Contracts

The contracts are non-upgradeable, administrator-free, and appropriately
narrow:

- `BuilderAccountFactory` predicts and deploys controller accounts.
- `BuilderAccount` validates authorized P-256 protocol actions and device
  administration.
- successor `ShotRegistry` uses permissionless commit plus signed reveal and
  witnesses only controller, intentionally public lineage head,
  checkpoint-local sequence, and nonce.

Contracts do not store prompts, repositories, genomes, feedback, or private
material or app-runtime/end-user commitments. The undeployed v0.7
`ShotRelations` ABI remains available only in the immutable `v0.7.1` tag and
release archive as a frozen private-verification input; it will never be
deployed by TOHSENO. Current Token Association is signed lineage, separate
from Shot identity, and can identify explicit chains including Base `8453`.

### Network and Studio

There is no TOHSENO peer node yet. `engine/public_network.rs` is a guarded
read-only chain client, and Studio is a loopback-only product UI. Neither is a
replication network.

Studio already provides secure localhost intake, library/evolution inspection,
offline verification, previews, Simulator launch, identity facts, and
contract-plan status. It has no feedback capture, genome review, expression
identity, availability view, node identity, peers, or synchronization.

## Compatibility decisions

The evolution follows these rules:

1. Every `/1` canonical byte and signature law remains readable and testable.
2. A v1 `ShotRecord` becomes a compatibility source for one accepted version
   of one Apple expression. It is never silently rewritten or re-signed.
3. New signed lineage actions use the same deterministic serialization,
   digest, signature, Shot ID, and actor identity foundations.
4. Derived `app.toml`, `shot.json`, and current-state files are reproducible
   snapshots; the append-only action stream is canonical for new ontology
   events.
5. The original intention remains exact local material. Public lineage may
   carry its digest and honest availability, never private bytes by default.
6. The Shot-specific genome is distinct from the factory Genome. Genome
   proposals and acceptances are explicit; ordinary implementation work cannot
   mutate it silently.
7. Expression identity and expression version are separate from Shot identity
   and Shot lineage position. Existing bundle and Fascia continuity rules stay
   inside the v1 Apple-expression adapter.
8. Existing immutable Evolution directories remain the material bodies of
   accepted versions. “Version” names the state; “Evolution” names the
   authorized transition, with compatibility labels retained where required.
9. Feedback always binds to an expression and exact version. Private is the
   default.
10. Nodes validate and replicate signed public records and declared available
    artifacts. They do not run creative agents, require shared mutable storage,
    expose private inputs, or invent distributed consensus.
11. A portable Shot bundle is a verified protocol projection with explicit
    omissions and availability, not an ownership transfer and not merely a
    source archive.
12. The successor contracts remain narrow public witnesses; optional
    relationships live in signed lineage instead of a generic relationship
    registry. A token association never becomes the Shot, its owner, its
    repository, or its expression.
13. Existing v0.6 adoption, `latest_shot` deserialization, and candidate folder
    records receive deterministic adapters. Missing historical facts are marked
    unknown rather than fabricated.
14. Folder moves and renames may update a mutable local display name but never
    the stable Shot or expression identity.
15. The Apple factory remains the first operational product. Neutral protocol
    records may describe other media, but current materialization stays native
    Apple software.

## Required implementation boundaries

- `protocol/` defines neutral records, signed actions, validation, reduction,
  migration adapters, schemas, and fixtures without filesystem, UI, network,
  model-provider, or Apple toolchain policy.
- `engine/` owns local layout, migrations, materialization orchestration,
  genome synchronization, feedback, portability, and derived snapshots.
- `cli/` exposes direct automation-safe commands and the secure loopback
  Studio transport.
- `node/` owns public action storage, deterministic validation, rebuildable
  indexes, explicit peer synchronization, and the node HTTP/CLI transport.
- `studio/` presents the same engine state; it does not create a second data
  model.
- `fascia/apple/` embeds compatible expression/version/genome/build identity
  while retaining v1 decoding.
- `contracts/` remains limited to authorization and public witnessing; tests
  freeze Base token association and replacement semantics.

## Baseline verification

Before ontology edits:

- Rust workspace: 163 tests passed.
- Protocol formatting, clippy, frozen vectors, and 34 tests passed.
- Foundry: 52 tests passed; ABI/bytecode/deployment-plan drift checks passed.
- Apple Fascia: 8 tests passed.
- Apple identity: 7 tests passed.
- Hello World iOS Simulator build passed.
- Studio JavaScript, shell syntax, Python syntax, archive regressions, and
  release-package regressions passed.
- Workspace `cargo fmt --all --check` alone failed on already-committed
  formatting in three engine files.

Seven tracked SwiftPM build-cache files under `fascia/apple/.build` were dirty
before this work. They are not part of the ontology change and must not be
silently reverted.

## 2026-08-03 additive web-to-local transport map

This appendix describes current implementation boundaries; the historical
baseline above remains preserved for migration context.

- `website/apps/site/public/modules/` builds and encrypts the private package,
  persists Browser Draft and immutable transfer state in IndexedDB, and drives
  bounded relay retries. `public/app.js` is only the UI controller.
- `website/apps/site/src/relay-storage.ts` owns the filesystem state machine;
  `relay-routes.ts` owns strict HTTP schemas, origins, capabilities, bounds,
  and no-store responses. `config.ts` owns the fail-closed release gate.
- `oneshot/oneshot.sh` is the claim-capable canonical installer source. The
  public copies remain on the published release until activation.
- `cli/src/intent_commands.rs` owns strict claim transport and invokes the
  engine's shared package/import code. It does not create a Shot.
- `engine/src/intent_package.rs` owns the nonprotocol package parser;
  `pending_intention.rs` owns atomic durable local state; `shot_layout.rs`
  remains authoritative for reference-image byte validation.
- `cli/src/studio_server.rs` exposes bounded localhost pending-intention reads
  and selects either an inline or local-pending source before calling the one
  existing planning/preparation path. `studio/` renders that state.

The package and relay schemas are versioned defensive transport contracts, not
additions to `protocol/`. They carry no app name, Shot ID, controller, wallet,
credential, or protocol action.
