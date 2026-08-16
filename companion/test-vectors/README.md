# TOHSENO companion test vectors

`companion-v1.json` is the shared Rust/Swift fixture for private companion
cryptography. It contains deterministic **test-only** keys and recovery words;
none of its identities is valid operational state.

Regenerate the fixture with:

```sh
cargo run -p tohseno-companion --example generate_vectors
```

The crate's tests require the checked-in JSON to be exactly equal to the
deterministic generator and exercise every negative vector. Companion records
remain outside `protocol/` and are not public lineage artifacts.

The same fixture also contains exact canonical bytes for a bounded private
icon blob. Rust and Swift both verify its PNG dimensions and SHA-256 content
commitment and reject the shared tamper case.
