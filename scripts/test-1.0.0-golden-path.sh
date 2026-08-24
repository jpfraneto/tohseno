#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
cd "$repository_root"

cargo test --locked -p tohseno-application --test entitlement_golden_path
cargo test --locked -p tohseno cable_genesis::tests
node --test studio/tests/static_assets.test.mjs
(cd packages/cli && npm test)
swift test --package-path companion/apple/TohsenoCompanion --filter entitlementScreens

printf '%s\n' "TOHSENO 1.0.0 local golden path passed."
