#!/bin/sh
# Offline drill for the separate Claims activation domain and envelope. All
# keys, addresses, and deployment facts are throwaway fixtures under /tmp.
set -eu

script_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(CDPATH= cd -- "$script_directory/../.." && pwd)
temporary_directory=$(mktemp -d "${TMPDIR:-/tmp}/tohseno-claims-ceremony.XXXXXX")

cleanup() {
  case "$temporary_directory" in
    "${TMPDIR:-/tmp}"/tohseno-claims-ceremony.*) rm -rf -- "$temporary_directory" ;;
    *) printf '%s\n' "refusing unsafe cleanup: $temporary_directory" >&2 ;;
  esac
}
trap cleanup EXIT HUP INT TERM

expect_failure() {
  if "$@" >"$temporary_directory/unexpected-output" 2>"$temporary_directory/expected-error"; then
    printf '%s\n' "Claims ceremony unexpectedly accepted invalid input: $*" >&2
    exit 1
  fi
}

for custodian in 1 2 3; do
  python3 "$repository_root/scripts/generate-release-authority-key.py" \
    --output-key "$temporary_directory/custodian-$custodian.pem" \
    >"$temporary_directory/custodian-$custodian.txt"
done

coordinate() {
  awk -v want="public $2 (display 1):" \
    'index($0, want) == 1 { print $NF }' "$temporary_directory/custodian-$1.txt"
}
{
  printf '{"schema":"tohseno.release-authority-public-keys/1","keys":['
  printf '{"x":"%s","y":"%s"},' "$(coordinate 1 x)" "$(coordinate 1 y)"
  printf '{"x":"%s","y":"%s"},' "$(coordinate 2 x)" "$(coordinate 2 y)"
  printf '{"x":"%s","y":"%s"}]}' "$(coordinate 3 x)" "$(coordinate 3 y)"
} >"$temporary_directory/public-keys.json"

python3 "$repository_root/scripts/prepare-release-authority-policy.py" \
  --public-keys "$temporary_directory/public-keys.json" \
  --issued-at 2026-08-30T12:00:00Z \
  --output "$temporary_directory/policy.json" \
  >"$temporary_directory/policy-report.txt"
policy_digest=$(
  cd "$repository_root" &&
    cargo run --quiet --locked -p tohseno-protocol \
      --example verify_release_authority_policy -- "$temporary_directory/policy.json"
)

jq -cnS \
  --arg policy "$policy_digest" \
  '{
    schema:"tohseno.claims-activation/1",
    protocol:"tohseno",
    component:"TohsenoClaimsV1",
    contract_version:1,
    activation_sequence:1,
    previous_activation:null,
    authority_policy_sha256:$policy,
    chain_id:4663,
    claims_contract:"0x6666666666666666666666666666666666666666",
    shot_registry:"0x3fe6508ba2660bc575080024f402c192a2e035a0",
    creation_code_keccak256:"0x1111111111111111111111111111111111111111111111111111111111111111",
    runtime_code_keccak256:"0x2222222222222222222222222222222222222222222222222222222222222222",
    source_commit:"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    source_tree_sha256:"0x3333333333333333333333333333333333333333333333333333333333333333",
    deployment:{
      transaction_hash:"0x4444444444444444444444444444444444444444444444444444444444444444",
      block_number:12345,
      block_hash:"0x5555555555555555555555555555555555555555555555555555555555555555"
    },
    issued_at:"2026-08-30T13:00:00Z"
  }' | tr -d '\n' >"$temporary_directory/claims-activation.json"

for custodian in 1 2; do
  python3 "$repository_root/scripts/sign-contract-activation.py" \
    --key "$temporary_directory/custodian-$custodian.pem" \
    --activation "$temporary_directory/claims-activation.json" \
    --output "$temporary_directory/approval-$custodian.json" \
    >"$temporary_directory/sign-$custodian.txt"
done

expect_failure python3 "$repository_root/scripts/assemble-signed-activation.py" \
  --activation "$temporary_directory/claims-activation.json" \
  --policy "$temporary_directory/policy.json" \
  --approval "$temporary_directory/approval-1.json" \
  --output "$temporary_directory/insufficient.json"

python3 "$repository_root/scripts/assemble-signed-activation.py" \
  --activation "$temporary_directory/claims-activation.json" \
  --policy "$temporary_directory/policy.json" \
  --approval "$temporary_directory/approval-1.json" \
  --approval "$temporary_directory/approval-2.json" \
  --output "$temporary_directory/signed-claims-activation.json" \
  >"$temporary_directory/assembly.txt"

verification=$(
  cd "$repository_root" &&
    cargo run --quiet --locked -p tohseno-network \
      --example verify_signed_claims_activation -- \
      "$temporary_directory/policy.json" \
      "$temporary_directory/signed-claims-activation.json" \
      "$policy_digest"
)
printf '%s\n' "$verification" | grep -Fqx \
  'claims_contract: 0x6666666666666666666666666666666666666666'
printf '%s\n' "$verification" | grep -Fqx 'approvals_verified: 2'

# The generation activation domain is distinct; changing only the outer
# schema cannot turn these approvals into a generation activation.
jq -cS '.schema = "tohseno.contract-activation/1"' \
  "$temporary_directory/claims-activation.json" | tr -d '\n' \
  >"$temporary_directory/wrong-domain.json"
expect_failure python3 "$repository_root/scripts/assemble-signed-activation.py" \
  --activation "$temporary_directory/wrong-domain.json" \
  --policy "$temporary_directory/policy.json" \
  --approval "$temporary_directory/approval-1.json" \
  --approval "$temporary_directory/approval-2.json" \
  --output "$temporary_directory/wrong-domain-signed.json"

printf '%s\n' "Separate Claims activation ceremony drill passed."
