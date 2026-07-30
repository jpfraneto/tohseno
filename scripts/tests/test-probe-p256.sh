#!/bin/sh
set -eu

for tool in python3 jq cmp grep mktemp kill sed; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'test-probe-p256.sh: required tool is missing: %s\n' "$tool" >&2
    exit 1
  fi
done

script_directory="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_directory/../.." && pwd)"
probe="$repository_root/scripts/probe-p256.sh"
strict="$repository_root/scripts/strict-jsonrpc.py"
stub="$repository_root/scripts/tests/p256-rpc-stub.py"
temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-p256-probe-test.XXXXXX")"
server_pid=""

cleanup() {
  if [ -n "$server_pid" ]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  case "$temporary_directory" in
    "${TMPDIR:-/tmp}"/tohseno-p256-probe-test.*)
      rm -rf -- "$temporary_directory"
      ;;
    *)
      printf '%s\n' "test-probe-p256.sh: refusing unsafe temporary cleanup" >&2
      ;;
  esac
}
trap cleanup EXIT HUP INT TERM

start_server() {
  scenario="$1"
  port_file="$temporary_directory/$scenario.port"
  server_log="$temporary_directory/$scenario.server.log"
  python3 "$stub" \
    --scenario "$scenario" \
    --port-file "$port_file" \
    >"$server_log" 2>&1 &
  server_pid="$!"

  attempts=0
  while [ ! -s "$port_file" ] && [ "$attempts" -lt 100 ]; do
    if ! kill -0 "$server_pid" 2>/dev/null; then
      printf 'test-probe-p256.sh: stub exited for scenario %s\n' "$scenario" >&2
      sed -n '1,120p' "$server_log" >&2
      exit 1
    fi
    attempts=$((attempts + 1))
    sleep 0.05
  done
  if [ ! -s "$port_file" ]; then
    printf 'test-probe-p256.sh: stub did not become ready for scenario %s\n' "$scenario" >&2
    exit 1
  fi
  rpc_url="http://127.0.0.1:$(sed -n '1p' "$port_file")"
}

stop_server() {
  if [ -n "$server_pid" ]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
    server_pid=""
  fi
}

run_success() {
  scenario="$1"
  stdout_file="$temporary_directory/$scenario.stdout"
  stderr_file="$temporary_directory/$scenario.stderr"
  evidence_file="$temporary_directory/$scenario.evidence.json"
  start_server "$scenario"
  if ! "$probe" \
    --rpc-url "$rpc_url" \
    --output "$evidence_file" \
    >"$stdout_file" 2>"$stderr_file"; then
    stop_server
    printf 'test-probe-p256.sh: expected scenario %s to pass\n' "$scenario" >&2
    sed -n '1,120p' "$stderr_file" >&2
    exit 1
  fi
  stop_server

  cmp "$stdout_file" "$evidence_file"
  jq -e '
    .schema == "tohseno.p256-probe/2"
    and .chain_id == 4663
    and .rpc == "explicit-target"
    and .block.source_tag == "latest"
    and .block.require_canonical == true
    and .block.rechecked_after_probe == true
    and .vectors.positive.output
      == "0x0000000000000000000000000000000000000000000000000000000000000001"
    and .vectors.negative.output == "0x"
    and .vectors.infinity.output == "0x"
    and .gas.meter_overhead == 157
    and .gas.measured_total == 7057
    and .gas.measured_precompile == 6900
    and .valid == true
  ' "$evidence_file" >/dev/null

  if "$probe" \
    --rpc-url "http://127.0.0.1:1" \
    --output "$evidence_file" \
    >"$temporary_directory/overwrite.stdout" \
    2>"$temporary_directory/overwrite.stderr"; then
    printf '%s\n' "test-probe-p256.sh: probe overwrote existing evidence" >&2
    exit 1
  fi
  grep -F "refusing to overwrite" "$temporary_directory/overwrite.stderr" >/dev/null
}

run_failure() {
  scenario="$1"
  expected_message="$2"
  stdout_file="$temporary_directory/$scenario.stdout"
  stderr_file="$temporary_directory/$scenario.stderr"
  evidence_file="$temporary_directory/$scenario.evidence.json"
  start_server "$scenario"
  if "$probe" \
    --rpc-url "$rpc_url" \
    --output "$evidence_file" \
    >"$stdout_file" 2>"$stderr_file"; then
    stop_server
    printf 'test-probe-p256.sh: expected scenario %s to fail\n' "$scenario" >&2
    exit 1
  fi
  stop_server
  test ! -s "$stdout_file"
  test ! -e "$evidence_file"
  grep -F "$expected_message" "$stderr_file" >/dev/null
}

strict_failure() {
  name="$1"
  payload="$2"
  expected_message="$3"
  stdout_file="$temporary_directory/strict-$name.stdout"
  stderr_file="$temporary_directory/strict-$name.stderr"
  if printf '%s' "$payload" \
    | python3 "$strict" --expected-id 1 >"$stdout_file" 2>"$stderr_file"; then
    printf 'test-probe-p256.sh: strict parser accepted %s\n' "$name" >&2
    exit 1
  fi
  test ! -s "$stdout_file"
  grep -F "$expected_message" "$stderr_file" >/dev/null
}

if "$probe" >"$temporary_directory/missing-rpc.stdout" 2>"$temporary_directory/missing-rpc.stderr"; then
  printf '%s\n' "test-probe-p256.sh: probe accepted a missing explicit RPC" >&2
  exit 1
fi
grep -F -- "--rpc-url is required" "$temporary_directory/missing-rpc.stderr" >/dev/null

if "$probe" \
  --rpc-url "http://example.invalid" \
  >"$temporary_directory/plaintext.stdout" \
  2>"$temporary_directory/plaintext.stderr"; then
  printf '%s\n' "test-probe-p256.sh: probe accepted a remote plaintext RPC" >&2
  exit 1
fi
grep -F "remote RPC URLs must use https" "$temporary_directory/plaintext.stderr" >/dev/null

strict_failure duplicate-id \
  '{"jsonrpc":"2.0","id":1,"id":1,"result":"0x"}' \
  "duplicate JSON member: id"
strict_failure duplicate-result-valid-last \
  '{"jsonrpc":"2.0","id":1,"result":"0x00","result":"0x"}' \
  "duplicate JSON member: result"
strict_failure duplicate-result-valid-first \
  '{"jsonrpc":"2.0","id":1,"result":"0x","result":"0x00"}' \
  "duplicate JSON member: result"
strict_failure duplicate-nested \
  '{"jsonrpc":"2.0","id":1,"result":{"hash":"0xaa","hash":"0xaa"}}' \
  "duplicate JSON member: hash"
strict_failure wrong-id \
  '{"jsonrpc":"2.0","id":2,"result":"0x"}' \
  "response id did not match request"
strict_failure bool-id \
  '{"jsonrpc":"2.0","id":true,"result":"0x"}' \
  "response id did not match request"
strict_failure string-id \
  '{"jsonrpc":"2.0","id":"1","result":"0x"}' \
  "response id did not match request"
strict_failure missing-id \
  '{"jsonrpc":"2.0","result":"0x"}' \
  "success response must contain exactly"
strict_failure wrong-jsonrpc \
  '{"jsonrpc":"1.0","id":1,"result":"0x"}' \
  "jsonrpc must equal 2.0"
strict_failure missing-jsonrpc \
  '{"id":1,"result":"0x"}' \
  "success response must contain exactly"
strict_failure result-and-error \
  '{"jsonrpc":"2.0","id":1,"result":"0x","error":null}' \
  "success response must contain exactly"
strict_failure batch \
  '[{"jsonrpc":"2.0","id":1,"result":"0x"}]' \
  "response must be one object"
strict_failure truncated \
  '{"jsonrpc":"2.0","id":1,"result":' \
  "Expecting value"
strict_failure trailing \
  '{"jsonrpc":"2.0","id":1,"result":"0x"} trailing' \
  "Extra data"
strict_failure nan \
  '{"jsonrpc":"2.0","id":1,"result":NaN}' \
  "non-standard JSON number: NaN"

run_success pass
run_failure http-error "eth_chainId transport failed"
run_failure wrong-chain "refusing chain"
run_failure null-block "latest block result was malformed"
run_failure short-block-hash "latest block hash was not 32 bytes"
run_failure wrong-positive "positive vector returned"
run_failure zero-positive "positive vector returned"
run_failure short-positive "positive vector returned"
run_failure malformed-result "positive vector returned a non-string result"
run_failure duplicate-result "duplicate JSON member: result"
run_failure duplicate-block-member "duplicate JSON member: hash"
run_failure nonempty-negative "negative vector returned"
run_failure nonempty-infinity "infinity vector returned"
run_failure legacy-gas "expected exact total 7057 (6900 + 157)"
run_failure off-by-one-gas "expected exact total 7057 (6900 + 157)"
run_failure short-gas-word "expected exact total 7057 (6900 + 157)"
run_failure nonhex-gas-word "expected exact total 7057 (6900 + 157)"
run_failure nonempty-meter "gas meter address is not empty"
run_failure no-state-override "positive gas meter returned an error, duplicate member, or malformed JSON-RPC result"
run_failure reorg-after-probe "pinned latest block was no longer canonical after the probe"

printf '%s\n' "P256 actual-RPC deployment gate stub tests passed."
