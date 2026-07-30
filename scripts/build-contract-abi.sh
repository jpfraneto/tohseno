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

for tool in forge cast jq cmp mktemp shasum awk wc tr; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'build-contract-abi.sh: required tool is missing: %s\n' "$tool" >&2
    exit 1
  fi
done

script_directory="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_directory/.." && pwd)"
contracts_directory="$repository_root/contracts"
generation="0.8.0"
generation_source_commit="862ca6cd3d396271b56b336fee0513ddcf6ecc64"
generation_source_tree_sha256="5d8c56423f9b9cb97d8e05834a6a2e776034b1257a186e47f25869bf509910c3"
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

mkdir -p \
  "$temporary_directory/abi" \
  "$temporary_directory/bytecode" \
  "$temporary_directory/deployments" \
  "$temporary_directory/generations/$generation/abi" \
  "$temporary_directory/generations/$generation/bytecode"

file_sha256() {
  shasum -a 256 "$1" | awk '{print $1}'
}

file_byte_length() {
  wc -c <"$1" | tr -d ' '
}

(
  cd "$contracts_directory"
  forge build >/dev/null

  for contract in BuilderAccount BuilderAccountFactory ShotRegistry; do
    forge inspect "$contract" abi --json | jq -S . >"$temporary_directory/abi/$contract.json"
    cp \
      "$temporary_directory/abi/$contract.json" \
      "$temporary_directory/generations/$generation/abi/$contract.json"
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
  builder_account_runtime="$(forge inspect BuilderAccount deployedBytecode)"
  factory_runtime="$(forge inspect BuilderAccountFactory deployedBytecode)"
  registry_runtime="$(forge inspect ShotRegistry deployedBytecode)"

  printf '%s\n' "$builder_account_creation" \
    >"$temporary_directory/bytecode/BuilderAccount.next.creation.hex"
  printf '%s\n' "$builder_account_creation" \
    >"$temporary_directory/generations/$generation/bytecode/BuilderAccount.creation.hex"

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

  source_tree_preimage="$temporary_directory/source-tree.preimage"
  printf 'TOHSENO-CONTRACT-SOURCE-TREE-V1\0' >"$source_tree_preimage"
  source_files='[]'
  source_paths="
foundry.toml
src/BuilderAccount.sol
src/BuilderAccountFactory.sol
src/EIP712Domain.sol
src/IERC1271.sol
src/P256Verifier.sol
src/ShotRegistry.sol
"
  for source_path in $source_paths; do
    source_file="$contracts_directory/$source_path"
    source_sha256="$(file_sha256 "$source_file")"
    source_byte_length="$(file_byte_length "$source_file")"
    printf '0x%s %s %s\n' \
      "$source_sha256" \
      "$source_byte_length" \
      "$source_path" >>"$source_tree_preimage"
    source_files="$(
      jq -c -n \
        --argjson files "$source_files" \
        --arg path "$source_path" \
        --arg sha256 "0x$source_sha256" \
        --argjson byte_length "$source_byte_length" \
        '$files + [{
          path: $path,
          sha256: $sha256,
          byte_length: $byte_length
        }]'
    )"
  done
  observed_source_tree_sha256="$(file_sha256 "$source_tree_preimage")"
  if [ "$observed_source_tree_sha256" != "$generation_source_tree_sha256" ]; then
    printf '%s\n' \
      "build-contract-abi.sh: contract source differs from frozen generation $generation; define a new generation" >&2
    exit 1
  fi

  builder_abi="$temporary_directory/generations/$generation/abi/BuilderAccount.json"
  factory_abi="$temporary_directory/generations/$generation/abi/BuilderAccountFactory.json"
  registry_abi="$temporary_directory/generations/$generation/abi/ShotRegistry.json"
  builder_bytecode="$temporary_directory/generations/$generation/bytecode/BuilderAccount.creation.hex"
  builder_abi_sha256="$(file_sha256 "$builder_abi")"
  factory_abi_sha256="$(file_sha256 "$factory_abi")"
  registry_abi_sha256="$(file_sha256 "$registry_abi")"
  builder_bytecode_sha256="$(file_sha256 "$builder_bytecode")"
  builder_creation_keccak256="$(cast keccak "$builder_account_creation")"
  builder_runtime_keccak256="$(cast keccak "$builder_account_runtime")"
  factory_runtime_keccak256="$(cast keccak "$factory_runtime")"
  registry_runtime_keccak256="$(cast keccak "$registry_runtime")"
  forge_version="$(forge --version | awk 'NR == 1 { print $3 }')"
  forge_commit="$(forge --version | awk '$1 == "Commit" && $2 == "SHA:" { print $3 }')"

  jq -S -n \
    --arg generation "$generation" \
    --arg source_commit "$generation_source_commit" \
    --arg source_tree_sha256 "0x$observed_source_tree_sha256" \
    --argjson source_files "$source_files" \
    --arg forge_version "$forge_version" \
    --arg forge_commit "$forge_commit" \
    --arg builder_abi_sha256 "0x$builder_abi_sha256" \
    --argjson builder_abi_length "$(file_byte_length "$builder_abi")" \
    --arg builder_bytecode_sha256 "0x$builder_bytecode_sha256" \
    --argjson builder_bytecode_length "$(file_byte_length "$builder_bytecode")" \
    --arg builder_creation_keccak256 "$builder_creation_keccak256" \
    --arg builder_runtime_keccak256 "$builder_runtime_keccak256" \
    --arg factory_abi_sha256 "0x$factory_abi_sha256" \
    --argjson factory_abi_length "$(file_byte_length "$factory_abi")" \
    --arg factory_creation_keccak256 "$factory_init_code_hash" \
    --arg factory_runtime_keccak256 "$factory_runtime_keccak256" \
    --arg registry_abi_sha256 "0x$registry_abi_sha256" \
    --argjson registry_abi_length "$(file_byte_length "$registry_abi")" \
    --arg registry_creation_keccak256 "$registry_init_code_hash" \
    --arg registry_runtime_keccak256 "$registry_runtime_keccak256" \
    --arg deterministic_deployer "$deterministic_deployer" \
    --arg factory_salt "$factory_salt" \
    --arg factory_address "$factory_address" \
    --arg registry_salt "$registry_salt" \
    --arg registry_address "$registry_address" \
    '{
      schema: "tohseno.contract-generation/1",
      protocol: "tohseno",
      protocol_major: 2,
      generation: $generation,
      chain: {
        chain_id: 4663,
        p256_verifier: {
          standard: "EIP-7951",
          address: "0x0000000000000000000000000000000000000100",
          gas: 6900
        }
      },
      source: {
        commit: $source_commit,
        tree_law: "tohseno.contract-source-tree/1",
        tree_sha256: $source_tree_sha256,
        files: $source_files
      },
      build: {
        profile: "default",
        solc_version: "0.8.30",
        evm_version: "cancun",
        optimizer: true,
        optimizer_runs: 10000,
        via_ir: false,
        bytecode_hash: "none",
        cbor_metadata: false,
        forge_version: $forge_version,
        forge_commit: $forge_commit
      },
      contracts: {
        builder_account: {
          component_version: $generation,
          abi: {
            path: "abi/BuilderAccount.json",
            sha256: $builder_abi_sha256,
            byte_length: $builder_abi_length
          },
          creation_bytecode: {
            path: "bytecode/BuilderAccount.creation.hex",
            sha256: $builder_bytecode_sha256,
            byte_length: $builder_bytecode_length
          },
          creation_code_keccak256: $builder_creation_keccak256,
          runtime_code_keccak256: $builder_runtime_keccak256
        },
        builder_account_factory: {
          component_version: $generation,
          abi: {
            path: "abi/BuilderAccountFactory.json",
            sha256: $factory_abi_sha256,
            byte_length: $factory_abi_length
          },
          creation_bytecode: null,
          creation_code_keccak256: $factory_creation_keccak256,
          runtime_code_keccak256: $factory_runtime_keccak256
        },
        shot_registry: {
          component_version: $generation,
          abi: {
            path: "abi/ShotRegistry.json",
            sha256: $registry_abi_sha256,
            byte_length: $registry_abi_length
          },
          creation_bytecode: null,
          creation_code_keccak256: $registry_creation_keccak256,
          runtime_code_keccak256: $registry_runtime_keccak256
        }
      },
      create2: {
        deployer: $deterministic_deployer,
        builder_account_factory: {
          salt: $factory_salt,
          init_code_keccak256: $factory_creation_keccak256,
          predicted_address: $factory_address
        },
        shot_registry: {
          salt: $registry_salt,
          init_code_keccak256: $registry_creation_keccak256,
          predicted_address: $registry_address
        }
      }
    }' >"$temporary_directory/generations/$generation/generation.json"

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
generations/0.8.0/abi/BuilderAccount.json
generations/0.8.0/abi/BuilderAccountFactory.json
generations/0.8.0/abi/ShotRegistry.json
generations/0.8.0/bytecode/BuilderAccount.creation.hex
generations/0.8.0/generation.json
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
  "$contracts_directory/deployments" \
  "$contracts_directory/generations/$generation/abi" \
  "$contracts_directory/generations/$generation/bytecode"
if [ -e "$contracts_directory/abi/ShotRelations.json" ]; then
  rm -- "$contracts_directory/abi/ShotRelations.json"
fi
for artifact in $artifacts; do
  cp "$temporary_directory/$artifact" "$contracts_directory/$artifact"
done
printf '%s\n' \
  "wrote current development artifacts and immutable contract generation $generation."
