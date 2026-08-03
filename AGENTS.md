# Agent guidance

This repository has an explicit authority hierarchy. Read this before editing
anything that looks like documentation — several prose files here are governed
protocol material, not free-form docs.

## Where authority lives

1. **`protocol/`** — normative and authoritative over all prose.
   `protocol/SPECIFICATION.md`, `protocol/CONFORMANCE.md`, the schemas, and the
   test vectors define exact byte encodings and validation rules. If any prose
   file disagrees with `protocol/`, `protocol/` wins.
2. **`docs/adr/`** — accepted architectural decisions. ADR 0006 governs the
   successor (0.8) contract generation and public-witness design.
3. **`MASTER_PROMPT.md`** — the *historical* constitutional center of the
   frozen v0.7 lineage. It says so itself: it is superseded implementation
   input and must not be used as current protocol or deployment authority.
4. **`genome/LAWS.md`** — agent-facing planning law injected into the
   app-building harness. It constrains what generated apps may do (for
   example, which Apple capabilities pass the gates). Its statements must
   match the engine code in `engine/src/protocol_lifecycle.rs`; do not edit it
   as if it were ordinary prose.
5. **`docs/STATE.md`** — plain-prose snapshot of what currently ships,
   what is inactive, and what is deferred.

The web-to-local handoff in ADR 0011 is transport, not protocol law. Keep its
terms distinct: Browser Draft, Pending Relay Intention, Local Pending
Intention, Shot, and Evolution. A relay record is never a Shot. Production
handoff must stay fail-closed until the matching immutable claim-capable
release is published and the public installer pin is verified.

## Build and verify

```sh
cargo test --locked --workspace --all-targets --all-features
swift build --package-path apple-identity && swift test --package-path apple-identity
swift test --package-path fascia/apple
forge build --root contracts && forge test --root contracts -vvv
./scripts/test-ontology-lifecycle.sh   # needs macOS, Xcode, a signing identity
```

No contract generation is active and there is no deployment command on `main`;
`scripts/deploy-candidate.sh` fails closed by design. Do not add one.
