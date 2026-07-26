# TOHSENO protocol

## One ontology

A **Shot** is one coherent software intention. It has one stable Shot ID and
one independently owned repository. Changing that intention in place creates
an **Evolution** of the same Shot; it does not allocate another Shot, another
repository, or another Shot ID.

A Shot has exactly one current lifecycle state:

1. `EVOLVING` — the Shot exists and can change, but it is not represented as
   publicly downloadable.
2. `PUBLISHED` — the Shot is downloadable through TOHSENO from published
   source.
3. `APP_STORE` — the Shot has shipped through Apple.

The only lifecycle transitions are `EVOLVING → PUBLISHED → APP_STORE`.
Evolutions can be appended in any state without changing the Shot ID or moving
the lifecycle backwards.

The lifecycle is about distribution of the Shot. Creation-job states such as
preparing, building, interrupted, or ready are operational status, not
lifecycle. Generated-app runtime lifecycle is a separate domain.

## Identity roles

Protocol identities are role-qualified:

- **Builder identity** is the declared public authority that signs Shot
  attestations.
- **Runtime identity** is whatever identity or account mechanic an individual
  generated app declares in its own manifest.
- Apple signing, release signing, and external-action authority remain
  separate credentials and roles.

The protocol does not infer Builder identity from Git authorship, an app's
runtime credential, an Apple credential, a wallet, or a TOHSENO account.
The local Ed25519 signer in this repository is an executable test
implementation of the signer interface, not a production key-custody or
recovery decision.

## Append-only public attestations

Protocol version 1 records form a hash-linked, append-only sequence per Shot.
`SHOT_CREATED` is always sequence zero. After that, the other record kinds may
repeat and interleave as their kind-specific invariants allow:

```text
SHOT_CREATED
    ↓
    ├─ EVOLUTION_RECORDED
    ├─ LIFECYCLE_TRANSITIONED
    └─ APPCOIN_LINKED
```

Every record carries the stable Shot ID, monotonically increasing sequence,
previous record hash, canonical timestamp, Builder authority, closed
kind-specific body, and a domain-separated signature. The record hash is
SHA-256 over deterministic canonical JSON. Object keys are sorted and Unicode
input is never normalized.

Verification proves that the declared key signed the canonical record bytes.
It does not independently prove the Builder's real-world identity, ownership
of a Shot ID, accuracy of a summary or evidence URL, or that one valid genesis
history should win over another. Records are therefore portable Builder
attestations, not network truth.

Protocol versions are intrinsic to the thing being interpreted. Record
schemas, canonicalization rules, signature domains, identity methods, and
public projection schemas each identify their own version. A protocol version
is not a lifecycle state, an Evolution number, a CLI version, or a factory
release. Implementations reject unknown versions without inference or
mutation.

Within one accepted history, the initial record fixes the Shot's public name,
bounded deliberately public summary, platform, and Builder identity. An
Evolution increments the Shot's evolution number while retaining the same Shot
ID. A transition to `PUBLISHED` requires both a published source pointer and
TOHSENO download evidence. A transition to `APP_STORE` requires Apple listing
evidence.

An **Appcoin** is only an optional, deployment-agnostic link from a Shot to an
external asset identifier and evidence. Recording a link does not deploy,
mint, trade, transfer, price, or endorse an asset.

Unknown fields fail validation. There are deliberately no fields for raw
prompts, private provenance digests, local databases, app-user content,
conversations, credentials, secrets, unpublished source bytes, or model
reasoning. A safe-looking summary can still disclose an idea, so signing and
submitting a record must remain a deliberate public action.

## Signers, registries, and nodes

A **Signer** signs canonical record bytes through a versioned interface. The
wire record names its identity, suite, key ID, public verification material,
encoding, and signature. Signature verification is separate from schema and
record-hash computation so a future mobile signer can add a suite without
changing record or CLI semantics.

A **Registry** is a deterministic projection of one verified append-only
record history. After accepting a genesis record, it rejects competing records
at an occupied sequence, sequence gaps, authority changes, skipped or reversed
lifecycle transitions, and duplicate evolution numbers. Given the same
accepted history, a registry can be rebuilt from portable records. Two
separately seeded registries can nevertheless accept different valid genesis
attestations that claim the same Shot ID.

A **Node** accepts signed public records and serves registry projections. A
node is an index and transport, not an ownership authority or consensus
system. Clients can choose another node, self-host one, or verify exported
records without any node. The reference Bun/SQLite node in this repository is
the first implementation, not the definition of the network, and no protocol
package hardcodes a TOHSENO host.

The node has no designated endpoint or record field for generated-app runtime
content, private prompts, or local databases. Protocol
policy permits only deliberately submitted public records; the bounded summary
still requires Builder review because schema validation cannot determine its
meaning.

## Local independence

Taking, evolving, building, verifying, and running a local Shot require no
TOHSENO account, node, server, wallet, chain, mobile app, or TOHSENO
credentials. Public protocol participation is an optional layer around an
already independent repository.

The protocol packages use interfaces at their boundaries:

```text
identity ← signer ← protocol ← registry
                         ↘ node-client
                          ↘ reference node + SQLite registry
```

The CLI's local metadata starts at `EVOLVING` with no public record head.
Factory-created Shot IDs are checked against their baseline Git commit. A
local Shot ID is not by itself a public identity claim. Local metadata cannot
claim `PUBLISHED` or `APP_STORE`;
those states require a verified signed record chain. Remote submission is
never implicit. A future signer or node adapter must preserve the same record
schema, canonical bytes, public-data boundary, and ejection behavior.

## Status

**Implemented:** closed protocol records, deterministic serialization and
hashing, signer and verifier interfaces, an ephemeral local Ed25519 test
signer, append-only registry validation and projection, the chain-neutral
record-anchor interface, a replaceable node client, and the local Bun/SQLite
reference node, including its tested OpenAPI route.

**Prepared:** the local node runbook. No public node deployment or deployment
configuration exists.

**Proposed:** persistent Builder-key custody and recovery, mobile signing,
multi-node discovery, record-anchor implementations, and privacy-preserving
communal counts.

**Open:** the production Builder identity method, recovery policy, and trust
root; how clients discover and resolve competing valid histories across nodes;
and whether a stronger uniqueness or consensus mechanism is needed. Protocol
v1 has no cross-node fork-resolution rule. No production identity suite is
implied by the test signer.
