#!/bin/sh
set -eu
umask 077

script_name="test-ontology-lifecycle.sh"
repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
temporary_parent="$(CDPATH= cd -- "${TMPDIR:-/tmp}" && pwd -P)"
temporary_root="$(mktemp -d "$temporary_parent/tohseno-recording-test.XXXXXX")"

fail() {
  printf '%s: %s\n' "$script_name" "$1" >&2
  exit 1
}

cleanup() {
  case "$temporary_root" in
    "$temporary_parent"/tohseno-recording-test.*)
      if [ -d "$temporary_root" ] && [ ! -L "$temporary_root" ]; then
        rm -rf -- "$temporary_root"
      fi
      ;;
    *) fail "refusing unsafe cleanup" ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

command -v cargo >/dev/null 2>&1 || fail "cargo is unavailable"

(cd "$repository_root" && cargo build --locked -p tohseno >/dev/null)
binary="$repository_root/target/debug/tohseno"
test -x "$binary" || fail "TOHSENO binary was not built"

test_home="$temporary_root/home"
family="$temporary_root/apps"
mkdir -p "$test_home" "$family"

run_tohseno() {
  HOME="$test_home" TOHSENO_HOME="$family" "$binary" "$@"
}

run_tohseno create anky >"$temporary_root/create.log"
app="$family/anky"
test -d "$app/.tohseno/evolutions" || fail "create did not embed .tohseno history"
test -f "$app/.tohseno/recording-layer-v1" || fail "recording marker is missing"
test ! -e "$app/AGENTS.md" || fail "create wrote app instructions"
test ! -e "$test_home/.tohseno/config.toml" || fail "create invented harness configuration"

printf '%s\n' '# Anky' >"$app/README.md"
printf '%s\n' 'LOCAL_SETTING=yes' >"$app/.env"
printf '%s\n' 'ordinary app file' >"$app/TASK.md"
mkdir -p "$app/build" "$app/.git"
printf '%s\n' 'keep this log' >"$app/build/compiler.log"
printf '%s\n' 'git metadata' >"$app/.git/config"
printf 'first note\nsecond line\n' >"$temporary_root/note.txt"

run_tohseno evolve anky --note-file "$temporary_root/note.txt" \
  >"$temporary_root/evolve-1.log"
first="$app/.tohseno/evolutions/0001"
test -f "$first/.complete" || fail "Version 1 was not finalized"
test -f "$first/tree.sha256" || fail "Version 1 has no integrity digest"
cmp "$temporary_root/note.txt" "$first/note.md" >/dev/null ||
  fail "Version note bytes changed"
for relative in README.md .env TASK.md build/compiler.log; do
  cmp "$app/$relative" "$first/src/$relative" >/dev/null ||
    fail "Version 1 did not preserve $relative"
done
test ! -e "$first/src/.tohseno" || fail "Version recursively captured its ledger"
test ! -e "$first/src/.git" || fail "Version captured Git metadata"
test ! -e "$first/images" || fail "Version contains legacy image staging"
test ! -e "$first/artifact" || fail "Version contains legacy build artifacts"

run_tohseno evolve anky >"$temporary_root/unchanged.log"
test ! -e "$app/.tohseno/evolutions/0002" || fail "unchanged files created a Version"
grep -Fq 'Nothing to record' "$temporary_root/unchanged.log" ||
  fail "unchanged recording was not reported clearly"

printf '%s\n' '# Anky two' >"$app/README.md"
printf 'exact piped note\n' |
  HOME="$test_home" TOHSENO_HOME="$family" "$binary" evolve anky \
    >"$temporary_root/evolve-2.log"
second="$app/.tohseno/evolutions/0002"
test -f "$second/.complete" || fail "Version 2 was not finalized"
printf 'exact piped note\n' | cmp - "$second/note.md" >/dev/null ||
  fail "piped Version note bytes changed"

run_tohseno advanced verify anky >"$temporary_root/verify.log"
grep -Fq 'Verified 2 Versions of anky.' "$temporary_root/verify.log" ||
  fail "recording history did not verify"

printf '%s\n' 'tampered' >"$first/src/README.md"
if run_tohseno advanced verify anky >"$temporary_root/tamper.log" 2>&1; then
  fail "verification accepted a tampered Version"
fi
if run_tohseno evolve anky >"$temporary_root/tamper-evolve.log" 2>&1; then
  fail "evolve appended after tampered history"
fi
test ! -e "$app/.tohseno/evolutions/0003" ||
  fail "tampered history consumed another Version number"

help="$($binary --help)"
for command_name in create evolve studio list doctor update uninstall advanced; do
  printf '%s\n' "$help" | grep -Eq "^  $command_name([[:space:]]|$)" ||
    fail "ordinary help is missing $command_name"
done
for removed in install refresh retire intent adopt shot verify token; do
  if printf '%s\n' "$help" | grep -Eq "^  $removed([[:space:]]|$)"; then
    fail "ordinary help exposes $removed"
  fi
done

printf '%s\n' "recording lifecycle smoke passed"
