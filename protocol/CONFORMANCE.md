# Deterministic conformance

Conformance is an offline, fail-closed judgment. It never calls an LLM and does
not infer intent from prose. A report uses schema `tohseno.conformance/1`.
`conformant` is true exactly when every check has status `pass`; `fail` and
`not_checked` both prevent conformance.

## Required protocol checks

A verifier for a local Evolution records at least these stable check IDs:

| Check ID | Required observation |
|---|---|
| `schema.closed_json` | Every protocol JSON object parses once against its closed versioned type; no duplicate, unknown, or trailing data exists. |
| `record.shape` | Protocol, schema, identifiers, timestamp, sequence, bundle version, and optional origin are valid. |
| `record.canonical` | RFC 8785 bytes reproduce the declared SHA-256 digest. |
| `record.signature` | P-256 key is valid, digest matches, signature verifies as one prehash, and s is low. |
| `record.device_authority` | The signing DeviceKey is authorized by complete evidence available to the verifier at this Evolution. The bundled GENESIS verifier has no replacement-proof format and therefore accepts only the initial key that reproduces the CREATE2 BuilderID; every other key fails closed. |
| `lineage.contiguous` | A normal lineage begins at 1/null; an adopted lineage begins at legacy N+1/null with declared origin. Descendants have no origin, are contiguous from that root, and every previous commitment matches. |
| `lineage.identity` | ShotID, BuilderID, bundle ID, and Fascia identifier remain stable. |
| `genesis.input` | Exact prompt/image stream reproduces `genesis_input_sha256`, when private input evidence is intentionally available. |
| `source_tree.commitment` | The explicit source root and exclusion law reproduce `source_tree_sha256`. |
| `source_tree.no_symlinks` | No included or excluded traversal encountered a symbolic link. |
| `fascia.commitment` | The pinned reusable Apple Fascia tree reproduces `fascia_sha256`. |
| `fascia.manifest` | Concrete `TOHSENO/fascia.json` parses and its declarations match the record. |
| `fascia.required_files` | Every normative sidecar, document, and Swift source is present exactly once. |
| `fascia.target_membership` | Required Swift sources are members of the generated application target. |
| `fascia.installation_identity` | Installation identity is app-specific P-256, non-exportable, this-device-only, and hardware-backed when available. |
| `fascia.capabilities` | Declared capabilities match source imports, project settings, entitlements, usage descriptions, and inspectable APIs. |
| `fascia.dependencies` | No undeclared third-party runtime dependency exists. |
| `fascia.storage` | Storage is local-first and declared; cloud use is explicit. |
| `fascia.network` | Every endpoint, purpose, and transmitted data category is declared. |
| `privacy.boundary` | No Builder recovery, private key, private prompt, or private app data is embedded or uploaded. |
| `provenance.embedded` | Embedded public provenance exactly matches the signed record despite its explicit source-tree exclusion. |
| `apple.bundle_identity` | Project/product bundle ID equals the record. |
| `apple.bundle_version` | `CFBundleVersion` equals the integer Evolution sequence. |
| `apple.offline_build` | The declared source builds without undeclared network acquisition. |

Installation, launch, Apple signing, publication, and registry witnessing are
operational checks above the pure protocol. A lifecycle report may include
them, but it MUST distinguish `implemented`, `automatically_verified`,
`manually_observed`, `deployed`, `published`, and `pending`.

## Public-state checks

When verifying signed public actions, additionally record:

| Check ID | Required observation |
|---|---|
| `action.domain` | Exact domain name/version, chain 4663, and intended verifying contract. |
| `action.type_hash` | Frozen EIP-712 type string reproduces the candidate type hash. |
| `action.digest` | ABI struct hash and `0x1901` digest reproduce the declared digest. |
| `action.signature` | Compact P-256 signature is version 1, curve-valid, authorized, and low-s. |
| `action.nonce` | Nonce equals current contract state. |
| `action.deadline` | Deadline has not passed. |
| `action.transition` | Create starts at native sequence 1 or a verified legacy-adoption N+1 root, always PUBLISHED; append is contiguous; public state never regresses. |

Recovery verification uses the separate secp256k1 authority and MUST require a
nonzero new recovery address.

The bundled candidate does not currently verify a DeviceKey replacement,
revocation, or recovery transition. An action schema, valid detached signature,
caller-supplied nonce, or encrypted local backup is not authorization evidence
for `record.device_authority`.

## Candidate gates

Run:

```sh
cargo fmt --all --check
cargo clippy -p tohseno-protocol --all-targets -- -D warnings
cargo test -p tohseno-protocol --all-targets
cargo run -q -p tohseno-protocol --example generate_vectors
```

The generator output must exactly match
`test-vectors/protocol-v1.json`. The test suite independently recomputes record
canonical bytes and signatures, mutation and high-s failures, DeviceKey and
Installation IDs, all EIP-712 type hashes, representative domain/struct/action
digests, compact encoding, CREATE2 prediction, Genesis input, and source-tree
commitments.

Each JSON Schema must identify Draft 2020-12, have a stable `$id`, and close
every object with `additionalProperties: false`. Schema validation alone is
not a conformance result; all semantic checks applicable to the artifact are
required.
