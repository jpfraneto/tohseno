# TOHSENO protocol candidate

`tohseno-protocol` is the pure Rust implementation of the TOHSENO
`1.0.0-rc.1` GENESIS protocol candidate. It contains wire types, exact byte
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
