#!/bin/sh
set -eu
umask 077

script_name="local-companion-e2e.sh"
repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
. "$repository_root/scripts/fixtures/verification-keychain.sh"
temporary_parent="$(CDPATH= cd -- "${TMPDIR:-/tmp}" && pwd -P)"
temporary_root="$(mktemp -d "$temporary_parent/tohseno-local-companion.XXXXXX")"
relay_pid=""
service_pid=""
workspace_secret_reference=""
simulator_secret_reference=""
workspace_record=""
simulation_record=""
builder_key_tag=""
identity_helper=""

fail() {
  printf '%s: %s\n' "$script_name" "$1" >&2
  exit 1
}

stop_child() {
  child_pid="$1"
  if [ -n "$child_pid" ] && kill -0 "$child_pid" 2>/dev/null; then
    kill "$child_pid" 2>/dev/null || true
    attempts=0
    while kill -0 "$child_pid" 2>/dev/null && [ "$attempts" -lt 50 ]; do
      sleep 0.1
      attempts=$((attempts + 1))
    done
    if kill -0 "$child_pid" 2>/dev/null; then
      kill -KILL "$child_pid" 2>/dev/null || true
    fi
  fi
  if [ -n "$child_pid" ]; then
    wait "$child_pid" 2>/dev/null || true
  fi
}

delete_test_secret() {
  secret_reference="$1"
  case "$secret_reference" in
    workspace-seed:workspace_*|simulator-phrase:device_*)
      /usr/bin/security delete-generic-password \
        -a "$secret_reference" \
        -s "$verification_keychain_service" \
        "$verification_keychain_path" \
        >/dev/null 2>&1
      ;;
    "") return 0 ;;
    *)
      printf '%s: refusing to delete an unexpected Keychain reference\n' "$script_name" >&2
      return 1
      ;;
  esac
}

cleanup() {
  cleanup_status=$?
  trap - EXIT HUP INT TERM
  stop_child "$service_pid" 2>/dev/null
  stop_child "$relay_pid" 2>/dev/null
  if [ -z "$simulator_secret_reference" ] &&
    [ -n "$simulation_record" ] &&
    [ -f "$simulation_record" ] &&
    [ ! -L "$simulation_record" ] &&
    [ "$(wc -c <"$simulation_record")" -le 65536 ]; then
    recovered_device_id="$(sed -n 's/.*"device_id":"\([^"]*\)".*/\1/p' "$simulation_record" | head -n 1)"
    case "$recovered_device_id" in
      device_*) simulator_secret_reference="simulator-phrase:$recovered_device_id" ;;
    esac
  fi
  if [ -z "$workspace_secret_reference" ] &&
    [ -n "$workspace_record" ] &&
    [ -f "$workspace_record" ] &&
    [ ! -L "$workspace_record" ] &&
    [ "$(wc -c <"$workspace_record")" -le 65536 ]; then
    recovered_workspace_reference="$(sed -n 's/.*"secret_reference":"\([^"]*\)".*/\1/p' "$workspace_record" | head -n 1)"
    case "$recovered_workspace_reference" in
      workspace-seed:workspace_*) workspace_secret_reference="$recovered_workspace_reference" ;;
    esac
  fi
  delete_test_secret "$simulator_secret_reference" || cleanup_status=1
  delete_test_secret "$workspace_secret_reference" || cleanup_status=1
  case "$builder_key_tag" in
    org.tohseno.builder.device.*)
      if [ -x "$identity_helper" ]; then
        TOHSENO_VERIFICATION_MODE=1 \
          TOHSENO_VERIFICATION_KEYCHAIN_PATH="$verification_keychain_path" \
          "$identity_helper" delete --tag "$builder_key_tag" \
          >"$temporary_root/builder-delete.json" 2>/dev/null || cleanup_status=1
      else
        cleanup_status=1
      fi
      ;;
    "")
      builder_record="$temporary_root/data/identity/builder.json"
      if [ -f "$builder_record" ] && [ ! -L "$builder_record" ] &&
        [ "$(wc -c <"$builder_record")" -le 65536 ]; then
        recovered_builder_tag="$(sed -n 's/.*"local_key_tag":"\([^"]*\)".*/\1/p' "$builder_record" | head -n 1)"
        case "$recovered_builder_tag" in
          org.tohseno.builder.device.*)
            if [ -x "$identity_helper" ]; then
              TOHSENO_VERIFICATION_MODE=1 \
                TOHSENO_VERIFICATION_KEYCHAIN_PATH="$verification_keychain_path" \
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
  delete_tohseno_verification_keychain "$temporary_root" || cleanup_status=1
  if [ "${TOHSENO_LOCAL_COMPANION_KEEP_ARTIFACTS:-0}" = "1" ]; then
    printf '%s: preserving isolated artifacts at %s\n' "$script_name" "$temporary_root" >&2
    exit "$cleanup_status"
  fi
  case "$temporary_root" in
    "$temporary_parent"/tohseno-local-companion.*)
      if [ -d "$temporary_root" ] && [ ! -L "$temporary_root" ]; then
        rm -rf -- "$temporary_root"
      fi
      ;;
    *)
      printf '%s: refusing unsafe temporary cleanup\n' "$script_name" >&2
      cleanup_status=1
      ;;
  esac
  exit "$cleanup_status"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

for dependency in cargo bun curl python3 swift xcodebuild xcrun; do
  command -v "$dependency" >/dev/null 2>&1 || fail "$dependency is unavailable"
done
test "$(uname -s)" = "Darwin" || fail "the Local Workspace Service requires macOS"
test -x /usr/bin/security || fail "macOS Keychain tooling is unavailable"
create_tohseno_verification_keychain "$temporary_root" "local-companion" ||
  fail "an isolated verification Keychain could not be created"

if [ -n "${TOHSENO_LOCAL_COMPANION_RELAY_PORT:-}" ]; then
  relay_port="$TOHSENO_LOCAL_COMPANION_RELAY_PORT"
else
  relay_port="$(python3 -c '
import socket
with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
    listener.bind(("127.0.0.1", 0))
    print(listener.getsockname()[1], end="")
')" || fail "an ephemeral loopback relay port could not be selected"
fi
case "$relay_port" in
  ""|*[!0-9]*) fail "TOHSENO_LOCAL_COMPANION_RELAY_PORT must be a decimal port" ;;
esac
if [ "$relay_port" -lt 1024 ] || [ "$relay_port" -gt 65535 ]; then
  fail "TOHSENO_LOCAL_COMPANION_RELAY_PORT must be between 1024 and 65535"
fi
relay_origin="http://127.0.0.1:$relay_port"

if [ -n "${TOHSENO_CANDIDATE_BIN:-}" ]; then
  binary="$TOHSENO_CANDIDATE_BIN"
else
  (cd "$repository_root" && cargo build --locked -p tohseno >/dev/null)
  binary="$repository_root/target/debug/tohseno"
fi
(cd "$repository_root" && swift build --package-path apple-identity >/dev/null)
identity_helper="$repository_root/apple-identity/.build/debug/tohseno-apple-identity"
test -f "$binary" && test ! -L "$binary" && test -x "$binary" ||
  fail "TOHSENO_CANDIDATE_BIN is not a regular executable"
test -f "$identity_helper" && test ! -L "$identity_helper" && test -x "$identity_helper" ||
  fail "the Apple identity helper is unavailable"
fixture_harness="$repository_root/scripts/fixtures/factory-harness.sh"
test -f "$fixture_harness" && test ! -L "$fixture_harness" && test -x "$fixture_harness" ||
  fail "the deterministic factory harness is unavailable"
binary_directory="$(CDPATH= cd -- "$(dirname -- "$binary")" && pwd -P)"
binary="$binary_directory/$(basename -- "$binary")"
test "$("$binary" --version)" = "tohseno 1.2.0" ||
  fail "the local companion flow requires a TOHSENO 1.2.0 binary"

install_root="$temporary_root/install"
data_root="$temporary_root/data"
shots_root="$temporary_root/shots"
relay_root="$temporary_root/relay"
mkdir -p "$install_root" "$data_root" "$shots_root" "$relay_root"
workspace_record="$install_root/service/workspace.json"
simulation_record="$temporary_root/simulation.json"

run_tohseno() {
  env \
    TOHSENO_INSTALL_ROOT="$install_root" \
    TOHSENO_DATA_ROOT="$data_root" \
    TOHSENO_HOME="$shots_root" \
    TOHSENO_COMPANION_RELAY_ORIGIN="$relay_origin" \
    TOHSENO_APPLE_IDENTITY_HELPER="$identity_helper" \
    TOHSENO_IDENTITY_BACKEND=software-test \
    TOHSENO_VERIFICATION_MODE=1 \
    TOHSENO_VERIFICATION_KEYCHAIN_PATH="$verification_keychain_path" \
    TOHSENO_VERIFICATION_KEYCHAIN_SERVICE="$verification_keychain_service" \
    TOHSENO_VERIFICATION_SERVICE_LABEL="$verification_service_label" \
    TOHSENO_TEST_FACTORY_HARNESS="$fixture_harness" \
    TOHSENO_TEST_FACTORY_NO_DEVICE=1 \
    "$binary" "$@"
}

(
  cd "$repository_root/website"
  exec env \
    NODE_ENV=development \
    BASE_URL="$relay_origin" \
    HOST=127.0.0.1 \
    PORT="$relay_port" \
    COMPANION_RELAY_ENABLED=true \
    COMPANION_RELAY_ROOT="$relay_root" \
    COMPANION_RELAY_PUSH_MODE=noop \
    COMPANION_RELAY_SOURCE_RATE=1000 \
    COMPANION_RELAY_GLOBAL_RATE=5000 \
    bun apps/companion-relay/server.ts
) >"$temporary_root/relay.log" 2>&1 &
relay_pid=$!

attempts=0
until curl -fsS --max-time 1 "$relay_origin/healthz" \
  >"$temporary_root/relay-health.json" 2>/dev/null; do
  kill -0 "$relay_pid" 2>/dev/null || fail "the local Companion Relay exited before health"
  attempts=$((attempts + 1))
  [ "$attempts" -lt 150 ] || fail "the local Companion Relay did not become healthy"
  sleep 0.1
done
kill -0 "$relay_pid" 2>/dev/null || fail "the local Companion Relay did not own the healthy origin"

JSON_PATH="$temporary_root/relay-health.json" bun -e '
  const value = await Bun.file(process.env.JSON_PATH).json();
  if (value.schema !== "tohseno.companion-relay-health/1" || value.ready !== true) process.exit(1);
' || fail "the Companion Relay health response did not verify"

env \
  TOHSENO_INSTALL_ROOT="$install_root" \
  TOHSENO_DATA_ROOT="$data_root" \
  TOHSENO_HOME="$shots_root" \
  TOHSENO_SERVICE_PORT=0 \
  TOHSENO_COMPANION_RELAY_ORIGIN="$relay_origin" \
  TOHSENO_APPLE_IDENTITY_HELPER="$identity_helper" \
  TOHSENO_IDENTITY_BACKEND=software-test \
  TOHSENO_VERIFICATION_MODE=1 \
  TOHSENO_VERIFICATION_KEYCHAIN_PATH="$verification_keychain_path" \
  TOHSENO_VERIFICATION_KEYCHAIN_SERVICE="$verification_keychain_service" \
  TOHSENO_VERIFICATION_SERVICE_LABEL="$verification_service_label" \
  TOHSENO_TEST_FACTORY_HARNESS="$fixture_harness" \
  TOHSENO_TEST_FACTORY_NO_DEVICE=1 \
  TOHSENO_DEVELOPMENT_ENTITLEMENT=1 \
  "$binary" service run \
  >"$temporary_root/service.log" 2>&1 &
service_pid=$!

runtime_path="$install_root/service/runtime.json"
attempts=0
while [ ! -f "$runtime_path" ]; do
  kill -0 "$service_pid" 2>/dev/null || fail "the Local Workspace Service exited before health"
  attempts=$((attempts + 1))
  [ "$attempts" -lt 150 ] || fail "the Local Workspace Service did not publish runtime identity"
  sleep 0.1
done

service_origin="$(JSON_PATH="$runtime_path" bun -e '
  const value = await Bun.file(process.env.JSON_PATH).json();
  if (value.schema !== "tohseno.local-workspace-runtime/1" ||
      typeof value.origin !== "string" ||
      !value.origin.startsWith("http://127.0.0.1:")) process.exit(1);
  process.stdout.write(value.origin);
')" || fail "the Local Workspace Service runtime did not verify"

attempts=0
until curl -fsS --max-time 1 "$service_origin/api/v1/health" \
  >"$temporary_root/service-health.json" 2>/dev/null; do
  kill -0 "$service_pid" 2>/dev/null || fail "the Local Workspace Service exited before health"
  attempts=$((attempts + 1))
  [ "$attempts" -lt 150 ] || fail "the Local Workspace Service did not become healthy"
  sleep 0.1
done

RUNTIME_PATH="$runtime_path" HEALTH_PATH="$temporary_root/service-health.json" bun -e '
  const runtime = await Bun.file(process.env.RUNTIME_PATH).json();
  const health = await Bun.file(process.env.HEALTH_PATH).json();
  if (health.schema !== "tohseno.local-workspace-health/1" ||
      health.status !== "healthy" ||
      health.origin !== runtime.origin ||
      health.workspace_id !== runtime.workspace_id ||
      health.studio_device_id !== runtime.studio_device_id ||
      health.instance_id !== runtime.instance_id ||
      health.service_version !== "1.2.0") process.exit(1);
' || fail "the Local Workspace Service health identity did not verify"

workspace_secret_reference="$(JSON_PATH="$workspace_record" bun -e '
  const value = await Bun.file(process.env.JSON_PATH).json();
  if (value.schema !== "tohseno.local-workspace/1" ||
      typeof value.secret_reference !== "string" ||
      !value.secret_reference.startsWith("workspace-seed:workspace_")) process.exit(1);
  process.stdout.write(value.secret_reference);
')" || fail "the isolated workspace identity record did not verify"

factory_intention="$temporary_root/factory-intention.md"
printf '%s\n' \
  '# Coherent Intention' \
  '' \
  'Show a complete, quiet native iPhone expression with a clear visible identity.' \
  'The primary screen must state that it is a TOHSENO expression and that the Apple materialization gates passed.' \
  >"$factory_intention"
run_tohseno --json create companionfixture \
  --prompt-file "$factory_intention" --wait \
  >"$temporary_root/factory-create.json"
FACTORY_PATH="$temporary_root/factory-create.json" bun -e '
  const value = await Bun.file(process.env.FACTORY_PATH).json();
  if (value.schema !== "tohseno.create-command-result/1" ||
      value.execution?.accepted !== true ||
      value.execution?.state !== "accepted" ||
      value.execution?.version_ordinal !== 1) process.exit(1);
' || fail "the deterministic companion fixture was not accepted"
builder_key_tag="$(JSON_PATH="$data_root/identity/builder.json" bun -e '
  const value = await Bun.file(process.env.JSON_PATH).json();
  if (typeof value.local_key_tag !== "string" ||
      !value.local_key_tag.startsWith("org.tohseno.builder.device.")) process.exit(1);
  process.stdout.write(value.local_key_tag);
')" || fail "the isolated Builder identity did not verify"

run_tohseno --json companion simulate pair \
  >"$simulation_record"

simulator_device_id="$(JSON_PATH="$simulation_record" bun -e '
  const value = await Bun.file(process.env.JSON_PATH).json();
  if (value.schema !== "tohseno.companion-simulation/1" ||
      value.operation !== "pair" ||
      typeof value.device_id !== "string" ||
      !value.device_id.startsWith("device_") ||
      typeof value.capability_id !== "string" ||
      value.identity_storage !== "macos_keychain" ||
      value.state_storage !== "chacha20poly1305_encrypted" ||
      value.invitation_verified !== true ||
      value.real_relay_pairing !== true ||
      value.pairing_response_duplicate !== true ||
      value.mailbox_acknowledged !== true ||
      value.encrypted_snapshot_received !== true ||
      !Number.isInteger(value.shot_count)) process.exit(1);
  process.stdout.write(value.device_id);
')" || fail "the simulated iPhone pairing receipt did not verify"
simulator_secret_reference="simulator-phrase:$simulator_device_id"
simulator_state="$install_root/service/simulator/$simulator_device_id.json"
[ -f "$simulator_state" ] && [ ! -L "$simulator_state" ] && \
  [ "$(wc -c <"$simulator_state")" -le 25165824 ] ||
  fail "the simulator did not persist bounded encrypted state"

run_tohseno --json companion status \
  >"$temporary_root/companion-status.json"

STATUS_PATH="$temporary_root/companion-status.json" SIMULATION_PATH="$simulation_record" bun -e '
  const status = await Bun.file(process.env.STATUS_PATH).json();
  const simulation = await Bun.file(process.env.SIMULATION_PATH).json();
  if (status.schema !== "tohseno.companion-status/1" ||
      status.relay_connection !== "ready" ||
      status.paired_devices !== 1 ||
      status.revoked_devices !== 0 ||
      status.workspace_id !== simulation.workspace_id) process.exit(1);
' || fail "the paired companion state did not verify"

run_tohseno --json companion simulate exercise "$simulator_device_id" \
  >"$temporary_root/exercise.json"
EXERCISE_PATH="$temporary_root/exercise.json" bun -e '
  const value = await Bun.file(process.env.EXERCISE_PATH).json();
  if (value.schema !== "tohseno.companion-simulation/1" ||
      value.operation !== "exercise" ||
      value.real_relay_commands !== true ||
      value.feedback_exact_version !== true ||
      value.duplicate_delivery_exactly_once !== true ||
      value.offline_outbox_encrypted !== true ||
      value.offline_outbox_relaunch !== true ||
      value.marketing_note_recorded !== true ||
      value.executions_accepted !== 2 ||
      value.receipts_acknowledged !== 4 ||
      value.device_revoked !== true ||
      value.post_revocation_rejected !== true ||
      typeof value.evolution_execution_id !== "string" ||
      typeof value.creation_execution_id !== "string") process.exit(1);
' || fail "the real relay-backed companion exercise did not verify"

run_tohseno --json companion status >"$temporary_root/revoked-status.json"
STATUS_PATH="$temporary_root/revoked-status.json" bun -e '
  const status = await Bun.file(process.env.STATUS_PATH).json();
  if (status.paired_devices !== 0 || status.revoked_devices !== 1) process.exit(1);
' || fail "the Local Workspace Service did not retain companion revocation"

for private_text in \
  'Simulator feedback bound to this exact accepted Version.' \
  'Simulator private marketing note; never relay plaintext.' \
  'Keep the exact Shot and accepted Genome while making continuity into Version 0002 visible.' \
  'Show a complete, quiet native iPhone expression with a clear visible identity.'; do
  if grep -Fq "$private_text" "$temporary_root/relay.log" "$temporary_root/service.log"; then
    fail "operational logs exposed private companion content"
  fi
  if grep -Fq "$private_text" "$simulator_state"; then
    fail "the simulator persisted plaintext private companion content"
  fi
done

STATUS_PATH="$temporary_root/revoked-status.json" \
  PAIR_PATH="$simulation_record" \
  EXERCISE_PATH="$temporary_root/exercise.json" bun -e '
  const status = await Bun.file(process.env.STATUS_PATH).json();
  const pair = await Bun.file(process.env.PAIR_PATH).json();
  const exercise = await Bun.file(process.env.EXERCISE_PATH).json();
  console.log(JSON.stringify({
    schema: "tohseno.local-companion-e2e/1",
    service_version: "1.2.0",
    relay_healthy: true,
    service_healthy: true,
    paired_devices: 1,
    revoked_devices: status.revoked_devices,
    encrypted_snapshot_received: pair.encrypted_snapshot_received,
    invitation_verified: pair.invitation_verified,
    pairing_response_duplicate: pair.pairing_response_duplicate,
    real_relay_commands: exercise.real_relay_commands,
    feedback_exact_version: exercise.feedback_exact_version,
    duplicate_delivery_exactly_once: exercise.duplicate_delivery_exactly_once,
    offline_outbox_relaunch: exercise.offline_outbox_relaunch,
    marketing_note_recorded: exercise.marketing_note_recorded,
    executions_accepted: exercise.executions_accepted,
    post_revocation_rejected: exercise.post_revocation_rejected,
    shot_count: pair.shot_count,
  }));
' || fail "the final companion E2E receipt could not be produced"
