# TOHSENO protocol candidate specification

Status: `0.7.0`, codename `GENESIS`,
`protocol_candidate_not_canonical`.

The key words MUST, MUST NOT, REQUIRED, SHOULD, and MAY are normative.

## Scope

This specification defines a Shot, its immutable Evolutions, the Builder
identity that controls it, the replaceable devices authorized by that Builder,
app-local Installation identities, commitments, signatures, public actions,
and continuity proofs. It does not define a terminal UI, relayer, RPC client,
server account, Apple code-signing policy, coding harness, or global
filesystem layout.

The wire law below defines future DeviceKey and recovery actions, but the
bundled GENESIS implementation does not claim those state transitions are
complete. Its local signer and offline verifier accept only the initial
DeviceKey committed through CREATE2 BuilderID prediction. It exposes no
authorize, revoke, rotate, or recover command; encrypted recovery material is
a local backup only. A replacement key requires a canonical authorization
proof chain and evidence-backed nonce source before it can sign a conforming
Evolution.

Every wire object is UTF-8 JSON. Schemas are closed: unknown members, duplicate
members, trailing JSON values, wrong-width hexadecimal values, and uppercase
hexadecimal are invalid. Byte strings are lowercase `0x`-prefixed hexadecimal.
All cross-language `u64` JSON values in this candidate are restricted to the
JavaScript-safe range `0..=9007199254740991`.

## Four distinct identities

The protocol keeps these authorities separate:

1. `BuilderID` is `eip155:4663:0x` plus the 20-byte BuilderAccount address. It
   is stable while physical devices rotate.
2. A Builder `DeviceKey` is a P-256 key authorized by BuilderAccount state.
   Its key ID is exactly `Keccak-256(x32 || y32)`, with fixed-width big-endian
   coordinates and no prefix or domain string.
3. Builder recovery is a BIP-39/BIP-44 secp256k1 authority at
   `m/44'/60'/0'/0/0`. It is not an ordinary DeviceKey.
4. An `InstallationKey` is a P-256 key scoped to one installation of one
   generated app. Its identifier is
   `SHA-256("TOHSENO-INSTALLATION-ID-V1\0" || x32 || y32)`.

Apple development/distribution certificates are a fifth, external operational
concern and MUST NOT be treated as any protocol identity. Implementations MUST
NOT export private device or installation key material into records, logs,
bundles, reports, or pairing requests.

## Shot and Evolution records

A ShotID is 32 cryptographically random bytes created once and never derived
from a slug, bundle ID, domain, appcoin, handle, server, or BuilderID.

The record schema is `tohseno.shot/1`. A normal protocol lineage starts at
sequence 1 with `previous` null. Every non-root protocol record references the
commitment of its immediately preceding protocol Evolution.
`bundle_version` MUST equal `sequence`. ShotID, BuilderID, bundle ID, and
Fascia identifier remain stable through the local lineage. Timestamps use
exactly `YYYY-MM-DDTHH:MM:SSZ` in UTC.

Legacy adoption is the single exception to a protocol lineage beginning at
sequence 1. If an existing legacy app's latest filesystem Shot is N, its first
protocol record MUST:

```text
origin.kind = legacy_adoption
origin.legacy_latest_shot = N
sequence = bundle_version = N + 1
previous = null
```

`origin.legacy_source_sha256` MUST be nonzero and commit the adopted legacy
source. No protocol commitment existed for legacy Shot N, so an implementation
MUST NOT fabricate `previous`. The next protocol Evolution is N+2, omits
`origin`, and references the adopted root commitment. Every subsequent record
also omits `origin`. N must be in `1..=4294967294`.

The Evolution commitment is:

```text
SHA-256(RFC8785(record))
```

The signature sidecar is a separate object and is never part of this
commitment. JSON duplicate members MUST be rejected before canonicalization.

## Genesis input commitment

Prompt bytes are committed exactly as supplied; no Unicode, newline, or
whitespace normalization occurs. Every input image is named by a nonempty,
simple NFC filename and committed as SHA-256 of its exact raw bytes. Names
containing a path separator, control character, `.` or `..` are invalid.
Duplicate normalized names are invalid.

Sort image entries by unsigned UTF-8 filename bytes, then compute:

```text
SHA-256(
  "TOHSENO-GENESIS-INPUT-V1\0" ||
  u64be(prompt_byte_length) ||
  prompt_bytes ||
  u64be(image_count) ||
  Σ(
    u32be(filename_byte_length) ||
    filename_utf8 ||
    SHA-256(raw_image_bytes)
  )
)
```

## Source-tree commitment

The caller supplies one explicit source root. The hasher MUST NOT consult Git,
ignore files, the current working directory, environment variables, or global
state. It recursively includes regular files as raw bytes, rejects every
symbolic link and non-regular entry, normalizes relative path components to
NFC with `/` separators, rejects normalized or Apple-case collisions, and
sorts by unsigned UTF-8 path bytes.

For sorted entries `(path, SHA-256(raw_file_bytes))`, compute:

```text
SHA-256(
  "TOHSENO-SOURCE-TREE-V1\0" ||
  u64be(file_count) ||
  Σ(
    u32be(path_byte_length) ||
    path_utf8 ||
    SHA-256(raw_file_bytes)
  )
)
```

The exact self-reference exclusions are:

```text
TOHSENO/shot.json
TOHSENO/signature.json
TOHSENO/conformance.json
TOHSENO/embedded-provenance.json
```

`TOHSENO/fascia.json` and actual Fascia source are included. VCS, build,
user-local, log, signing-secret, and environment paths are forbidden inside
the source root rather than silently excluded. The exact forbidden path law is
frozen in `tree_hash.rs` and described in `IMPLEMENTERS.md`. An excluded
embedded-provenance file MUST still be independently compared to the signed
record during conformance.

The embedded file uses the closed `tohseno.app-metadata/1` object defined by
`schemas/app-metadata.schema.json` and `app_metadata.rs`. Its record,
distribution, capability, network, origin, and registry fields are the same
object decoded by the normative Apple `TohsenoMetadata` implementation.
Transport decoders MUST reject duplicate object members before typed decoding.

## Reusable Fascia-tree commitment

`fascia_sha256` commits the pinned reusable Fascia reference tree, independently
of the generated app source tree. Paths are root-relative UTF-8 NFC with `/`,
sorted by unsigned UTF-8 bytes. For each included regular file, use its exact
raw bytes:

```text
SHA-256(
  Σ(
    u64be(path_byte_length) ||
    path_utf8 ||
    u64be(content_byte_length) ||
    raw_content
  )
)
```

There is deliberately no domain prefix and no file count; an empty included
tree is SHA-256 of the empty byte string. Root-relative exclusions are
`.build`, `.swiftpm`, and `Package.resolved`. Exclusion `E` matches only
`P == E` or a path beginning `E + "/"`. Encountered symlinks and special files
are failures and are never followed.

## P-256 signatures

Public coordinates and signature scalars are fixed-width 32-byte big-endian
values. A verifier MUST validate curve membership and nonzero scalars. It MUST
reject `s > floor(P-256-order / 2)`.

For an Evolution or continuity statement:

1. compute the declared SHA-256 digest of the RFC 8785 object;
2. pass that exact 32-byte value to P-256 prehash sign/verify once;
3. do not hash it again inside the signature operation.

The compact on-chain P-256 signature encoding is exactly 129 bytes:

```text
0x01 || x32 || y32 || r32 || s32
```

## BuilderAccount prediction

BuilderAccount uses CREATE2. Given factory address `F`, salt `S`, exact
BuilderAccount creation bytecode `C`, and the initial P-256 public key:

```text
device_key_id = Keccak-256(x32 || y32)
S = SHA-256("TOHSENO-BUILDER-SALT-V1\0" || device_key_id)
init_code = C || abi_word(x) || abi_word(y)
address = low20(Keccak-256(0xff || F20 || S32 || Keccak-256(init_code)))
BuilderID = "eip155:4663:" || lowercase_hex(address)
```

P-256 coordinates already occupy one 32-byte ABI word each. Recovery authority
MUST NOT affect the salt, init code, or predicted address. Implementations use
`identity::initial_builder_account_salt` after validating the initial key.

## Frozen v0.7 EIP-712 public and device actions

Every domain uses:

```text
EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)
```

Version is `1`; chain ID is `4663`. Domain names are exactly:

```text
TOHSENO BuilderAccount
TOHSENO ShotRegistry
TOHSENO ShotRelations
```

The action type strings are:

```text
CreateShot(bytes32 shotId,address controller,bytes32 head,uint64 sequence,uint8 publicState,bytes32 contentCommitment,uint64 nonce,uint64 deadline)
AppendEvolution(bytes32 shotId,bytes32 previousHead,bytes32 newHead,uint64 sequence,bytes32 contentCommitment,uint64 nonce,uint64 deadline)
TransferShot(bytes32 shotId,address currentController,address newController,bytes32 currentHead,uint64 sequence,uint64 nonce,uint64 deadline)
SetPublicState(bytes32 shotId,bytes32 currentHead,uint64 sequence,uint8 publicState,bytes32 contentCommitment,uint64 nonce,uint64 deadline)
AuthorizeDevice(address account,bytes32 keyId,uint256 x,uint256 y,uint32 permissions,uint64 nonce,uint64 deadline)
RevokeDevice(address account,bytes32 keyId,uint64 nonce,uint64 deadline)
SetRecovery(address account,address recovery,uint64 nonce,uint64 deadline)
RecoverAccount(address account,address currentRecovery,address newRecovery,bytes32 newKeyId,uint256 newX,uint256 newY,uint64 nonce,uint64 deadline)
ClaimHandle(bytes32 shotId,bytes32 handleHash,uint64 nonce,uint64 deadline)
ReleaseHandle(bytes32 shotId,bytes32 handleHash,uint64 nonce,uint64 deadline)
AssociateAppcoin(bytes32 shotId,uint256 chainId,address token,uint64 nonce,uint64 deadline)
RemoveAppcoin(bytes32 shotId,uint256 chainId,address token,uint64 nonce,uint64 deadline)
AttestAppStore(bytes32 shotId,bytes32 bundleIdHash,uint64 storeId,bytes32 evolutionHead,uint64 nonce,uint64 deadline)
```

ABI values are encoded as 32-byte words and the final digest is:

```text
Keccak-256(0x1901 || domain_separator || struct_hash)
```

Device permission bits are `PROTOCOL=1` and `DEVICE_ADMIN=2`; the initial and
post-recovery replacement key receives 3. `CREATE_SHOT` requires a nonzero
starting sequence and public state `PUBLISHED` (numeric state 1). A native
root starts at 1; a signed legacy-adoption root starts at its declared legacy
N+1. `APP_STORE` is numeric state 2 and public state is monotonic.
`RecoverAccount.newRecovery` MUST be nonzero. A recovery action is signed by
the distinct secp256k1 recovery authority, not by the P-256
device-authorization envelope.

All type hashes, domain separators, struct hashes, and representative digests
are frozen in `test-vectors/protocol-v1.json`.

### BuilderAccount contract generation 0.8

The v0.7 `RecoverAccount` payload remains frozen for verification but is not
accepted by the successor BuilderAccount. Contract generation 0.8 replaces
immediate recovery with these exact actions:

```text
ChangeRecovery(address account,address currentRecovery,address newRecovery,uint64 nonce,uint64 deadline)
InitiateRecovery(address account,address currentRecovery,address newRecovery,bytes32 newKeyId,uint256 newX,uint256 newY,uint64 nonce,uint64 deadline)
CancelRecovery(address account,bytes32 recoveryId,uint64 nonce,uint64 deadline)
```

`ChangeRecovery` and `CancelRecovery` are authorized by a current P-256
`DEVICE_ADMIN` key and consume `deviceNonce`. `InitiateRecovery` is authorized
by the current recovery authority, which may be an EOA or an ERC-1271
contract, and consumes `recoveryNonce`. The recovery ID is the complete
EIP-712 `InitiateRecovery` digest.

A successful initiation starts a three-day delay. Any active device admin may
cancel before finalization, including after the delay has elapsed if no
finalizer has yet landed. Finalization itself is unsigned and permissionless;
it succeeds only for the exact pending digest after the delay, replaces the
device epoch with one all-permissions key, and rotates recovery. Changing the
recovery authority clears any pending attempt and advances the recovery nonce.

The frozen v0.7 model remains `DeviceAction`; the successor model is
`BuilderAccountActionV2`. Its schema and deterministic cross-language vectors
are `schemas/builder-account-action-v2.schema.json` and
`test-vectors/builder-account-v2.json`.

## ShotRegistry contract generation 0.8

The v0.7 registry and relations actions above remain frozen decoding inputs
and will never be deployed by the TOHSENO project. The successor public
witness has EIP-712 domain:

```text
name = "TOHSENO ShotRegistry"
version = "2"
```

Its opaque commitment and signed action encodings are exactly:

```text
ShotRegistrationCommitment(address controller,bytes32 shotId,bytes32 salt,address registry,uint256 chainId,uint64 deadline)
RegisterShot(bytes32 shotId,address controller,bytes32 head,bytes32 salt,uint64 nonce,uint64 deadline)
AppendCheckpoint(bytes32 shotId,bytes32 previousHead,bytes32 newHead,uint64 checkpointSequence,uint64 nonce,uint64 deadline)
TransferShot(bytes32 shotId,address currentController,address newController,bytes32 currentHead,uint64 checkpointSequence,uint64 nonce,uint64 deadline)
```

`commitShot` is permissionless and unsigned. It MUST make no controller call.
A duplicate live commitment MUST preserve its first timestamp. Reveal is valid
only in the inclusive interval:

```text
[committedAt + 60 seconds, min(committedAt + 24 hours, deadline)]
```

The commitment preimage binds its type hash, controller, independent random
ShotID, salt, registry address, current chain ID, and deadline. The signed
reveal additionally binds the initial public lineage head and exact controller
registration nonce. Successful reveal deletes the commitment and creates
public checkpoint 1. Every append requires the exact previous head and
increments `checkpointSequence` by one. Transfer preserves head and checkpoint
count and consumes the shared Shot nonce.

The registry accepts a deployed exact-magic ERC-1271 controller without
pinning one BuilderAccount code hash. Candidate policy recognizes a BuilderID
only against an approved contract generation. An exact 23-byte EIP-7702
delegation designator beginning `0xef0100` is never an eligible controller.

`checkpointSequence` is witness-local. It MUST NOT be derived from or compared
to ontology Version ordinal, `ShotRecord.sequence`, `CFBundleVersion`, or App
Store build history. A Shot first published at local version N still registers
as checkpoint 1.

The successor has no `publicState`, generic `contentCommitment`, handle, App
Store attestation, or Appcoin contract state. A registry head may identify only
an intentionally public canonical Shot lineage action. It MUST NOT be derived
from app-runtime continuity records, installation or end-user data, private
feedback or references, raw private intentions, or hashes of those values.

`ShotRegistrationCommitmentV2` and `RegistryActionV2` are the exact Rust
generation types. The frozen v0.7 `PublicAction` remains a separate decoding
type; implementations MUST dispatch by explicit schema/generation and MUST NOT
reinterpret it by changing only the EIP-712 domain version. The closed schemas
and deterministic interoperability fixture are
`schemas/shot-registration-commitment-v2.schema.json`,
`schemas/registry-action-v2.schema.json`, and
`test-vectors/registry-v2.json`.

The contract accepts any `bytes32` salt. A TOHSENO publication client MUST use
a fresh CSPRNG salt, persist it privately before commitment submission, and
never regenerate or infer it from Shot metadata. `SignedRegistryActionV2`
proves detached P-256 evidence only; current device authority remains the live
ERC-1271 decision. Neutral non-Builder controllers may use controller-defined
signature bytes outside that client envelope.

The current coherent-intention lineage is one causal chain and may include
private ancestors. Implementations MUST NOT use a
`SignedLineageAction` commitment as a registry head merely because the selected
action is public: its `previous` field may still commit to private material.

The first privacy-safe projection is `tohseno.public-checkpoint/1`. It contains
only the fixed protocol/schema/scope, generation/chain/registry witness
coordinates, random ShotID, witness-local checkpoint sequence, prior public
checkpoint, and a newly declared canonical publication time. Its commitment
is:

```text
SHA-256(RFC8785(public_checkpoint))
```

That commitment alone becomes `ShotRegistry.head`. Checkpoint 1 has a null
predecessor. Every continuation names the exact prior public checkpoint,
increments by one, keeps the same witness and ShotID, and never moves time
backward. Its witness coordinates MUST equal the paired `RegistryActionV2`
domain. Registration binds checkpoint 1 as `head`; append binds the prior
checkpoint as `previousHead`, this digest as `newHead`, and the same sequence.
Transfer creates no checkpoint and preserves the head.

The projection MUST NOT contain a local lineage head/sequence, payload digest,
intention, genome, source/build/artifact digest, ExpressionID, VersionID,
feedback, token relation, availability claim, omission count, controller,
content, or free text. Authorization comes only from the paired registry
action, live ERC-1271 decision, and receipt evidence; the checkpoint does not
invent another signature or ownership system. A private local mapping from a
checkpoint to the state that motivated it MAY exist, but MUST NOT be exported
or replicated.

An unanchored public-checkpoint segment can prove its own canonical bytes and
internal adjacency, not controller authority or on-chain acceptance. The
schema and frozen bytes are `schemas/public-checkpoint.schema.json` and
`test-vectors/public-checkpoint.json`.

An ordinary `SignedLineageAction` MUST NOT enter a current publication outbox,
even when its own availability is `publicly_available`: `previous` can still
commit private ancestry. Existing outbox records MAY be retained as legacy
partial evidence but MUST NOT become registry heads. Token Associations remain
private until a distinct closed, ancestry-free public relation record exists.

## Immutable contract-generation definitions

`tohseno.contract-generation/1` identifies one reproducible build, not one
deployment. It closes over the protocol major, generation and component
versions, target chain and EIP-7951 requirement, exact source inventory,
compiler profile, ABI artifacts, portable BuilderAccount creation bytecode,
creation/runtime code hashes, and conditional CREATE2 coordinates.

The definition MUST NOT contain deployment status, transaction or block
evidence, activation authority, signatures, or a trust root. A predicted
CREATE2 address is arithmetic, not proof that the deployer exists on the target
chain or that code was deployed there. The definition digest is:

```text
SHA-256(RFC8785(contract_generation))
```

The source inventory is strictly ordered by relative path. Its tree preimage
is `TOHSENO-CONTRACT-SOURCE-TREE-V1\0` followed by one UTF-8 line per file:

```text
<0x-sha256> <decimal-byte-length> <relative-path>\n
```

The committed 0.8.0 definition and its versioned ABI/bytecode artifacts live
at `contracts/generations/0.8.0/`. The closed schema and frozen canonical
fixture are `schemas/contract-generation-v1.schema.json` and
`test-vectors/contract-generation-v1.json`.

Activation is a separate future release decision. Until a signed activation
record binds a trusted release policy, this definition's digest, observed
target-chain addresses and runtime code hashes, canonical activation block,
and deploy-gate evidence, generation 0.8.0 is inactive. A build definition
alone MUST NOT authorize identity creation, Shot publication, or RPC trust.

The closed activation payload is `tohseno.contract-activation/1`. It binds an
ordered activation sequence and predecessor, the generation and authority
policy digests, chain, approved BuilderAccount runtime hash, exact observed
factory/registry address and runtime hashes, deployment transactions and
blocks, canonical activation block, fresh target-RPC P256 probe digest, and
issuance time. Its signing digest is:

```text
SHA-256(
  "TOHSENO-CONTRACT-ACTIVATION-V1\0" ||
  RFC8785(contract_activation)
)
```

`tohseno.release-authority-policy/1` contains a threshold and a strictly
ordered set of curve-valid offline P-256 keys. Release key IDs use
`SHA-256("TOHSENO-RELEASE-AUTHORITY-KEY-V1\0" || x || y)`, deliberately not
the Builder DeviceKey law. `tohseno.signed-contract-activation/1` requires
strictly ordered unique low-s approvals from keys in the bound policy and at
least its threshold.

Threshold verification proves approval under the supplied policy; it does not
make that policy trusted. A client MUST separately pin its accepted policy
digest. No activation instance or policy trust root is committed for 0.8.0.
The corresponding closed schemas are
`schemas/contract-activation.schema.json`,
`schemas/release-authority-policy.schema.json`, and
`schemas/signed-contract-activation.schema.json`.

## Neutral coherent-intention lineage v2

The stable ShotID identifies the committed coherent intention, not its Apple
bundle, source tree, repository, controller, token, or current expression.
ExpressionID is a random stable 32-byte identity independent of expression
names and platforms. VersionID is:

```text
SHA-256(
  "TOHSENO-VERSION-ID-V2\0" ||
  ShotID32 || ExpressionID32 || u64be(ordinal) ||
  genome_digest32 || source_digest32
)
```

`tohseno.lineage-action/2` is a closed object containing exact protocol and
schema versions, a positive JSON-safe sequence, previous action commitment,
ShotID, Builder actor, canonical UTC timestamp, publisher handling
declaration, internally tagged closed payload, and payload digest. The payload
digest is `SHA-256(RFC8785(payload))`. The action commitment and signing digest
are `SHA-256(RFC8785(action))`. The unchanged `tohseno.signature/1` P-256
sidecar signs that digest once and still requires a curve-valid key and low-s
signature.

Sequence 1/null must be the commitment. It binds the original intention record
commitment, initial controller, initial P-256 controller key, origin, and time.
The original Intention action may appear later but must match the committed
digest and may appear only once. The reducer rejects gaps, replayed links,
backward timestamps, a changed ShotID, an actor or signer that is not current
authority, and every action-specific invalid transition.

A Genome becomes current only through a proposal followed by an explicit
acceptance referencing the proposal action. Revision 1 has no base. Later
proposals name the exact current revision and digest and include a nonempty
mutation summary. Ordinary Version records bind the current accepted genome;
they cannot mutate it implicitly.

Each Version binds Shot, expression, expression-local ordinal, accepted genome
revision and digest, source digest, materialization provenance, exact
capability-graph digest, successful verification action, known incompleteness,
time, actor, and optional build identity/digest. The referenced
VerificationResult MUST carry the same genome, source, candidate VersionID,
known incompleteness, and capability-graph digest. A failed VerificationResult
remains honest history but cannot produce an accepted Version. Feedback must
reference an existing exact ExpressionID and VersionID, and an optional build
identity must match that Version. Private Feedback must be carried by an
`intentionally_private` action; public Feedback must be carried by a
`publicly_available` action. The two declarations cannot disagree.

An EvolutionaryIntent references selected Feedback by the commitment of the
signed Feedback lineage action, never by the payload-only Feedback digest or a
local filename. Every selected action must exist in the same reducible Shot
lineage and bind the intent's exact `from_version_id` and ExpressionID.

An Evolution that changes the genome must name an acceptance of the exact
proposal selected by its EvolutionaryIntent, and that acceptance revision and
digest must match the target Version. A genome-scoped intent cannot be
finalized as an unchanged-genome Evolution.

Organ declarations are immutable per `(ExpressionID, organ_id)`. Capability
changes use new declaration identities; declarations are not mutable
source-folder labels. The canonical graph is the RFC 8785 encoding of the full
Organ objects sorted by ascending UTF-8 `organ_id`. Every member MUST have the
same ExpressionID, Organ IDs MUST be unique, and every dependency MUST name a
member of that graph:

```text
capability_graph_digest = SHA-256(
  RFC8785(sort_by_organ_id([full Organ declarations]))
)
```

A VerificationResult and the Version it authorizes MUST both name the digest
of the exact current graph. The reducer recomputes the graph at both actions,
so an Organ inserted after verification cannot inherit that result. Every
declared Organ acceptance test MUST have a VerificationGate named:

```text
organ.<organ_id>.acceptance.<one-based ordinal>.<sha256(test UTF-8)>
```

The gate binds the exact declaration and contributes to the VerificationResult
conjunction; it does not by itself prove that an external test runner is
trustworthy. Evidence and verifier policy remain explicit. An Evolution whose
Version graph digest changes MUST carry an `organ`-scoped desired change, and
an `organ`-scoped intent cannot complete with an unchanged graph.

Ownership actions are signed by the current controller and install the next
BuilderID and controller key. Pure reduction trusts the initial declared
BuilderID/key binding. Candidate policy must independently reproduce that
BuilderID from the pinned factory before accepting a new production root.

TokenAssociation is optional and chain-specific. Its address never supplies
Shot, expression, version, or ownership identity. The frozen v0.7 relations
model had one Appcoin slot and remains decodable for private verification, but
that contract will not be deployed. Successor Token Associations are signed
lineage relationships; `8453` is Base mainnet and is valid. An optional anchor
may live on a different witness chain than the token.

Availability states are `absent`, `unknown`, `intentionally_private`,
`locally_available`, `publicly_available`, `replicated`,
`cryptographically_verified`, and `on_chain_anchored`. They are not a ladder:
private is not a weaker public state, and an anchor does not imply byte
availability. Signed actions declare only private or public handling.
Artifact-availability actions record later observations without rewriting the
referenced record.

`adapt_v1_lineage` first runs the frozen `/1` verifier and then derives neutral
ExpressionID and VersionIDs under separate legacy domains. It preserves each
original `ShotRecord` and `SignatureSidecar` verbatim. It does not manufacture
historical genome, feedback, availability, ownership, or signatures.

## Pairing and continuity

A pairing request is public transport data, never a private-key container. It
binds the BuilderID, proposed device public key and derived key ID, requested
permission set, independent nonzero challenge and nonce, expiry, chain 4663,
BuilderAccount, and factory. An authorizer MUST confirm these facts and the
human-visible request out of band.

Continuity is unlinkable by default. A statement names one app Installation
issuer, a Shot audience and explicit nullable recipient InstallationID, the
originating ShotID, and 1 to 16 unique claim tokens strictly sorted in ASCII
lexicographic order. Claim tokens match
`[a-z0-9]+(?:[._-][a-z0-9]+)*`. The statement also carries a nonzero nonce and
a bounded validity interval. Its digest is `SHA-256(RFC8785(statement))`; the
issuer signs that prehash. A verifier checks the issuer-derived InstallationID,
signature, audience, claims, and `issued_at <= now < expires_at`. No central
service is required.

## Fascia schemas

`tohseno.apple-fascia-definition/1` is the rich reusable Apple Fascia
definition in `../fascia/apple/FASCIA.json`. Its exact schema is
`../fascia/apple/FASCIA.schema.json`.

`tohseno.fascia/1` is a concrete per-Shot `TOHSENO/fascia.json` instance. Its
closed schema is `schemas/fascia-manifest.schema.json`. The namespaces MUST NOT
be interchanged.

## Schemas and semantic verification

Committed Draft 2020-12 schemas close every object shape. JSON Schema cannot
prove P-256 curve membership, low-s arithmetic, equality between two object
members, authorization against deployed state, expiration against a supplied
clock, or lineage against prior records. Passing a schema is therefore
necessary but not sufficient; implementations MUST also run the protocol
verifier.
