# System architecture

## Product boundary

TOHSENO is a local intention compiler and app factory. It converts private
human input into a sanitized plan, then deterministically turns that plan into
an independent iOS repository. New apps start with local, account-free data
defaults and declare any other storage or network behavior in their manifest.
TOHSENO itself operates no generated-app content backend.

The resulting Shot is one coherent software intention. Evolutions retain its
stable Shot ID and repository. The optional public protocol sits outside the
local creation path: it can index deliberately public signed facts, but it is
never required to build or run the app.

## Creation pipeline

```text
CLI or loopback Studio
        |
        v
input normalization ──> private .tohseno/provenance/
        |
        v
selected-provider planner ──> strict sanitized ShotPlan
        |                          |
        | invalid/offline          v
        └──────────────> Blank fallback / owner review
                                   |
                                   v
released catalog resolver
                                   |
                                   v
iOS kernel + template + dependency-ordered skills
                                   |
                                   v
composition lock + app manifest + SHOT/DONE
                                   |
                                   v
atomic Git repository creation
                                   |
                                   v
coding agent -> pinned verifier -> native Simulator runner
```

The AI boundary ends at semantic planning and coding work. Catalog loading,
dependency resolution, collision checks, file ownership, hashing, manifest
validation, repository creation, and verification are deterministic.

## Catalog layers

The released `catalog/` contains:

- `kernels/ios-kernel`: neutral SwiftUI shell and build configuration;
- `templates/blank` and `templates/daily-game`: starting shapes;
- `skills/<id>`: descriptors, instructions, overlays, dependencies,
  conflicts, and acceptance files.

`packages/skills` is the shared engine. It bounds input sizes, rejects links
and unsafe paths, validates exact descriptor fields, sorts dependency closure,
prevents undeclared overwrites, applies explicit replacements, and emits the
lock.

The lock records release ID, kernel/template/skill versions and digests,
ordered skills, immutable applied-file hashes, and ownership. Files intended
for later customization are excluded from immutable hashes explicitly, not
implicitly.

## App manifest

`app.manifest.json` declares application identity, composition, local and
remote data, storage, network, generated-app runtime identity strategy,
entitlements, integrations,
native operations, privacy, production readiness, and irreversible operations.
It is the one canonical application manifest. Its intrinsic schema version
identifies how to interpret the document; it is not a second product ontology
or a distribution lifecycle state. The manifest pins the composition and a
sanitized-plan digest, and the verifier rejects unknown schema versions.

## Ownership and privacy

Raw intention and reference bytes are mode-restricted and gitignored.
Protected provenance is snapshotted before the coding agent runs and restored
or isolated if modified. Public worktree verification searches for copied
private input, unsafe links, changed pinned machinery, manifest drift, and lock
violations.

The selected provider is the only planning/coding provider used. Provider
credentials are removed from child environments unless a recognized,
explicitly authorized operation needs its own credential path. Logs and
progress journals are bounded and content-free.

## Runtime doors

The global CLI authenticates cached release tools before dispatch. Each Shot
also embeds its verifier and machine protocol for ejection. iOS operations
read project, scheme, and product names from the app manifest and need no
factory service.

Studio is a loopback application over the same services. It does not own Shot
state and can be closed without affecting the CLI or an app.

## Public protocol boundary

The protocol uses four append-only record kinds:

```text
SHOT_CREATED
EVOLUTION_RECORDED
LIFECYCLE_TRANSITIONED
APPCOIN_LINKED
```

Records are canonicalized deterministically, hash-linked per Shot, and signed
by an explicit Builder identity. The registry reducer is the lifecycle state
machine: `EVOLVING → PUBLISHED → APP_STORE`. It also enforces sequence,
previous-hash, authority, evolution-number, publication-evidence, and Appcoin
link invariants.

Roles and dependencies remain narrow:

```text
packages/identity      public identity roles and verification methods
packages/signer        signer/verifier interfaces + ephemeral local test suite
packages/protocol      wire records, schemas, canonical bytes, hashes
packages/registry      append-only validator and deterministic projection
packages/node-client   replaceable transport interface
apps/reference-node    Bun HTTP adapter + SQLite registry
```

The local Ed25519 implementation proves suite pluggability. It does not decide
production Builder key custody, recovery, or mobile signing.

## Reference node

The reference node's append endpoint accepts only closed-schema public Shot
records. It verifies the signature over canonical unsigned-record bytes,
computes the record ID as SHA-256 over canonical signed-record JSON, and
validates the per-registry sequence, previous-hash link, and lifecycle rules.
It then stores canonical record JSON and the derived projection in one SQLite
transaction. The versioned HTTP API has append, read, and export operations,
but no record update or delete operation.

The node has no accounts, Builder secrets, generated-app runtime API, or
hardcoded TOHSENO host. Its database is an index; another implementation can
reverify the same accepted history and derive the same projection. The
append-only API and SQLite table constraints are not a guarantee of global
durability, operator-independent immutability, or cross-node consensus.
Production trust roots and resolution of competing valid histories across
nodes remain Open. Deploying an official or third-party node remains a
separate owner-approved external action.

## Genesis boundary

This repository contains no TOHSENO mobile application. Protocol interfaces,
app capabilities, and the reference node are inputs to the factory; the
mobile application must be an output. The first stable factory release creates
that app as its first Shot in a clean external workspace, preserving ordinary
composition, ownership, ejection, and verification.
