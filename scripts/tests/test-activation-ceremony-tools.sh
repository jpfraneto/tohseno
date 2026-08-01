#!/bin/sh
# End-to-end drill for the activation ceremony tools with throwaway test keys.
# It exercises keygen -> policy -> activation payload -> custodian signing ->
# envelope assembly, then proves both independent verifier implementations
# accept the result and reject tampering. It never touches production keys,
# writes only under a temporary directory, and activates nothing.
set -eu

script_directory="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_directory/../.." && pwd)"
temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-activation-ceremony.XXXXXX")"

cleanup() {
  case "$temporary_directory" in
    "${TMPDIR:-/tmp}"/tohseno-activation-ceremony.*)
      rm -rf -- "$temporary_directory"
      ;;
    *)
      printf '%s\n' "refusing unsafe temporary cleanup: $temporary_directory" >&2
      ;;
  esac
}
trap cleanup EXIT HUP INT TERM

evidence="$repository_root/contracts/audits/robinhood-inactive-deployment-0.8.0-20260801T021920Z.json"
probe="$repository_root/contracts/audits/robinhood-p256-ceremony-20260801T021920Z.json"
generation="$repository_root/contracts/generations/0.8.0/generation.json"

expect_failure() {
  if "$@" >"$temporary_directory/unexpected-output" 2>"$temporary_directory/expected-error"; then
    printf '%s\n' "ceremony tooling unexpectedly accepted invalid input: $*" >&2
    exit 1
  fi
}

# 1. Three custodian keys on "separate devices" (separate files here).
for custodian in 1 2 3; do
  python3 "$repository_root/scripts/generate-release-authority-key.py" \
    --output-key "$temporary_directory/custodian-$custodian.pem" \
    >"$temporary_directory/custodian-$custodian.txt"
done
expect_failure python3 "$repository_root/scripts/generate-release-authority-key.py" \
  --output-key "$temporary_directory/custodian-1.pem"

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

# 2. The 2-of-3 policy, digest cross-checked by the Rust implementation.
python3 "$repository_root/scripts/prepare-release-authority-policy.py" \
  --public-keys "$temporary_directory/public-keys.json" \
  --issued-at 2026-08-01T12:00:00Z \
  --output "$temporary_directory/policy.json" \
  >"$temporary_directory/policy-report.txt"
policy_digest="$(
  cd "$repository_root" &&
    cargo run --quiet --locked -p tohseno-protocol \
      --example verify_release_authority_policy -- "$temporary_directory/policy.json"
)"

# 3. The activation payload from the real deployment evidence and real probe.
builder_runtime="$(jq -r \
  '.activation_conformance_issue.builder_account_locally_instantiated_runtime_keccak256' \
  "$evidence")"
activation_block="$(jq -r '.chain.latest_observed_block' "$evidence")"
python3 "$repository_root/scripts/prepare-contract-activation.py" \
  --generation "$generation" \
  --policy "$temporary_directory/policy.json" \
  --deployment-evidence "$evidence" \
  --p256-probe "$probe" \
  --builder-account-runtime-keccak256 "$builder_runtime" \
  --activation-block-number "$activation_block" \
  --activation-block-hash 0x3333333333333333333333333333333333333333333333333333333333333333 \
  --issued-at 2026-08-01T13:00:00Z \
  --output "$temporary_directory/activation.json" \
  >"$temporary_directory/activation-report.txt"

# The constructor refuses a compiler-template BuilderAccount hash (ADR 0010).
template_runtime="$(jq -r '.contracts.builder_account.runtime_code_keccak256' "$generation")"
expect_failure python3 "$repository_root/scripts/prepare-contract-activation.py" \
  --generation "$generation" \
  --policy "$temporary_directory/policy.json" \
  --deployment-evidence "$evidence" \
  --p256-probe "$probe" \
  --builder-account-runtime-keccak256 "$template_runtime" \
  --activation-block-number "$activation_block" \
  --activation-block-hash 0x3333333333333333333333333333333333333333333333333333333333333333 \
  --issued-at 2026-08-01T13:00:00Z \
  --output "$temporary_directory/rejected-activation.json"

# 4. Two custodians sign; a signer refuses a non-canonical record.
for custodian in 1 2; do
  python3 "$repository_root/scripts/sign-contract-activation.py" \
    --key "$temporary_directory/custodian-$custodian.pem" \
    --activation "$temporary_directory/activation.json" \
    --output "$temporary_directory/approval-$custodian.json" \
    >"$temporary_directory/approval-$custodian.txt"
done
{ cat "$temporary_directory/activation.json"; printf '\n'; } \
  >"$temporary_directory/non-canonical.json"
expect_failure python3 "$repository_root/scripts/sign-contract-activation.py" \
  --key "$temporary_directory/custodian-1.pem" \
  --activation "$temporary_directory/non-canonical.json" \
  --output "$temporary_directory/never-written.json"

# 5. Assembly enforces the threshold and rejects tampered signatures.
expect_failure python3 "$repository_root/scripts/assemble-signed-activation.py" \
  --activation "$temporary_directory/activation.json" \
  --policy "$temporary_directory/policy.json" \
  --approval "$temporary_directory/approval-1.json" \
  --output "$temporary_directory/underspecified.json"

jq '.authorization.signature.s = "0x1111111111111111111111111111111111111111111111111111111111111111"' \
  "$temporary_directory/approval-2.json" >"$temporary_directory/tampered-approval.json"
expect_failure python3 "$repository_root/scripts/assemble-signed-activation.py" \
  --activation "$temporary_directory/activation.json" \
  --policy "$temporary_directory/policy.json" \
  --approval "$temporary_directory/approval-1.json" \
  --approval "$temporary_directory/tampered-approval.json" \
  --output "$temporary_directory/tampered.json"

python3 "$repository_root/scripts/assemble-signed-activation.py" \
  --activation "$temporary_directory/activation.json" \
  --policy "$temporary_directory/policy.json" \
  --approval "$temporary_directory/approval-1.json" \
  --approval "$temporary_directory/approval-2.json" \
  --output "$temporary_directory/signed-activation.json" \
  >"$temporary_directory/assembly-report.txt"

# 6. Both independent implementations accept the envelope under the pinned
# digest and reject a foreign trust root.
python3 "$repository_root/scripts/verify-contract-activation.py" \
  --repository-root "$repository_root" \
  --generation "$generation" \
  --policy "$temporary_directory/policy.json" \
  --signed-activation "$temporary_directory/signed-activation.json" \
  --p256-probe "$probe" \
  --trusted-policy-sha256 "$policy_digest" \
  >"$temporary_directory/python-report.json"
jq -e \
  --arg policy_digest "$policy_digest" \
  '
    .approved_under_explicit_policy_digest == true
    and .authority_policy_sha256 == $policy_digest
    and .threshold == 2
    and .approvals_verified == 2
  ' "$temporary_directory/python-report.json" >/dev/null

(
  cd "$repository_root"
  cargo run --quiet --locked -p tohseno-protocol \
    --example verify_signed_contract_activation -- \
    "$generation" \
    "$temporary_directory/policy.json" \
    "$temporary_directory/signed-activation.json" \
    "$policy_digest"
) >"$temporary_directory/rust-report.txt"

python_activation="$(jq -r '.activation_signing_sha256' "$temporary_directory/python-report.json")"
rust_activation="$(awk '/^activation_signing_sha256: /{print $2}' "$temporary_directory/rust-report.txt")"
if [ "$python_activation" != "$rust_activation" ]; then
  printf '%s\n' "independent implementations disagree on the activation digest" >&2
  exit 1
fi

wrong_trust="0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
expect_failure sh -c "
  cd '$repository_root' &&
  cargo run --quiet --locked -p tohseno-protocol \
    --example verify_signed_contract_activation -- \
    '$generation' \
    '$temporary_directory/policy.json' \
    '$temporary_directory/signed-activation.json' \
    '$wrong_trust'
"

printf '%s\n' "activation ceremony tooling drill passed: $python_activation"
