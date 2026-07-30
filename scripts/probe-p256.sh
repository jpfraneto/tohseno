#!/bin/sh
set -eu

rpc_url=""
output_path=""

usage() {
  printf '%s\n' "usage: probe-p256.sh --rpc-url URL [--output PATH]"
}

fail() {
  printf 'probe-p256.sh: %s\n' "$1" >&2
  exit 1
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

if [ -z "$rpc_url" ]; then
  printf '%s\n' "probe-p256.sh: --rpc-url is required; deployment evidence must name the actual target RPC" >&2
  exit 2
fi
case "$rpc_url" in
  https://* | http://127.0.0.1 | http://127.0.0.1:*) ;;
  *)
    fail "remote RPC URLs must use https; plaintext http is allowed only for literal 127.0.0.1 tests"
    ;;
esac
case "$rpc_url" in
  *"@"* | *"#"* | *[[:space:]]*)
    fail "RPC URL must not contain userinfo, fragments, or whitespace"
    ;;
esac

for tool in curl jq python3 shasum tr sed grep dirname date; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    fail "required tool is missing: $tool"
  fi
done

script_directory="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_directory/.." && pwd)"
vector_file="$repository_root/contracts/test-vectors/eip-7951.json"
strict_jsonrpc="$repository_root/scripts/strict-jsonrpc.py"
expected_vector_file_sha256="71fc0ab5e10c553093be6b47b6b9e3c0a59f0a523d1bdcd41d6b66a59aa12657"

if [ ! -f "$vector_file" ] || [ -L "$vector_file" ]; then
  fail "committed EIP-7951 vector fixture is missing or symlinked"
fi
if [ ! -f "$strict_jsonrpc" ] || [ -L "$strict_jsonrpc" ]; then
  fail "duplicate-key-strict JSON-RPC parser is missing or symlinked"
fi
observed_vector_file_sha256="$(shasum -a 256 "$vector_file" | sed 's/[[:space:]].*$//')"
if [ "$observed_vector_file_sha256" != "$expected_vector_file_sha256" ]; then
  fail "committed EIP-7951 vector fixture digest does not match the reviewed probe"
fi
if ! jq -e '
  .schema == "tohseno.eip7951-probe-vectors/1"
  and .source.sha256 == "6ed75dffbc0dd70defc7aceb7299df75dfb2a18184fbac229482c0a4b6601d6d"
  and .gas.normative_eip7951 == 6900
  and .gas.legacy_rip7212 == 3450
  and .gas.meter_overhead == 157
  and .gas.meter_total == 7057
  and ([.vectors[].case] == ["positive", "negative", "infinity"])
  and ([.vectors[].input | test("^0x[0-9a-f]{320}$")] | all)
  and .vectors[0].expected_output
    == "0x0000000000000000000000000000000000000000000000000000000000000001"
  and .vectors[1].expected_output == "0x"
  and .vectors[2].expected_output == "0x"
' "$vector_file" >/dev/null; then
  fail "committed EIP-7951 vector fixture is malformed"
fi

if [ -n "$output_path" ]; then
  output_parent="$(dirname "$output_path")"
  if [ ! -d "$output_parent" ]; then
    fail "output directory does not exist: $output_parent"
  fi
  if [ -e "$output_path" ] || [ -L "$output_path" ]; then
    fail "output path already exists; refusing to overwrite it: $output_path"
  fi
fi

positive_input="$(jq -er '.vectors[0].input' "$vector_file")"
negative_input="$(jq -er '.vectors[1].input' "$vector_file")"
infinity_input="$(jq -er '.vectors[2].input' "$vector_file")"
positive_expected="$(jq -er '.vectors[0].expected_output' "$vector_file")"
negative_expected="$(jq -er '.vectors[1].expected_output' "$vector_file")"
infinity_expected="$(jq -er '.vectors[2].expected_output' "$vector_file")"

precompile="0x0000000000000000000000000000000000000100"
meter_address="0xfffffffffffffffffffffffffffffffffffffffe"
# GAS; CALLDATASIZE; PUSH0; PUSH0; CALLDATACOPY; PUSH1 0x20; PUSH0;
# CALLDATASIZE; PUSH0; PUSH2 0x0100; GAS; STATICCALL; POP; GAS; SWAP1;
# SUB; PUSH0; MSTORE; PUSH1 0x20; PUSH0; RETURN.
meter_runtime="0x5a365f5f3760205f365f6101005afa505a90035f5260205ff3"
expected_meter_output="0x0000000000000000000000000000000000000000000000000000000000001b91"

rpc() {
  curl \
    --disable \
    --fail \
    --silent \
    --show-error \
    --connect-timeout 10 \
    --max-time 30 \
    --max-filesize 1048576 \
    --proto "=http,https" \
    -H "Content-Type: application/json" \
    --data "$1" \
    "$rpc_url"
}

rpc_success() {
  request="$1"
  expected_id="$2"
  label="$3"
  if ! response="$(rpc "$request")"; then
    fail "$label transport failed"
  fi
  if ! response="$(
    printf '%s' "$response" \
      | python3 "$strict_jsonrpc" --expected-id "$expected_id"
  )"; then
    fail "$label returned an error, duplicate member, or malformed JSON-RPC result"
  fi
  printf '%s' "$response"
}

chain_request='{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}'
chain_response="$(rpc_success "$chain_request" 1 "eth_chainId")"
if ! printf '%s' "$chain_response" | jq -e '.result | type == "string"' >/dev/null; then
  fail "eth_chainId result was not a string"
fi
chain_hex="$(printf '%s' "$chain_response" | jq -r '.result')"
if ! printf '%s' "$chain_hex" | LC_ALL=C grep -Eq '^0x[0-9a-fA-F]+$'; then
  fail "eth_chainId was not hexadecimal"
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
  fail "refusing chain $chain_hex; expected Robinhood mainnet chain 4663"
fi

block_request='{"jsonrpc":"2.0","id":2,"method":"eth_getBlockByNumber","params":["latest",false]}'
block_response="$(rpc_success "$block_request" 2 "eth_getBlockByNumber")"
if ! printf '%s' "$block_response" | jq -e '
  (.result | type == "object")
  and (.result.number | type == "string")
  and (.result.hash | type == "string")
' >/dev/null; then
  fail "latest block result was malformed"
fi
block_number="$(printf '%s' "$block_response" | jq -r '.result.number' | tr 'A-F' 'a-f')"
block_hash="$(printf '%s' "$block_response" | jq -r '.result.hash' | tr 'A-F' 'a-f')"
if ! printf '%s' "$block_number" | LC_ALL=C grep -Eq '^0x[0-9a-f]+$'; then
  fail "latest block number was not canonical hexadecimal"
fi
if ! printf '%s' "$block_hash" | LC_ALL=C grep -Eq '^0x[0-9a-f]{64}$'; then
  fail "latest block hash was not 32 bytes"
fi
block_reference="$(
  jq -cn \
    --arg block_hash "$block_hash" \
    '{blockHash:$block_hash,requireCanonical:true}'
)"

code_request="$(
  jq -cn \
    --arg address "$meter_address" \
    --argjson block "$block_reference" \
    '{jsonrpc:"2.0",id:3,method:"eth_getCode",params:[$address,$block]}'
)"
code_response="$(rpc_success "$code_request" 3 "eth_getCode(meter)")"
if ! printf '%s' "$code_response" | jq -e '.result == "0x"' >/dev/null; then
  fail "gas meter address is not empty at the pinned target block"
fi

direct_call() {
  vector_id="$1"
  vector_case="$2"
  vector_input="$3"
  expected_output="$4"
  request="$(
    jq -cn \
      --argjson id "$vector_id" \
      --arg to "$precompile" \
      --arg data "$vector_input" \
      --argjson block "$block_reference" \
      '{jsonrpc:"2.0",id:$id,method:"eth_call",params:[{to:$to,data:$data,gas:"0x100000"},$block]}'
  )"
  response="$(rpc_success "$request" "$vector_id" "P256VERIFY $vector_case vector")"
  if ! printf '%s' "$response" | jq -e '.result | type == "string"' >/dev/null; then
    fail "P256VERIFY $vector_case vector returned a non-string result"
  fi
  observed="$(printf '%s' "$response" | jq -r '.result' | tr 'A-F' 'a-f')"
  if [ "$observed" != "$expected_output" ]; then
    fail "P256VERIFY $vector_case vector returned $observed; expected $expected_output"
  fi
  printf '%s' "$observed"
}

positive_output="$(direct_call 4 positive "$positive_input" "$positive_expected")"
negative_output="$(direct_call 5 negative "$negative_input" "$negative_expected")"
infinity_output="$(direct_call 6 infinity "$infinity_input" "$infinity_expected")"

meter_call() {
  vector_id="$1"
  vector_case="$2"
  vector_input="$3"
  state_override="$(
    jq -cn \
      --arg address "$meter_address" \
      --arg code "$meter_runtime" \
      '{($address):{code:$code}}'
  )"
  request="$(
    jq -cn \
      --argjson id "$vector_id" \
      --arg to "$meter_address" \
      --arg data "$vector_input" \
      --argjson block "$block_reference" \
      --argjson override "$state_override" \
      '{jsonrpc:"2.0",id:$id,method:"eth_call",params:[{to:$to,data:$data,gas:"0x100000"},$block,$override]}'
  )"
  response="$(rpc_success "$request" "$vector_id" "P256VERIFY $vector_case gas meter")"
  if ! printf '%s' "$response" | jq -e '.result | type == "string"' >/dev/null; then
    fail "P256VERIFY $vector_case gas meter returned a non-string result"
  fi
  observed="$(printf '%s' "$response" | jq -r '.result' | tr 'A-F' 'a-f')"
  if [ "$observed" != "$expected_meter_output" ]; then
    fail "P256VERIFY $vector_case gas meter returned $observed; expected exact total 7057 (6900 + 157)"
  fi
  printf '%s' "$observed"
}

positive_meter_output="$(meter_call 7 positive "$positive_input")"
negative_meter_output="$(meter_call 8 negative "$negative_input")"
infinity_meter_output="$(meter_call 9 infinity "$infinity_input")"

canonical_request="$(
  jq -cn \
    --arg block_number "$block_number" \
    '{jsonrpc:"2.0",id:10,method:"eth_getBlockByNumber",params:[$block_number,false]}'
)"
canonical_response="$(rpc_success "$canonical_request" 10 "final canonical block check")"
if ! printf '%s' "$canonical_response" | jq -e \
  --arg block_number "$block_number" \
  --arg block_hash "$block_hash" '
    (.result | type == "object")
    and ((.result.number | ascii_downcase) == $block_number)
    and ((.result.hash | ascii_downcase) == $block_hash)
  ' >/dev/null; then
  fail "pinned latest block was no longer canonical after the probe"
fi

evidence="$(
  jq -cn \
    --arg checked_at "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
    --arg block_number "$block_number" \
    --arg block_hash "$block_hash" \
    --arg precompile "$precompile" \
    --arg meter_address "$meter_address" \
    --arg meter_runtime "$meter_runtime" \
    --arg vector_fixture_sha256 "$observed_vector_file_sha256" \
    --arg upstream_vector_sha256 "6ed75dffbc0dd70defc7aceb7299df75dfb2a18184fbac229482c0a4b6601d6d" \
    --arg positive_input "$positive_input" \
    --arg positive_output "$positive_output" \
    --arg negative_input "$negative_input" \
    --arg negative_output "$negative_output" \
    --arg infinity_input "$infinity_input" \
    --arg infinity_output "$infinity_output" \
    --arg positive_meter_output "$positive_meter_output" \
    --arg negative_meter_output "$negative_meter_output" \
    --arg infinity_meter_output "$infinity_meter_output" \
    '{
      schema:"tohseno.p256-probe/2",
      checked_at:$checked_at,
      chain_id:4663,
      rpc:"explicit-target",
      block:{
        source_tag:"latest",
        number:$block_number,
        hash:$block_hash,
        require_canonical:true,
        rechecked_after_probe:true
      },
      precompile:$precompile,
      vector_provenance:{
        fixture_sha256:$vector_fixture_sha256,
        upstream_sha256:$upstream_vector_sha256
      },
      vectors:{
        positive:{input:$positive_input,output:$positive_output},
        negative:{input:$negative_input,output:$negative_output},
        infinity:{input:$infinity_input,output:$infinity_output}
      },
      gas:{
        meter_address:$meter_address,
        meter_runtime:$meter_runtime,
        meter_overhead:157,
        measured_total:7057,
        measured_precompile:6900,
        outputs:{
          positive:$positive_meter_output,
          negative:$negative_meter_output,
          infinity:$infinity_meter_output
        }
      },
      valid:true
    }'
)"

if [ -n "$output_path" ]; then
  if ! (
    set -C
    umask 022
    printf '%s\n' "$evidence" >"$output_path"
  ) 2>/dev/null; then
    fail "could not create output without overwriting an existing path: $output_path"
  fi
fi
printf '%s\n' "$evidence"
