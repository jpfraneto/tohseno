#!/bin/sh
set -eu

mode="write"
if [ "$#" -gt 1 ]; then
  printf '%s\n' "usage: build-contract-abi.sh [--check]" >&2
  exit 2
fi
if [ "$#" -eq 1 ]; then
  if [ "$1" != "--check" ]; then
    printf '%s\n' "usage: build-contract-abi.sh [--check]" >&2
    exit 2
  fi
  mode="check"
fi

for tool in forge cast jq cmp mktemp; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'build-contract-abi.sh: required tool is missing: %s\n' "$tool" >&2
    exit 1
  fi
done

script_directory="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_directory/.." && pwd)"
contracts_directory="$repository_root/contracts"
if [ -e "$contracts_directory/src/ShotRelations.sol" ]; then
  printf '%s\n' \
    "build-contract-abi.sh: removed contracts/src/ShotRelations.sol must not exist" >&2
  exit 1
fi
temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-contract-artifacts.XXXXXX")"

cleanup() {
  case "$temporary_directory" in
    "${TMPDIR:-/tmp}"/tohseno-contract-artifacts.*)
      rm -rf -- "$temporary_directory"
      ;;
    *)
      printf '%s\n' "build-contract-abi.sh: refusing unsafe temporary cleanup" >&2
      ;;
  esac
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$temporary_directory/abi" "$temporary_directory/bytecode" "$temporary_directory/deployments"

(
  cd "$contracts_directory"
  forge build >/dev/null

  for contract in BuilderAccount BuilderAccountFactory ShotRegistry; do
    forge inspect "$contract" abi --json | jq -S . >"$temporary_directory/abi/$contract.json"
  done
  if jq -e '
    [.. | objects | .name? // empty]
    | any(
        . == "contentCommitment"
        or . == "publicState"
        or . == "sequenceOf"
        or . == "createNonces"
        or . == "handleText"
        or . == "appcoinOf"
        or . == "appStoreAttestationOf"
      )
  ' "$temporary_directory/abi/ShotRegistry.json" >/dev/null; then
    printf '%s\n' \
      "build-contract-abi.sh: narrowed ShotRegistry ABI contains a removed surface" >&2
    exit 1
  fi

  builder_account_creation="$(forge inspect BuilderAccount bytecode)"
  factory_creation="$(forge inspect BuilderAccountFactory bytecode)"
  registry_creation="$(forge inspect ShotRegistry bytecode)"

  printf '%s\n' "$builder_account_creation" \
    >"$temporary_directory/bytecode/BuilderAccount.next.creation.hex"

  deterministic_deployer="0x4e59b44847b379578588920ca78fbf26c0b4956c"
  factory_salt="$(
    cast keccak "TOHSENO NEXT CONTRACT GENERATION BuilderAccountFactory"
  )"
  registry_salt="$(
    cast keccak "TOHSENO NEXT CONTRACT GENERATION ShotRegistry"
  )"
  factory_init_code_hash="$(cast keccak "$factory_creation")"
  registry_init_code_hash="$(cast keccak "$registry_creation")"
  factory_address="$(
    cast create2 \
      --deployer "$deterministic_deployer" \
      --salt "$factory_salt" \
      --init-code-hash "$factory_init_code_hash"
  )"
  factory_address="$(printf '%s' "$factory_address" | tr 'A-F' 'a-f')"
  registry_address="$(
    cast create2 \
      --deployer "$deterministic_deployer" \
      --salt "$registry_salt" \
      --init-code-hash "$registry_init_code_hash"
  )"
  registry_address="$(printf '%s' "$registry_address" | tr 'A-F' 'a-f')"

  jq -S -n \
    --arg deterministic_deployer "$deterministic_deployer" \
    --arg factory_salt "$factory_salt" \
    --arg factory_init_code_hash "$factory_init_code_hash" \
    --arg factory_address "$factory_address" \
    --arg registry_salt "$registry_salt" \
    --arg registry_init_code_hash "$registry_init_code_hash" \
    --arg registry_address "$registry_address" \
    '{
      schema: "tohseno.deployment-plan/1",
      protocol: "tohseno",
      candidate: {
        version: "next",
        codename: "DRAFT",
        status: "planned, undeployed, non-canonical and unaudited"
      },
      chain: {
        name: "Robinhood Chain mainnet",
        chain_id: 4663,
        p256verify: "0x0000000000000000000000000000000000000100"
      },
      create2: {
        deployer: $deterministic_deployer,
        deployer_code_must_be_verified_before_broadcast: true
      },
      source_commit: null,
      contracts: {
        BuilderAccountFactory: {
          deployment_order: 1,
          constructor_arguments: [],
          salt: $factory_salt,
          init_code_hash: $factory_init_code_hash,
          planned_address: $factory_address,
          deployed: false,
          transaction_hash: null,
          runtime_code_hash: null
        },
        ShotRegistry: {
          deployment_order: 2,
          constructor_arguments: [],
          salt: $registry_salt,
          init_code_hash: $registry_init_code_hash,
          planned_address: $registry_address,
          deployed: false,
          transaction_hash: null,
          runtime_code_hash: null
        }
      }
    }' >"$temporary_directory/deployments/robinhood-mainnet-next.json"
)

artifacts="
abi/BuilderAccount.json
abi/BuilderAccountFactory.json
abi/ShotRegistry.json
bytecode/BuilderAccount.next.creation.hex
deployments/robinhood-mainnet-next.json
"

if [ "$mode" = "check" ]; then
  stale=0
  if [ -e "$contracts_directory/abi/ShotRelations.json" ]; then
    printf '%s\n' \
      "stale removed contract artifact: contracts/abi/ShotRelations.json" >&2
    stale=1
  fi
  for artifact in $artifacts; do
    if [ ! -f "$contracts_directory/$artifact" ] \
      || ! cmp -s "$temporary_directory/$artifact" "$contracts_directory/$artifact"; then
      printf 'stale contract artifact: contracts/%s\n' "$artifact" >&2
      stale=1
    fi
  done
  if [ "$stale" -ne 0 ]; then
    exit 1
  fi
  printf '%s\n' "contract artifacts are current."
  exit 0
fi

mkdir -p \
  "$contracts_directory/abi" \
  "$contracts_directory/bytecode" \
  "$contracts_directory/deployments"
if [ -e "$contracts_directory/abi/ShotRelations.json" ]; then
  rm -- "$contracts_directory/abi/ShotRelations.json"
fi
for artifact in $artifacts; do
  cp "$temporary_directory/$artifact" "$contracts_directory/$artifact"
done
printf '%s\n' "wrote current ABIs and explicitly non-versioned next-generation draft artifacts."
