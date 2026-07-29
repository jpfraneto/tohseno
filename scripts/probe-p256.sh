#!/bin/sh
set -eu

# Official, rate-limited public endpoint documented by Robinhood:
# https://docs.robinhood.com/chain/connecting/
default_rpc_url="https://rpc.mainnet.chain.robinhood.com"
rpc_url="${ROBINHOOD_RPC_URL:-$default_rpc_url}"
output_path=""

usage() {
  printf '%s\n' "usage: probe-p256.sh [--rpc-url URL] [--output PATH]"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --rpc-url)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        printf '%s\n' "probe-p256.sh: --rpc-url requires a value" >&2
        exit 2
      fi
      rpc_url="$2"
      shift 2
      ;;
    --output)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        printf '%s\n' "probe-p256.sh: --output requires a value" >&2
        exit 2
      fi
      output_path="$2"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      printf 'probe-p256.sh: unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

for tool in curl jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'probe-p256.sh: required tool is missing: %s\n' "$tool" >&2
    exit 1
  fi
done

rpc() {
  curl \
    --fail \
    --silent \
    --show-error \
    --connect-timeout 10 \
    --max-time 30 \
    -H "Content-Type: application/json" \
    --data "$1" \
    "$rpc_url"
}

chain_response="$(rpc '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}')"
if ! printf '%s' "$chain_response" | jq -e '.error == null and (.result | type == "string")' >/dev/null; then
  printf '%s\n' "probe-p256.sh: eth_chainId returned an error or malformed result" >&2
  exit 1
fi
chain_hex="$(printf '%s' "$chain_response" | jq -r '.result')"
if ! printf '%s' "$chain_hex" | LC_ALL=C grep -Eq '^0x[0-9a-fA-F]+$'; then
  printf '%s\n' "probe-p256.sh: eth_chainId was not hexadecimal" >&2
  exit 1
fi
chain_digits="$(
  printf '%s' "$chain_hex" \
    | tr 'A-F' 'a-f' \
    | sed -e 's/^0x//' -e 's/^0*//'
)"
if [ -z "$chain_digits" ]; then
  chain_digits="0"
fi
if [ "$chain_digits" != "1237" ]; then
  printf 'probe-p256.sh: refusing chain %s; expected Robinhood mainnet chain 4663\n' "$chain_hex" >&2
  exit 1
fi

# First valid EIP-7951 vector:
# https://eips.ethereum.org/assets/eip-7951/test-vectors.json
# Input is digest || r || low-s || publicKeyX || publicKeyY.
probe_input="0xbb5a52f42f9c9261ed4361f59422a1e30036e7c32b270c8807a419feca6050232ba3a8be6b94d5ec80a6d9d1190a436effe50d85a1eee859b8cc6af9bd5c2e184cd60b855d442f5b3c7b11eb6c4e0ae7525fe710fab9aa7c77a67f79e6fadd762927b10512bae3eddcfe467828128bad2903269919f7086069c8c4df6c732838c7787964eaac00e5921fb1498a60f4606766b3d9685001558d1a974e7341513e"
precompile="0x0000000000000000000000000000000000000100"
call_request="$(
  jq -cn \
    --arg to "$precompile" \
    --arg data "$probe_input" \
    '{jsonrpc:"2.0",id:2,method:"eth_call",params:[{to:$to,data:$data},"latest"]}'
)"
call_response="$(rpc "$call_request")"
if ! printf '%s' "$call_response" | jq -e '.error == null and (.result | type == "string")' >/dev/null; then
  printf '%s\n' "probe-p256.sh: P256VERIFY eth_call returned an error or malformed result" >&2
  exit 1
fi
observed="$(printf '%s' "$call_response" | jq -r '.result' | tr 'A-F' 'a-f')"
expected="0x0000000000000000000000000000000000000000000000000000000000000001"
if [ "$observed" != "$expected" ]; then
  printf 'probe-p256.sh: P256VERIFY returned %s; expected exact 32-byte integer 1\n' "$observed" >&2
  exit 1
fi

rpc_kind="configured"
if [ "$rpc_url" = "$default_rpc_url" ]; then
  rpc_kind="official-public"
fi
evidence="$(
  jq -cn \
    --arg checked_at "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
    --arg rpc_kind "$rpc_kind" \
    --arg precompile "$precompile" \
    --arg input "$probe_input" \
    --arg output "$observed" \
    '{
      schema:"tohseno.p256-probe/1",
      checked_at:$checked_at,
      chain_id:4663,
      rpc:$rpc_kind,
      precompile:$precompile,
      input:$input,
      output:$output,
      valid:true
    }'
)"

printf '%s\n' "$evidence"
if [ -n "$output_path" ]; then
  output_parent="$(dirname "$output_path")"
  if [ ! -d "$output_parent" ]; then
    printf 'probe-p256.sh: output directory does not exist: %s\n' "$output_parent" >&2
    exit 1
  fi
  if [ -d "$output_path" ]; then
    printf 'probe-p256.sh: output path is a directory: %s\n' "$output_path" >&2
    exit 1
  fi
  umask 022
  printf '%s\n' "$evidence" >"$output_path"
fi
