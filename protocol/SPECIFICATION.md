# TOHSENO protocol candidate specification

Status: `1.0.0-rc.1`, codename `GENESIS`,
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
init_code = C || abi_word(x) || abi_word(y)
address = low20(Keccak-256(0xff || F20 || S32 || Keccak-256(init_code)))
BuilderID = "eip155:4663:" || lowercase_hex(address)
```

P-256 coordinates already occupy one 32-byte ABI word each. Recovery authority
MUST NOT affect the salt, init code, or predicted address.

## EIP-712 public and device actions

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
