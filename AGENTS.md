# Agent guidance

This repository has an explicit authority hierarchy. Read this before editing
anything that looks like documentation — several prose files here are governed
protocol material, not free-form docs.

## Where authority lives

1. **`protocol/`** — normative and authoritative over all prose.
   `protocol/SPECIFICATION.md`, `protocol/CONFORMANCE.md`, the schemas, and the
   test vectors define exact byte encodings and validation rules. If any prose
   file disagrees with `protocol/`, `protocol/` wins.
2. **`docs/adr/`** — accepted architectural decisions. ADR 0016 governs the
   current user-facing surface: App → Intent → App on your iPhone, with Studio
   and the Companion as thin projections. ADR 0015 governs the persistent local
   factory and private companion boundary beneath it while preserving ADR
   0014's recording format. ADR 0006 governs the successor (0.8) contract
   generation and public-witness design.

   ADR 0016 is a deletion decision as much as an addition: the Studio dashboard,
   its execution-pipeline renderer, its Feedback/Marketing forms, and its
   exact-Version binding controls are gone deliberately. `studio/tests/` asserts
   they stay gone. Do not rebuild them.
3. **`MASTER_PROMPT.md`** — the *historical* constitutional center of the
   frozen v0.7 lineage. It says so itself: it is superseded implementation
   input and must not be used as current protocol or deployment authority.
4. **`genome/LAWS.md`** — historical agent-facing planning law retained for
   verification and compatibility with app-factory records. Its statements
   must match the engine code in `engine/src/protocol_lifecycle.rs`; do not
   edit it as if it were ordinary prose.
5. **`docs/STATE.md`** — plain-prose snapshot of what currently ships,
   what is inactive, and what is deferred.

The web-to-local handoff in ADR 0011 is transport, not protocol law. Keep its
terms distinct: Browser Draft, Pending Relay Intention, Local Pending
Intention, Shot, and Evolution. A relay record is never a Shot. Production
handoff must stay fail-closed until the matching immutable claim-capable
release is published and the public installer pin is verified.

## Build and verify

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
swift build --package-path apple-identity && swift test --package-path apple-identity
swift test --package-path fascia/apple
swift test --package-path sdk/apple/TohsenoCompanionKit
swift test --package-path companion/apple/TohsenoCompanion
forge build --root contracts && forge test --root contracts -vvv
node --test studio/tests/static_assets.test.mjs
(cd website && bun run typecheck && bun test)
./scripts/test-ontology-lifecycle.sh
./scripts/test-local-companion-e2e.sh
./scripts/test-macos-service-lifecycle.sh
```

The three lifecycle scripts use isolated service, Shot, relay, and
LaunchAgent fixtures. They do not call the developer's real LaunchAgent or
remove unrelated Keychain records.

No new contract-generation or deployment ceremony is active on `main`;
`scripts/deploy-candidate.sh` fails closed by design. Do not add one.
