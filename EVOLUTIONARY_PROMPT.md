# HISTORICAL MASTER PROMPT — TOHSENO GENESIS CANDIDATE

> **Superseded implementation input.** This prompt is preserved as historical
> evidence of the v0.7 build and is not current protocol, contract, or
> deployment authority. It asks for surfaces later rejected by security review,
> including `ShotRelations`, public handles, App Store self-claims, and a v0.7
> mainnet lifecycle. Those instructions must not be executed. Current law lives
> in `protocol/SPECIFICATION.md` and accepted ADR 0006; no contract generation
> is active.

You are Codex operating directly inside the existing `jpfraneto/tohseno` monorepo on JP’s Mac.

Read this prompt completely. Then inspect the entire repository, its Git history, the current working tree, installed tools, existing documentation, current CLI behavior, current Studio behavior, and every relevant source file before changing anything.

Do not respond with a plan and stop.

Do the work.

Evolve the current system into one integrated **TOHSENO Genesis Candidate** in which the complete lifecycle works:

```text
install TOHSENO
    ↓
create a local BuilderID
    ↓
take a Shot from one intention
    ↓
generate a complete Apple app
    ↓
embed the TOHSENO Apple Fascia
    ↓
build and install it on an iPhone
    ↓
produce a signed, immutable Shot record
    ↓
verify it deterministically
    ↓
publish its commitment through a neutral registry
    ↓
evolve it into the next complete Shot
    ↓
verify the lineage locally and publicly
    ↓
allow an independent implementation to reproduce the verification
```

The mission is not to create another roadmap or scatter this work across hypothetical versions.

Compress the previously imagined `0.7.0 → 1.0.0` progression into **one coherent repository evolution**.

The resulting code should be marked:

```text
0.7.0
codename: GENESIS
status: protocol candidate, not canonical release
```

This is a real production candidate.

It may deploy experimental infrastructure to Robinhood Chain mainnet and exercise the full lifecycle against it.

It must **not** yet:

* tag `v1.0.0`;
* declare an immutable deployment canonical;
* update the stable public installer;
* update DEX Screener;
* announce the protocol;
* publish the final permanent Arweave Genesis Bundle;
* open an unrestricted public gas relayer;
* claim that incomplete or unverified work succeeded.

The purpose is to make the whole system live, run it against reality, discover what breaks, and leave exact evidence.

---

# 1. Operating mode

## Work on the current branch

Do not create a feature branch unless the repository is already on one.

JP works directly on the living system.

Never force-push.

Before editing:

```sh
git status --short
git log -10 --oneline
cargo metadata --no-deps
```

Preserve all pre-existing uncommitted work. Never overwrite user changes merely to simplify implementation.

## Work autonomously

Do not stop to ask conceptual questions already answered by this prompt.

When implementation details are uncertain:

1. inspect the existing code;
2. inspect the installed tool’s `--help`;
3. inspect authoritative local or official documentation;
4. choose the smallest design preserving the invariants;
5. record the decision in code or an ADR;
6. continue.

If a physical or secret-bearing action genuinely requires JP—Touch ID, plugging in an iPhone, unlocking a signing key, selecting an Apple team, or authorizing a mainnet deployment—emit exactly one imperative sentence describing the immediate action, then continue every other independent task.

Do not expose, print, commit, copy into logs, or request raw private keys or mnemonic words.

## Do not rewrite working machinery without cause

The present system already has:

* a Rust engine library;
* a thin CLI frontend;
* a Studio frontend;
* an event bus;
* append-only integer Shots;
* a filesystem ledger;
* a coding-harness plugin;
* generation and repair loops;
* Apple signing and USB installation;
* a one-line installer;
* three machine voices plus harness output.

Preserve those working rails.

Refactor only where the protocol requires a clean boundary.

---

# 2. The constitutional center

Encode these principles in code, schemas, tests, documentation, and contracts.

## What TOHSENO is

> TOHSENO is an open protocol for creating and evolving Apple apps from a shared, machine-verifiable structure.

> A Shot is one app, its permanent identity, and its signed history.

> Every Shot belongs to its builder.

> Any compatible factory can understand and continue it.

> One canonical rulebook. No canonical doorway.

> Nodes index. Builders own.

> Intelligence imagines without bounds; machines keep the promises exact.

## What the TOHSENO implementation is

The application, CLI, Studio, node, and services built by JP and Anky, Inc. are the **first entry point** into TOHSENO.

They are not privileged by the protocol.

The contracts must not recognize an official client, server, factory, relayer, node, company, token interface, or website.

The first implementation may be historically first and experientially best.

It must never be technically necessary.

## What is canonical

The canonical objects are:

### The TOHSENO Protocol

**Ownership** — which cryptographic identity has authority over a Shot.

**Signed actions** — explicit commands authorized by that identity, such as creating, evolving, publishing, transferring, pairing a device, claiming a handle, or associating an appcoin.

**Lineage** — the ordered chain connecting every accepted Evolution of the same Shot.

**Public records** — the minimal facts anyone can inspect and independently verify about a Shot.

### The TOHSENO Apple Fascia

The finite connective structure shared by every compatible TOHSENO app.

A TOHSENO app may appear on iPhone, iPad, Apple Vision, or any Apple surface capable of running the resulting application. The Mac is the workshop where Shots are created, built, signed, and materialized.

The app’s purpose may be infinitely expressive.

Its connective tissue must be finite, declared, inspectable, and known.

### The protocol schemas

The exact fields, types, byte encodings, hashing rules, signature formats, and allowed values used by TOHSENO objects.

### The deployed registry contract

A neutral public witness that verifies authorized actions and records the current public state of each Shot.

### The conformance tests

Pure, deterministic checks an application, factory, node, or implementation must pass before truthfully claiming TOHSENO compatibility.

---

# 3. Identity model

Implement four separate identity concepts. Never collapse them.

## BuilderID

A BuilderID is the stable public identity that owns and controls Shots.

For the Genesis Candidate, represent it as a deterministic smart-account address on Robinhood Chain:

```text
eip155:4663:<account-address>
```

The address remains stable when device keys are added, revoked, or replaced.

A BuilderID is not:

* an Apple ID;
* a TOHSENO username;
* an Anky, Inc. account;
* a Mac;
* an iPhone;
* one permanent private key;
* a passkey relying-party account;
* a seed phrase;
* the TOHSENO application.

A BuilderID is the durable center around which replaceable signing authorities are organized.

## Builder DeviceKey

Each authorized physical device has its own P-256 key.

The private key must be generated and retained using Apple’s hardware-backed facilities where available.

The device key authorizes protocol actions.

The BuilderID smart account records which P-256 public keys are currently authorized.

A lost Mac key can be revoked without changing the BuilderID or Shot ownership.

## Recovery Root

Recovery is separate from ordinary signing.

Implement an optional recovery mechanism using established cryptographic standards. Do not invent a new mnemonic format.

For the Genesis Candidate:

* support a BIP-39 mnemonic as one possible recovery mechanism;
* derive a standard Ethereum secp256k1 recovery authority using a documented BIP-44 path;
* use that recovery authority only to recover BuilderID control, authorize replacement devices, or revoke compromised devices;
* never import the mnemonic into generated applications;
* never store the mnemonic in plaintext;
* never print it in logs;
* never regenerate or silently replace it;
* clearly document the derivation path and domain;
* permit a BuilderID to begin locally before recovery is exported;
* warn before public publication when no recovery path has been secured, but do not block private local app creation.

Do not implement Solana or other wallet derivations merely because the same mnemonic could support them. TOHSENO v1 needs only the recovery authority it actually uses.

## InstallationKey

Every generated TOHSENO application creates its own app-specific P-256 InstallationKey on first launch.

This key represents one installation of one app.

It is not automatically connected to:

* the BuilderID;
* another TOHSENO app;
* another installation;
* an Apple ID;
* a TOHSENO server;
* a universal user profile.

No username.

No password.

No mandatory account screen.

No global cross-app tracking key.

## Consentful continuity

Create an exact protocol object allowing two installation identities to establish a narrow, explicit relationship.

The default must be unlinkability.

A continuity proof must contain:

* issuer installation public key or installation identifier;
* intended recipient or audience;
* originating ShotID;
* narrowly scoped claims;
* nonce;
* expiration;
* explicit signature.

A continuity proof must not expose:

* a universal human identifier;
* every app the person uses;
* the BuilderID unless explicitly required;
* unrelated local data;
* unrelated continuity relationships.

Demonstrate this invariant:

> Two independently generated TOHSENO apps can recognize a continuity relationship chosen by the user without usernames, passwords, shared private keys, Apple ID, or an Anky, Inc. identity server—and cannot correlate the person before consent.

This is a core TOHSENO innovation.

---

# 4. The user is the center

Make this statement architecturally true.

## Identity is independent of the client

The CLI may create or display a BuilderID.

Studio may visualize it.

The TOHSENO iPhone app may authorize another device.

Another implementation may produce the same action.

No client owns the identity.

## Identity is independent of a particular device

A device is an authorized signer, not the permanent human identity.

## Protocol ownership is independent of Apple ID

Apple ID remains relevant only to Apple-specific functions such as:

* Xcode signing;
* device installation;
* App Store publication;
* optional iCloud services.

Apple ID must never determine TOHSENO ownership or membership.

Rename the existing engine concept currently called `identity` when it actually means Apple signing readiness.

For example:

```text
engine/src/gates/identity.rs
```

should become the equivalent of:

```text
engine/src/gates/apple_signing.rs
```

Update all symbols and tests accordingly.

## Builder identity is independent of application-user identity

The public BuilderID may say who created and controls a Shot.

It must not automatically reveal who is using an installation of the resulting app.

## Private activity is independent of public provenance

Private by default:

* user prompts;
* reference images;
* local application data;
* unpublished source;
* private Shots;
* app usage;
* continuity links;
* recovery material;
* local device inventory.

Public only through an explicit signed action:

* Shot commitment;
* public lineage head;
* controller;
* publication state;
* source or manifest URI chosen for publication;
* handle claim;
* appcoin association;
* App Store graduation attestation.

---

# 5. Meaning of signatures

Encode this division clearly:

```text
Cryptography proves:
“This key signed this digest.”

The TOHSENO schemas establish:
“This digest represents CREATE_SHOT.”

The BuilderID establishes:
“This key is authorized to act.”

The contract enforces:
“Therefore this public transition is valid.”
```

The specification defines meaning.

The schemas define bytes.

The device produces the signature.

The BuilderID defines authority.

The contracts enforce public consequences.

The Rust verifier must reproduce the same judgment locally.

The contract is the public judge.

It is not the author of the language.

---

# 6. Target repository architecture

Adapt to existing code rather than following names blindly, but create clean equivalents of this structure:

```text
tohseno/
├── MASTER_PROMPT.md
├── README.md
├── WHITEPAPER.md
├── Cargo.toml
│
├── protocol/                    # Rust crate + normative protocol material
│   ├── Cargo.toml
│   ├── src/
│   │   ├── builder.rs
│   │   ├── canonical.rs
│   │   ├── continuity.rs
│   │   ├── digest.rs
│   │   ├── evolution.rs
│   │   ├── fascia.rs
│   │   ├── identity.rs
│   │   ├── record.rs
│   │   ├── schema.rs
│   │   ├── signature.rs
│   │   ├── tree_hash.rs
│   │   └── lib.rs
│   ├── schemas/
│   ├── test-vectors/
│   ├── SPECIFICATION.md
│   ├── IMPLEMENTERS.md
│   └── CONFORMANCE.md
│
├── apple-identity/              # tiny Apple-native key bridge
│   ├── Package.swift
│   ├── Sources/
│   └── Tests/
│
├── fascia/
│   └── apple/
│       ├── FASCIA.json
│       ├── FASCIA.md
│       ├── IDENTITY.md
│       ├── STORAGE.md
│       ├── CONTINUITY.md
│       ├── PRIVACY.md
│       ├── PROVENANCE.md
│       ├── DISTRIBUTION.md
│       ├── swift/
│       └── tests/
│
├── contracts/
│   ├── foundry.toml
│   ├── src/
│   │   ├── P256Verifier.sol
│   │   ├── BuilderAccount.sol
│   │   ├── BuilderAccountFactory.sol
│   │   ├── ShotRegistry.sol
│   │   └── ShotRelations.sol
│   ├── script/
│   ├── test/
│   └── deployments/
│
├── node/                        # optional reference indexer + relayer crate
│
├── engine/                      # existing local factory engine
├── cli/                         # existing CLI
├── studio/                      # existing Studio
├── genome/                      # instructions for generative intelligence
├── oneshot/                     # installer
├── genesis/
│   ├── SHOT_1_INTENT.md
│   ├── GENESIS_BUNDLE.md
│   └── lifecycle/
│
├── site/
│   └── protocol/                # generated single public entry-point page
│
├── scripts/
│   ├── probe-p256.sh
│   ├── deploy-candidate.sh
│   ├── lifecycle-local.sh
│   ├── lifecycle-mainnet.sh
│   ├── build-genesis-bundle.sh
│   └── release-candidate.sh
│
└── .github/workflows/
```

Keep dependencies disciplined, pinned, and justified.

Generated applications must continue to have zero third-party runtime dependencies unless a future declared Fascia explicitly permits otherwise.

The TOHSENO tool itself may use carefully chosen cryptographic, canonicalization, RPC, and contract-tooling dependencies.

---

# 7. Separate genome from Fascia

This distinction is essential.

## Genome

The existing files such as:

```text
genome/LAWS.md
genome/STRUCTURE.md
genome/TASTE.md
```

are instructions given to generative intelligence.

They tell the coding agent what to build and how to behave.

They may contain prose, taste, examples, and generative constraints.

## Fascia

The Fascia is deterministic compatibility law.

It must be understandable by a simple program that has no intelligence.

The Fascia must define:

* required files;
* required interfaces;
* required identity behavior;
* local-first persistence behavior;
* privacy behavior;
* continuity behavior;
* provenance embedding;
* distribution metadata;
* allowed schema values;
* capability declarations;
* hash rules;
* conformance gates.

A coding model creates the expression.

A deterministic verifier determines whether the result kept its promises.

Do not ask an LLM whether an app conforms.

---

# 8. The initial TOHSENO Apple Fascia

Freeze the smallest useful connective structure.

Do not attempt to standardize every possible Apple API.

The Fascia defines the shared organs, not every domain-specific behavior.

Every generated app must contain the equivalent of:

```text
<generated-project>/
├── TOHSENO/
│   ├── shot.json
│   ├── signature.json
│   ├── fascia.json
│   ├── conformance.json
│   ├── FASCIA.md
│   ├── IDENTITY.md
│   ├── STORAGE.md
│   ├── CONTINUITY.md
│   ├── PRIVACY.md
│   ├── PROVENANCE.md
│   └── DISTRIBUTION.md
│
└── <app-source>/
    └── TohsenoFascia/
        ├── InstallationIdentity.swift
        ├── ContinuityEnvelope.swift
        ├── LocalPersistence.swift
        ├── Provenance.swift
        └── TohsenoMetadata.swift
```

The exact Xcode paths may differ according to the existing generator and target layout, but their meaning must be exact and discoverable.

## Required Fascia behavior

### Application shell

* SwiftUI;
* iOS 17+ or the repository’s verified minimum;
* standard Apple project;
* no external package dependencies by default;
* useful screen reachable immediately;
* no mandatory onboarding carousel;
* no account wall.

### Installation identity

* app-specific P-256 key;
* created automatically;
* private key remains device-bound;
* public identifier derived deterministically from the public key;
* no central registration required.

### Storage

Default local first:

* SwiftData for durable structured domain state where appropriate;
* UserDefaults only for small preferences;
* Keychain for small secrets and persistent key references;
* Secure Enclave for eligible signing authority;
* files for exported or document-like content.

CloudKit must be optional and explicitly declared.

No Fascia requirement may make Apple ID mandatory.

### Continuity

* explicit signed envelopes;
* pairwise or narrowly scoped;
* no automatic universal correlation;
* transport-neutral schema;
* QR and file/deep-link transport can be reference implementations;
* no dependency on the official TOHSENO app.

### Privacy

* no telemetry by default;
* no tracking;
* no silent identity linkage;
* no upload of prompts, private Shot data, or local app content;
* all network use must be declared in `fascia.json`.

### Provenance

Embed:

* ShotID;
* evolution number;
* evolution commitment;
* BuilderID or public creator reference where appropriate;
* Fascia identifier;
* factory identifier and version;
* source commit or source commitment;
* public registry coordinates if published.

Do not embed Builder DeviceKey secrets or user InstallationKey secrets.

### Distribution

Record:

* bundle identifier;
* `CFBundleVersion`, identical to the integer Shot number;
* supported Apple surfaces derived from the Xcode project;
* local, published, or App Store state;
* optional App Store identifier once explicitly attested.

## Finite capability vocabulary

Define a small machine-readable capability declaration.

Do not pretend to enumerate every future app behavior.

At minimum distinguish:

* local storage;
* network access;
* private CloudKit sync;
* StoreKit;
* notifications;
* camera;
* microphone;
* location;
* contacts;
* health;
* Bluetooth;
* other Apple entitlements.

Conformance must compare declared capabilities with actual Info.plist entries and entitlements wherever deterministic inspection is possible.

Undeclared sensitive capabilities are a conformance failure.

---

# 9. Protocol record model

The protocol name is timeless:

```json
{
  "protocol": "tohseno"
}
```

Version the exact machine expressions, not the meaning of the name:

```json
{
  "schema": "tohseno.shot/1",
  "fascia": "tohseno.apple/1"
}
```

## ShotID

Generate one cryptographically random 32-byte ShotID when an app is first created.

It never changes.

Do not derive identity from the slug, domain, appcoin, bundle identifier, or TOHSENO server.

## Evolution

Each integer Shot in the existing filesystem ledger becomes one complete Evolution of the protocol Shot.

Preserve:

```text
Shot number = CFBundleVersion
```

The first complete generation is Evolution 1.

Evolution 2 points to Evolution 1.

Old shot directories remain immutable.

## Canonical local Shot record

Define a normative schema equivalent to:

```json
{
  "protocol": "tohseno",
  "schema": "tohseno.shot/1",
  "shot_id": "0x<32-byte-id>",
  "slug": "tohseno",
  "builder_id": "eip155:4663:0x<builder-account>",
  "sequence": 1,
  "previous": null,
  "fascia": "tohseno.apple/1",
  "bundle_id": "com.example.tohseno",
  "bundle_version": 1,
  "genesis_input_sha256": "0x...",
  "source_tree_sha256": "0x...",
  "fascia_sha256": "0x...",
  "factory": {
    "implementation": "jpfraneto/tohseno",
    "version": "0.7.0",
    "source_commit": "<git-commit>"
  },
  "created_at": "<RFC3339>"
}
```

For later Evolutions:

```json
{
  "sequence": 2,
  "previous": "0x<evolution-1-commitment>"
}
```

Do not include self-referential hashes.

The Evolution commitment is computed from the canonical record bytes excluding the signature sidecar.

## Signature sidecar

Use an exact signature schema equivalent to:

```json
{
  "schema": "tohseno.signature/1",
  "algorithm": "p256",
  "digest": "0x...",
  "public_key": {
    "x": "0x<32-bytes>",
    "y": "0x<32-bytes>"
  },
  "signature": {
    "r": "0x<32-bytes>",
    "s": "0x<32-bytes>"
  },
  "low_s": true
}
```

Enforce low-`s` in Rust and Solidity.

## Canonicalization

Choose and document one deterministic representation.

For JSON records, implement RFC 8785-compatible canonical JSON or another equivalently precise, cross-language rule.

Provide test vectors showing:

```text
human-readable object
canonical bytes
SHA-256 digest
P-256 public key
r
s
verification result
```

Swift, Rust, and Solidity-facing fixtures must agree.

## Source tree commitment

Define a deterministic source-tree hash.

At minimum:

1. enumerate included files;
2. normalize relative paths to UTF-8 with `/` separators;
3. exclude build products, signing artifacts, logs, temporary files, `.git`, private prompt material, and user-local secrets;
4. sort paths lexicographically;
5. hash each file’s raw bytes;
6. combine normalized path and content digest through an exact length-prefixed encoding;
7. hash the resulting stream with SHA-256.

Document every exclusion.

Provide fixtures.

The same source tree must always produce the same commitment on different machines.

---

# 10. Pure protocol crate

Add a new Rust crate to the workspace that has no terminal, Studio, RPC, filesystem-global, server, Apple-signing, or harness opinions.

The crate owns only:

* protocol types;
* schema validation;
* canonicalization;
* hashing;
* P-256 signature formats;
* low-`s` normalization and validation;
* Evolution commitments;
* lineage verification;
* continuity envelopes;
* source-tree hashing;
* public action digest construction;
* conformance report types;
* test vectors.

The protocol crate must be usable by another Rust application without importing the TOHSENO engine.

The engine depends on the protocol crate.

The protocol crate must never depend on the engine.

---

# 11. Apple device key bridge

The Rust CLI needs a production Apple-native method for generating and using P-256 keys without exporting private material.

Implement a small Swift executable or an equally narrow Security.framework bridge.

It should expose machine-readable commands equivalent to:

```sh
tohseno-apple-identity create --tag <tag>
tohseno-apple-identity public --tag <tag>
tohseno-apple-identity sign --tag <tag> --digest <32-byte-hex>
tohseno-apple-identity delete --tag <tag>
```

Requirements:

* use Secure Enclave P-256 where available;
* store persistent key references through Keychain;
* return public `x` and `y`;
* convert Apple’s signature format into exact fixed-width `r` and `s`;
* never return the private key;
* never log key material beyond public data;
* provide a software-backed test backend for CI and machines lacking Secure Enclave support;
* clearly identify test keys so they can never be mistaken for production authority;
* package the helper with the release artifact;
* make the Rust engine discover it relative to the installed binary rather than relying on a global PATH accident.

Generated applications should receive equivalent native Swift source for their separate InstallationKeys.

---

# 12. Local BuilderID lifecycle

Add local identity state under a structure equivalent to:

```text
~/.tohseno/
└── identity/
    ├── builder.json
    ├── recovery.json.enc
    ├── devices/
    └── actions/
```

No private key bytes in these files.

`builder.json` may contain:

* chain ID;
* BuilderAccountFactory address;
* predicted BuilderAccount address;
* BuilderID string;
* active local device public key;
* local key tag;
* deployment status;
* recovery public address;
* creation timestamp.

## First launch behavior

Before or during the first `tohseno create`:

1. generate a Mac DeviceKey automatically if none exists;
2. calculate the deterministic BuilderAccount address using the configured factory;
3. create the local BuilderID descriptor;
4. continue app creation without an account screen;
5. sign the local Shot Genesis after the app passes build, install, and conformance;
6. keep everything private until the builder explicitly publishes.

The experience should feel like:

```text
Open TOHSENO.
Take a Shot.
Your identity is already there.
```

## Recovery commands

Implement the equivalent of:

```sh
tohseno identity show
tohseno identity backup
tohseno identity recover
tohseno identity devices
tohseno identity authorize
tohseno identity revoke
```

Do not overload first-run creation with crypto explanations.

`identity backup` may reveal recovery words only after an explicit local confirmation and must tell the user never to paste them into a website or generated app.

## Pairing protocol

Define a signed pairing request and device authorization schema.

An unauthorized new device creates a key and produces a QR payload containing:

* BuilderID;
* proposed device public key;
* requested permissions;
* challenge;
* nonce;
* expiration;
* network coordinates.

An already authorized device scans or imports it.

Approval means the authorized device signs an exact `AUTHORIZE_DEVICE` action.

No secret moves between devices.

Implement transport-neutral data first.

Provide QR encoding in Studio and the TOHSENO companion app.

---

# 13. Engine and ledger integration

Preserve the current append-only filesystem model.

Extend the ledger without mutating finalized historical shot directories.

For new Shots, create before finalization:

```text
shots/0001/
├── prompt.md
├── images/
├── genome/
├── previous-src/
├── src/
├── artifact/
├── build.log
├── harness.log
├── TASK.md
├── TOHSENO/
│   ├── shot.json
│   ├── signature.json
│   ├── fascia.json
│   └── conformance.json
└── .complete
```

The order must become:

```text
reserve Shot
    ↓
record user input
    ↓
compose genome + Fascia
    ↓
generate complete app
    ↓
repair until build passes
    ↓
verify source anatomy
    ↓
build, sign, install, launch
    ↓
run full local conformance
    ↓
calculate commitments
    ↓
sign canonical Shot record
    ↓
write protocol sidecars
    ↓
finalize immutable Shot
```

Never finalize a Shot before its protocol record and conformance receipt exist.

A failed Shot remains incomplete and must never become the app’s recognized head.

## Existing apps

Do not mutate old finalized Shot directories.

Add an explicit migration command equivalent to:

```sh
tohseno adopt <app-name>
```

Adoption must:

* inspect the latest legacy Shot;
* create a new complete integer Shot;
* preserve the prior source as context;
* create a new protocol identity and signed Evolution;
* mark its origin as a legacy adoption;
* never fabricate cryptographic history for old unsigned Shots.

---

# 14. CLI evolution

Preserve existing commands and add coherent protocol commands.

The final CLI should support equivalents of:

```sh
tohseno create <app-name>
tohseno evolve <app-name>
tohseno refresh [<app-name>]
tohseno retire <app-name>
tohseno list
tohseno studio
tohseno doctor

tohseno verify <app-name-or-path>
tohseno inspect <app-name-or-path>
tohseno adopt <app-name>

tohseno identity show
tohseno identity backup
tohseno identity devices
tohseno identity authorize
tohseno identity revoke

tohseno publish <app-name>
tohseno network status
tohseno registry show <app-name>
tohseno handle claim <handle> <app-name>
tohseno appcoin associate <app-name> <chain-id> <token-address>

tohseno protocol info
tohseno protocol vectors
tohseno protocol verify-record <path>
tohseno page build <app-name>
```

Group subcommands naturally using Clap rather than creating an incoherent flat namespace.

## Output discipline

Retain:

* `Status`;
* `Handoff`;
* `Result`;
* `HarnessLine`.

A protocol operation should still tell the human one next thing at a time.

Do not dump cryptographic internals into ordinary output.

Examples:

```text
your TOHSENO identity is ready.
shot 1 is locally verified.
publishing shot 1…
shot 1 is witnessed on Robinhood Chain.
```

Detailed identifiers belong in `--json`, `inspect`, and report output.

## Machine-readable mode

Add `--json` or equivalent structured output for protocol commands so other implementations and automation can use the CLI without parsing prose.

---

# 15. Conformance verifier

`tohseno verify` is a central protocol artifact.

It must be deterministic and usable offline for local verification.

It must never call an LLM.

The verifier should check at least:

## Record integrity

* required schemas exist;
* JSON is valid;
* canonical bytes reproduce the declared digest;
* ShotID is valid;
* BuilderID is valid;
* sequence is valid;
* previous commitment matches;
* source-tree commitment matches;
* Fascia commitment matches;
* local DeviceKey signature verifies;
* signature uses authorized public key according to available identity state;
* `s` is low;
* no required field is ambiguous.

## Fascia integrity

* required files exist;
* required Swift types or interfaces exist;
* required files are part of the Xcode target;
* no undeclared third-party dependencies;
* declared capabilities match project settings, entitlements, and Info.plist where inspectable;
* sensitive capabilities are declared;
* InstallationKey implementation is app-specific;
* no Builder recovery secret is embedded;
* no global TOHSENO user key is embedded;
* continuity schema is present;
* provenance metadata is present;
* storage is local first;
* network use is declared.

## Apple project integrity

* expected project and target exist;
* bundle identifier matches the record;
* `CFBundleVersion` equals Shot sequence;
* deployment target is acceptable;
* source builds;
* signed artifact corresponds to the source Shot where verifiable.

## Public verification

When `--public` is supplied:

* query the configured Robinhood Chain RPC;
* verify registry code and configured deployment identity;
* verify the public controller;
* verify the public head;
* verify sequence;
* verify transaction receipt;
* verify optional handle;
* verify optional appcoin relation.

## Output

Human output:

```text
TOHSENO SHOT

✓ record
✓ signature
✓ lineage
✓ Apple Fascia
✓ source commitment
✓ project
✓ privacy boundary
✓ local-first storage
✓ public witness

CONFORMANT
```

Machine output must include every check, expected value, observed value, and evidence path.

A failure must return a non-zero exit code.

Conformance tests are executable protocol law, not cosmetic test coverage.

---

# 16. Smart contracts

Use Foundry.

Pin all external dependencies to exact commits or versions.

Avoid upgradeable proxies.

Avoid founder admin powers.

Avoid token custody.

Avoid unnecessary standards and abstractions.

## Robinhood Chain candidate configuration

Use:

```text
chain ID: 4663
native gas token: ETH
P256VERIFY: 0x0000000000000000000000000000000000000100
```

Implement and test the supplied EIP-7951 semantics:

```text
input:
messageDigest || r || s || publicKeyX || publicKeyY

five big-endian 32-byte values
total: 160 bytes

success:
32-byte integer 1

failure:
empty bytes

gas:
6,900 for the precompile

application rule:
enforce low-s separately
```

Before deployment, run a real `eth_call` probe with a known vector and store the exact request and response as evidence.

Treat empty output, malformed output, or any value other than exactly 1 as failure.

## P256Verifier

Implement a minimal library that:

* checks component lengths;
* checks low-`s`;
* performs a static call to `0x100`;
* validates exact return length and value;
* never treats non-reverting failure as success.

## BuilderAccount

Implement a minimal, non-upgradeable smart account.

It must:

* maintain authorized P-256 DeviceKeys;
* identify keys by an exact public-key hash;
* expose ERC-1271 `isValidSignature`;
* verify the supplied DeviceKey is authorized;
* verify low-`s`;
* call P256VERIFY;
* maintain an independent nonce for device-management actions;
* add a device through a signed authorization;
* revoke a device through a signed authorization;
* support a documented recovery authority;
* emit complete events;
* have no privileged Anky, Inc. key;
* have no official TOHSENO client key.

Signature encoding must be exact and versioned.

A suggested compact encoding is:

```text
version byte
publicKeyX: 32 bytes
publicKeyY: 32 bytes
r: 32 bytes
s: 32 bytes
```

Document it normatively.

## BuilderAccountFactory

Implement deterministic account creation with CREATE2.

It must expose:

```text
predictAccount(...)
createAccount(...)
```

The CLI must be able to compute the BuilderID before account deployment.

The initial account may be deployed lazily at the first public protocol action.

The factory must not own or control created accounts.

## ShotRegistry

Implement the smallest neutral public witness.

It should store only what independent parties need to establish:

* Shot controller;
* current Evolution head;
* integer sequence;
* public state;
* optional public content commitment.

Do not store source code, prompts, images, private app data, or the entire manifest.

Support actions equivalent to:

```text
CREATE_SHOT
APPEND_EVOLUTION
TRANSFER_SHOT
SET_PUBLIC_STATE
```

Anyone may submit a correctly signed action.

The caller gains no ownership.

The registry must ask:

> Is this transition authorized by the identity controlling this Shot?

It must never ask:

> Did the official TOHSENO server submit it?

For every action:

* construct an exact typed digest;
* include chain ID;
* include verifying contract;
* include action type;
* include ShotID;
* include current or previous head as appropriate;
* include expected sequence;
* include nonce or transition-specific replay protection;
* include deadline where appropriate;
* verify through ERC-1271;
* reject stale heads;
* reject incorrect sequence;
* reject replay;
* emit complete events.

EIP-712 may be used for public action encoding. Provide exact Rust and Solidity matching test vectors.

## ShotRelations

Keep human-readable and economic relationships outside the neutral core.

Implement a separate contract supporting signed relationships equivalent to:

```text
CLAIM_HANDLE
RELEASE_HANDLE
ASSOCIATE_APPCOIN
REMOVE_APPCOIN
ATTEST_APP_STORE
```

A handle is an alias for a ShotID.

The ShotID remains fundamental.

`tohseno.com/anky` is a gateway rendering an alias, not the source of identity.

An appcoin relation stores an explicit chain identifier and token address chosen by the Shot controller.

The existence of a token must never be required to:

* create;
* own;
* verify;
* publish;
* evolve;
* transfer;
* use a Shot.

Do not deploy or modify `$TOHSENO`.

Only associate its exact existing address when supplied through explicit configuration.

Never guess the token address.

## Do not require ERC-4337 for the first proof

Robinhood Chain supports account abstraction, but the essential TOHSENO property can be proven more simply:

```text
builder signs
    ↓
any relayer submits
    ↓
BuilderAccount validates
    ↓
registry transitions
```

Implement ERC-1271-compatible relayed actions first.

Architect BuilderAccount so ERC-4337 support can be added without changing Shot identity.

Only implement ERC-4337 in this mission if it is small, demonstrably correct, and does not delay or destabilize the core lifecycle.

Do not introduce a bundler or paymaster dependency merely to sound modern.

---

# 17. Reference node and relayer

Add an optional reference node only after the pure protocol, local lifecycle, and contracts work.

A node may:

* validate signed public actions;
* relay them;
* pay gas from its own funded operator account;
* index registry events;
* resolve Shot records;
* serve public manifests and static pages;
* expose health and network status.

A node never owns a Shot because it relayed, stored, indexed, or served it.

## Minimal API

Provide versioned endpoints equivalent to:

```text
GET  /v1/health
GET  /v1/network
GET  /v1/shots/:shot_id
GET  /v1/handles/:handle
POST /v1/relay
```

Use an append-only filesystem data directory for the candidate unless a stronger database is demonstrably necessary.

## Experimental anti-spam policy

The candidate relayer must not initially be open to the whole internet.

Support:

* explicit BuilderID allowlist;
* action-size limit;
* signature prevalidation;
* chain simulation before submission;
* per-BuilderID quotas;
* per-action quotas;
* maximum sponsored gas;
* idempotency;
* replay rejection;
* structured audit logs with no private content.

Do not create a shared ETH deposit pool contract.

A node can pay gas from its own relay account.

A future paymaster or sponsorship market is an extension, not part of Shot validity.

---

# 18. Studio evolution

Do not spend this mission redesigning the Studio.

Preserve its current visual system and local-first architecture.

Add functional protocol surfaces:

* BuilderID display;
* current Mac DeviceKey status;
* recovery status;
* device authorization QR;
* local/private Shot state;
* conformance state;
* published state;
* public registry head;
* handle;
* appcoin association;
* exact transaction links or identifiers;
* verify button;
* publish button guarded as experimental;
* protocol and network status.

Use understandable language.

Do not force users to understand:

* elliptic curves;
* calldata;
* ERC-1271;
* CREATE2;
* account abstraction;
* gas sponsorship.

The ordinary interface should say:

```text
Your identity
This Mac
Private
Verified
Published
App Store
```

Expose raw facts in an advanced inspector.

Studio and CLI must call the same engine and protocol crates.

Studio must not secretly become a second source of truth.

---

# 19. Static Shot pages

Implement a deterministic page generator:

```sh
tohseno page build <app-name>
```

It should produce a self-contained static directory equivalent to:

```text
public/<slug>/
├── index.html
├── shot.json
├── fascia.json
├── icon.png
└── assets/
```

The HTML should contain its CSS and minimal JavaScript directly or use adjacent static files with no runtime build requirement.

Show:

* app name;
* one-line intention;
* icon;
* screenshots if public;
* ShotID;
* BuilderID or creator label;
* current Evolution;
* verification state;
* source link if public;
* registry coordinates;
* handle;
* appcoin relation;
* App Store link if attested.

The page must be renderable by any gateway.

Do not encode `tohseno.com` as protocol law.

A gateway maps:

```text
handle → ShotID → public artifact
```

The Shot owns the identity.

The domain renders it.

Create a single protocol entry-point site under:

```text
site/protocol/
```

It must be capable of becoming `tohseno.com/protocol`, but do not deploy or replace the stable public page in this mission unless an explicit release-candidate environment already exists.

---

# 20. TOHSENO as Shot #1

Create:

```text
genesis/SHOT_1_INTENT.md
```

This file defines the first TOHSENO Apple app.

Its initial functional purpose is to be an Apple-native protocol companion and entry point, not the protocol itself.

The functional candidate should:

* show the user’s BuilderID;
* show authorized devices;
* generate or display its local app identity;
* scan or import a pairing request;
* allow an authorized user to approve a new device;
* make approval feel like a deliberate swipe-right action;
* sign the exact authorization payload;
* verify Shot records or QR payloads;
* show local and public Shot provenance;
* contain no username/password system;
* work locally without an Anky, Inc. server;
* use the same Apple Fascia as every other Shot.

Do not hand-code special protocol privileges for this app.

Create it through the same public factory path available to any app:

```sh
tohseno create tohseno --prompt-file genesis/SHOT_1_INTENT.md
```

It must become Shot #1 historically, not constitutionally.

After publication, claim the handle:

```text
tohseno
```

only through the same signed relation action available to any other Shot.

Associate `$TOHSENO` only when the exact token coordinates are explicitly configured.

Record creator metadata equivalent to:

```text
JP / Anky, Inc.
jpfraneto.eth
```

as provenance, not permission.

The deployed contracts must not obey `jpfraneto.eth` after deployment merely because it deployed or authored them.

First in history.

Equal under the protocol.

---

# 21. Whitepaper and normative documentation

Replace the old root `MASTER_PROMPT.md` with the final repository constitution derived from this work, while preserving the previous file under a clear historical path.

Create a concise `WHITEPAPER.md`.

It must explain the system simply.

Its center is:

> A Shot is an Apple app with a permanent identity, a known anatomy, and a history controlled by its builder.

> TOHSENO is the open protocol that lets any compatible machine create, understand, verify, and continue a Shot.

Include:

* problem;
* Shot;
* BuilderID;
* Apple Fascia;
* local-first creation;
* signed Evolutions;
* neutral public witness;
* independent factories;
* nodes;
* continuity and privacy;
* conformance;
* relationship to the first implementation;
* relationship to `$TOHSENO`;
* explicit non-goals.

Avoid inflated claims that TOHSENO invented its underlying cryptography.

Say plainly:

> TOHSENO uses established cryptography to create a portable ownership and continuity layer for personal software.

Create normative documents separately from the whitepaper:

```text
protocol/SPECIFICATION.md
protocol/IMPLEMENTERS.md
protocol/CONFORMANCE.md
```

A stranger should be able to build another implementation without reading engine internals.

---

# 22. Genesis Bundle

Create a deterministic build command:

```sh
scripts/build-genesis-bundle.sh
```

It should produce:

```text
dist/genesis/
├── WHITEPAPER.md
├── SPECIFICATION.md
├── IMPLEMENTERS.md
├── CONFORMANCE.md
├── FASCIA.json
├── schemas/
├── test-vectors/
├── contracts/
├── ABI/
├── DEPLOYMENT.json
├── SOURCE_COMMIT.txt
├── FILES.sha256
└── GENESIS.json
```

`GENESIS.json` must identify:

* protocol name;
* candidate version;
* codename;
* source commit;
* Fascia commitment;
* schema commitments;
* contract source commitments;
* test-vector commitment;
* candidate deployment addresses if deployed;
* candidate status;
* creation time;
* explicit statement that this is not yet the final canonical release.

The bundle must be reproducible from a clean checkout.

Do not upload the final canonical bundle to Arweave in this mission.

Optionally prepare the exact publishing command or an explicitly labeled candidate publication, but never call it final.

---

# 23. Installer and release-candidate channel

Preserve the current stable installer behavior.

Add an explicit release-candidate channel without silently replacing stable.

Support an invocation equivalent to:

```sh
curl -fsSL https://tohseno.com/oneshot.sh | TOHSENO_CHANNEL=genesis bash
```

or create an equivalent unambiguous candidate installer path.

The candidate release must package:

* `tohseno`;
* the Apple identity helper;
* protocol schemas;
* Fascia material;
* Studio assets;
* required metadata;
* checksums.

The installer must:

* detect macOS and architecture;
* install under `~/.tohseno/bin`;
* verify checksums;
* install the helper;
* verify both binaries;
* avoid telemetry;
* avoid accounts;
* avoid secrets;
* leave the stable channel untouched.

Update release automation to support a GitHub prerelease:

```text
v0.7.0
```

Do not create that tag until all automated checks pass.

Do not create `v1.0.0`.

---

# 24. Contract deployment procedure

Implement a safe deployment script for Robinhood Chain mainnet.

Configuration should come from explicit environment variables or secure Foundry account configuration, not hardcoded secrets.

Use equivalents of:

```text
ROBINHOOD_RPC_URL
ETHEREUM_MAINNET_RPC_URL
TOHSENO_DEPLOYER_ACCOUNT
TOHSENO_RELAYER_ACCOUNT
TOHSENO_TOKEN_ADDRESS
TOHSENO_ALLOW_EXPERIMENTAL_MAINNET
TOHSENO_ALLOW_APPCOIN_ASSOCIATION
```

Prefer encrypted local keystores or hardware-backed signers.

Never require a raw private key in shell history.

Before deployment:

1. verify chain ID is exactly `4663`;
2. probe P256VERIFY at `0x100`;
3. run all Foundry tests;
4. run cross-language digest vectors;
5. simulate deployment;
6. display expected deployer;
7. resolve `jpfraneto.eth` through Ethereum mainnet when possible;
8. require the actual deployment signer to match the intended JP address;
9. estimate gas;
10. require the explicit experimental-mainnet guard.

Deploy with an unmistakable codename such as:

```text
TOHSENO GENESIS CANDIDATE
```

The candidate contracts must store or emit:

* codename;
* Genesis candidate bundle hash;
* source commit;
* deployment chain;
* deployment time.

After deployment:

* verify bytecode;
* write `contracts/deployments/robinhood-mainnet-genesis.json`;
* write ABI files;
* record transaction hashes;
* confirm every address through RPC;
* rerun the P-256 vector through the deployed path;
* create a BuilderAccount;
* register a temporary protocol smoke Shot;
* append an Evolution;
* verify controller and head;
* record all evidence.

Do not declare these candidate addresses eternal.

---

# 25. End-to-end lifecycle command

Add a command or script equivalent to:

```sh
tohseno lifecycle genesis
```

or:

```sh
scripts/lifecycle-mainnet.sh
```

It must execute the system as one observable chain.

Use a clean candidate data root so the test does not corrupt JP’s normal local ledger:

```text
~/.tohseno-genesis/
```

The lifecycle is:

## Phase A — installation

* build or obtain the release-candidate artifacts;
* install through the candidate one-liner rather than running only from `cargo run`;
* verify binary and helper versions;
* run `tohseno doctor`.

## Phase B — identity

* create a local Apple DeviceKey;
* derive or predict BuilderID;
* verify no private key was exported;
* deploy the BuilderAccount lazily or explicitly;
* verify its code and initial device;
* create or verify recovery configuration;
* record public identifiers.

## Phase C — Shot #1

Run:

```sh
tohseno create tohseno --prompt-file genesis/SHOT_1_INTENT.md
```

Use available brand reference images, capped at the existing eight-image boundary.

The harness must produce a complete app.

The engine must repair it until it builds.

The app must be signed, installed, and launched on the connected iPhone.

The generated app must contain the Apple Fascia.

The engine must calculate commitments, sign the Shot record, run conformance, and finalize Evolution 1.

## Phase D — local verification

Run strict verification.

Save JSON evidence.

Confirm:

* ShotID;
* BuilderID;
* source commitment;
* Fascia commitment;
* P-256 signature;
* sequence 1;
* no previous head;
* bundle version 1;
* installation success;
* privacy boundaries.

## Phase E — publication

Create the public artifact bundle.

Submit the signed public action through:

* a configured candidate relayer; or
* direct submission by a funded operator account.

The messenger must not become owner.

Verify through independent RPC reads:

* Shot exists;
* controller is BuilderID;
* sequence is 1;
* head equals local commitment;
* public state is correct.

## Phase F — handle and appcoin

Claim `tohseno` through the relation contract.

Only associate `$TOHSENO` when:

* `TOHSENO_TOKEN_ADDRESS` is explicitly present;
* chain ID is explicit;
* token code exists;
* `TOHSENO_ALLOW_APPCOIN_ASSOCIATION=1`.

Never infer or guess.

## Phase G — Evolution 2

Run a real evolution prompt adding one small useful feature to the TOHSENO app, such as an advanced record-verification inspector.

The engine must create a full second Shot, not a diff.

Confirm:

* sequence 2;
* `CFBundleVersion = 2`;
* previous equals Evolution 1 commitment;
* new source commitment;
* valid DeviceKey signature;
* conformance;
* installation;
* registry append;
* registry head updated to Evolution 2;
* Evolution 1 remains immutable.

## Phase H — independent verification

Provide at least one independently implemented verification path.

Acceptable examples:

* Solidity tests recomputing a digest created by Rust;
* a tiny standalone Swift verifier reading a protocol fixture;
* a separate minimal executable depending only on the protocol schemas, not the engine.

It must verify both Evolutions without consulting the TOHSENO Studio or proprietary server.

## Phase I — restart and recovery behavior

* restart Studio;
* restart CLI;
* reload local ledger;
* confirm BuilderID stability;
* confirm device key stability;
* confirm Shot lineage;
* run `refresh`;
* verify the installed app remains the same Shot;
* ensure no state depends on a process that was previously running.

---

# 26. Testing strategy

JP prefers reality over prolonged staging.

Honor that without confusing production with guesswork.

Tests here are executable laws and evidence.

Run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Run Swift tests and builds.

Run Foundry formatting, compilation, unit tests, fuzz tests, and invariant tests.

At minimum include:

* canonicalization fixtures;
* malformed schema cases;
* source-tree hash fixtures;
* P-256 valid signature;
* P-256 invalid signature;
* malformed precompile return;
* high-`s` rejection;
* unauthorized device rejection;
* revoked device rejection;
* recovery action;
* wrong chain rejection;
* wrong registry rejection;
* expired action rejection;
* replay rejection;
* stale previous-head rejection;
* skipped sequence rejection;
* caller not becoming owner;
* relayer substitution success;
* handle collision;
* unauthorized appcoin association;
* Shot transfer;
* local record verification;
* public record verification;
* continuity proof scope;
* continuity expiry;
* pairwise unlinkability;
* generated app Fascia conformance;
* generated app undeclared capability failure;
* immutable finalized Shot behavior;
* legacy adoption behavior.

Use fuzz and invariant tests around contract state transitions.

Do not claim smart-contract safety merely because unit tests pass.

Mark unaudited status honestly.

---

# 27. CI

Update GitHub Actions to exercise everything possible without a physical iPhone:

* Rust formatting;
* Clippy;
* Rust tests;
* schema generation and clean-diff check;
* protocol test vectors;
* Swift helper build and software-backend tests;
* Foundry build and tests;
* static Studio checks;
* installer shell checks;
* release package assembly;
* Genesis Bundle reproducibility;
* candidate binary smoke tests;
* generated fixture app simulator build where possible.

Keep the real-device lifecycle as an explicit local production gate.

CI must fail if generated schemas, test vectors, ABIs, or bundle checksums are stale.

---

# 28. Security boundaries

Never compromise these for convenience.

## No secret reuse across generated apps

Do not derive every InstallationKey from the Builder recovery mnemonic.

Do not inject the Builder DeviceKey into generated apps.

Do not share one universal app identity.

Each installation creates its own key.

## No official-server dependency

Local creation, signing, verification, evolution, installation, and use must work without an Anky, Inc. server.

Publication may use a replaceable relayer.

## No Apple ID protocol dependency

Apple ID may be necessary for Apple signing.

It is never required for TOHSENO identity.

## No invisible publication

A local Shot is private.

Publishing is an explicit signed transition.

## No privileged registry caller

Any address may relay a valid action.

## No hidden mutability

Candidate contracts are non-upgradeable.

A corrected protocol candidate requires a new deployment.

## No token dependency

The protocol remains valid without `$TOHSENO`.

## No invented success

If the real iPhone is absent, say the device gate was not completed.

If deployment credentials are absent, say contracts were not deployed.

If the token address is absent, leave the appcoin relation pending.

If GitHub authentication is absent, leave the release command prepared.

Never synthesize transaction hashes, addresses, signatures, releases, screenshots, or lifecycle evidence.

---

# 29. Public and private states

Represent state explicitly.

## Private local Shot

* exists in the filesystem;
* has BuilderID;
* has signed lineage;
* has Fascia;
* can be verified;
* has no necessary public record.

## Published Shot

* builder signed an explicit publication;
* registry records the public head;
* nodes may index or mirror declared public artifacts;
* private app data remains private.

## App Store Shot

* builder signed an App Store attestation;
* record connects ShotID, bundle identifier, store identifier, and Evolution;
* App Store distribution does not replace Shot identity.

Do not make the registry the creator of a Shot.

The registry witnesses a Shot that already exists.

---

# 30. Update the core product experience

After this evolution, the ordinary loop remains extremely small:

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
tohseno create my-app
```

The user describes an app.

TOHSENO creates it.

The identity layer should appear as infrastructure, not homework.

The full conceptual lifecycle is present even when the ordinary user only sees:

```text
taking shot 1…
building…
installing…
shot 1 is on your phone.
```

Protocol complexity belongs beneath the surface.

Advanced users and implementers can inspect every byte.

---

# 31. Production-candidate release procedure

After all automated checks and the local lifecycle pass:

1. update the workspace version to `0.7.0`;
2. update README and candidate documentation;
3. generate schemas, vectors, ABIs, deployment files, and Genesis Bundle;
4. ensure `git diff` contains no generated drift or secrets;
5. commit coherent work with clear messages;
6. push the current branch to `origin`;
7. create `v0.7.0` only if the complete candidate checks pass;
8. mark the GitHub release as a prerelease;
9. test the candidate installer from the released artifact;
10. repeat the complete lifecycle using that artifact rather than the local build;
11. record evidence;
12. leave the stable channel and `v1.0.0` untouched.

Do not announce success merely because the code reached `main`.

The acceptance gate is the released candidate performing the lifecycle.

---

# 32. Required evidence report

Create:

```text
genesis/lifecycle/GENESIS_CANDIDATE_REPORT.md
```

Also create a machine-readable JSON report.

Include:

* start and end timestamps;
* operating system;
* architecture;
* Xcode version;
* Rust version;
* Swift version;
* Foundry version;
* Git source commit;
* release-candidate version;
* installed artifact checksums;
* BuilderID;
* public device-key fingerprint;
* recovery status without secrets;
* contract addresses;
* deployment transaction hashes;
* contract bytecode hashes;
* P256VERIFY probe input and output;
* ShotID;
* Evolution 1 commitment;
* Evolution 2 commitment;
* source-tree commitments;
* Fascia commitment;
* conformance outputs;
* iPhone device model without unnecessary personal identifiers;
* installation and launch result;
* registry reads;
* handle claim;
* appcoin relation or exact reason pending;
* independent-verifier output;
* release URL or exact reason not created;
* every command executed;
* every remaining blocker;
* explicit statement of whether the candidate is ready for cleanup;
* explicit statement that it is not yet the canonical shipped protocol.

The report must distinguish:

```text
implemented
automatically verified
manually observed
deployed
not completed
```

---

# 33. Definition of done

This mission is complete only when the repository can demonstrate all of the following:

## Factory

* the one-line installer works through the candidate channel;
* `tohseno create` accepts one intention;
* the harness generates a complete Apple app;
* build repair works;
* the app installs and launches on a connected iPhone;
* `tohseno evolve` creates a second complete world.

## Identity

* a Mac receives a hardware-backed P-256 DeviceKey;
* a deterministic BuilderID exists;
* the private device key never leaves Apple key storage;
* device authorization and revocation schemas exist;
* recovery exists or is honestly marked pending;
* Apple ID is not the protocol identity.

## Fascia

* every new app contains the same finite connective structure;
* InstallationKey exists automatically;
* storage is local first;
* continuity is explicit;
* privacy separation is enforced;
* provenance is embedded;
* conformance is deterministic.

## Shot

* ShotID remains stable across Evolutions;
* sequence equals `CFBundleVersion`;
* each Evolution is a complete source world;
* previous commitments form valid lineage;
* records are signed;
* finalized Shots remain immutable.

## Public protocol

* experimental BuilderAccountFactory is deployed;
* experimental BuilderAccount is created;
* P-256 validation works through the chain precompile;
* experimental ShotRegistry is deployed;
* anyone can relay valid signed actions;
* caller does not gain ownership;
* Evolution 1 is registered;
* Evolution 2 updates the head;
* independent RPC reads confirm public state.

## Replaceability

* another implementation can parse the schemas;
* test vectors exist;
* a second verifier validates the records;
* no official TOHSENO server or app is required;
* contracts contain no privileged official-client path.

## Genesis

* the TOHSENO app is created through the same factory as every other app;
* it is Shot #1 historically;
* it receives no protocol privilege;
* `tohseno` handle is claimed through the normal relation;
* `$TOHSENO` is associated only through an explicit controller action and exact configured token address.

## Release discipline

* code is on the intended Git branch;
* candidate release can be installed;
* production-candidate lifecycle was exercised;
* stable release remains untouched;
* final Arweave publication remains pending;
* the evidence report is complete.

---

# 34. Final execution instruction

Begin by inspecting the repository and preserving working behavior.

Then implement the system end to end.

Do not stop after creating types, schemas, contracts, or documentation.

Do not stop after unit tests.

Do not stop after deploying contracts.

Do not stop after generating an app.

The mission is the closed loop:

```text
intention
→ app
→ Fascia
→ signed Shot
→ deterministic verification
→ public witness
→ Evolution
→ independent verification
```

Fix every failure you can fix autonomously.

When a human action is unavoidable, emit one exact handoff and continue all independent work.

At the end, return only:

1. what changed;
2. what was actually executed;
3. what passed;
4. what failed;
5. commit and release identifiers;
6. deployed addresses and transaction hashes;
7. ShotID and Evolution commitments;
8. the path to `GENESIS_CANDIDATE_REPORT.md`;
9. the single next action required before canonical shipment.

The result should make this statement mechanically true:

> A person can install TOHSENO, take a Shot, own its identity, verify its anatomy, publish its lineage, continue it through any compatible factory, and survive the disappearance of the first doorway.
