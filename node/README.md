# TOHSENO node

`tohseno-node` validates, preserves, indexes, and explicitly synchronizes a
bounded subset of public TOHSENO lineage evidence. It is a library and a small
HTTP/CLI process. It depends directly on `../protocol`; it does not define
another lineage action.

No release-authorized contract generation is active. That fact is
load-bearing: a node may verify ordinary signed lineage neutrally, but it
cannot promote any record to public candidate authority. Predicted v0.7
addresses are retired offline verification inputs, not deployments,
activations, or authority.

## What nodes agree on

For every action they possess, conforming nodes reproduce:

- the closed `tohseno.lineage-action/2` schema;
- the payload digest;
- RFC 8785 canonical action bytes and SHA-256 content address;
- the low-s P-256 signature;
- the Shot ID, sequence, previous-action link, and monotonic timestamp within
  every locally available contiguous segment;
- neutral controller authority and state laws only along a complete causal
  branch from the Shot commitment;
- the publisher's immutable public/private handling declaration.

Private actions are rejected by replication. Artifact-availability actions are
reported exactly as signed. Storing an availability statement never causes
this node to claim it has the referenced bytes. Each missing holding names its
declaring action and that action's candidate-authority status.

A public action whose predecessor is unavailable is retained as an unanchored
segment. Its schema, digest, signature, and internally available adjacency may
verify, but both neutral and candidate authority remain `unresolved`, and the
exact missing parent is exposed. When predecessors arrive, the derived index
is deterministically rebuilt:

- a fully reducible prefix becomes neutrally `verified` while candidate
  authority remains `unresolved` because no generation is active;
- a cryptographically valid action by an unauthorized signer becomes
  authority `rejected`;
- a newly exposed adjacency violation becomes segment `rejected`.

The frozen v0.7 CREATE2 helper can reproduce the BuilderID carried by a private
legacy artifact. It is never called by node classification and can never
produce candidate-authority `verified`. A neutrally valid self-declared
controller is preserved as `unresolved`, not rejected merely because no active
generation can recognize it.

Append-only bytes are never rewritten during promotion or rejection. A
retained rejected observation remains retrievable by digest, but it is not an
accepted Shot transition. Peer authority labels are never used as relay
filters: a receiver fetches advertised bytes and derives its own result from
the causal context it possesses.

## What nodes do not agree on

There is no universal mutable head, quorum, leader, chain selection, shared
database, background gossip, or node-conferred ownership. Two nodes may retain
different public prefixes, different valid branches, and different artifact
subsets. The API therefore says `observed_heads`, never “the network head.”

A node does not judge whether an intention is metaphysically coherent. It does
not turn a local or private record public, infer artifact availability,
transfer ownership, or make an on-chain anchor contain off-chain bytes.

`/v1/node` reports `active_generation: null`. Its generation policy says
candidate authority is unavailable until a release-authorized activation is
independently verified. Its legacy policy says v0.7 prediction is offline-only
and ordinary signed lineage is neutral legacy evidence. The descriptor does
not advertise the retired ShotRelations surface or any predicted address as a
current contract configuration.

The protocol now defines an ancestry-free public checkpoint suitable for the
narrow registry head. This node revision does not yet inventory checkpoint
records or receipts. That work is intentionally deferred rather than
misrepresenting ordinary lineage actions—which may commit private ancestry—as
registry-head preimages.

## Storage

The node creates:

```text
<root>/
├── node.json
└── actions/
    └── ab/
        └── ab…64-lowercase-hex….json
```

`node.json` contains a persistent random 32-byte node ID and creation time. It
is not a key and grants no protocol authority. Actions are stored under the
SHA-256 commitment of the canonical unsigned action; the signed wrapper is the
file content.

Writes use private create-new temporary files, `fsync`, and atomic hard-link
publication. Existing action files are never replaced. Storage roots,
directories, reads, and action entries reject symlinks. All indexes are
in-memory derived caches rebuilt from the append-only action files at startup
or through `integrity --rebuild`.

The current node deliberately stores public action records only. Referenced
artifacts remain explicit missing holdings in Shot views, even when an action
says those bytes were public, replicated, verified, or anchored elsewhere.

## CLI

```sh
# Start one node with two static peers.
cargo run --manifest-path node/Cargo.toml -- \
  --root /absolute/path/to/node-a \
  --peer http://127.0.0.1:8789 \
  --peer https://known-node.example \
  serve --listen 127.0.0.1:8788

# Inspect local contribution and configured peers.
cargo run --manifest-path node/Cargo.toml -- \
  --root /absolute/path/to/node-a status

# Pull once from exactly the configured peers. Ingestion never auto-relays.
cargo run --manifest-path node/Cargo.toml -- \
  --root /absolute/path/to/node-a \
  --peer http://127.0.0.1:8789 sync

# Load one bounded, regular, non-symlink canonical signed action file.
cargo run --manifest-path node/Cargo.toml -- \
  --root /absolute/path/to/node-a \
  ingest /absolute/path/to/signed-action.json

cargo run --manifest-path node/Cargo.toml -- \
  --root /absolute/path/to/node-a \
  inspect 0x<32-byte-shot-id>

cargo run --manifest-path node/Cargo.toml -- \
  --root /absolute/path/to/node-a integrity

cargo run --manifest-path node/Cargo.toml -- \
  --root /absolute/path/to/node-a integrity --rebuild
```

The CLI no longer creates ordinary-lineage public outbox files. Such a file can
commit a private predecessor even when its own availability is public.
Existing files remain valid legacy evidence and a node may preserve them as an
explicitly partial segment with unresolved authority, but they are not
registry-head preimages or current publication artifacts. Current registry
heads use the closed, ancestry-free public-checkpoint projection.

The installed binary uses the same surface:

```sh
tohseno-node --root /srv/tohseno-node serve --listen 127.0.0.1:8788
tohseno-node --root /srv/tohseno-node --peer https://peer.example sync
```

Without `--root`, storage defaults to `~/.tohseno-node`.

## HTTP API

The node/inventory surface advertises `tohseno.node/2`. The signed lineage
objects remain the protocol's neutral `/2` records; node validation metadata
is a derived transport view and is never signed back into lineage.

| Method | Path | Meaning |
|---|---|---|
| `GET` | `/v1/health` | Local health, counts, and integrity state |
| `GET` | `/v1/node` | Persistent node identity and supported lineage schema |
| `GET` | `/v1/peers` | Exact configured static peer origins |
| `GET` | `/v1/shots` | Locally indexed Shot summaries and observed heads |
| `GET` | `/v1/shots/{shot_id}` | Local action references, authority context, missing parents, and missing artifacts |
| `GET` | `/v1/actions/{digest}` | Canonical signed public action bytes |
| `POST` | `/v1/actions` | Validate and append one signed public action |
| `GET` | `/v1/integrity` | Fresh disk validation without mutation |
| `GET` | `/v1/sync` | Last in-memory explicit-sync result |
| `POST` | `/v1/sync` | Pull once from configured peers only |

`POST /v1/actions` accepts the closed signed lineage JSON directly. There is no
caller-selected peer URL on `POST /v1/sync`, which avoids turning the node into
an SSRF proxy.

## Bounds and failure behavior

- Action and request body: 256 KiB.
- Actions per node: 100,000.
- Actions per Shot held by one node: 10,000.
- Static peers: 32.
- Remote node descriptor: 64 KiB.
- Remote Shot inventories: 4 MiB and 10,000 Shots.
- Outbound connect timeout: 5 seconds; complete request timeout: 20 seconds.
- Redirects are disabled; peer origins cannot contain credentials, paths,
  queries, or fragments.

Synchronization retrieves advertised actions in deterministic sequence/digest
order and verifies returned bytes against every advertised reference before
storing. Peer-derived authority labels and generation-policy text are not
trusted and need not equal local policy; every fetched record is classified
again under local rules. A retired v0.7 peer descriptor carrying the old
contract-configuration object remains readable for evidence synchronization,
but none of those coordinates are compared, re-advertised, or used for
authority. An honest causal gap is retained with unresolved authority; a lie,
invalid signature, known-invalid adjacency, or known unauthorized transition
fails closed. A failure may leave earlier valid or explicitly unresolved
actions appended; it never rolls them back or upgrades an invalid action. One
surviving node can continue serving every public action or segment it actually
possesses with its current validation context.

`integrity` distinguishes storage integrity from lineage authority. An intact
append-only store can be healthy while explicitly containing unresolved or
authority-rejected observations. Its report includes separate signed-record,
segment, neutral-authority, and candidate-authority counts plus a bounded list
of exact missing-parent findings.

## Workspace integration

The node is a first-class member of the root Cargo workspace and shares the
candidate's Rust version, package metadata, dependency lockfile, formatting,
linting, and test gates.
