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

## Frozen v0.7 public-state checks

When verifying a historical frozen v0.7 signed public action, additionally
record:

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

## ShotRegistry generation 0.8 checks

| Check ID | Required observation |
|---|---|
| `registry_v2.generation` | The action dispatches as exact `tohseno.registry-action/2`; no frozen v0.7 or unknown action is reinterpreted. |
| `registry_v2.domain` | Name is `TOHSENO ShotRegistry`, version is `2`, chain is 4663, and verifying contract is the intended generation. |
| `registry_v2.commitment` | Registration preimage binds controller, random ShotID, privately persisted client salt, registry, chain, and deadline; its on-chain commitment has matured without timestamp reset. |
| `registry_v2.action_digest` | Exact type string, ABI word order, struct hash, and `0x1901` digest reproduce the frozen v2 vector law. |
| `registry_v2.detached_signature` | Builder-client evidence is a valid low-s P-256 prehash signature and compact encoding; this alone is not live ERC-1271 authorization. |
| `registry_v2.live_authority` | Controller eligibility and signature acceptance are observed against the intended registry state and receipt block. |
| `registry_v2.checkpoint` | Registration starts at checkpoint 1; append advances exactly one; transfer preserves the checkpoint and head. |
| `registry_v2.sequence_separation` | Checkpoint count is never compared to local lineage sequence, Version ordinal, `CFBundleVersion`, or App Store build history. |
| `registry_v2.privacy` | Head provenance contains no runtime/installation/end-user data, private feedback or references, private intention, or hash-derived link to private ancestry. |
| `registry_v2.public_checkpoint` | Head equals SHA-256 of exact `tohseno.public-checkpoint/1` RFC 8785 bytes; witness coordinates match the action domain and the public-only predecessor chain is complete or explicitly partial. |
| `registry_v2.authority_boundary` | Checkpoint-body verification is not reported as controller authority or on-chain acceptance without the paired action, live ERC-1271 decision, and receipt evidence. |

## Contract-generation definition checks

| Check ID | Required observation |
|---|---|
| `contract_generation.schema` | Exact closed `tohseno.contract-generation/1`, protocol major 2, and canonical component versions are used. |
| `contract_generation.digest` | SHA-256 of exact RFC 8785 definition bytes matches the frozen vector. |
| `contract_generation.source` | Every declared source file matches its byte length and SHA-256; the strictly ordered domain-separated source-tree digest recomputes exactly. |
| `contract_generation.build` | solc, EVM target, optimizer, metadata, IR, and Foundry facts match the committed profile. |
| `contract_generation.artifacts` | Every versioned ABI and BuilderAccount creation-bytecode artifact matches its byte length and SHA-256; raw creation and runtime code hashes match the build. |
| `contract_generation.create2` | Each predicted coordinate recomputes under EIP-1014 from the declared deployer, salt, and init-code hash and is reported only as conditional arithmetic. |
| `contract_generation.p256` | The requirement is final EIP-7951 at `0x100` with 6,900 gas, never legacy RIP-7212 semantics. |
| `contract_generation.inactive` | The build definition contains no deployment/transaction/block/authority/signature/trust-root evidence and is never accepted as activation. |

## Contract-activation checks

| Check ID | Required observation |
|---|---|
| `contract_activation.definition` | Activation generation, protocol major, chain, predicted factory/registry addresses, approved BuilderAccount runtime, and all observed runtime hashes match the exact immutable definition digest. |
| `contract_activation.evidence` | Deployment transaction/block evidence is nonzero; the canonical activation block is at or after both deployments; the actual-target P256 probe digest is present. |
| `contract_activation.causality` | Sequence starts at one/null and each successor increments once, names the prior activation signing digest, advances block height, and does not move time backward. |
| `contract_activation.digest` | SHA-256 of `TOHSENO-CONTRACT-ACTIVATION-V1\0` plus exact RFC 8785 payload bytes is the approval digest. |
| `contract_activation.policy` | Policy purpose is contract activation; dedicated release key IDs reproduce from curve-valid P-256 keys; authorities are unique and ordered; threshold is in range. |
| `contract_activation.approvals` | Approvals are unique and ordered, belong to the bound policy, carry the exact digest, are low-s, verify under their policy keys, and meet threshold. |
| `contract_activation.trust` | A valid threshold is reported only under the supplied policy until a separate client trust root pins its digest. No Builder/Shot/installation/deployer identity is treated as release authority implicitly. |

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

## Neutral lineage checks

| Check ID | Required observation |
|---|---|
| `lineage_v2.payload_digest` | SHA-256 of RFC 8785 payload bytes equals `payload_digest`. |
| `lineage_v2.action_signature` | The unchanged strict P-256 sidecar signs SHA-256 of RFC 8785 action bytes exactly once. |
| `lineage_v2.causality` | ShotID is stable; sequence and previous commitments are contiguous; timestamps do not move backward. |
| `lineage_v2.authority` | Commitment actor/key agree; each later actor and signing key equal current derived authority; ownership changes take effect only after an authorized action. |
| `lineage_v2.factory_binding` | Candidate policy independently reproduces the initial BuilderID from the pinned factory, salt, creation bytecode, and declared key. Pure reduction alone does not satisfy this check. |
| `lineage_v2.intention` | Exact original material descriptors match the commitment; inline UTF-8, when present, matches digest and length; the source is never replaced. |
| `lineage_v2.genome` | Proposal base matches current accepted genome and every mutation has a distinct explicit acceptance. |
| `lineage_v2.organ_graph` | The full same-Expression Organ declarations, sorted by `organ_id` and RFC 8785 encoded, reproduce the graph digest in both VerificationResult and Version at their respective actions. |
| `lineage_v2.organ_acceptance` | Every acceptance-test declaration has its exact deterministic gate name, including the test-text SHA-256, and every gate contributes to the VerificationResult conjunction. |
| `lineage_v2.version` | VersionID derivation, expression ordinal, genome, source, provenance, exact graph digest, verification result, known incompleteness, and optional build identity agree. |
| `lineage_v2.evolution_scope` | A graph transition has an `organ`-scoped desired change; an `organ`-scoped intent does not finalize against an unchanged graph. |
| `lineage_v2.failed_attempt` | Failed verification is representable but cannot produce an accepted Version. |
| `lineage_v2.feedback` | Feedback names an accepted exact ExpressionID/VersionID and matching optional build identity. |
| `lineage_v2.token_separation` | Token relation is optional, chain-specific, authorized, and never substituted for ShotID or ownership. |
| `lineage_v2.availability` | Missing, unknown, private, local, public, replicated, verified, and anchored states are reported without implication or upgrade. |
| `lineage_v2.partial_segment` | An unanchored middle segment reports that authority context is unavailable even when signatures and adjacency verify. |
| `lineage_v2.v1_adapter` | Every original `/1` record and sidecar remains byte-identical; unavailable historical ontology fields remain unknown. |
| `provenance_v2.embedded` | New-world `/2` metadata at the existing excluded provenance path binds Shot, expression, VersionID, genome, lineage head, source, optional build digest, and protocol version. |
