# Agent guidance

This repository has an explicit authority hierarchy. Read this before editing
anything that looks like documentation — several prose files here are governed
protocol material, not free-form docs.

## Where authority lives

1. **`protocol/`** — normative and authoritative over all prose.
   `protocol/SPECIFICATION.md`, `protocol/CONFORMANCE.md`, the schemas, and the
   test vectors define exact byte encodings and validation rules. If any prose
   file disagrees with `protocol/`, `protocol/` wins.
2. **`docs/adr/`** — accepted architectural decisions. ADR 0024 governs the
   app-local `.tohseno/` Git boundary: the directory is integral and never
   blanket-ignored, while exact private and transient paths remain ignored.
   ADR 0025 governs the native macOS product transition: `Tohseno.app` is the
   primary surface over the one existing Rust factory; Companion setup,
   successful-day qualification, subscription gating of local/BYO execution,
   and npm/browser first run are no longer consumer requirements; managed
   inference instead uses an append-only balance, a constrained TOHSENO proxy,
   and Bankr behind explicit consent and a hard reservation. ADR 0025 changes
   no public protocol encoding and authorizes no external billing, Bankr,
   signing, notarization, or release activation.
   ADR 0026 governs the keyboard-first native creation surface, the optional
   truthful local Registry/Builder track-record destination, and the
   fail-closed one-line native installer. Plain Return sends from a focused
   intention composer while Shift-Return inserts a line. The Registry may show
   verified local Shot heads and local/test-only identity status, but it must
   not imply public Builder authority or publication when registry RPC is not
   implemented. `/install` and `/download` remain unavailable until the exact
   immutable notarized DMG URL and SHA-256 are activated.
   ADR 0027 governs the native selected-app workspace: Build/App/Source tabs,
   bounded owner-local semantic activity and changed-file projection, an
   honest non-interactive Simulator capture, a permanent cable handoff card,
   and a button-gated keyboard-first evolution composer. It does not restore
   the deleted Studio dashboard or expose internal phases, identities, raw
   harness output, prompts, or protocol controls on the normal path.
   ADR 0022 governs
   optional app naming: a supplied name is authoritative; when omitted, local
   machinery reserves a technical slug and the one existing implementation
   model chooses the user-facing product name from the intent. ADRs 0021 and
   0020 still govern their retained installer, cable, entitlement, and receipt
   compatibility mechanisms, but ADR 0025 supersedes their npm-first,
   Companion-first, qualification, and subscription-gate product decisions.
   ADR 0019 governs the
   bounded intent-to-usable-app transition: one implementation harness, at
   most one code/build repair, one shared wall-clock budget, and one private
   State Transition Receipt. ADR 0017 governs how a
   birth runs: the engine composes and accepts the Genome itself and the one
   harness invocation reads the exact intention, so there is no Conception
   phase and no `.tohseno/CONCEPTION.md`. Do not reintroduce a planning round
   trip in front of the build. ADR 0016's App → Intent → App on your iPhone
   abstraction and six-state/deletion constraints remain current; ADR 0025
   makes the native Mac app its primary projection while Studio and Companion
   are optional support projections. ADR 0015 governs the persistent local factory and private
   companion boundary beneath it while preserving ADR 0014's recording format.
   ADR 0006 governs the successor (0.8) contract generation and public-witness
   design. Generation 0.8.0 is deployed and is the current client-trusted
   active generation under `release/contract-activations/`; activation does
   not imply that secure Builder creation, registry RPC, receipts, source
   hosting, catalog discovery, or download are implemented.

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
