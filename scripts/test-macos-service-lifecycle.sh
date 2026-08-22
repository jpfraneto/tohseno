#!/bin/sh
set -eu
umask 077

script_name="test-macos-service-lifecycle.sh"
repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
temporary_parent="$(CDPATH= cd -- "${TMPDIR:-/tmp}" && pwd -P)"
temporary_root="$(mktemp -d "$temporary_parent/tohseno-macos-service.XXXXXX")"
workspace_secret_reference=""
workspace_record=""
install_root=""
launch_agents=""
launchctl_state=""
fake_launchctl=""

fail() {
  printf '%s: %s\n' "$script_name" "$1" >&2
  exit 1
}

fake_environment() {
  env \
    TOHSENO_INSTALL_ROOT="$install_root" \
    TOHSENO_LAUNCH_AGENTS_DIR="$launch_agents" \
    TOHSENO_TEST_LAUNCHCTL_STATE="$launchctl_state" \
    TOHSENO_DATA_ROOT="$temporary_root/data" \
    TOHSENO_HOME="$temporary_root/shots" \
    TOHSENO_SERVICE_PORT=0 \
    TOHSENO_TEST_LAUNCHER_LOG="$install_root/launcher-selections.log" \
    "$@"
}

stop_exact_fixture_process() {
  pid_path="$launchctl_state/service.pid"
  [ -n "$launchctl_state" ] && [ -f "$pid_path" ] && [ ! -L "$pid_path" ] || return 0
  [ "$(wc -c <"$pid_path")" -le 64 ] || return 1
  fixture_pid="$(tr -d '\r\n' <"$pid_path")"
  case "$fixture_pid" in
    ""|*[!0-9]*) return 1 ;;
  esac
  if kill -0 "$fixture_pid" 2>/dev/null; then
    runtime_record="$install_root/service/runtime.json"
    [ -f "$runtime_record" ] && [ ! -L "$runtime_record" ] &&
      [ "$(wc -c <"$runtime_record")" -le 65536 ] || return 1
    runtime_pid="$(python3 -c '
import json, sys
print(json.load(open(sys.argv[1], encoding="utf-8"))["process_id"], end="")
' "$runtime_record")" || return 1
    [ "$runtime_pid" = "$fixture_pid" ] || return 1
    kill -INT "$fixture_pid" 2>/dev/null || true
    attempts=0
    while kill -0 "$fixture_pid" 2>/dev/null && [ "$attempts" -lt 100 ]; do
      sleep 0.05
      attempts=$((attempts + 1))
    done
    if kill -0 "$fixture_pid" 2>/dev/null; then
      kill -KILL "$fixture_pid" 2>/dev/null || true
    fi
  fi
}

cleanup() {
  cleanup_status=$?
  trap - EXIT HUP INT TERM
  if [ "$cleanup_status" -ne 0 ] && [ -n "$install_root" ]; then
    for diagnostic in \
      "$install_root/logs/workspace-service.error.log" \
      "$install_root/logs/workspace-service.error.log.previous" \
      "$install_root/logs/workspace-service.log" \
      "$install_root/logs/workspace-service.log.previous"; do
      if [ -f "$diagnostic" ] && [ ! -L "$diagnostic" ]; then
        tail -n 20 "$diagnostic" >&2
      fi
    done
    if [ -n "$launchctl_state" ] && [ -f "$launchctl_state/service.pid" ] &&
      [ ! -L "$launchctl_state/service.pid" ]; then
      failed_pid="$(tr -d '\r\n' <"$launchctl_state/service.pid")"
      printf '%s: active fixture PID %s state:\n' "$script_name" "$failed_pid" >&2
      ps -p "$failed_pid" -o pid=,ppid=,stat=,command= >&2 || true
    fi
  fi
  launch_agent="$launch_agents/com.tohseno.workspace-service.plist"
  if [ -n "$fake_launchctl" ] && [ -x "$fake_launchctl" ] &&
    [ -n "$launchctl_state" ] && [ -f "$launchctl_state/registered-plist" ] &&
    [ -f "$launch_agent" ] && [ ! -L "$launch_agent" ]; then
    fake_environment "$fake_launchctl" \
      bootout "gui/$(id -u)" "$launch_agent" >/dev/null 2>&1 || cleanup_status=1
  fi
  stop_exact_fixture_process || cleanup_status=1
  if [ -z "$workspace_secret_reference" ] &&
    [ -n "$workspace_record" ] && [ -f "$workspace_record" ] &&
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
  case "$temporary_root" in
    "$temporary_parent"/tohseno-macos-service.*)
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

for dependency in cargo curl id ps python3 security; do
  command -v "$dependency" >/dev/null 2>&1 || fail "$dependency is unavailable"
done
[ "$(uname -s)" = "Darwin" ] || fail "the real service lifecycle requires macOS"

(cd "$repository_root" && cargo build --locked -p tohseno >/dev/null)
candidate="$repository_root/target/debug/tohseno"
fake_launchctl="$repository_root/scripts/fixtures/fake-launchctl.py"
stable_launcher_fixture="$repository_root/scripts/fixtures/test-stable-launcher.sh"
for executable in "$candidate" "$fake_launchctl" "$stable_launcher_fixture"; do
  [ -f "$executable" ] && [ ! -L "$executable" ] && [ -x "$executable" ] ||
    fail "a required service lifecycle executable is unsafe"
done
[ "$("$candidate" --version)" = "tohseno 0.9.9" ] ||
  fail "the service lifecycle requires a TOHSENO 0.9.9 debug binary"
grep -a -Fq 'TOHSENO_TEST_LAUNCHCTL' "$candidate" ||
  fail "the candidate does not contain the debug-only launchctl boundary"

install_root="$temporary_root/install"
launch_agents="$temporary_root/LaunchAgents"
launchctl_state="$temporary_root/fake-launchctl-state"
release_a="$install_root/releases/release-a"
release_b="$install_root/releases/release-b"
mkdir -m 0700 \
  "$install_root" "$install_root/bin" "$install_root/releases" \
  "$release_a" "$release_a/bin" "$release_b" "$release_b/bin" \
  "$launch_agents" "$launchctl_state" "$temporary_root/data" "$temporary_root/shots"
printf '%s\n' 'TOHSENO_TEST_LAUNCHCTL_V1' \
  >"$launchctl_state/.tohseno-test-launchctl-v1"
chmod 0600 "$launchctl_state/.tohseno-test-launchctl-v1"
cp "$candidate" "$release_a/bin/tohseno"
cp "$candidate" "$release_b/bin/tohseno"
cp "$stable_launcher_fixture" "$install_root/bin/tohseno"
chmod 0755 \
  "$release_a/bin/tohseno" "$release_b/bin/tohseno" "$install_root/bin/tohseno"
ln -s 'releases/release-a' "$install_root/current"
printf '%s\n' '<plist>unowned neighbor</plist>' >"$launch_agents/com.example.keep.plist"
mkdir -m 0700 "$temporary_root/shots/ordinary-app"
printf '%s\n' 'app data survives service administration' \
  >"$temporary_root/shots/ordinary-app/source.txt"

run_tohseno() {
  fake_environment \
    TOHSENO_TEST_LAUNCHCTL="$fake_launchctl" \
    "$candidate" "$@"
}

runtime_path="$install_root/service/runtime.json"
workspace_record="$install_root/service/workspace.json"
launch_agent="$launch_agents/com.tohseno.workspace-service.plist"
pid_path="$launchctl_state/service.pid"

if ! run_tohseno --json service install \
  >"$temporary_root/install.json" 2>"$temporary_root/install-command.error.log"; then
  for diagnostic in \
    "$temporary_root/install-command.error.log" \
    "$install_root/logs/workspace-service.error.log" \
    "$install_root/logs/workspace-service.error.log.previous" \
    "$install_root/logs/workspace-service.log"; do
    if [ -f "$diagnostic" ] && [ ! -L "$diagnostic" ]; then
      tail -n 20 "$diagnostic" >&2
    fi
  done
  if [ -f "$install_root/launcher-selections.log" ]; then
    printf '%s: selected release: ' "$script_name" >&2
    tail -n 1 "$install_root/launcher-selections.log" >&2
  fi
  if [ -f "$pid_path" ] && [ ! -L "$pid_path" ]; then
    failed_pid="$(tr -d '\r\n' <"$pid_path")"
    printf '%s: fake launchctl PID %s state:\n' "$script_name" "$failed_pid" >&2
    ps -p "$failed_pid" -o pid=,ppid=,stat=,command= >&2 || true
  fi
  fail "the real service install command failed"
fi
python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
assert value.get("schema") == "tohseno.service-status/1"
assert value.get("operation") == "install"
assert value.get("installed") is True
assert value.get("healthy") is True
assert value.get("service_version") == "0.9.9"
assert value.get("state_preserved") is True
' "$temporary_root/install.json" || fail "service install did not return verified health"
[ -f "$launch_agent" ] && [ ! -L "$launch_agent" ] ||
  fail "service install did not publish a regular isolated LaunchAgent"
[ -f "$pid_path" ] && [ ! -L "$pid_path" ] ||
  fail "the fake launchctl did not persist the exact service PID"
install_pid="$(tr -d '\r\n' <"$pid_path")"
kill -0 "$install_pid" 2>/dev/null ||
  fail "the real Local Workspace Service did not survive the install command"

workspace_secret_reference="$(python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
reference = value.get("secret_reference")
if value.get("schema") != "tohseno.local-workspace/1" or not isinstance(reference, str) or not reference.startswith("workspace-seed:workspace_"):
    raise SystemExit(1)
print(reference, end="")
' "$workspace_record")" || fail "the real service identity record did not verify"
cp "$workspace_record" "$temporary_root/workspace-before.json"

validate_runtime_and_health() {
  evidence_name="$1"
  [ -f "$runtime_path" ] && [ ! -L "$runtime_path" ] ||
    fail "the service runtime record is unavailable"
  service_origin="$(python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
origin = value.get("origin")
if value.get("schema") != "tohseno.local-workspace-runtime/1" or not isinstance(origin, str) or not origin.startswith("http://127.0.0.1:"):
    raise SystemExit(1)
print(origin, end="")
' "$runtime_path")" || fail "the service runtime origin did not verify"
  curl -fsS --max-time 2 "$service_origin/api/v1/health" \
    >"$temporary_root/$evidence_name-health.json" ||
    fail "the real Local Workspace Service is not healthy"
  python3 -c '
import json, sys
runtime = json.load(open(sys.argv[1], encoding="utf-8"))
health = json.load(open(sys.argv[2], encoding="utf-8"))
pid = int(open(sys.argv[3], encoding="ascii").read().strip())
assert health.get("schema") == "tohseno.local-workspace-health/1"
assert health.get("status") == "healthy"
assert health.get("service_version") == "0.9.9"
for field in ("workspace_id", "studio_device_id", "origin", "instance_id", "service_version"):
    assert health.get(field) == runtime.get(field)
assert runtime.get("process_id") == pid
' "$runtime_path" "$temporary_root/$evidence_name-health.json" "$pid_path" ||
    fail "service health did not match runtime and fake-launchctl PID identity"
}

validate_runtime_and_health install
install_instance="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["instance_id"], end="")' "$runtime_path")"
install_workspace="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["workspace_id"], end="")' "$runtime_path")"
run_tohseno --json service status >"$temporary_root/status.json"
python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
assert value.get("schema") == "tohseno.service-status/1"
assert value.get("installed") is True
assert value.get("launchd_loaded") is True
assert value.get("healthy") is True
assert value.get("service_version") == "0.9.9"
' "$temporary_root/status.json" || fail "service status did not verify launchd and health"

printf '%s\n' preserve >"$install_root/service/private-state-preserved"
mkdir -p "$install_root/service/devices"
printf '%s\n' preserve >"$install_root/service/devices/pairing-state-preserved"

run_tohseno --json service stop >"$temporary_root/stop.json"
[ ! -e "$pid_path" ] || fail "service stop retained fake launchctl PID state"
[ ! -e "$runtime_path" ] || fail "service stop retained a live runtime record"
if kill -0 "$install_pid" 2>/dev/null; then
  fail "service stop did not terminate the exact original process"
fi

run_tohseno --json service start >"$temporary_root/start.json"
validate_runtime_and_health start
start_pid="$(tr -d '\r\n' <"$pid_path")"
[ "$start_pid" != "$install_pid" ] || fail "service start reused the stopped process"

before_restart_instance="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["instance_id"], end="")' "$runtime_path")"
before_restart_pid="$start_pid"
run_tohseno --json service restart >"$temporary_root/restart.json"
validate_runtime_and_health restart
restart_pid="$(tr -d '\r\n' <"$pid_path")"
restart_instance="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["instance_id"], end="")' "$runtime_path")"
[ "$restart_pid" != "$before_restart_pid" ] || fail "service restart did not replace its process"
[ "$restart_instance" != "$before_restart_instance" ] ||
  fail "service restart did not publish a new runtime identity"
[ "$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["workspace_id"], end="")' "$runtime_path")" = "$install_workspace" ] ||
  fail "service restart changed the durable workspace identity"

ln -s 'releases/release-b' "$install_root/.current.next"
python3 -c 'import os,sys; os.replace(sys.argv[1], sys.argv[2])' \
  "$install_root/.current.next" "$install_root/current"
[ "$(readlink "$install_root/current")" = 'releases/release-b' ] ||
  fail "the isolated release pointer did not switch atomically"
run_tohseno --json service restart >"$temporary_root/pointer-restart.json"
validate_runtime_and_health pointer-restart
[ "$(tail -n 1 "$install_root/launcher-selections.log")" = 'release-b' ] ||
  fail "service restart did not resolve the new current release"
grep -Fqx 'release-a' "$install_root/launcher-selections.log" ||
  fail "the original current release was never launched"

run_tohseno --json service logs >"$temporary_root/logs.json"
python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
assert value.get("schema") == "tohseno.service-log-tail/1"
assert isinstance(value.get("lines"), list)
assert len(value["lines"]) <= 200
' "$temporary_root/logs.json" || fail "bounded service logs did not return stable JSON"

final_pid="$(tr -d '\r\n' <"$pid_path")"
run_tohseno --json service uninstall >"$temporary_root/uninstall.json"
python3 -c '
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
assert value.get("schema") == "tohseno.service-admin-receipt/1"
assert value.get("operation") == "uninstall"
assert value.get("installed") is False
assert value.get("state_preserved") is True
' "$temporary_root/uninstall.json" || fail "service uninstall receipt is invalid"
[ ! -e "$launch_agent" ] && [ ! -L "$launch_agent" ] ||
  fail "service uninstall retained the owned LaunchAgent"
[ -f "$launch_agents/com.example.keep.plist" ] ||
  fail "service uninstall removed an unowned neighboring LaunchAgent"
[ ! -e "$pid_path" ] || fail "service uninstall retained fake launchctl PID state"
if kill -0 "$final_pid" 2>/dev/null; then
  fail "service uninstall did not stop the real service"
fi
cmp "$workspace_record" "$temporary_root/workspace-before.json" >/dev/null ||
  fail "service lifecycle changed the durable workspace identity record"
grep -Fqx preserve "$install_root/service/private-state-preserved" ||
  fail "service uninstall removed private service state"
grep -Fqx preserve "$install_root/service/devices/pairing-state-preserved" ||
  fail "service uninstall removed pairing state"
grep -Fqx 'app data survives service administration' \
  "$temporary_root/shots/ordinary-app/source.txt" ||
  fail "service uninstall changed app data"
[ -x "$install_root/bin/tohseno" ] && [ -L "$install_root/current" ] ||
  fail "service uninstall removed program release state"

printf '%s\n' "real isolated macOS service install, health, restart, pointer switch, and uninstall passed"
