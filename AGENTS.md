# Agent guidance

This repository has an explicit authority hierarchy. Read this before editing
anything that looks like documentation — several prose files here are governed
protocol material, not free-form docs.

## Where authority lives

1. **`protocol/`** — normative and authoritative over all prose.
   `protocol/SPECIFICATION.md`, `protocol/CONFORMANCE.md`, the schemas, and the
   test vectors define exact byte encodings and validation rules. If any prose
   file disagrees with `protocol/`, `protocol/` wins.
2. **`docs/adr/`** — accepted architectural decisions. ADR 0035 governs Claim,
   exactly one Ship followed by Updates, immutable per-Shot Claim Editions, the
   additive non-transferable `TohsenoClaimsV1` receipt, the Discover timeline,
   private Following/Updates, and the Companion circle ritual. Claims has a
   separate threshold-signed activation and remains dark until exact runtime,
   Registry, relayer, released clients, and owner-attended physical evidence
   agree. It changes no frozen protocol encoding or generation-0.8 ABI, and
   authorizes no price, transfer, wallet-connect, fake Claim/installation, or
   release-gate bypass. ADR 0034 governs the
   person-to-person native software network. The Mac is the factory, Companion
   is the human authority holding the non-exportable Builder DeviceKey, and the
   active generation-0.8 Registry plus signed off-chain catalog is the public
   witness. Public source requires explicit Companion approval; recipients
   independently verify and build with their own Xcode signing identity. It
   changes no frozen protocol encoding or deployed ABI, and permits no fake
   receipt, physical evidence, generic relayer, or release-gate bypass.
   ADR 0024 governs the
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
   verified local Shot heads and local/test-only identity status; ADR 0034 now
   separately authorizes the implemented public Registry path only when its
   signed manifest and fresh chain evidence agree. `/install` and `/download` remain unavailable until the exact
   immutable notarized DMG URL and SHA-256 are activated.
   ADR 0027 governs the native selected-app workspace: Build/App/Source tabs,
   bounded owner-local semantic activity and changed-file projection, an
   honest non-interactive Simulator capture, a permanent cable handoff card,
   and a button-gated keyboard-first evolution composer. It does not restore
   the deleted Studio dashboard or expose internal phases, identities, raw
   harness output, prompts, or protocol controls on the normal path.
   ADR 0028 governs the Finder-first native handoff and first-open welcome:
   the one-liner asks only for Enter or Escape, visibly downloads and verifies
   the pinned DMG, reveals its exact Downloads location, and leaves the
   familiar drag into Applications to Finder. It also replaces the empty
   first-run placeholder with the small TAKE A SHOT invitation while keeping
   Create an App as the primary-path action. ADR 0029 supersedes only that
   passive welcome composition: before an empty factory appears, TAKE A SHOT
   is the real existing creation composer, accepts up to eight picked or
   dropped PNG/JPEG references, and offers an explicit persisted Skip beside
   Create App. It adds no second factory or command path.
   ADR 0030 supersedes the one-line command only as the website's normal
   consumer door: the landing page detects the visitor's system and links
   directly to the fail-closed macOS DMG route. The immutable HTTPS artifact,
   exact SHA-256, Developer ID, notarization, Gatekeeper, Finder handoff, and
   publication gates remain mandatory; no release is activated by that
   decision.
   ADR 0031 permits an explicitly owner-authorized, visibly labeled public
   release-candidate DMG channel so clean-Mac acceptance can exercise the real
   website-to-Finder path. The exact candidate must already be signed,
   notarized, stapled, digest-pinned, and origin-verified; stable promotion
   remains closed until acceptance and the remaining release gates pass.
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
   active generation under `release/contract-activations/`. ADR 0034 connects
   it to secure Builder bootstrap, constrained registry RPC, receipts, source
   hosting, catalog discovery, and download without changing that generation.

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
swift test --package-path macos/Tohseno
forge build --root contracts && forge test --root contracts -vvv
node --test studio/tests/static_assets.test.mjs
(cd website && bun run typecheck && bun test)
./scripts/test-ontology-lifecycle.sh
./scripts/test-local-companion-e2e.sh
./scripts/test-macos-service-lifecycle.sh
./scripts/test-network-e2e.sh
```

The three lifecycle scripts use isolated service, Shot, relay, and
LaunchAgent fixtures. They do not call the developer's real LaunchAgent or
remove unrelated Keychain records.

No new contract-generation or deployment ceremony is active on `main`;
`scripts/deploy-candidate.sh` fails closed by design. Do not add one.
