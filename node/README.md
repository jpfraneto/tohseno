# TOHSENO node

`tohseno-node` validates, preserves, indexes, and explicitly synchronizes a
bounded subset of public TOHSENO lineage. It is a library and a small HTTP/CLI
process. It depends directly on `../protocol`; it does not define another
lineage action.

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
- GENESIS candidate authority only when the commitment's BuilderID reproduces
  from the protocol salt law, pinned planned BuilderAccountFactory, exact
  BuilderAccount creation bytecode, and declared initial key, and only before
  the branch crosses an ownership action;
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

- a fully reducible, factory-bound prefix before an ownership action becomes
  candidate-authority `verified`;
- an ownership action and every descendant may remain neutrally `verified`
  while candidate authority is `unresolved`; GENESIS does not yet define the
  ownership-transfer authorization proof needed to promote that branch;
- a cryptographically valid action by an unauthorized signer becomes
  authority `rejected`;
- a newly exposed adjacency violation becomes segment `rejected`.

Append-only bytes are never rewritten during promotion or rejection. A
retained rejected observation remains retrievable by digest, but it is not an
accepted Shot transition and explicit sync does not intentionally relay it.

## What nodes do not agree on

There is no universal mutable head, quorum, leader, chain selection, shared
database, background gossip, or node-conferred ownership. Two nodes may retain
different public prefixes, different valid branches, and different artifact
subsets. The API therefore says `observed_heads`, never “the network head.”

A node does not judge whether an intention is metaphysically coherent. It does
not turn a local or private record public, infer artifact availability,
transfer ownership, or make an on-chain anchor contain off-chain bytes.

The current contract configuration is equally explicit: GENESIS
`0.7.0` targets Robinhood Chain `4663`, and the embedded
BuilderAccountFactory, ShotRegistry, and ShotRelations coordinates are planned,
undeployed, non-canonical, and unaudited. `/v1/node` reports those facts,
including null transaction evidence, rather than treating planned addresses as
deployments.

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

`tohseno token associate … --public` is one source of such a file. When the
Shot's earlier actions remain private, the outbox action intentionally arrives
as a partial segment: the node verifies and serves its exact signed bytes,
reports the missing predecessor, and keeps both authority layers unresolved.
It does not infer Shot identity from the token address or Base chain ID.

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
storing. An honest causal gap is retained with unresolved authority; a lie,
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
