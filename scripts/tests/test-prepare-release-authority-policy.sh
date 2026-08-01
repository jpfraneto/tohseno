#!/bin/sh
set -eu

script_directory="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_directory/../.." && pwd)"
preparer="$repository_root/scripts/prepare-release-authority-policy.py"
temporary_directory="$(mktemp -d)"
trap 'rm -rf "$temporary_directory"' EXIT HUP INT TERM

cargo run \
  --quiet \
  --locked \
  -p tohseno-protocol \
  --example generate_contract_activation_fixture \
  >"$temporary_directory/fixture.json"
jq '{
  schema:"tohseno.release-authority-public-keys/1",
  keys:[.policy.authorities[].public_key]
}' "$temporary_directory/fixture.json" >"$temporary_directory/public-keys.json"

python_report="$($preparer \
  --public-keys "$temporary_directory/public-keys.json" \
  --issued-at 2026-08-01T00:00:00Z \
  --output "$temporary_directory/policy.json")"
rust_digest="$(
  cargo run \
    --quiet \
    --locked \
    -p tohseno-protocol \
    --example verify_release_authority_policy \
    -- "$temporary_directory/policy.json"
)"
printf '%s' "$python_report" | jq -e \
  --arg digest "$rust_digest" '
    .schema == "tohseno.release-authority-policy-preparation/1"
    and .policy_sha256 == $digest
    and .authority_count == 3
    and .threshold == 2
    and .authorities_strictly_ordered == true
    and .curve_membership_valid == true
    and .private_key_accessed == false
    and .owner_approved == false
    and .trust_root_installed == false
  ' >/dev/null
jq -e '
  .schema == "tohseno.release-authority-policy/1"
  and .protocol == "tohseno"
  and .protocol_major == 2
  and .purpose == "contract_generation_activation"
  and .threshold == 2
  and (.authorities | length) == 3
  and ([.authorities[].key_id] == ([.authorities[].key_id] | sort))
  and .issued_at == "2026-08-01T00:00:00Z"
' "$temporary_directory/policy.json" >/dev/null

if "$preparer" \
  --public-keys "$temporary_directory/public-keys.json" \
  --issued-at 2026-08-01T00:00:00Z \
  --output "$temporary_directory/policy.json" \
  >"$temporary_directory/overwrite.out" 2>"$temporary_directory/overwrite.err"; then
  printf '%s\n' "policy preparer overwrote an existing output" >&2
  exit 1
fi

jq '.keys[1] = .keys[0]' \
  "$temporary_directory/public-keys.json" >"$temporary_directory/duplicate.json"
if "$preparer" \
  --public-keys "$temporary_directory/duplicate.json" \
  --issued-at 2026-08-01T00:00:00Z \
  --output "$temporary_directory/duplicate-policy.json" \
  >"$temporary_directory/duplicate.out" 2>"$temporary_directory/duplicate.err"; then
  printf '%s\n' "policy preparer accepted a duplicate key" >&2
  exit 1
fi

jq '.keys[0].y = "0x0000000000000000000000000000000000000000000000000000000000000000"' \
  "$temporary_directory/public-keys.json" >"$temporary_directory/off-curve.json"
if "$preparer" \
  --public-keys "$temporary_directory/off-curve.json" \
  --issued-at 2026-08-01T00:00:00Z \
  --output "$temporary_directory/off-curve-policy.json" \
  >"$temporary_directory/off-curve.out" 2>"$temporary_directory/off-curve.err"; then
  printf '%s\n' "policy preparer accepted an off-curve key" >&2
  exit 1
fi

if "$preparer" \
  --public-keys "$temporary_directory/public-keys.json" \
  --issued-at 2026-08-01T00:00:00+00:00 \
  --output "$temporary_directory/bad-time-policy.json" \
  >"$temporary_directory/bad-time.out" 2>"$temporary_directory/bad-time.err"; then
  printf '%s\n' "policy preparer accepted a non-canonical timestamp" >&2
  exit 1
fi

if grep -Eq 'eth_send|personal_sign|security find|PRIVATE_KEY|SigningKey' "$preparer"; then
  printf '%s\n' "policy preparer contains a signing, Keychain, or transaction path" >&2
  exit 1
fi

printf '%s\n' "Release-authority public-policy preparation tests passed."
