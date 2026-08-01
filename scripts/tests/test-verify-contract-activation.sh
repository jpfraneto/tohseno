#!/bin/sh
set -eu

script_directory="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_directory/../.." && pwd)"
temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-activation-verifier.XXXXXX")"

cleanup() {
  case "$temporary_directory" in
    "${TMPDIR:-/tmp}"/tohseno-activation-verifier.*)
      rm -rf -- "$temporary_directory"
      ;;
    *)
      printf '%s\n' "refusing unsafe temporary cleanup: $temporary_directory" >&2
      ;;
  esac
}
trap cleanup EXIT HUP INT TERM

fixture="$temporary_directory/fixture.json"
policy="$temporary_directory/policy.json"
signed_activation="$temporary_directory/signed-activation.json"
p256_probe="$temporary_directory/p256-probe.json"
report="$temporary_directory/report.json"

(
  cd "$repository_root"
  cargo run -q -p tohseno-protocol --example generate_contract_activation_fixture >"$fixture"
)
jq '.policy' "$fixture" >"$policy"
jq '.signed_activation' "$fixture" >"$signed_activation"
jq -rj '.p256_probe_raw' "$fixture" >"$p256_probe"
trusted_policy_sha256="$(jq -r '.authority_policy_sha256' "$fixture")"

verify() {
  python3 "$repository_root/scripts/verify-contract-activation.py" \
    --repository-root "$repository_root" \
    --generation "$repository_root/contracts/generations/0.8.0/generation.json" \
    --policy "$1" \
    --signed-activation "$2" \
    --p256-probe "$3" \
    --trusted-policy-sha256 "$4"
}

verify "$policy" "$signed_activation" "$p256_probe" "$trusted_policy_sha256" >"$report"
jq -e \
  --arg generation "$(jq -r '.generation_definition_sha256' "$fixture")" \
  --arg policy_digest "$trusted_policy_sha256" \
  --arg activation "$(jq -r '.activation_signing_sha256' "$fixture")" \
  '
    .schema == "tohseno.contract-activation-independent-verification/1"
    and .approved_under_explicit_policy_digest == true
    and .generation_definition_sha256 == $generation
    and .authority_policy_sha256 == $policy_digest
    and .activation_signing_sha256 == $activation
    and .threshold == 2
    and .approvals_verified == 2
  ' "$report" >/dev/null

expect_failure() {
  if "$@" >"$temporary_directory/unexpected-output" 2>"$temporary_directory/expected-error"; then
    printf '%s\n' "activation verifier unexpectedly accepted invalid evidence" >&2
    exit 1
  fi
}

wrong_trust="0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
expect_failure verify "$policy" "$signed_activation" "$p256_probe" "$wrong_trust"

insufficient="$temporary_directory/insufficient.json"
jq '.approvals = [.approvals[0]]' "$signed_activation" >"$insufficient"
expect_failure verify "$policy" "$insufficient" "$p256_probe" "$trusted_policy_sha256"

reordered="$temporary_directory/reordered.json"
jq '.approvals = [.approvals[1], .approvals[0]]' "$signed_activation" >"$reordered"
expect_failure verify "$policy" "$reordered" "$p256_probe" "$trusted_policy_sha256"

high_s="$temporary_directory/high-s.json"
jq '.approvals[0].authorization.signature.s = "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"' \
  "$signed_activation" >"$high_s"
expect_failure verify "$policy" "$high_s" "$p256_probe" "$trusted_policy_sha256"

wrong_runtime="$temporary_directory/wrong-runtime.json"
jq '.activation.factory.runtime_code_keccak256 = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"' \
  "$signed_activation" >"$wrong_runtime"
expect_failure verify "$policy" "$wrong_runtime" "$p256_probe" "$trusted_policy_sha256"

instantiated_runtime="$temporary_directory/instantiated-runtime.json"
jq '
  .activation.builder_account_runtime_keccak256 = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  | .activation.registry.runtime_code_keccak256 = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
' "$signed_activation" >"$instantiated_runtime"
# Re-signing is intentionally outside this shell fixture. Shape/generation
# validation must accept instantiated hashes even though the existing detached
# approvals no longer match the modified payload.
REPOSITORY_ROOT="$repository_root" SIGNED_ACTIVATION="$instantiated_runtime" \
  GENERATION="$repository_root/contracts/generations/0.8.0/generation.json" python3 - <<'PY'
import importlib.util
import json
import os
from pathlib import Path

root = Path(os.environ["REPOSITORY_ROOT"])
spec = importlib.util.spec_from_file_location(
    "tohseno_activation_verifier",
    root / "scripts/verify-contract-activation.py",
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
generation_path = Path(os.environ["GENERATION"])
generation = module.load_strict(generation_path)
generation_digest = module.validate_generation(generation, generation_path, root)
activation = json.loads(Path(os.environ["SIGNED_ACTIVATION"]).read_text())["activation"]
module.validate_activation(
    activation,
    generation,
    generation_digest,
    module.hex32(activation["authority_policy_sha256"], "authority_policy_sha256"),
    module.hex32(activation["p256_probe_sha256"], "p256_probe_sha256"),
)
PY

wrong_probe="$temporary_directory/wrong-probe.json"
jq '.gas.measured_precompile = 3450' "$p256_probe" >"$wrong_probe"
expect_failure verify "$policy" "$signed_activation" "$wrong_probe" "$trusted_policy_sha256"

wrong_generation_directory="$temporary_directory/wrong-generation"
cp -R "$repository_root/contracts/generations/0.8.0" "$wrong_generation_directory"
jq '.create2.builder_account_factory.predicted_address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"' \
  "$wrong_generation_directory/generation.json" >"$wrong_generation_directory/mutated.json"
if python3 "$repository_root/scripts/verify-contract-activation.py" \
  --repository-root "$repository_root" \
  --generation "$wrong_generation_directory/mutated.json" \
  --policy "$policy" \
  --signed-activation "$signed_activation" \
  --p256-probe "$p256_probe" \
  --trusted-policy-sha256 "$trusted_policy_sha256" \
  >"$temporary_directory/unexpected-coordinate-output" \
  2>"$temporary_directory/expected-coordinate-error"; then
  printf '%s\n' "activation verifier unexpectedly accepted an invalid CREATE2 coordinate" >&2
  exit 1
fi
grep -F -- 'predicted_address is not the CREATE2 result' \
  "$temporary_directory/expected-coordinate-error" >/dev/null

REPOSITORY_ROOT="$repository_root" SIGNED_ACTIVATION="$signed_activation" python3 - <<'PY'
import importlib.util
import json
import os
from pathlib import Path

root = Path(os.environ["REPOSITORY_ROOT"])
spec = importlib.util.spec_from_file_location(
    "tohseno_activation_verifier",
    root / "scripts/verify-contract-activation.py",
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
previous = json.loads(Path(os.environ["SIGNED_ACTIVATION"]).read_text())["activation"]
previous["chain_id"] = 0
try:
    module.validate_activation_shape(previous, "previous_activation")
except module.VerificationError as error:
    if "previous_activation.chain_id" not in str(error):
        raise
else:
    raise SystemExit("malformed predecessor activation was accepted")
PY

duplicate_policy="$temporary_directory/duplicate-policy.json"
printf '%s\n' \
  '{"schema":"tohseno.release-authority-policy/1","schema":"tohseno.release-authority-policy/1"}' \
  >"$duplicate_policy"
expect_failure verify "$duplicate_policy" "$signed_activation" "$p256_probe" "$trusted_policy_sha256"

printf '%s\n' "independent contract-activation verifier tests passed."
