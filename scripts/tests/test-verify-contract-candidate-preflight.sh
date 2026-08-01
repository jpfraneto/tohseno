#!/bin/sh
set -eu

script_directory="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_directory/../.." && pwd)"
verifier="$repository_root/scripts/verify-contract-candidate-preflight.py"

python3 - "$verifier" "$repository_root/contracts/generations/0.8.0/generation.json" <<'PY'
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

verifier_path = Path(sys.argv[1])
generation_path = Path(sys.argv[2])
spec = importlib.util.spec_from_file_location("candidate_preflight", verifier_path)
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

expected_methods = {
    "eth_call",
    "eth_chainId",
    "eth_estimateGas",
    "eth_gasPrice",
    "eth_getBalance",
    "eth_getBlockByNumber",
    "eth_getCode",
    "eth_getTransactionCount",
}
assert module.READ_ONLY_RPC_METHODS == expected_methods
source = verifier_path.read_text(encoding="utf-8")
assert "eth_sendTransaction" not in source
assert "eth_sendRawTransaction" not in source
assert "personal_sign" not in source

generation = json.loads(generation_path.read_bytes(), object_pairs_hook=module.closed_object)
digest = hashlib.sha256(module.canonical_json(generation)).hexdigest()
assert digest == module.GENERATION_DEFINITION_SHA256

address = "0xb1bd208cd2af98e701f43d06aaa889d3a594df65"
assert module.simulated_address(address, "test") == address
assert module.simulated_address("0x" + "00" * 12 + address[2:], "test") == address

try:
    module.Rpc("http://example.com")
except module.PreflightError:
    pass
else:
    raise AssertionError("plaintext remote RPC URL was accepted")

try:
    module.Rpc("https://user:secret@example.com")
except module.PreflightError:
    pass
else:
    raise AssertionError("RPC URL userinfo was accepted")
PY

"$verifier" --help >/dev/null
printf '%s\n' "Contract-candidate read-only preflight verifier tests passed."
