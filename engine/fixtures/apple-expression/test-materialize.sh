#!/bin/sh
set -eu

fixture_directory="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)"
materializer="$fixture_directory/materialize.sh"
temporary_parent="$(CDPATH= cd -- "${TMPDIR:-/tmp}" && pwd -P)"
temporary_root="$(
  mktemp -d "$temporary_parent/tohseno-apple-expression-test.XXXXXX"
)"

cleanup() {
  case "$temporary_root" in
    "$temporary_parent"/tohseno-apple-expression-test.*)
      if [ -d "$temporary_root" ] && [ ! -L "$temporary_root" ]; then
        rm -rf -- "$temporary_root"
      fi
      ;;
    *)
      printf '%s\n' \
        "test-materialize.sh: refusing unsafe cleanup." >&2
      ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

if [ ! -f "$materializer" ] || [ -L "$materializer" ]; then
  printf '%s\n' \
    "test-materialize.sh: materializer is missing or unsafe." >&2
  exit 1
fi
if ! command -v xcodebuild >/dev/null 2>&1; then
  printf '%s\n' \
    "test-materialize.sh: xcodebuild is required." >&2
  exit 1
fi

world="$temporary_root/world"
mkdir "$world"
printf '%s\n' "outside sentinel" >"$temporary_root/sentinel"

sh "$materializer" \
  "$world" \
  FixtureWorld \
  com.tohseno.fixture-world >/dev/null

for path in \
  "$world/TemplateApp.swift" \
  "$world/MEMORY.md" \
  "$world/WORLD.md" \
  "$world/FixtureWorld.xcodeproj/project.pbxproj" \
  "$world/TOHSENO/fascia.json" \
  "$world/TOHSENO/embedded-provenance.json" \
  "$world/TohsenoFascia/InstallationIdentity.swift" \
  "$world/TohsenoFascia/ContinuityEnvelope.swift" \
  "$world/TohsenoFascia/LocalPersistence.swift" \
  "$world/TohsenoFascia/Provenance.swift" \
  "$world/TohsenoFascia/TohsenoMetadata.swift"
do
  if [ ! -f "$path" ] || [ -L "$path" ]; then
    printf '%s\n' \
      "test-materialize.sh: missing safe materialized file: $path" >&2
    exit 1
  fi
done

if find "$world" ! -type f ! -type d -print -quit | grep -q .; then
  printf '%s\n' \
    "test-materialize.sh: materialized tree contains a special entry." >&2
  exit 1
fi
if grep -R -E '__APP_NAME__|__BUNDLE_ID__' \
  "$world/FixtureWorld.xcodeproj" >/dev/null; then
  printf '%s\n' \
    "test-materialize.sh: project retains an unsubstituted placeholder." >&2
  exit 1
fi
grep -F 'name = FixtureWorld;' \
  "$world/FixtureWorld.xcodeproj/project.pbxproj" >/dev/null
grep -F 'PRODUCT_BUNDLE_IDENTIFIER = com.tohseno.fixture-world;' \
  "$world/FixtureWorld.xcodeproj/project.pbxproj" >/dev/null
grep -Fqx '{}' "$world/TOHSENO/fascia.json"
grep -Fqx '{}' "$world/TOHSENO/embedded-provenance.json"
grep -Fqx 'outside sentinel' "$temporary_root/sentinel"

for source in \
  InstallationIdentity.swift \
  ContinuityEnvelope.swift \
  LocalPersistence.swift \
  Provenance.swift \
  TohsenoMetadata.swift
do
  cmp \
    "$fixture_directory/../../../fascia/apple/swift/$source" \
    "$world/TohsenoFascia/$source"
done

find "$world" -type f -exec shasum -a 256 {} \; |
  LC_ALL=C sort >"$temporary_root/before-collision.sha256"
set +e
sh "$materializer" \
  "$world" \
  FixtureWorld \
  com.tohseno.fixture-world \
  >"$temporary_root/collision.stdout" \
  2>"$temporary_root/collision.stderr"
collision_status=$?
set -e
if [ "$collision_status" -ne 73 ]; then
  printf '%s\n' \
    "test-materialize.sh: collision returned $collision_status, expected 73." >&2
  exit 1
fi
grep -F 'fixture refuses to overwrite:' \
  "$temporary_root/collision.stderr" >/dev/null
find "$world" -type f -exec shasum -a 256 {} \; |
  LC_ALL=C sort >"$temporary_root/after-collision.sha256"
cmp \
  "$temporary_root/before-collision.sha256" \
  "$temporary_root/after-collision.sha256"

derived_data="$temporary_root/derived-data"
build_log="$temporary_root/xcodebuild.log"
if ! xcodebuild \
  -project "$world/FixtureWorld.xcodeproj" \
  -scheme FixtureWorld \
  -sdk iphonesimulator \
  -destination 'generic/platform=iOS Simulator' \
  -derivedDataPath "$derived_data" \
  -disableAutomaticPackageResolution \
  -onlyUsePackageVersionsFromResolvedFile \
  CODE_SIGNING_ALLOWED=NO \
  ENABLE_USER_SCRIPT_SANDBOXING=YES \
  build >"$build_log" 2>&1; then
  tail -n 100 "$build_log" >&2
  exit 1
fi

application="$derived_data/Build/Products/Debug-iphonesimulator/FixtureWorld.app"
if [ ! -d "$application" ] || [ -L "$application" ]; then
  printf '%s\n' \
    "test-materialize.sh: Simulator application was not produced." >&2
  exit 1
fi

printf '%s\n' \
  "Apple expression fixture materialization, collision, and Simulator build passed."
