# Optional reference node

This Bun service is a replaceable, non-authoritative index of signed TOHSENO
public records. It has no accounts, Builder secrets, prompt intake, generated
app runtime content, or privileged TOHSENO identity. A conforming replacement
given the same accepted history can rebuild the same public view by verifying
record schemas and signatures, recomputing signed-record hashes, checking
previous-hash links and lifecycle transitions, and replaying the records.

The node accepts only the closed public protocol at `POST /v1/records`. It
parses the closed schema, verifies the signature over canonical unsigned bytes,
computes SHA-256 over canonical signed-record JSON, and checks the record's
sequence and previous-hash link against this node's accepted history. The
request JSON does not need to arrive with canonical key order, and no embedded
record hash is accepted as proof. Unknown fields are rejected, and there are no
designated fields for prompts, artifacts, conversations, credentials,
unpublished source, or generated-app content. Arbitrary summary text is public
and still requires Builder review; the node cannot decide whether prose
discloses something private. Evidence URLs are inert public strings, and the
node never fetches them.

The API is append-only: it accepts the next valid record or treats an exact
duplicate as idempotent, and it exposes no record update or delete operation.
SQLite table constraints also reject record updates and deletes. Those
semantics do not guarantee operator-independent durability, availability, or
immutability if the database is lost or an operator acts outside the API.
Exported copies may persist, so submission remains a deliberate public action.
`PUBLISHED` specifically means that the Shot is downloadable through TOHSENO
from published source, with the required source and download evidence in that
record. It does not mean that TOHSENO approved the app, deployed it, listed it
in an app store, verified a human action, guaranteed availability, or made a
financial claim.

Exact duplicate submissions are idempotent. A conflicting record ID, sequence
fork, or invalid lifecycle transition fails without replacing accepted bytes.
The node stores canonical signed-record JSON and a deterministic current
public projection. Consumers should reverify exported records instead of
treating the node as an identity authority.

A valid signature authenticates the declared key's attestation; it does not
establish a production trust root, prove the public claims externally, or
select between competing valid genesis histories for the same Shot ID.
Separate nodes can accept different genesis attestations before either has a
local history. Cross-node discovery and fork resolution remain Open in
protocol v1.

## Run locally

From the repository root:

```sh
TOHSENO_NODE_DATABASE_PATH="$PWD/data/reference-node.sqlite" \
bun apps/reference-node/server.ts
```

The default origin is `http://127.0.0.1:8787`. Set
`TOHSENO_NODE_HOST`, `TOHSENO_NODE_PORT`, and
`TOHSENO_NODE_DATABASE_PATH` explicitly when another local arrangement is
needed. No public hostname is built in.

The API description is served at `GET /openapi.json` and tracked in
`apps/reference-node/openapi.json`. This repository does not include a
deployment, DNS, or hosted-database configuration for the reference node.

## Privacy and persistence risk

Everything accepted by this service is intentionally public and exportable.
The service logs only semantic route, coarse method, status, error code, and
duration. It never logs identifiers, arbitrary URL paths, headers, record
bytes, signatures, or thrown error messages.

SQLite is deliberately a single-instance reference adapter. Filesystem
databases use mode `0600` under a mode-`0700` directory and reject symbolic or
hard links. The adapter bounds each Shot to 1,024 records and 3 MiB of
canonical history so an accepted history remains exportable within the HTTP
response limit. Those are adapter limits, not protocol consensus rules.

The default loopback service is not a public deployment recipe. Any
internet-facing operator must add admission, aggregate storage quotas, and
request-rate controls in front of it; this repository provides none and does
not deploy a node. A different implementation may use another database as
long as it preserves the accepted signed records, deterministic per-history
conflict rules, response bounds, and export/reverification behavior.
