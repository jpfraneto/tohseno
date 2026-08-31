#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
network_derived=$(mktemp -d "${TMPDIR:-/tmp}/tohseno-network-e2e.XXXXXX")
trap 'rm -rf -- "$network_derived"' EXIT HUP INT TERM

cd "$repository_root"

# Active-generation contract behavior: a protocol P-256 BuilderAccount can
# register only after the exact commitment window, then append exactly once.
forge test --root contracts --match-contract ShotRegistryTest \
  --match-test 'test(CounterfactualControllerCanCommitBeforeDeploymentThenReveal|AppendCheckpointAdvancesHeadSequenceAndNonceExactlyOnce)' -vv

# The additive Claims contract must preserve the same Registry authority while
# enforcing immutable editions, one non-transferable Claim per account/Shot,
# exact-head binding, and ERC-1271 P-256 authorization.
forge test --root contracts --match-contract TohsenoClaimsV1Test -vv

# Production source and recipient boundaries: deterministic sanitization,
# content verification, safe extraction, narrow build classification, and
# separate install/fork identity.
cargo test --locked -p tohseno-network
cargo test --locked -p tohseno private_records_round_trip_without_losing_source_or_install_state

# Claims actions and expressive marks carry the same exact canonical bytes in
# Rust, Solidity, TypeScript, and Swift. Compare parsed JSON so irrelevant key
# order and pretty-printing cannot mask or manufacture encoding drift.
assert_json_equal() {
  node -e '
    const assert = require("node:assert/strict");
    const fs = require("node:fs");
    assert.deepStrictEqual(
      JSON.parse(fs.readFileSync(process.argv[1], "utf8")),
      JSON.parse(fs.readFileSync(process.argv[2], "utf8")),
    );
  ' "$1" "$2"
}
cargo run --quiet --locked -p tohseno-network \
  --example generate_claim_action_vectors >"$network_derived/claim-actions-v1.json"
assert_json_equal "$network_derived/claim-actions-v1.json" fixtures/claim-actions-v1.json
cargo run --quiet --locked -p tohseno-network \
  --example generate_claim_mark_vectors >"$network_derived/claim-mark-v1.json"
assert_json_equal "$network_derived/claim-mark-v1.json" fixtures/claim-mark-v1.json

# Production-shaped native orchestration: canonical Claim confirmation while
# the Mac transport is offline persists both the private receipt update and
# the existing exact-release install intention, and rejects receipt
# substitution before either can be reported as Claimed/ready.
swift test --package-path companion/apple/TohsenoCompanion \
  --filter claimWhileMacOffline
swift test --package-path companion/apple/TohsenoCompanion \
  --filter substitutedClaimReceipt

# The actual green iOS fixture must compile without signing or arbitrary
# package resolution. Derived products live only in the isolated test root.
xcodebuild \
  -project engine/fixtures/hello-world/HelloWorld.xcodeproj \
  -scheme HelloWorld \
  -configuration Debug \
  -sdk iphonesimulator \
  -derivedDataPath "$network_derived/DerivedData" \
  -disableAutomaticPackageResolution \
  CODE_SIGNING_ALLOWED=NO \
  build >/dev/null

# One signed catalog flow covers staging, immutable source promotion, chain
# evidence, discovery, profile authorization, and permissioned alias claims.
(cd website && bun test apps/site/tests/registry.test.ts apps/site/tests/claims.test.ts)

printf '%s\n' "TOHSENO person-to-person network E2E passed."
