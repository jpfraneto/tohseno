#!/bin/sh
set -eu
umask 022

script_name="factory-harness.sh"
fixture_directory="$(CDPATH= cd -- "$(dirname -- "$0")/../../engine/fixtures/apple-expression" && pwd -P)"
conceptor="$fixture_directory/prepare-birth-fixture.py"
materializer="$fixture_directory/materialize.sh"
exerciser="$fixture_directory/exercise-birth.sh"

fail() {
  printf '%s: %s\n' "$script_name" "$1" >&2
  exit 1
}

for executable in "$conceptor" "$materializer" "$exerciser"; do
  [ -f "$executable" ] && [ ! -L "$executable" ] && [ -x "$executable" ] ||
    fail "a deterministic Apple-expression fixture is unavailable"
done

instruction=""
for argument in "$@"; do
  instruction="$argument"
done
[ -n "$instruction" ] || fail "the factory supplied no harness instruction"

shot_root="$(pwd -P)"
[ -d "$shot_root/.tohseno" ] && [ ! -L "$shot_root/.tohseno" ] ||
  fail "the harness must run inside one real Shot directory"
app_name="$(basename -- "$shot_root")"
case "$app_name" in
  ""|*[!A-Za-z0-9-]*|-*|*-|*--*) fail "the Shot name is unsafe" ;;
esac

case "$instruction" in
  *'.tohseno/CONCEPTION.md'*)
    input="$shot_root/.tohseno/private/planning/conception-input.json"
    output="$shot_root/.tohseno/private/planning/conception-output.json"
    [ -f "$input" ] && [ ! -L "$input" ] || fail "conception input is unavailable"
    "$conceptor" conception "$input" "$output"
    ;;
  *'.tohseno/EVOLUTION_INTENT.md'*)
    source_file="$shot_root/TemplateApp.swift"
    [ -f "$source_file" ] && [ ! -L "$source_file" ] ||
      fail "the accepted fixture source is unavailable"
    original='This fixture passes the real Apple materialization gates.'
    replacement='Version 0002 keeps the exact Shot identity and makes continuity visible.'
    original_count="$(grep -F -c "$original" "$source_file" || true)"
    replacement_count="$(grep -F -c "$replacement" "$source_file" || true)"
    if [ "$original_count" -eq 1 ] && [ "$replacement_count" -eq 0 ]; then
      stage="$shot_root/.tohseno/private/evolution-fixture.swift"
      [ ! -e "$stage" ] && [ ! -L "$stage" ] || fail "the evolution fixture stage already exists"
      sed "s|$original|$replacement|" "$source_file" >"$stage"
      chmod 0644 "$stage"
      mv "$stage" "$source_file"
    elif [ "$original_count" -ne 0 ] || [ "$replacement_count" -ne 1 ]; then
      fail "the deterministic evolution source boundary is ambiguous"
    fi
    ;;
  *'.tohseno/TASK.md'*)
    app_record="$shot_root/.tohseno/app.toml"
    [ -f "$app_record" ] && [ ! -L "$app_record" ] || fail "Shot metadata is unavailable"
    bundle_id="$(sed -n 's/^bundle_id = "\([A-Za-z0-9.-]*\)"$/\1/p' "$app_record")"
    [ -n "$bundle_id" ] || fail "the fixture bundle identifier is unavailable"
    project="$shot_root/$app_name.xcodeproj"
    if [ ! -e "$project" ] && [ ! -L "$project" ]; then
      "$materializer" "$shot_root" "$app_name" "$bundle_id"
    elif [ ! -d "$project" ] || [ -L "$project" ]; then
      fail "the deterministic Xcode project boundary is unsafe"
    fi
    "$exerciser" "$shot_root" "$app_name"
    ;;
  *) fail "the factory supplied an unknown harness phase" ;;
esac
