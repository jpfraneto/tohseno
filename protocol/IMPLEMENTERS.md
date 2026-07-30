# Implementer guide

This guide describes how to integrate the pure protocol candidate without
moving policy or secrets into the protocol layer.

## Pure boundary

Use the crate for parsing closed objects, canonical JSON, commitments, P-256
verification, EIP-712 hashing, CREATE2 prediction, lineage checks, and report
types. Keep terminal output, RPC submission, relayers, Apple signing, keychain
access, project generation, installation, launch, ledger reservations, and
filesystem placement in adapters above it.

The protocol crate accepts an explicit source root when it hashes files. It
does not discover a repository or mutate a Shot.

## Safe Evolution ordering

A creation adapter should use this order:

1. Acquire or create the local Builder DeviceKey without exporting it.
2. Predict or read BuilderAccount and form the BuilderID.
3. Generate a random 32-byte ShotID for a new Shot.
4. Hash exact raw prompt and reference-image inputs with
   `genesis_input_sha256_from_bytes`.
5. Generate the app and concrete `TOHSENO/fascia.json`.
6. Build and repair until the declared target builds.
7. Hash the explicit source root. The four protocol-generated self-referential
   sidecars are excluded; no other private input may be copied into the root.
8. Form the record, set `bundle_version == sequence`, and compute its RFC 8785
   commitment.
9. Sign the commitment through a key adapter using one P-256 prehash
   operation, normalize low-s, and immediately verify the sidecar locally.
10. Write embedded public provenance, rebuild, install, launch, and run
    deterministic conformance. Embedded provenance is excluded from the source
    tree but checked independently.
11. Only after all required checks pass, atomically finalize the immutable
    Evolution directory and advance the ledger head.

A failed build, signature, install, launch, or required conformance check MUST
NOT create a finalized Evolution or consume a public sequence. A temporary
reservation may be abandoned, but the next finalized Evolution remains
contiguous.

For sequence N greater than 1, load and verify the complete prior lineage
before generation and use the verified head as `previous`. Do not trust a
filename or ledger counter without record and signature verification.

For one-time legacy adoption, first determine and hash the immutable legacy
source and read its latest filesystem Shot N. The first protocol Evolution is
N+1 and is a new lineage root: set `origin.kind=legacy_adoption`,
`legacy_latest_shot=N`, the nonzero legacy-source digest, and `previous=null`.
Do not invent a commitment for legacy Shot N. After the adopted root is signed
and finalized, Evolution N+2 omits `origin` and points to the N+1 protocol
commitment. `verify_lineage` accepts this root and remains contiguous from it.

## Parsing and canonicalization

Parse with `canonical::from_slice`, validate the concrete type, then
canonicalize the typed value. Do not canonicalize an unvalidated generic JSON
map: that can hide duplicate fields, unknown fields, or numbers outside the
wire contract.

Use RFC 8785 exactly. Do not pretty-print, reorder using locale rules, normalize
strings, or convert integers through floating point before hashing.

## Key adapters

Hardware-backed keys commonly expose a “sign digest” operation. Confirm that
the API signs the supplied 32 bytes as a prehash. If an API only offers
message-signing that hashes internally, pass canonical object bytes to its
matching SHA-256 algorithm or use a prehash API; never feed an already hashed
digest to a second SHA-256 layer.

Normalize `s` before serialization and verify the just-produced signature with
the public key. A verifier always rejects high-s even if an operating-system
API would accept it.

The BuilderID, DeviceKey, recovery authority, InstallationKey, and Apple
signing identity require separate storage records and labels. Never synchronize
an InstallationKey through a shared keychain access group or use it as a
cross-app profile.

## Source-tree exclusions

Four exact protocol sidecars are excluded to prevent self-reference:

```text
TOHSENO/shot.json
TOHSENO/signature.json
TOHSENO/conformance.json
TOHSENO/embedded-provenance.json
```

No other source path is excluded. Directory components `.git`, `.build`,
`.swiftpm`, `DerivedData`, `build`, and `xcuserdata`; private/user-local names
`.DS_Store`, `.env`, `.env.*`, `build.log`, `harness.log`, `prompt.md`, and
`TASK.md`; and extensions `log`, `mobileprovision`, `p8`, `p12`, `pem`, `pfx`,
and `xcuserstate` are hard failures inside the source root. They are rejected
instead of omitted so an Xcode target cannot consume unsigned bytes and a
commitment cannot accidentally cover private signing material.

These are exact candidate rules, not ignore-file patterns. Any symbolic link is
a hard failure, even when it would point inside the root. Implementations
should snapshot or otherwise prevent concurrent mutation; the Rust
implementation checks file identity before and after reading.

The reusable Fascia tree uses the separate raw-content law implemented by
`hash_fascia_tree`: no prefix or count, then sorted
`u64be(path_len)||path||u64be(content_len)||raw_content` entries. Its only
anchored subtree exclusions are `.build`, `.swiftpm`, and `Package.resolved`.
Do not substitute the generated-app source-tree algorithm.

## Builder and public actions

Before signing:

- validate the domain name, version, chain, and nonzero verifying contract;
- compute a proposed key ID as raw `Keccak-256(x32 || y32)`;
- require a positive `CREATE_SHOT.sequence`, then require either a native
  sequence-1 root or a verified legacy-adoption N+1 root;
- require `CREATE_SHOT.public_state == PUBLISHED`;
- require replacement and recovery addresses to be nonzero;
- bind nonce and deadline from the target contract;
- show the account, action, permissions, chain, contract, and deadline to the
  authorizing human.

Do not expose replacement or recovery commands merely because the action
encoder exists. The implementation must first obtain an evidence-backed nonce,
persist an ordered authorization/revocation proof, and make the offline
Evolution verifier consume that proof. The bundled GENESIS implementation has
none of those three pieces and therefore accepts only its CREATE2 initial
DeviceKey. Its encrypted mnemonic vault is a local backup, not a completed
recovery path.

The compact P-256 signature sent to BuilderAccount is
`0x01||x||y||r||s`. Recovery uses the contract’s separate low-s secp256k1
encoding. A relayer is a messenger and never becomes owner or signer.

For ShotRegistry generation 0.8, use `RegistryActionV2`; never mutate or
reinterpret the frozen `PublicAction` type. Generate the registration salt
from a CSPRNG and durably persist it in private state before submitting the
permissionless commitment. A lost salt means the matured commitment cannot be
revealed; a repeated commitment must not be treated as a new maturity time.
Bind reveal planning to the exact registry, chain, controller, ShotID,
salt, deadline, and observed registration nonce.

The v2 detached envelope verifies a P-256 signature but does not prove that
the key is currently active in BuilderAccount. Resolve authorization from
live ERC-1271 state before submission and again from the receipt block when
claiming acceptance.

Do not map `ShotRecord.sequence`, Version ordinal, or `CFBundleVersion` to
`checkpointSequence`. Do not feed a coherent-intention lineage action digest
directly into `head` unless every byte in the entire committed ancestry is
approved for public disclosure. A selected public action may still link to a
private predecessor. Use a separately defined public-only checkpoint
projection when that ancestry is mixed.

The offline CREATE2 predictor requires the exact creation bytecode shipped for
the deployment. Runtime bytecode or compiler source is not a substitute. The
constructor suffix is only `x` and `y`; recovery configuration occurs later.

## Dependency audit

Runtime dependencies are pinned in `Cargo.toml`:

- `serde` and `serde_json` implement closed typed JSON parsing;
- `serde_json_canonicalizer` implements RFC 8785;
- `sha2` and `sha3` implement SHA-256 and Keccak-256;
- `p256` validates keys and verifies/signs test prehashes;
- `rand_core` obtains ShotID randomness from the operating system;
- `time` parses the single timestamp form;
- `unicode-normalization` enforces NFC paths and filenames;
- `thiserror` supplies typed failures.

`tempfile` is test-only. Production private-key custody is deliberately not a
crate dependency. Review `Cargo.lock`, run `cargo tree -p tohseno-protocol`,
and run the project’s supply-chain audit before promoting a candidate.

## Frozen artifacts

Schemas and vectors are versioned protocol material. A semantic change requires
a new schema/candidate version and regenerated cross-language vectors. The
generator prints vectors to stdout and never overwrites the frozen file.
Review its diff, then run the conformance gates.

## Integrating neutral lineage

Parse `LineageAction` and every payload into the closed Rust type before
canonicalization. Do not accept a lower `protocol_version` or `schema_version`
through optional-field probing. Verify `payload_digest`, the action commitment,
and the unchanged P-256 sidecar before storage.

For a full prefix, call `reduce_lineage`. To ingest a continuation, retain the
trusted `ShotState` and call `apply_lineage_actions`. A node that has only a
middle segment may call `verify_lineage_segment` without an anchor, but must
preserve `authority_context_available = false`; a valid signature alone does
not establish current ownership. Retain competing causally valid heads instead
of selecting whichever arrived last.

Before admitting a new commitment as a production candidate, reproduce its
BuilderID from the configured factory, salt, pinned BuilderAccount creation
bytecode, and declared key. The neutral reducer intentionally cannot infer
that deployment evidence. Derive the salt with
`identity::initial_builder_account_salt`; do not duplicate or reinterpret its
domain-separated law. After an Ownership action, require the new signer for
every subsequent transition; the old signer fails even if its signature is
cryptographically valid.

Persist raw intention bytes separately when they are private. The canonical
Intention record commits their exact digest, length, media type, and honest
availability; inline text is optional and must hash byte-for-byte. Never
publish a private artifact because its descriptor appears in public lineage.

For Apple expressions, use the existing Fascia as the concrete capability
source and project its declarations into Organ records. Do not replace Fascia
or broaden the factory. The initial factory plan is one native iPhone
Expression with the four bounded default Organs for installation identity,
local memory, native navigation, and exact-version feedback. Reject the plan
when any accepted Genome required capability is absent, an Organ does not
support `iphone`, a dependency is not an earlier declared Organ, or a Genome
platform commitment requires a different surface.

Organ IDs are immutable. Canonicalize the full graph by sorting the complete
Organ declarations by `organ_id`, RFC 8785 encoding the array, and hashing it
with SHA-256. Do not hash map iteration order or a list of IDs. Put the exact
digest in both VerificationResult and Version, and recompute it during
reduction. Emit one deterministic VerificationGate for every declared Organ
acceptance test; the gate name includes the full SHA-256 of the test text.
Changing the text therefore requires a new gate result. Record graph
transitions as `organ`-scoped EvolutionaryIntent changes.

New generated apps use `tohseno.app-metadata/2` at the existing excluded
`TOHSENO/embedded-provenance.json` path. Dispatch strict decoders by schema.
Do not add another embedded identity file and do not change the frozen v1
source-tree exclusions.
