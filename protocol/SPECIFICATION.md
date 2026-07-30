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
Shot, expression, version, or ownership identity. The v1 relations contract
has one current Appcoin slot: any fresh authorized association replaces the
current value, including an identical or conflicting one; event history
preserves earlier values; removal must exactly match the current pair; stale
nonces replay-fail. `8453` is Base mainnet and is valid. An optional anchor may
live on a different witness chain than the token.

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
