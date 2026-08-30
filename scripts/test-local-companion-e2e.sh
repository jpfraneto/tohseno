#!/bin/sh
set -eu
umask 077

script_name="test-local-companion-e2e.sh"
repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
temporary_parent="$(CDPATH= cd -- "${TMPDIR:-/tmp}" && pwd -P)"
temporary_root="$(mktemp -d "$temporary_parent/tohseno-local-companion-test.XXXXXX")"

fail() {
  printf '%s: %s\n' "$script_name" "$1" >&2
  exit 1
}

cleanup() {
  cleanup_status=$?
  trap - EXIT HUP INT TERM
  case "$temporary_root" in
    "$temporary_parent"/tohseno-local-companion-test.*)
      if [ -d "$temporary_root" ] && [ ! -L "$temporary_root" ]; then
        rm -rf -- "$temporary_root"
      fi
      ;;
    *)
      printf '%s: refusing unsafe cleanup\n' "$script_name" >&2
      cleanup_status=1
      ;;
  esac
  exit "$cleanup_status"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

sh -n "$repository_root/scripts/local-companion-e2e.sh"
(cd "$repository_root" && cargo build --locked -p tohseno >/dev/null)
TOHSENO_CANDIDATE_BIN="$repository_root/target/debug/tohseno" \
  "$repository_root/scripts/local-companion-e2e.sh" \
  >"$temporary_root/result.json"

JSON_PATH="$temporary_root/result.json" bun -e '
  const value = await Bun.file(process.env.JSON_PATH).json();
  if (value.schema !== "tohseno.local-companion-e2e/1" ||
      value.service_version !== "1.1.0" ||
      value.relay_healthy !== true ||
      value.service_healthy !== true ||
      value.paired_devices !== 1 ||
      value.revoked_devices !== 1 ||
      value.encrypted_snapshot_received !== true ||
      value.invitation_verified !== true ||
      value.pairing_response_duplicate !== true ||
      value.real_relay_commands !== true ||
      value.feedback_exact_version !== true ||
      value.duplicate_delivery_exactly_once !== true ||
      value.offline_outbox_relaunch !== true ||
      value.marketing_note_recorded !== true ||
      value.executions_accepted !== 2 ||
      value.post_revocation_rejected !== true ||
      !Number.isInteger(value.shot_count)) process.exit(1);
' || fail "the local companion flow returned an invalid receipt"

printf '%s\n' "local Companion Relay, Workspace Service, and simulator smoke passed"
