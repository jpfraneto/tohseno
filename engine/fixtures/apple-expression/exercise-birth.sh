#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  printf '%s\n' \
    "usage: engine/fixtures/apple-expression/exercise-birth.sh SHOT_ROOT APP_NAME" >&2
  exit 64
fi

shot_root=$1
app_name=$2
case "$app_name" in
  ""|*[!A-Za-z0-9-]*|-*|*-|*--*)
    printf '%s\n' "fixture app name is not a TOHSENO-safe component: $app_name" >&2
    exit 64
    ;;
esac
if [ -L "$shot_root" ] || [ ! -d "$shot_root" ]; then
  printf '%s\n' "fixture Shot root must be an existing non-symlink directory" >&2
  exit 66
fi

fixture_directory=$(
  CDPATH= cd -- "$(dirname -- "$0")" >/dev/null 2>&1
  pwd -P
)
for command in jq mktemp python3 xcodebuild xcrun; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf '%s\n' "required fixture command is unavailable: $command" >&2
    exit 69
  fi
done

simulator_json=$(mktemp "${TMPDIR:-/tmp}/tohseno-simulators.XXXXXX")
derived_data=$(mktemp -d "${TMPDIR:-/tmp}/tohseno-birth-test.XXXXXX")
cleanup() {
  rm -f -- "$simulator_json"
  rm -rf -- "$derived_data"
}
trap cleanup EXIT HUP INT TERM

xcrun simctl list devices available -j >"$simulator_json"
simulator_udid=$(
  jq -er '
    [.devices[][] | select(.deviceTypeIdentifier | contains("iPhone"))]
    | (map(select(.state == "Booted")) + .)
    | .[0].udid
  ' "$simulator_json"
)
simulator_state=$(
  jq -r --arg udid "$simulator_udid" \
    '.devices[][] | select(.udid == $udid) | .state' "$simulator_json"
)
if [ "$simulator_state" != "Booted" ]; then
  xcrun simctl boot "$simulator_udid"
fi
xcrun simctl bootstatus "$simulator_udid" -b

evidence_root="$shot_root/.tohseno/private/birth/evidence"
mkdir -p "$evidence_root"
test_log="$evidence_root/simulator-test.log"
if ! xcodebuild \
  -project "$shot_root/$app_name.xcodeproj" \
  -scheme "$app_name" \
  -configuration Release \
  -sdk iphonesimulator \
  -destination "id=$simulator_udid" \
  -derivedDataPath "$derived_data" \
  -disableAutomaticPackageResolution \
  -onlyUsePackageVersionsFromResolvedFile \
  CODE_SIGNING_ALLOWED=NO \
  ENABLE_USER_SCRIPT_SANDBOXING=YES \
  test >"$test_log" 2>&1; then
  tail -n 100 "$test_log" >&2
  exit 1
fi
grep -F "Test Suite 'All tests' passed" "$test_log" >/dev/null
printf '%s\n' \
  "Deterministic fixture review: the exact bounded intention, visible labels, app-specific organ, forbidden build-only substitution, and XCUITest result agree." \
  >"$evidence_root/intent-review.txt"

python3 "$fixture_directory/prepare-birth-fixture.py" trial \
  "$shot_root/.tohseno/private/planning" \
  "$shot_root"

printf '%s\n' "Apple expression target-user fixture trial passed."
