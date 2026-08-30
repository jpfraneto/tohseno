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

# Production source and recipient boundaries: deterministic sanitization,
# content verification, safe extraction, narrow build classification, and
# separate install/fork identity.
cargo test --locked -p tohseno-network
cargo test --locked -p tohseno private_records_round_trip_without_losing_source_or_install_state

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
(cd website && bun test apps/site/tests/registry.test.ts)

printf '%s\n' "TOHSENO person-to-person network E2E passed."
