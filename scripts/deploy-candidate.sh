#!/bin/sh
set -eu

mode="deploy"
if [ "$#" -gt 1 ]; then
  printf '%s\n' "usage: deploy-candidate.sh [--dry-run]" >&2
  exit 2
fi
if [ "$#" -eq 1 ]; then
  case "$1" in
    --dry-run)
      mode="dry-run"
      ;;
    -h | --help)
      printf '%s\n' "usage: deploy-candidate.sh [--dry-run]"
      printf '%s\n' "  --dry-run  run every read-only gate and stateful fork simulation, then stop"
      exit 0
      ;;
    *)
      printf '%s\n' "usage: deploy-candidate.sh [--dry-run]" >&2
      exit 2
      ;;
  esac
fi

fail() {
  printf 'deploy-candidate.sh: %s\n' "$1" >&2
  exit 1
}

lowercase() {
  printf '%s' "$1" | tr 'A-F' 'a-f'
}

valid_address() {
  printf '%s' "$1" | LC_ALL=C grep -Eq '^0x[0-9a-fA-F]{40}$'
}

# Only the named encrypted Foundry account is accepted. In particular, no raw
# signing material or password-bearing Foundry environment variable may
# silently influence this procedure.
if [ "${PRIVATE_KEY+x}" = "x" ] \
  || [ "${ETH_PRIVATE_KEY+x}" = "x" ] \
  || [ "${ETH_PRIVATE_KEYS+x}" = "x" ] \
  || [ "${CAST_PRIVATE_KEY+x}" = "x" ] \
  || [ "${FOUNDRY_PRIVATE_KEY+x}" = "x" ] \
  || [ "${FOUNDRY_PRIVATE_KEYS+x}" = "x" ] \
  || [ "${DAPP_PRIVATE_KEY+x}" = "x" ] \
  || [ "${DAPP_KEYS+x}" = "x" ] \
  || [ "${MNEMONIC+x}" = "x" ] \
  || [ "${ETH_MNEMONIC+x}" = "x" ] \
  || [ "${FOUNDRY_MNEMONIC+x}" = "x" ] \
  || [ "${FOUNDRY_MNEMONICS+x}" = "x" ] \
  || [ "${DAPP_MNEMONIC+x}" = "x" ] \
  || [ "${TOHSENO_PRIVATE_KEY+x}" = "x" ] \
  || [ "${TOHSENO_DEPLOYER_PRIVATE_KEY+x}" = "x" ] \
  || [ "${TOHSENO_MNEMONIC+x}" = "x" ] \
  || [ "${ETH_KEYSTORE+x}" = "x" ] \
  || [ "${ETH_KEYSTORE_ACCOUNT+x}" = "x" ] \
  || [ "${ETH_PASSWORD+x}" = "x" ] \
  || [ "${ETH_FROM+x}" = "x" ]; then
  fail "remove raw-key, mnemonic, password, keystore-path, and implicit-signer environment variables"
fi

if [ "${TOHSENO_ALLOW_EXPERIMENTAL_MAINNET:-0}" != "1" ]; then
  fail "TOHSENO_ALLOW_EXPERIMENTAL_MAINNET must be the literal value 1"
fi

rpc_url="${ROBINHOOD_RPC_URL:-}"
account_name="${TOHSENO_DEPLOYER_ACCOUNT:-}"
expected_signer="${TOHSENO_EXPECTED_DEPLOYER_ADDRESS:-}"
if [ -z "$rpc_url" ]; then
  fail "ROBINHOOD_RPC_URL must be explicit"
fi
if [ -z "$account_name" ]; then
  fail "TOHSENO_DEPLOYER_ACCOUNT must name an encrypted Foundry account"
fi
if ! printf '%s' "$account_name" | LC_ALL=C grep -Eq '^[A-Za-z0-9][A-Za-z0-9._-]*$'; then
  fail "TOHSENO_DEPLOYER_ACCOUNT contains unsupported characters"
fi
account_name_length="$(printf '%s' "$account_name" | wc -c | tr -d ' ')"
if [ "$account_name_length" -gt 128 ]; then
  fail "TOHSENO_DEPLOYER_ACCOUNT is too long"
fi
if ! valid_address "$expected_signer"; then
  fail "TOHSENO_EXPECTED_DEPLOYER_ADDRESS must be an explicit 20-byte address"
fi
expected_signer="$(lowercase "$expected_signer")"
if [ "$expected_signer" = "0x0000000000000000000000000000000000000000" ]; then
  fail "TOHSENO_EXPECTED_DEPLOYER_ADDRESS cannot be zero"
fi

for tool in cast forge jq git grep sed tr wc mktemp ln chmod date tail rm dirname; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    fail "required tool is missing: $tool"
  fi
done

script_directory="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_directory/.." && pwd)"
contracts_directory="$repository_root/contracts"
plan_path="$contracts_directory/deployments/robinhood-mainnet-genesis.json"
evidence_path="$contracts_directory/deployments/robinhood-mainnet-genesis.actual.json"
foundry_script="$contracts_directory/script/DeployCandidate.s.sol"

for required_file in \
  "$plan_path" \
  "$foundry_script" \
  "$repository_root/scripts/build-contract-abi.sh" \
  "$repository_root/scripts/probe-p256.sh"; do
  if [ ! -f "$required_file" ] || [ -L "$required_file" ]; then
    fail "required regular file is missing or symbolic: $required_file"
  fi
done
if [ "$mode" = "deploy" ] && { [ -e "$evidence_path" ] || [ -L "$evidence_path" ]; }; then
  fail "actual deployment evidence already exists; refusing to overwrite it"
fi

deterministic_deployer="0x4e59b44847b379578588920ca78fbf26c0b4956c"
deterministic_deployer_code="0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf3"
deterministic_deployer_code_hash="0x2fa86add0aed31f33a762c9d88e807c475bd51d0f52bd0955754b2608f7e4989"

factory_address="0x9a48926c82fe766fe599116dfc7111ba6f7171dd"
registry_address="0x02d2a9ed5ba8843b82b4e5976c686dce4af3ba5e"
relations_address="0x75abff418c4cad3c4bd56f467cc2737237dd6ea5"
factory_salt="0xe0fd0e28bcdb28bfdfa44c2bba736c6206a798abf890d7d5690e5b77610c603a"
registry_salt="0x28355a607bb3452ad437f71bc1b14e43f270e721b2bfbf028a867711aa473af1"
relations_salt="0xdb5c183795d37085c73de55b04fb086beaa54ab4bde52b3c573952af82200ab3"
factory_plan_init_hash="0xc54e36542c975b6bde3868afded9d3d342e01defdfd0b2c8bc3e25c417526b28"
registry_plan_init_hash="0x8d76b602133b97f4d0adb171cb0a8339f77be15e76b64d3cb2c4077434ffb482"
relations_plan_init_hash="0x5822cd2c638153e6922885fde3a201c232428271aa38ef4fd42716a30c7dd2a5"
factory_runtime_hash="0x1f44f9fa643277e05f5a9d1f6a05b4cee9264c261a423021c5e0c7f5da3b312a"
registry_runtime_hash="0xac64e4933d88d40c18af598f7ebf7bc8f7b829e1a61acb8e380d4ac670f31478"
relations_runtime_hash="0x909ba083f6b186b08f80d5ea465878f7a0c909f1c65b11d2ed8ca11a40669de5"
factory_runtime_size=9846
registry_runtime_size=8203
relations_runtime_size=9199

if ! jq -e \
  --arg deployer "$deterministic_deployer" \
  --arg factory "$factory_address" \
  --arg registry "$registry_address" \
  --arg relations "$relations_address" \
  --arg factory_salt "$factory_salt" \
  --arg registry_salt "$registry_salt" \
  --arg relations_salt "$relations_salt" \
  --arg factory_init_hash "$factory_plan_init_hash" \
  --arg registry_init_hash "$registry_plan_init_hash" \
  --arg relations_init_hash "$relations_plan_init_hash" \
  '
    .schema == "tohseno.deployment-plan/1"
    and .candidate.version == "0.7.0"
    and .candidate.codename == "GENESIS"
    and .candidate.status == "planned, undeployed, non-canonical and unaudited"
    and .chain.chain_id == 4663
    and .chain.p256verify == "0x0000000000000000000000000000000000000100"
    and .create2.deployer == $deployer
    and .contracts.BuilderAccountFactory.planned_address == $factory
    and .contracts.ShotRegistry.planned_address == $registry
    and .contracts.ShotRelations.planned_address == $relations
    and .contracts.BuilderAccountFactory.salt == $factory_salt
    and .contracts.ShotRegistry.salt == $registry_salt
    and .contracts.ShotRelations.salt == $relations_salt
    and .contracts.BuilderAccountFactory.init_code_hash == $factory_init_hash
    and .contracts.ShotRegistry.init_code_hash == $registry_init_hash
    and .contracts.ShotRelations.init_code_hash == $relations_init_hash
    and .contracts.ShotRelations.constructor_arguments == [$registry]
    and ([.contracts[] | .deployed] | all(. == false))
    and ([.contracts[] | .transaction_hash] | all(. == null))
    and ([.contracts[] | .runtime_code_hash] | all(. == null))
  ' "$plan_path" >/dev/null; then
  fail "the undeployed candidate plan is malformed or does not match the pinned procedure"
fi

chain_id="$(cast chain-id --rpc-url "$rpc_url")"
if [ "$chain_id" != "4663" ]; then
  fail "refusing chain ID $chain_id; GENESIS requires 4663"
fi

observed_deployer_code="$(lowercase "$(cast code "$deterministic_deployer" --rpc-url "$rpc_url")")"
if [ "$observed_deployer_code" != "$deterministic_deployer_code" ]; then
  fail "the 0x4e59 deterministic deployer runtime code does not match the pinned Arachnid proxy"
fi
observed_deployer_hash="$(cast keccak "$observed_deployer_code")"
if [ "$observed_deployer_hash" != "$deterministic_deployer_code_hash" ]; then
  fail "the 0x4e59 deterministic deployer code hash does not match"
fi

printf '%s\n' "Running strict P256VERIFY public-RPC probe..."
probe_output="$("$repository_root/scripts/probe-p256.sh" --rpc-url "$rpc_url")"
if ! printf '%s' "$probe_output" | jq -e \
  '.chain_id == 4663
   and .precompile == "0x0000000000000000000000000000000000000100"
   and .output == "0x0000000000000000000000000000000000000000000000000000000000000001"
   and .valid == true' >/dev/null; then
  fail "P256VERIFY probe evidence was malformed"
fi

printf '%s\n' "Running contract format, build, and test gates..."
(
  cd "$contracts_directory"
  forge fmt --check
  forge build
  forge test
)
printf '%s\n' "Checking deterministic ABI, bytecode, and deployment-plan drift..."
"$repository_root/scripts/build-contract-abi.sh" --check

factory_init_code="$(cd "$contracts_directory" && forge inspect BuilderAccountFactory bytecode)"
registry_init_code="$(cd "$contracts_directory" && forge inspect ShotRegistry bytecode)"
relations_creation_code="$(cd "$contracts_directory" && forge inspect ShotRelations bytecode)"
encoded_registry="$(cast abi-encode "constructor(address)" "$registry_address")"
relations_init_code="${relations_creation_code}${encoded_registry#0x}"

if [ "$(cast keccak "$factory_init_code")" != "$factory_plan_init_hash" ] \
  || [ "$(cast keccak "$registry_init_code")" != "$registry_plan_init_hash" ] \
  || [ "$(cast keccak "$relations_init_code")" != "$relations_plan_init_hash" ]; then
  fail "compiled initcode hashes do not match the undeployed plan"
fi

computed_factory="$(cast create2 \
  --deployer "$deterministic_deployer" \
  --salt "$factory_salt" \
  --init-code-hash "$factory_plan_init_hash")"
computed_registry="$(cast create2 \
  --deployer "$deterministic_deployer" \
  --salt "$registry_salt" \
  --init-code-hash "$registry_plan_init_hash")"
computed_relations="$(cast create2 \
  --deployer "$deterministic_deployer" \
  --salt "$relations_salt" \
  --init-code-hash "$relations_plan_init_hash")"
if [ "$(lowercase "$computed_factory")" != "$factory_address" ] \
  || [ "$(lowercase "$computed_registry")" != "$registry_address" ] \
  || [ "$(lowercase "$computed_relations")" != "$relations_address" ]; then
  fail "offline CREATE2 predictions do not match the undeployed plan"
fi

candidate_state() {
  candidate_label="$1"
  candidate_address="$2"
  expected_hash="$3"
  expected_size="$4"
  candidate_code="$(lowercase "$(cast code "$candidate_address" --rpc-url "$rpc_url")")"
  if [ "$candidate_code" = "0x" ]; then
    printf '%s' "missing"
    return
  fi
  if ! printf '%s' "$candidate_code" | LC_ALL=C grep -Eq '^0x([0-9a-f]{2})+$'; then
    fail "$candidate_label returned malformed runtime code"
  fi
  candidate_size="$(( (${#candidate_code} - 2) / 2 ))"
  candidate_hash="$(cast keccak "$candidate_code")"
  if [ "$candidate_size" -ne "$expected_size" ] || [ "$candidate_hash" != "$expected_hash" ]; then
    fail "$candidate_label has unexpected code at its planned address"
  fi
  printf '%s' "already-deployed"
}

factory_state="$(candidate_state "BuilderAccountFactory" "$factory_address" "$factory_runtime_hash" "$factory_runtime_size")"
registry_state="$(candidate_state "ShotRegistry" "$registry_address" "$registry_runtime_hash" "$registry_runtime_size")"
relations_state="$(candidate_state "ShotRelations" "$relations_address" "$relations_runtime_hash" "$relations_runtime_size")"

printf '%s\n' "Unlock the named Foundry keystore only at its password prompt."
resolved_signer="$(cast wallet address --account "$account_name" | tr -d '\r\n')"
if ! valid_address "$resolved_signer"; then
  fail "the encrypted Foundry account did not resolve to one address"
fi
resolved_signer="$(lowercase "$resolved_signer")"
if [ "$resolved_signer" != "$expected_signer" ]; then
  fail "the encrypted Foundry account does not match TOHSENO_EXPECTED_DEPLOYER_ADDRESS"
fi

signer_balance="$(cast balance "$resolved_signer" --rpc-url "$rpc_url")"
signer_nonce="$(cast nonce "$resolved_signer" --rpc-url "$rpc_url")"
if ! printf '%s' "$signer_balance" | LC_ALL=C grep -Eq '^[0-9]+$' \
  || ! printf '%s' "$signer_nonce" | LC_ALL=C grep -Eq '^[0-9]+$'; then
  fail "signer balance or nonce was malformed"
fi

temporary_directory="$(mktemp -d "$contracts_directory/deployments/.deploy-candidate.XXXXXX")"
broadcast_started=0
deployment_complete=0
cleanup() {
  exit_status="$?"
  if [ -n "${temporary_directory:-}" ]; then
    case "$temporary_directory" in
      "$contracts_directory"/deployments/.deploy-candidate.*)
        rm -rf -- "$temporary_directory"
        ;;
      *)
        printf '%s\n' "deploy-candidate.sh: refusing unsafe temporary cleanup" >&2
        ;;
    esac
  fi
  if [ "$exit_status" -ne 0 ] && [ "$broadcast_started" -eq 1 ] && [ "$deployment_complete" -eq 0 ]; then
    printf '%s\n' \
      "deploy-candidate.sh: broadcast may have changed chain state; no actual evidence file was written and the undeployed plan was not modified" >&2
  fi
}
trap cleanup EXIT
trap 'exit 130' HUP INT TERM

probe_path="$temporary_directory/p256-probe.json"
printf '%s\n' "$probe_output" >"$probe_path"
simulation_output="$temporary_directory/simulation.jsonl"
simulation_error="$temporary_directory/simulation.stderr"
if ! (
  cd "$contracts_directory"
  FOUNDRY_BROADCAST="$temporary_directory/foundry-broadcast" \
    FOUNDRY_CACHE_PATH="$temporary_directory/foundry-cache" \
    forge script script/DeployCandidate.s.sol:DeployCandidate \
      --fork-url "$rpc_url" \
      --sender "$resolved_signer" \
      --json
) >"$simulation_output" 2>"$simulation_error"; then
  sed -n '1,120p' "$simulation_error" >&2
  fail "stateful Foundry fork simulation failed"
fi
if ! jq -s -e \
  --arg factory_hash "$factory_runtime_hash" \
  --arg registry_hash "$registry_runtime_hash" \
  --arg relations_hash "$relations_runtime_hash" \
  '
    any(.[]; .success? == true)
    and any(.[]; .status? == "success")
    and ([.[] | select(.raw_logs? != null) | .raw_logs[] | .data[0:66]] as $hashes
      | ($hashes | index($factory_hash)) != null
      and ($hashes | index($registry_hash)) != null
      and ($hashes | index($relations_hash)) != null)
  ' "$simulation_output" >/dev/null; then
  fail "stateful simulation did not verify all candidate runtime hashes"
fi
simulation_gas="$(jq -r 'select(.estimated_total_gas_used? != null) | .estimated_total_gas_used' \
  "$simulation_output" | tail -n 1)"
simulation_gas_price="$(jq -r 'select(.estimated_gas_price? != null) | .estimated_gas_price' \
  "$simulation_output" | tail -n 1)"
simulation_amount="$(jq -r 'select(.estimated_amount_required? != null) | .estimated_amount_required' \
  "$simulation_output" | tail -n 1)"
if ! printf '%s' "$simulation_gas" | LC_ALL=C grep -Eq '^[0-9]+$'; then
  if [ "$factory_state" = "already-deployed" ] \
    && [ "$registry_state" = "already-deployed" ] \
    && [ "$relations_state" = "already-deployed" ]; then
    simulation_gas=0
    simulation_gas_price="not-applicable"
    simulation_amount=0
  else
    fail "Foundry simulation did not return a total gas estimate"
  fi
fi

printf '%s\n' "GENESIS candidate preflight and stateful simulation passed."
printf '  chain: 4663\n'
printf '  signer: %s (encrypted account: %s)\n' "$resolved_signer" "$account_name"
printf '  signer nonce: %s\n' "$signer_nonce"
printf '  signer balance (wei): %s\n' "$signer_balance"
printf '  deterministic deployer: %s\n' "$deterministic_deployer"
printf '  deterministic deployer code hash: %s\n' "$deterministic_deployer_code_hash"
printf '  BuilderAccountFactory: %s (%s)\n' "$factory_address" "$factory_state"
printf '  ShotRegistry: %s (%s)\n' "$registry_address" "$registry_state"
printf '  ShotRelations: %s (%s)\n' "$relations_address" "$relations_state"
printf '  Foundry estimated total gas: %s\n' "$simulation_gas"
printf '  Foundry estimated gas price (gwei): %s\n' "$simulation_gas_price"
printf '  Foundry estimated amount (ETH): %s\n' "$simulation_amount"

if [ "$mode" = "dry-run" ]; then
  printf '%s\n' "Dry run complete: no transaction was signed or broadcast, and no deployment evidence was written."
  exit 0
fi

required_confirmation="DEPLOY_GENESIS_1_0_0_RC1_TO_ROBINHOOD_MAINNET_4663"
if [ "${TOHSENO_DEPLOY_CONFIRMATION:-}" != "$required_confirmation" ]; then
  fail "simulation passed, but TOHSENO_DEPLOY_CONFIRMATION does not exactly authorize the chain-4663 broadcast"
fi

if ! git -C "$repository_root" diff-index --quiet HEAD -- \
  || [ -n "$(git -C "$repository_root" ls-files --others --exclude-standard)" ]; then
  fail "broadcast requires a completely committed and clean source tree"
fi
source_commit="$(git -C "$repository_root" rev-parse --verify HEAD)"
if ! printf '%s' "$source_commit" | LC_ALL=C grep -Eq '^[0-9a-f]{40}$'; then
  fail "could not resolve a full source commit"
fi

network_gas_price="$(cast gas-price --rpc-url "$rpc_url")"
if ! printf '%s' "$network_gas_price" | LC_ALL=C grep -Eq '^[0-9]+$'; then
  fail "network gas price was malformed"
fi
required_balance="$((simulation_gas * network_gas_price * 2))"
if [ "$signer_balance" -lt "$required_balance" ]; then
  fail "signer balance is below the two-times simulated gas reserve"
fi
printf '  current network gas price (wei): %s\n' "$network_gas_price"
printf '  required two-times gas reserve (wei): %s\n' "$required_balance"
printf '%s\n' "The next password prompt can authorize a real transaction."

deploy_one() {
  deployment_label="$1"
  deployment_address="$2"
  deployment_salt="$3"
  deployment_init_code="$4"
  deployment_runtime_hash="$5"
  deployment_runtime_size="$6"
  deployment_record="$7"

  current_state="$(candidate_state \
    "$deployment_label" \
    "$deployment_address" \
    "$deployment_runtime_hash" \
    "$deployment_runtime_size")"
  if [ "$current_state" = "already-deployed" ]; then
    jq -S -n \
      --arg state "$current_state" \
      '{
        state: $state,
        deployed_by_this_run: false,
        transaction_hash: null,
        block_hash: null,
        block_number: null,
        gas_used: null,
        receipt_status: null
      }' >"$deployment_record"
    printf 'Verified existing %s at %s; no transaction needed.\n' \
      "$deployment_label" "$deployment_address"
    return
  fi

  deployment_payload="${deployment_salt}${deployment_init_code#0x}"
  simulated_return="$(cast call \
    "$deterministic_deployer" \
    --data "$deployment_payload" \
    --from "$resolved_signer" \
    --rpc-url "$rpc_url")"
  if [ "$(lowercase "$simulated_return")" != "$deployment_address" ]; then
    fail "$deployment_label exact pre-broadcast eth_call returned the wrong address"
  fi
  estimated_gas="$(cast estimate \
    "$deterministic_deployer" \
    "$deployment_payload" \
    --from "$resolved_signer" \
    --rpc-url "$rpc_url")"
  if ! printf '%s' "$estimated_gas" | LC_ALL=C grep -Eq '^[0-9]+$'; then
    fail "$deployment_label gas estimate was malformed"
  fi
  gas_limit="$((estimated_gas + estimated_gas / 5 + 50000))"
  printf 'Broadcasting %s through %s: estimate=%s gas, limit=%s gas, expected=%s\n' \
    "$deployment_label" "$deterministic_deployer" "$estimated_gas" "$gas_limit" "$deployment_address"

  broadcast_started=1
  transaction_hash="$(cast send \
    "$deterministic_deployer" \
    "$deployment_payload" \
    --async \
    --account "$account_name" \
    --from "$resolved_signer" \
    --chain 4663 \
    --gas-limit "$gas_limit" \
    --rpc-url "$rpc_url" | tr -d '\r\n')"
  if ! printf '%s' "$transaction_hash" | LC_ALL=C grep -Eq '^0x[0-9a-fA-F]{64}$'; then
    fail "$deployment_label did not return a transaction hash"
  fi
  transaction_hash="$(lowercase "$transaction_hash")"

  receipt_path="$temporary_directory/$deployment_label.receipt.json"
  transaction_path="$temporary_directory/$deployment_label.transaction.json"
  cast receipt \
    "$transaction_hash" \
    --confirmations 2 \
    --rpc-url "$rpc_url" \
    --json >"$receipt_path"
  cast tx "$transaction_hash" --rpc-url "$rpc_url" --json >"$transaction_path"

  if ! jq -e \
    --arg hash "$transaction_hash" \
    --arg signer "$resolved_signer" \
    --arg deployer "$deterministic_deployer" \
    '
      (.transactionHash | ascii_downcase) == $hash
      and (.from | ascii_downcase) == $signer
      and (.to | ascii_downcase) == $deployer
      and .status == "0x1"
      and .contractAddress == null
    ' "$receipt_path" >/dev/null; then
    fail "$deployment_label receipt failed verification"
  fi
  if ! jq -e \
    --arg hash "$transaction_hash" \
    --arg signer "$resolved_signer" \
    --arg deployer "$deterministic_deployer" \
    --arg input "$deployment_payload" \
    '
      (.hash | ascii_downcase) == $hash
      and (.from | ascii_downcase) == $signer
      and (.to | ascii_downcase) == $deployer
      and (.input | ascii_downcase) == $input
      and .chainId == "0x1237"
      and .value == "0x0"
    ' "$transaction_path" >/dev/null; then
    fail "$deployment_label transaction envelope failed verification"
  fi
  if [ "$(candidate_state \
    "$deployment_label" \
    "$deployment_address" \
    "$deployment_runtime_hash" \
    "$deployment_runtime_size")" != "already-deployed" ]; then
    fail "$deployment_label runtime code failed post-receipt verification"
  fi

  jq -S -n \
    --arg state "deployed" \
    --arg transaction_hash "$transaction_hash" \
    --arg block_hash "$(jq -r '.blockHash' "$receipt_path" | tr 'A-F' 'a-f')" \
    --arg block_number "$(jq -r '.blockNumber' "$receipt_path")" \
    --arg gas_used "$(jq -r '.gasUsed' "$receipt_path")" \
    '{
      state: $state,
      deployed_by_this_run: true,
      transaction_hash: $transaction_hash,
      block_hash: $block_hash,
      block_number: $block_number,
      gas_used: $gas_used,
      receipt_status: "0x1"
    }' >"$deployment_record"
}

factory_record="$temporary_directory/factory.json"
registry_record="$temporary_directory/registry.json"
relations_record="$temporary_directory/relations.json"
deploy_one \
  "BuilderAccountFactory" \
  "$factory_address" \
  "$factory_salt" \
  "$factory_init_code" \
  "$factory_runtime_hash" \
  "$factory_runtime_size" \
  "$factory_record"
deploy_one \
  "ShotRegistry" \
  "$registry_address" \
  "$registry_salt" \
  "$registry_init_code" \
  "$registry_runtime_hash" \
  "$registry_runtime_size" \
  "$registry_record"
deploy_one \
  "ShotRelations" \
  "$relations_address" \
  "$relations_salt" \
  "$relations_init_code" \
  "$relations_runtime_hash" \
  "$relations_runtime_size" \
  "$relations_record"

# One final independent RPC pass must see all exact runtime hashes before
# complete evidence can exist.
candidate_state "BuilderAccountFactory" "$factory_address" "$factory_runtime_hash" "$factory_runtime_size" >/dev/null
candidate_state "ShotRegistry" "$registry_address" "$registry_runtime_hash" "$registry_runtime_size" >/dev/null
candidate_state "ShotRelations" "$relations_address" "$relations_runtime_hash" "$relations_runtime_size" >/dev/null
verified_block="$(cast block-number --rpc-url "$rpc_url")"
verified_at="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"

temporary_evidence="$temporary_directory/robinhood-mainnet-genesis.actual.json"
jq -S -n \
  --slurpfile plan "$plan_path" \
  --slurpfile probe "$probe_path" \
  --slurpfile factory_record "$factory_record" \
  --slurpfile registry_record "$registry_record" \
  --slurpfile relations_record "$relations_record" \
  --arg verified_at "$verified_at" \
  --arg verified_block "$verified_block" \
  --arg source_commit "$source_commit" \
  --arg signer "$resolved_signer" \
  --arg deployer "$deterministic_deployer" \
  --arg deployer_hash "$deterministic_deployer_code_hash" \
  --arg factory_runtime_hash "$factory_runtime_hash" \
  --arg registry_runtime_hash "$registry_runtime_hash" \
  --arg relations_runtime_hash "$relations_runtime_hash" \
  '{
    schema: "tohseno.deployment/1",
    status: "deployed",
    candidate: (
      $plan[0].candidate
      + {status: "deployed candidate, non-canonical and unaudited"}
    ),
    chain: $plan[0].chain,
    verified_at: $verified_at,
    verified_block: $verified_block,
    source_commit: $source_commit,
    signer: $signer,
    p256_probe: {
      checked_at: $probe[0].checked_at,
      precompile: $probe[0].precompile,
      output: $probe[0].output,
      valid: $probe[0].valid
    },
    create2: {
      deployer: $deployer,
      runtime_code_hash: $deployer_hash
    },
    contracts: {
      BuilderAccountFactory: (
        $plan[0].contracts.BuilderAccountFactory
        + $factory_record[0]
        + {runtime_code_hash: $factory_runtime_hash, deployed: true}
      ),
      ShotRegistry: (
        $plan[0].contracts.ShotRegistry
        + $registry_record[0]
        + {runtime_code_hash: $registry_runtime_hash, deployed: true}
      ),
      ShotRelations: (
        $plan[0].contracts.ShotRelations
        + $relations_record[0]
        + {runtime_code_hash: $relations_runtime_hash, deployed: true}
      )
    },
    addresses: {
      BuilderAccountFactory: $plan[0].contracts.BuilderAccountFactory.planned_address,
      ShotRegistry: $plan[0].contracts.ShotRegistry.planned_address,
      ShotRelations: $plan[0].contracts.ShotRelations.planned_address
    },
    transactions: {
      BuilderAccountFactory: $factory_record[0].transaction_hash,
      ShotRegistry: $registry_record[0].transaction_hash,
      ShotRelations: $relations_record[0].transaction_hash
    }
  }' >"$temporary_evidence"

if ! jq -e \
  '
    .schema == "tohseno.deployment/1"
    and .status == "deployed"
    and .chain.chain_id == 4663
    and .p256_probe.valid == true
    and ([.contracts[] | .deployed] | all(. == true))
    and ([.addresses[], .signer, .create2.deployer] | all(test("^0x[0-9a-f]{40}$")))
  ' "$temporary_evidence" >/dev/null; then
  fail "complete deployment evidence failed final validation"
fi
chmod 0644 "$temporary_evidence"
if ! ln "$temporary_evidence" "$evidence_path"; then
  fail "actual evidence appeared concurrently; refusing to overwrite it"
fi

deployment_complete=1
printf 'Deployment verified. Atomic actual evidence: %s\n' "$evidence_path"
printf '%s\n' "The undeployed plan remains unchanged."
