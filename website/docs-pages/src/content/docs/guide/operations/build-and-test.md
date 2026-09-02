---
title: Build and test
description: The repository verification matrix and focused commands for protocol, Apple, web, lifecycle, and docs work.
---

Run the verification appropriate to your change. Release evidence requires the complete matrix from a clean captured commit.

## Full repository matrix

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features

swift build --package-path apple-identity
swift test --package-path apple-identity
swift test --package-path fascia/apple
swift test --package-path sdk/apple/TohsenoCompanionKit
swift test --package-path companion/apple/TohsenoCompanion
swift test --package-path macos/Tohseno

forge build --root contracts
forge test --root contracts -vvv

node --test studio/tests/static_assets.test.mjs
(cd website && bun run typecheck && bun test)

./scripts/test-ontology-lifecycle.sh
./scripts/test-local-companion-e2e.sh
./scripts/test-macos-service-lifecycle.sh
./scripts/test-network-e2e.sh
```

The lifecycle scripts use isolated service, Shot, relay and LaunchAgent fixtures. They must not touch the developer's actual LaunchAgent or remove unrelated Keychain entries.

## Protocol-only

```sh
cargo fmt --all --check
cargo clippy -p tohseno-protocol --all-targets -- -D warnings
cargo test -p tohseno-protocol --all-targets
cargo run -q -p tohseno-protocol --example generate_vectors
```

Generator output must exactly match the committed vector. Do not overwrite a vector merely to make drift pass.

## Living-project loop

```sh
swift test --package-path macos/Tohseno
swift test --package-path sdk/apple/TohsenoCompanionKit
swift test --package-path companion/apple/TohsenoCompanion
./scripts/test-local-companion-e2e.sh
```

The E2E uses a real isolated relay/service and deterministic fixture harness; it proves durable encrypted delivery and exact-base behavior without paid model inference or real service state.

## Documentation

```sh
cd website/docs-pages
bun install
bun run check
bun run build
```

Starlight validates content/frontmatter and produces static output in `dist/`, including Pagefind search. The build syncs the shared tutorial assets first.
