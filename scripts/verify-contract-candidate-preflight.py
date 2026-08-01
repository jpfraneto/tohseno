#!/usr/bin/env python3
"""Read-only Robinhood preflight for the frozen TOHSENO 0.8.0 candidate.

This program cannot sign or broadcast. Its JSON-RPC allowlist contains only
read methods, and it never reads a wallet, Keychain item, private key, or seed.
The resulting evidence is historical preparation, not deployment authority.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from typing import Any


CHAIN_ID = 4663
GENERATION = "0.8.0"
GENERATION_DEFINITION_SHA256 = "618fbd1ef9f826a2a642f94733192b130023e7f85db95d9ab066768e8abdf895"
CREATE2_DEPLOYER_RUNTIME_KECCAK256 = (
    "0x2fa86add0aed31f33a762c9d88e807c475bd51d0f52bd0955754b2608f7e4989"
)
CREATE2_DEPLOYER_RUNTIME_BYTES = 69
READ_ONLY_RPC_METHODS = frozenset(
    {
        "eth_chainId",
        "eth_getBlockByNumber",
        "eth_getCode",
        "eth_getBalance",
        "eth_getTransactionCount",
        "eth_gasPrice",
        "eth_call",
        "eth_estimateGas",
    }
)
ADDRESS_PATTERN = re.compile(r"^0x[0-9a-fA-F]{40}$")
HEX_PATTERN = re.compile(r"^0x(?:[0-9a-fA-F]{2})*$")
QUANTITY_PATTERN = re.compile(r"^0x(?:0|[1-9a-fA-F][0-9a-fA-F]*)$")


class PreflightError(RuntimeError):
    pass


class DuplicateMember(ValueError):
    pass


def closed_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateMember(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def load_json_bytes(raw: bytes, label: str) -> Any:
    try:
        return json.loads(raw, object_pairs_hook=closed_object)
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise PreflightError(f"{label} is not duplicate-key-strict JSON: {error}") from error


def canonical_json(value: Any) -> bytes:
    """Encode the protocol's integer-only RFC 8785 JSON domain."""

    def encode(member: Any) -> str:
        if member is None:
            return "null"
        if member is True:
            return "true"
        if member is False:
            return "false"
        if isinstance(member, int) and not isinstance(member, bool):
            require(0 <= member <= 9_007_199_254_740_991, "JSON integer is outside the protocol domain")
            return str(member)
        if isinstance(member, str):
            require(not any(0xD800 <= ord(character) <= 0xDFFF for character in member), "JSON string contains a surrogate")
            return json.dumps(member, ensure_ascii=False, separators=(",", ":"))
        if isinstance(member, list):
            return "[" + ",".join(encode(item) for item in member) + "]"
        if isinstance(member, dict):
            require(all(isinstance(key, str) for key in member), "JSON object key is not a string")
            ordered = sorted(member, key=lambda key: key.encode("utf-16-be"))
            return "{" + ",".join(encode(key) + ":" + encode(member[key]) for key in ordered) + "}"
        raise PreflightError(f"unsupported JSON type: {type(member).__name__}")

    return encode(value).encode("utf-8")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PreflightError(message)


def run(command: list[str], cwd: Path) -> str:
    try:
        completed = subprocess.run(
            command,
            cwd=cwd,
            check=True,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError as error:
        raise PreflightError(f"required tool is missing: {command[0]}") from error
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or error.stdout or "command failed").strip()
        raise PreflightError(f"{command[0]} failed: {detail}") from error
    return completed.stdout.strip()


def canonical_address(value: str, field: str) -> str:
    require(bool(ADDRESS_PATTERN.fullmatch(value)), f"{field} must be a 20-byte address")
    return value.lower()


def canonical_data(value: str, field: str) -> str:
    require(bool(HEX_PATTERN.fullmatch(value)), f"{field} must be even-length hexadecimal data")
    return value.lower()


def quantity(value: Any, field: str) -> int:
    require(isinstance(value, str), f"{field} must be a hexadecimal quantity string")
    require(bool(QUANTITY_PATTERN.fullmatch(value)), f"{field} is not a canonical hexadecimal quantity")
    return int(value, 16)


class Rpc:
    def __init__(self, url: str) -> None:
        parsed = urllib.parse.urlsplit(url)
        local_test = parsed.scheme == "http" and parsed.hostname == "127.0.0.1"
        require(parsed.scheme == "https" or local_test, "remote RPC URL must use HTTPS")
        require(parsed.username is None and parsed.password is None, "RPC URL must not contain userinfo")
        require(not parsed.fragment, "RPC URL must not contain a fragment")
        require(bool(parsed.hostname), "RPC URL must have a host")
        self.url = url
        self.next_id = 1

    def call(self, method: str, params: list[Any]) -> Any:
        require(method in READ_ONLY_RPC_METHODS, f"JSON-RPC method is not read-only: {method}")
        request_id = self.next_id
        self.next_id += 1
        body = json.dumps(
            {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params},
            separators=(",", ":"),
        ).encode()
        request = urllib.request.Request(
            self.url,
            data=body,
            headers={"Content-Type": "application/json", "User-Agent": "tohseno-preflight/1"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                raw = response.read(1_048_577)
        except (urllib.error.URLError, TimeoutError, OSError) as error:
            raise PreflightError(f"{method} transport failed: {error}") from error
        require(len(raw) <= 1_048_576, f"{method} response exceeded 1 MiB")
        value = load_json_bytes(raw, f"{method} response")
        require(isinstance(value, dict), f"{method} response must be an object")
        require(set(value) == {"jsonrpc", "id", "result"}, f"{method} returned an error or extra members")
        require(value["jsonrpc"] == "2.0", f"{method} returned the wrong JSON-RPC version")
        require(type(value["id"]) is int and value["id"] == request_id, f"{method} response ID mismatch")
        return value["result"]


def inspect_bytecode(root: Path, contract: str, kind: str) -> str:
    value = run(["forge", "inspect", "--root", "contracts", contract, kind], root)
    canonical_data(value, f"{contract} {kind}")
    require(value != "0x", f"{contract} {kind} is empty")
    return value.lower()


def cast_keccak(root: Path, value: str) -> str:
    digest = run(["cast", "keccak", value], root).lower()
    require(bool(re.fullmatch(r"0x[0-9a-f]{64}", digest)), "cast keccak returned malformed output")
    return digest


def cast_create2(root: Path, deployer: str, salt: str, init_hash: str) -> str:
    result = run(
        ["cast", "create2", "--deployer", deployer, "--salt", salt, "--init-code-hash", init_hash],
        root,
    )
    return canonical_address(result, "cast create2 result")


def code_bytes(value: str, field: str) -> int:
    return (len(canonical_data(value, field)) - 2) // 2


def simulated_address(value: Any, field: str) -> str:
    require(isinstance(value, str), f"{field} must be hexadecimal data")
    data = canonical_data(value, field)
    if len(data) == 42:
        return data
    require(len(data) == 66, f"{field} must return a 20-byte address or one 32-byte word")
    require(data[2:26] == "0" * 24, f"{field} returned a non-address word")
    return "0x" + data[-40:]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rpc-url", required=True)
    parser.add_argument("--payer", required=True)
    parser.add_argument("--output")
    args = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    generation_path = root / "contracts" / "generations" / GENERATION / "generation.json"
    raw_generation = generation_path.read_bytes()
    generation = load_json_bytes(raw_generation, "generation definition")
    require(isinstance(generation, dict), "generation definition must be an object")
    require(
        hashlib.sha256(canonical_json(generation)).hexdigest() == GENERATION_DEFINITION_SHA256,
        "canonical generation definition digest differs from the frozen 0.8.0 candidate",
    )
    require(generation.get("schema") == "tohseno.contract-generation/1", "unexpected generation schema")
    require(generation.get("generation") == GENERATION, "unexpected generation version")

    # This check recompiles and byte-compares all frozen artifacts without writing
    # repository state.
    run([str(root / "scripts" / "build-contract-abi.sh"), "--check"], root)

    create2 = generation["create2"]
    contracts = generation["contracts"]
    deployer = canonical_address(create2["deployer"], "create2.deployer")
    payer = canonical_address(args.payer, "payer")

    factory_creation = inspect_bytecode(root, "BuilderAccountFactory", "bytecode")
    registry_creation = inspect_bytecode(root, "ShotRegistry", "bytecode")
    factory_runtime = inspect_bytecode(root, "BuilderAccountFactory", "deployedBytecode")
    registry_runtime = inspect_bytecode(root, "ShotRegistry", "deployedBytecode")

    entries: list[dict[str, Any]] = []
    for key, label, creation, runtime in (
        ("builder_account_factory", "BuilderAccountFactory", factory_creation, factory_runtime),
        ("shot_registry", "ShotRegistry", registry_creation, registry_runtime),
    ):
        definition = create2[key]
        contract_definition = contracts[key]
        salt = canonical_data(definition["salt"], f"create2.{key}.salt")
        require(len(salt) == 66, f"create2.{key}.salt must be 32 bytes")
        init_hash = cast_keccak(root, creation)
        runtime_hash = cast_keccak(root, runtime)
        require(init_hash == definition["init_code_keccak256"].lower(), f"{label} init-code hash drift")
        require(init_hash == contract_definition["creation_code_keccak256"].lower(), f"{label} creation hash drift")
        require(runtime_hash == contract_definition["runtime_code_keccak256"].lower(), f"{label} runtime hash drift")
        predicted = cast_create2(root, deployer, salt, init_hash)
        require(predicted == definition["predicted_address"].lower(), f"{label} predicted address drift")
        entries.append(
            {
                "key": key,
                "label": label,
                "salt": salt,
                "creation": creation,
                "init_code_keccak256": init_hash,
                "runtime_code_keccak256": runtime_hash,
                "predicted_address": predicted,
                "calldata": salt + creation[2:],
            }
        )

    rpc = Rpc(args.rpc_url)
    require(quantity(rpc.call("eth_chainId", []), "eth_chainId") == CHAIN_ID, "RPC is not Robinhood mainnet")
    latest = rpc.call("eth_getBlockByNumber", ["latest", False])
    require(isinstance(latest, dict), "latest block must be an object")
    block_number_hex = latest.get("number")
    block_hash = latest.get("hash")
    block_number = quantity(block_number_hex, "latest block number")
    require(isinstance(block_hash, str) and bool(re.fullmatch(r"0x[0-9a-fA-F]{64}", block_hash)), "latest block hash is malformed")
    block_hash = block_hash.lower()
    block_reference = {"blockHash": block_hash, "requireCanonical": True}

    deployer_code = rpc.call("eth_getCode", [deployer, block_reference])
    deployer_code = canonical_data(deployer_code, "CREATE2 deployer code")
    require(code_bytes(deployer_code, "CREATE2 deployer code") == CREATE2_DEPLOYER_RUNTIME_BYTES, "CREATE2 deployer runtime length mismatch")
    require(cast_keccak(root, deployer_code) == CREATE2_DEPLOYER_RUNTIME_KECCAK256, "CREATE2 deployer runtime hash mismatch")

    balance = quantity(rpc.call("eth_getBalance", [payer, block_reference]), "payer balance")
    nonce = quantity(rpc.call("eth_getTransactionCount", [payer, block_reference]), "payer nonce")
    gas_price = quantity(rpc.call("eth_gasPrice", []), "gas price")

    for entry in entries:
        target_code = rpc.call("eth_getCode", [entry["predicted_address"], block_reference])
        require(code_bytes(target_code, f"{entry['label']} target code") == 0, f"{entry['label']} target already has code")
        transaction = {"from": payer, "to": deployer, "data": entry["calldata"]}
        simulated = simulated_address(
            rpc.call("eth_call", [transaction, block_reference]),
            f"{entry['label']} simulation",
        )
        require(simulated == entry["predicted_address"], f"{entry['label']} simulation returned the wrong address")
        estimated_gas = quantity(
            rpc.call("eth_estimateGas", [transaction, block_reference]),
            f"{entry['label']} gas estimate",
        )
        entry["target_code_bytes"] = 0
        entry["simulated_return"] = simulated
        entry["calldata_byte_length"] = (len(entry["calldata"]) - 2) // 2
        entry["estimated_gas"] = estimated_gas
        entry["estimated_cost_wei_at_observed_gas_price"] = str(estimated_gas * gas_price)
        del entry["creation"]
        del entry["calldata"]

    canonical = rpc.call("eth_getBlockByNumber", [hex(block_number), False])
    require(isinstance(canonical, dict), "canonicality recheck block must be an object")
    require(canonical.get("hash", "").lower() == block_hash, "pinned block is no longer canonical")

    total_estimated_cost = sum(int(entry["estimated_cost_wei_at_observed_gas_price"]) for entry in entries)
    require(balance >= total_estimated_cost, "payer balance is below the combined observed gas estimate")
    evidence = {
        "schema": "tohseno.contract-candidate-preflight/1",
        "checked_at": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "authorization_value": "historical_read_only_preparation",
        "reusable_for_broadcast": False,
        "generation": {
            "version": GENERATION,
            "definition_sha256": "0x" + GENERATION_DEFINITION_SHA256,
            "definition_file_sha256": "0x" + hashlib.sha256(raw_generation).hexdigest(),
            "source_commit": generation["source"]["commit"],
            "source_tree_sha256": generation["source"]["tree_sha256"],
            "artifacts_rebuilt_and_byte_compared": True,
        },
        "chain": {
            "chain_id": CHAIN_ID,
            "rpc": "explicit-target",
            "block_number": block_number,
            "block_number_hex": block_number_hex.lower(),
            "block_hash": block_hash,
            "require_canonical": True,
            "rechecked_after_simulation": True,
        },
        "payer": {
            "address": payer,
            "nonce": nonce,
            "balance_wei": str(balance),
            "observed_gas_price_wei": str(gas_price),
            "combined_estimated_cost_wei": str(total_estimated_cost),
            "balance_covers_combined_estimate": True,
        },
        "create2_deployer": {
            "address": deployer,
            "runtime_byte_length": CREATE2_DEPLOYER_RUNTIME_BYTES,
            "runtime_keccak256": CREATE2_DEPLOYER_RUNTIME_KECCAK256,
        },
        "contracts": {entry.pop("key"): entry for entry in entries},
        "safety": {
            "read_only_rpc_methods": sorted(READ_ONLY_RPC_METHODS),
            "private_key_accessed": False,
            "transaction_signed": False,
            "transaction_broadcast": False,
            "deployment_authorized": False,
        },
        "valid": True,
    }
    rendered = json.dumps(evidence, sort_keys=True, separators=(",", ":")) + "\n"
    if args.output:
        output = Path(args.output)
        require(output.parent.is_dir(), "output parent directory does not exist")
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        descriptor = os.open(output, flags, 0o644)
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(rendered)
    sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, KeyError, TypeError, PreflightError) as error:
        print(f"verify-contract-candidate-preflight.py: {error}", file=sys.stderr)
        raise SystemExit(1)
