# TOHSENO protocol candidate

`tohseno-protocol` is the pure Rust implementation of the TOHSENO
`0.7.0` GENESIS protocol candidate. It contains wire types, exact byte
laws, commitments, cryptographic verification, lineage verification,
BuilderAccount actions, installation continuity, and conformance report types.
It has no CLI, RPC, Apple-signing, server, harness, or global-filesystem policy.

This candidate is not the final canonical protocol.

- [SPECIFICATION.md](SPECIFICATION.md) defines the normative byte and identity
  laws.
- [IMPLEMENTERS.md](IMPLEMENTERS.md) describes safe integration and lifecycle
  ordering.
- [CONFORMANCE.md](CONFORMANCE.md) defines deterministic offline checks.
- [`schemas/`](schemas) contains closed Draft 2020-12 JSON Schemas.
- [`test-vectors/protocol-v1.json`](test-vectors/protocol-v1.json) contains
  frozen cross-language vectors.

Run the crate gates with:

```sh
cargo fmt --all --check
cargo clippy -p tohseno-protocol --all-targets -- -D warnings
cargo test -p tohseno-protocol --all-targets
```

Regenerate vectors to standard output with:

```sh
cargo run -q -p tohseno-protocol --example generate_vectors
```

Print the normative reusable Fascia-tree commitment with:

```sh
cargo run -q -p tohseno-protocol --example fascia_commitment -- fascia/apple
```

## Additive coherent-intention lineage

The neutral `/2` layer is additive. It does not change one byte of
`tohseno.shot/1`, its sidecar, schemas, Swift fixture, or frozen vectors.
`ontology` defines closed records for original intention, Shot commitment,
revisioned genome proposal and acceptance, expressions, immutable organs,
versions, exact-version feedback, evolutionary intent, evolution, ownership,
token association, verification, artifact availability, and parent relations.
`lineage` places those records in RFC 8785/SHA-256/P-256 signed append-only
actions and deterministically reduces a complete authorized prefix.

An unanchored middle segment can prove canonical bytes, signatures, adjacency,
and tamper resistance with `verify_lineage_segment`, but it cannot claim
authority without the missing ownership prefix. `apply_lineage_actions`
continues from a trusted derived state. Nodes may retain multiple valid heads;
neither function chooses an ingestion-order winner or implements consensus.

Pure reduction trusts the controller/key binding declared in the first
commitment and proves consistent use thereafter. A production engine or node
candidate policy must additionally reproduce the BuilderID using the pinned
BuilderAccount factory, salt, creation bytecode, and declared initial key. This
keeps deployment policy outside neutral record semantics without mistaking a
self-declaration for factory authorization.

`adapt_v1_lineage` verifies and projects exact signed `/1` records without
rewriting or re-signing them. Missing historical intention bytes and genome
facts remain `unknown`; a Fascia digest is not relabeled as a Shot genome.

New Apple worlds may place strict `tohseno.app-metadata/2` in the already
source-tree-excluded `TOHSENO/embedded-provenance.json` path. Historical `/1`
files remain exact, and decoders dispatch on `schema`. A second embedded
identity file would create a source-tree self-reference and is not permitted.

Regenerate the v2 embedded fixture to standard output with:

```sh
cargo run -q -p tohseno-protocol --example generate_v2_vectors
```
