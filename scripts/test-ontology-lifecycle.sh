#!/bin/sh
set -eu
umask 077

script_name="test-ontology-lifecycle.sh"
repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
temporary_parent="$(CDPATH= cd -- "${TMPDIR:-/tmp}" && pwd -P)"
temporary_root="$(mktemp -d "$temporary_parent/tohseno-factory-test.XXXXXX")"
service_pid=""
workspace_secret_reference=""
workspace_record=""
builder_key_tag=""
identity_helper=""
machine=""

fail() {
  printf '%s: %s\n' "$script_name" "$1" >&2
  exit 1
}

cleanup() {
  cleanup_status=$?
  trap - EXIT HUP INT TERM
  if [ -n "$service_pid" ] && kill -0 "$service_pid" 2>/dev/null; then
    kill "$service_pid" 2>/dev/null || true
    wait "$service_pid" 2>/dev/null || true
  fi
  if [ -z "$workspace_secret_reference" ] &&
    [ -n "$workspace_record" ] &&
    [ -f "$workspace_record" ] &&
    [ ! -L "$workspace_record" ] &&
    [ "$(wc -c <"$workspace_record")" -le 65536 ]; then
    recovered_reference="$(sed -n 's/.*"secret_reference":"\([^"]*\)".*/\1/p' "$workspace_record" | head -n 1)"
    case "$recovered_reference" in
      workspace-seed:workspace_*) workspace_secret_reference="$recovered_reference" ;;
    esac
  fi
  case "$workspace_secret_reference" in
    workspace-seed:workspace_*)
      /usr/bin/security delete-generic-password \
        -a "$workspace_secret_reference" \
        -s com.tohseno.workspace-service \
        >/dev/null 2>&1 || cleanup_status=1
      ;;
    "") ;;
    *) cleanup_status=1 ;;
  esac
  case "$builder_key_tag" in
    org.tohseno.builder.device.*)
      if [ -x "$identity_helper" ]; then
        "$identity_helper" delete --tag "$builder_key_tag" \
          >"$temporary_root/builder-delete.json" 2>/dev/null || cleanup_status=1
      else
        cleanup_status=1
      fi
      ;;
    "")
      builder_record="$machine/identity/builder.json"
      if [ -n "$machine" ] && [ -f "$builder_record" ] && [ ! -L "$builder_record" ] &&
        [ "$(wc -c <"$builder_record")" -le 65536 ]; then
        recovered_builder_tag="$(sed -n 's/.*"local_key_tag":"\([^"]*\)".*/\1/p' "$builder_record" | head -n 1)"
        case "$recovered_builder_tag" in
          org.tohseno.builder.device.*)
            if [ -x "$identity_helper" ]; then
              "$identity_helper" delete --tag "$recovered_builder_tag" \
                >"$temporary_root/builder-delete.json" 2>/dev/null || cleanup_status=1
            else
              cleanup_status=1
            fi
            ;;
        esac
      fi
      ;;
    *) cleanup_status=1 ;;
  esac
  case "$temporary_root" in
    "$temporary_parent"/tohseno-factory-test.*)
      if [ -d "$temporary_root" ] && [ ! -L "$temporary_root" ]; then
        rm -rf -- "$temporary_root"
      fi
      ;;
    *) cleanup_status=1 ;;
  esac
  exit "$cleanup_status"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

for dependency in cargo curl python3 security swift xcodebuild xcrun; do
  command -v "$dependency" >/dev/null 2>&1 || fail "$dependency is unavailable"
done
test "$(uname -s)" = "Darwin" || fail "the 0.9 factory lifecycle requires macOS"

(cd "$repository_root" && cargo build --locked -p tohseno >/dev/null)
(cd "$repository_root" && swift build --package-path apple-identity >/dev/null)
binary="$repository_root/target/debug/tohseno"
identity_helper="$repository_root/apple-identity/.build/debug/tohseno-apple-identity"
fixture_harness="$repository_root/scripts/fixtures/factory-harness.sh"
for executable in "$binary" "$identity_helper" "$fixture_harness"; do
  test -f "$executable" && test ! -L "$executable" && test -x "$executable" ||
    fail "a factory lifecycle executable was not built safely"
done
test "$("$binary" --version)" = "tohseno 0.9.0" || fail "TOHSENO 0.9.0 was not built"
test "$("$identity_helper" --version)" = "tohseno-apple-identity 0.9.0" ||
  fail "the 0.9.0 Apple identity helper was not built"

family="$temporary_root/data"
machine="$family"
install_root="$temporary_root/install"
mkdir -p "$family" "$machine" "$install_root"
workspace_record="$install_root/service/workspace.json"

run_tohseno() {
  TOHSENO_HOME="$family" \
    TOHSENO_DATA_ROOT="$machine" \
    TOHSENO_INSTALL_ROOT="$install_root" \
    TOHSENO_APPLE_IDENTITY_HELPER="$identity_helper" \
    TOHSENO_IDENTITY_BACKEND=software-test \
    TOHSENO_TEST_FACTORY_HARNESS="$fixture_harness" \
    TOHSENO_TEST_FACTORY_NO_DEVICE=1 \
    "$binary" "$@"
}

run_tohseno init anky >"$temporary_root/init.log"
app="$family/anky"
test -d "$app/.tohseno/evolutions" || fail "init did not embed .tohseno history"
test -f "$app/.tohseno/recording-layer-v1" || fail "recording marker is missing"
test ! -e "$app/AGENTS.md" || fail "init wrote app instructions"
test ! -e "$machine/config.toml" || fail "init invented harness configuration"

printf '%s\n' '# Anky' >"$app/README.md"
printf '%s\n' 'LOCAL_SETTING=yes' >"$app/.env"
printf '%s\n' 'ordinary app file' >"$app/TASK.md"
mkdir -p "$app/build" "$app/.git"
printf '%s\n' 'keep this log' >"$app/build/compiler.log"
printf '%s\n' 'git metadata' >"$app/.git/config"
printf 'first note\nsecond line\n' >"$temporary_root/note.txt"

run_tohseno record anky --note-file "$temporary_root/note.txt" \
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

run_tohseno record anky >"$temporary_root/unchanged.log"
test ! -e "$app/.tohseno/evolutions/0002" || fail "unchanged files created a Version"
grep -Fq 'Nothing to record' "$temporary_root/unchanged.log" ||
  fail "unchanged recording was not reported clearly"

printf '%s\n' '# Anky two' >"$app/README.md"
printf 'exact piped note\n' |
  TOHSENO_HOME="$family" TOHSENO_DATA_ROOT="$machine" "$binary" record anky \
    >"$temporary_root/evolve-2.log"
second="$app/.tohseno/evolutions/0002"
test -f "$second/.complete" || fail "Version 2 was not finalized"
printf 'exact piped note\n' | cmp - "$second/note.md" >/dev/null ||
  fail "piped Version note bytes changed"

run_tohseno advanced verify anky >"$temporary_root/verify.log"
grep -Fq 'Verified 2 Versions of anky.' "$temporary_root/verify.log" ||
  fail "recording history did not verify"

TOHSENO_HOME="$family" \
  TOHSENO_DATA_ROOT="$machine" \
  TOHSENO_INSTALL_ROOT="$install_root" \
  TOHSENO_SERVICE_PORT=0 \
  TOHSENO_APPLE_IDENTITY_HELPER="$identity_helper" \
  TOHSENO_IDENTITY_BACKEND=software-test \
  TOHSENO_TEST_FACTORY_HARNESS="$fixture_harness" \
  TOHSENO_TEST_FACTORY_NO_DEVICE=1 \
  "$binary" service run \
  >"$temporary_root/service.log" 2>&1 &
service_pid=$!

runtime_path="$install_root/service/runtime.json"
attempts=0
while [ ! -f "$runtime_path" ]; do
  kill -0 "$service_pid" 2>/dev/null || fail "the isolated Local Workspace Service exited"
  attempts=$((attempts + 1))
  [ "$attempts" -lt 300 ] || fail "the isolated Local Workspace Service did not publish runtime identity"
  sleep 0.1
done

workspace_secret_reference="$(python3 -c '
import json, sys
with open(sys.argv[1], "r", encoding="utf-8") as handle:
    value = json.load(handle)
reference = value.get("secret_reference")
if value.get("schema") != "tohseno.local-workspace/1" or not isinstance(reference, str) or not reference.startswith("workspace-seed:workspace_"):
    raise SystemExit(1)
print(reference, end="")
' "$workspace_record")" || fail "the isolated workspace identity did not verify"
service_origin="$(python3 -c '
import json, sys
with open(sys.argv[1], "r", encoding="utf-8") as handle:
    value = json.load(handle)
origin = value.get("origin")
if value.get("schema") != "tohseno.local-workspace-runtime/1" or not isinstance(origin, str) or not origin.startswith("http://127.0.0.1:"):
    raise SystemExit(1)
print(origin, end="")
' "$runtime_path")" || fail "the isolated service origin did not verify"
curl -fsS --max-time 2 "$service_origin/api/v1/health" \
  >"$temporary_root/service-health.json" || fail "the isolated service is not healthy"
python3 -c '
import json, sys
runtime = json.load(open(sys.argv[1], encoding="utf-8"))
health = json.load(open(sys.argv[2], encoding="utf-8"))
assert health.get("schema") == "tohseno.local-workspace-health/1"
assert health.get("status") == "healthy"
for field in ("origin", "workspace_id", "studio_device_id", "instance_id", "service_version"):
    assert health.get(field) == runtime.get(field)
' "$runtime_path" "$temporary_root/service-health.json" ||
  fail "the service health identity did not match its durable runtime"

factory_name="factoryfixture"
factory_app="$family/$factory_name"
factory_intention="$temporary_root/factory-intention.md"
printf '%s\n' \
  '# Coherent Intention' \
  '' \
  'Show a complete, quiet native iPhone expression with a clear visible identity.' \
  'The primary screen must state that it is a TOHSENO expression and that the Apple materialization gates passed.' \
  >"$factory_intention"

run_tohseno --json create "$factory_name" \
  --prompt-file "$factory_intention" --wait \
  >"$temporary_root/create.json"
python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
assert value.get("schema") == "tohseno.create-command-result/1"
receipt = value.get("receipt", {})
execution = value.get("execution", {})
assert receipt.get("schema") == "tohseno.create-shot-receipt/1"
assert isinstance(receipt.get("command_id"), str)
assert isinstance(receipt.get("shot_id"), str)
assert isinstance(receipt.get("execution_id"), str)
assert execution.get("schema") == "tohseno.local-execution-status/1"
assert execution.get("execution_id") == receipt.get("execution_id")
assert execution.get("shot_id") == receipt.get("shot_id")
assert execution.get("version_ordinal") == 1
assert execution.get("complete") is True
assert execution.get("accepted") is True
assert execution.get("state") == "accepted"
' "$temporary_root/create.json" || fail "factory creation did not return an accepted stable receipt"

builder_record="$machine/identity/builder.json"
builder_key_tag="$(python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
tag = value.get("local_key_tag")
if value.get("schema") != "tohseno.builder/1" or value.get("key_backend") != "software_test" or value.get("test_only") is not True or not isinstance(tag, str) or not tag.startswith("org.tohseno.builder.device."):
    raise SystemExit(1)
print(tag, end="")
' "$builder_record")" || fail "factory creation did not use an isolated software-test Builder identity"

test -d "$factory_app/.tohseno/executions" || fail "factory creation has no durable execution"
test ! -e "$factory_app/.tohseno/recording-layer-v1" ||
  fail "factory creation was silently reinterpreted as a recording-only app"
cmp "$factory_intention" "$factory_app/INTENTION.md" >/dev/null ||
  fail "factory creation changed the exact intention bytes"
execution_count="$(find "$factory_app/.tohseno/executions" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')"
[ "$execution_count" -eq 1 ] || fail "factory creation did not preserve exactly one execution"
version_one="$(find "$factory_app/versions" -type f -path '*/0001/version.json' -print | head -n 1)"
[ -n "$version_one" ] && [ -f "$version_one" ] && [ ! -L "$version_one" ] ||
  fail "factory creation did not accept Version 0001"
shot_record="$factory_app/.tohseno/shot.json"
[ -f "$shot_record" ] && [ ! -L "$shot_record" ] || fail "factory creation has no safe Shot record"
stable_shot_id="$(python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
shot_id = value.get("shot_id")
if not isinstance(shot_id, str):
    raise SystemExit(1)
print(shot_id, end="")
' "$shot_record")" || fail "factory creation did not persist a stable Shot ID"

run_tohseno --json create "$factory_name" \
  --prompt-file "$factory_intention" --wait \
  >"$temporary_root/create-retry.json"
python3 -c '
import json, sys
first = json.load(open(sys.argv[1], encoding="utf-8"))
retry = json.load(open(sys.argv[2], encoding="utf-8"))
assert retry.get("receipt") == first.get("receipt")
assert retry.get("execution") == first.get("execution")
' "$temporary_root/create.json" "$temporary_root/create-retry.json" ||
  fail "idempotent creation retry changed its stable receipt"
execution_count="$(find "$factory_app/.tohseno/executions" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')"
[ "$execution_count" -eq 1 ] || fail "idempotent creation retry duplicated its execution"

curl -fsS --max-time 2 "$service_origin/api/v1/workspace" \
  >"$temporary_root/workspace-v1.json" || fail "the service did not return its workspace snapshot"
python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
factory = [shot for shot in value.get("shots", []) if shot.get("kind") == "factory_shot"]
recording = [shot for shot in value.get("shots", []) if shot.get("kind") == "recording_only"]
assert len(factory) == 1 and factory[0].get("display_name") == "factoryfixture"
assert factory[0].get("latest_version_ordinal") == 1
assert isinstance(factory[0].get("shot_id"), str)
assert isinstance(factory[0].get("expression_id"), str)
assert isinstance(factory[0].get("latest_version_id"), str)
assert len(recording) == 1 and recording[0].get("display_name") == "anky"
' "$temporary_root/workspace-v1.json" ||
  fail "the workspace snapshot did not distinguish factory and recording-only apps"

factory_evolution="$temporary_root/factory-evolution.md"
printf '%s\n' \
  '# Evolutionary Intention' \
  '' \
  'Keep the exact Shot and accepted Genome while making continuity into Version 0002 visible.' \
  >"$factory_evolution"
run_tohseno --json evolve "$factory_name" \
  --prompt-file "$factory_evolution" --wait \
  >"$temporary_root/evolve.json"
python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
assert value.get("schema") == "tohseno.evolve-command-result/1"
receipt = value.get("receipt", {})
execution = value.get("execution", {})
assert receipt.get("schema") == "tohseno.evolve-shot-receipt/1"
assert execution.get("execution_id") == receipt.get("execution_id")
assert execution.get("shot_id") == receipt.get("shot_id")
assert execution.get("version_ordinal") == 2
assert execution.get("complete") is True
assert execution.get("accepted") is True
assert execution.get("state") == "accepted"
' "$temporary_root/evolve.json" || fail "factory evolution did not return an accepted exact-base receipt"

version_two="$(find "$factory_app/versions" -type f -path '*/0002/version.json' -print | head -n 1)"
[ -n "$version_two" ] && [ -f "$version_two" ] && [ ! -L "$version_two" ] ||
  fail "factory evolution did not accept Version 0002"
python3 -c '
import json, sys
first = json.load(open(sys.argv[1], encoding="utf-8"))
second = json.load(open(sys.argv[2], encoding="utf-8"))
assert first.get("expression_id") == second.get("expression_id")
assert first.get("genome_revision") == second.get("genome_revision")
assert first.get("genome_digest") == second.get("genome_digest")
assert first.get("version_id") != second.get("version_id")
assert first.get("source_digest") != second.get("source_digest")
' "$version_one" "$version_two" ||
  fail "Version 0002 did not preserve exact Shot, Expression, and Genome continuity"
[ "$(python3 -c '
import json, sys
print(json.load(open(sys.argv[1], encoding="utf-8"))["shot_id"], end="")
' "$shot_record")" = "$stable_shot_id" ] || fail "Version 0002 changed the stable Shot ID"
cmp "$factory_intention" "$factory_app/INTENTION.md" >/dev/null ||
  fail "factory evolution mutated the original exact intention"

python3 -c '
import json, sys
workspace = json.load(open(sys.argv[1], encoding="utf-8"))
evolution = json.load(open(sys.argv[2], encoding="utf-8"))
intention = open(sys.argv[3], encoding="utf-8").read()
shot = next(item for item in workspace["shots"] if item.get("kind") == "factory_shot")
receipt = evolution["receipt"]
request = {
    "command_id": receipt["command_id"],
    "origin": "cli",
    "base_expression_id": shot["expression_id"],
    "base_version_id": shot["latest_version_id"],
    "base_version_ordinal": shot["latest_version_ordinal"],
    "intention": intention,
    "selected_feedback_actions": [],
    "references": [],
}
with open(sys.argv[4], "w", encoding="utf-8") as handle:
    json.dump(request, handle, separators=(",", ":"))
' "$temporary_root/workspace-v1.json" "$temporary_root/evolve.json" \
  "$factory_evolution" "$temporary_root/evolve-retry-request.json"
csrf_token="$(python3 -c '
import json, sys
print(json.load(open(sys.argv[1], encoding="utf-8"))["csrf_token"], end="")
' "$runtime_path")"
factory_shot_id="$(python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
print(next(item for item in value["shots"] if item.get("kind") == "factory_shot")["shot_id"], end="")
' "$temporary_root/workspace-v1.json")"
curl -fsS --max-time 5 \
  -X POST \
  -H "Origin: $service_origin" \
  -H 'Content-Type: application/json' \
  -H "x-tohseno-csrf: $csrf_token" \
  --data-binary "@$temporary_root/evolve-retry-request.json" \
  "$service_origin/api/v1/shots/$factory_shot_id/evolutions" \
  >"$temporary_root/evolve-retry.json" ||
  fail "the exact duplicate evolution request was not idempotent"
python3 -c '
import json, sys
first = json.load(open(sys.argv[1], encoding="utf-8"))["receipt"]
retry = json.load(open(sys.argv[2], encoding="utf-8"))
assert retry == first
' "$temporary_root/evolve.json" "$temporary_root/evolve-retry.json" ||
  fail "idempotent evolution retry changed its stable receipt"

python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
value["command_id"] = "command_stale_base_fixture"
with open(sys.argv[2], "w", encoding="utf-8") as handle:
    json.dump(value, handle, separators=(",", ":"))
' "$temporary_root/evolve-retry-request.json" \
  "$temporary_root/evolve-stale-request.json"
stale_status="$(curl -sS --max-time 5 \
  -o "$temporary_root/evolve-stale.json" \
  -w '%{http_code}' \
  -X POST \
  -H "Origin: $service_origin" \
  -H 'Content-Type: application/json' \
  -H "x-tohseno-csrf: $csrf_token" \
  --data-binary "@$temporary_root/evolve-stale-request.json" \
  "$service_origin/api/v1/shots/$factory_shot_id/evolutions")" ||
  fail "the stale evolution request did not return an HTTP response"
[ "$stale_status" = "409" ] || fail "the stale evolution request was not an HTTP conflict"
python3 -c '
import json, sys
response = json.load(open(sys.argv[1], encoding="utf-8"))
status = json.load(open(sys.argv[2], encoding="utf-8"))
assert response.get("schema") == "tohseno.local-api-error/1"
assert response.get("code") == "stale_base"
assert response.get("message", "").startswith("stale evolution:")
assert status.get("schema") == "tohseno.local-command-status/1"
assert status.get("command_id") == "command_stale_base_fixture"
assert status.get("state") == "rejected"
assert status.get("rejection") == response.get("message")
' "$temporary_root/evolve-stale.json" \
  "$install_root/service/command-journal/command_stale_base_fixture/status.json" ||
  fail "the stale evolution was not durably rejected with a stable error"
execution_count="$(find "$factory_app/.tohseno/executions" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')"
[ "$execution_count" -eq 2 ] || fail "the stale evolution created an extra execution"
command_count="$(find "$install_root/service/command-journal" -mindepth 1 -maxdepth 1 -type d ! -name '.staging-*' | wc -l | tr -d ' ')"
[ "$command_count" -eq 3 ] || fail "the shared service journal did not retain two accepted commands and one rejection"

curl -fsS --max-time 2 "$service_origin/api/v1/workspace" \
  >"$temporary_root/workspace-v2.json" || fail "the evolved workspace snapshot is unavailable"
python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
factory = [shot for shot in value.get("shots", []) if shot.get("kind") == "factory_shot"]
assert len(factory) == 1
assert factory[0].get("latest_version_ordinal") == 2
' "$temporary_root/workspace-v2.json" || fail "the workspace snapshot did not advance to Version 0002"

run_tohseno advanced verify "$factory_name" >"$temporary_root/factory-verify.log"
grep -Fiq 'verified' "$temporary_root/factory-verify.log" ||
  fail "the accepted factory lineage did not verify"

printf '%s\n' 'tampered' >"$first/src/README.md"
if run_tohseno advanced verify anky >"$temporary_root/tamper.log" 2>&1; then
  fail "verification accepted a tampered Version"
fi
if run_tohseno record anky >"$temporary_root/tamper-record.log" 2>&1; then
  fail "record appended after tampered history"
fi
test ! -e "$app/.tohseno/evolutions/0003" ||
  fail "tampered history consumed another Version number"

help="$($binary --help)"
for command_name in create evolve init record studio service companion list doctor update uninstall advanced; do
  printf '%s\n' "$help" | grep -Eq "^  $command_name([[:space:]]|$)" ||
    fail "ordinary help is missing $command_name"
done
for hidden_or_retired in intent install refresh retire adopt token; do
  if printf '%s\n' "$help" | grep -Eq "^  $hidden_or_retired([[:space:]]|$)"; then
    fail "ordinary help exposes hidden or retired command $hidden_or_retired"
  fi
done

printf '%s\n' "0.9 factory create/evolve and recording compatibility lifecycle passed"
