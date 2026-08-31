#!/usr/bin/env python3
"""Prepare a canonical Claims activation from a verified direct deployment.

This read-only ceremony tool requires a clean checkout at the exact source
commit. It re-derives the complete creation transaction from the committed
TohsenoClaimsV1 bytecode and Registry constructor argument, verifies the
canonical Robinhood receipt/block, re-reads deployed code and the immutable
Registry reference, and writes one unsigned activation for offline custodians.

It never broadcasts a transaction, holds a deployment key, signs an approval,
or activates a client or service.
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
import urllib.parse
import urllib.request
from typing import Any

ACTIVATION_DOMAIN = b"TOHSENO-CLAIMS-ACTIVATION-V1\0"
ACTIVATION_SCHEMA = "tohseno.claims-activation/1"
POLICY_SCHEMA = "tohseno.release-authority-policy/1"
CHAIN_ID = 4663
ACTIVE_REGISTRY = "0x3fe6508ba2660bc575080024f402c192a2e035a0"
MAX_SAFE_JSON_INTEGER = 9_007_199_254_740_991
HEX20 = re.compile(r"^0x[0-9a-f]{40}$")
HEX32 = re.compile(r"^0x[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
TIMESTAMP = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")


class PreparationError(RuntimeError):
    pass


class DuplicateMember(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PreparationError(message)


def closed_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, member in pairs:
        if key in value:
            raise DuplicateMember(f"duplicate JSON member: {key}")
        value[key] = member
    return value


def reject_float(value: str) -> None:
    raise PreparationError(f"floating-point JSON number is forbidden: {value}")


def reject_constant(value: str) -> None:
    raise PreparationError(f"non-finite JSON number is forbidden: {value}")


def load_json(path: Path) -> Any:
    require(path.is_file() and not path.is_symlink(), f"not a regular input file: {path}")
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=closed_object,
            parse_float=reject_float,
            parse_constant=reject_constant,
        )
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise PreparationError(f"invalid JSON input {path}: {error}") from error


def canonical_json(value: Any) -> bytes:
    def encode(member: Any) -> str:
        if member is None:
            return "null"
        if member is True:
            return "true"
        if member is False:
            return "false"
        if isinstance(member, int) and not isinstance(member, bool):
            require(0 <= member <= MAX_SAFE_JSON_INTEGER, "JSON integer is outside the safe domain")
            return str(member)
        if isinstance(member, str):
            require(
                not any(0xD800 <= ord(character) <= 0xDFFF for character in member),
                "JSON string contains a surrogate",
            )
            return json.dumps(member, ensure_ascii=False, separators=(",", ":"))
        if isinstance(member, list):
            return "[" + ",".join(encode(item) for item in member) + "]"
        if isinstance(member, dict):
            require(all(isinstance(key, str) for key in member), "JSON key is not a string")
            keys = sorted(member, key=lambda key: key.encode("utf-16-be"))
            return "{" + ",".join(encode(key) + ":" + encode(member[key]) for key in keys) + "}"
        raise PreparationError(f"unsupported JSON type: {type(member).__name__}")

    return encode(value).encode("utf-8")


def command(arguments: list[str], cwd: Path, *, binary: bool = False) -> bytes | str:
    result = subprocess.run(arguments, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    require(
        result.returncode == 0,
        f"{' '.join(arguments)} failed: {result.stderr.decode('utf-8', 'replace').strip()}",
    )
    return result.stdout if binary else result.stdout.decode("utf-8").strip()


class Rpc:
    def __init__(self, url: str):
        parsed = urllib.parse.urlsplit(url)
        require(parsed.scheme == "https" and parsed.hostname is not None, "RPC URL must be absolute HTTPS")
        require(
            parsed.username is None
            and parsed.password is None
            and not parsed.query
            and not parsed.fragment,
            "RPC URL must not contain credentials, a query, or a fragment",
        )
        self.url = url
        self.identifier = 0

    def call(self, method: str, parameters: list[Any]) -> Any:
        self.identifier += 1
        payload = json.dumps(
            {"jsonrpc": "2.0", "id": self.identifier, "method": method, "params": parameters},
            separators=(",", ":"),
        ).encode("utf-8")
        request = urllib.request.Request(
            self.url,
            data=payload,
            headers={"Content-Type": "application/json", "User-Agent": "tohseno-claims-activation/1"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=20) as response:
                body = response.read(2 * 1024 * 1024)
        except Exception as error:
            raise PreparationError(f"Robinhood RPC request failed for {method}: {error}") from error
        decoded = json.loads(body, object_pairs_hook=closed_object, parse_float=reject_float, parse_constant=reject_constant)
        require(decoded.get("error") is None, f"Robinhood RPC rejected {method}: {decoded.get('error')}")
        require("result" in decoded, f"Robinhood RPC omitted the {method} result")
        return decoded["result"]


def exact_hex(field: str, value: Any, pattern: re.Pattern[str]) -> str:
    require(isinstance(value, str) and pattern.fullmatch(value) is not None, f"{field} has invalid lowercase hex")
    return value


def quantity(field: str, value: Any) -> int:
    require(isinstance(value, str) and re.fullmatch(r"0x(?:0|[1-9a-f][0-9a-f]*)", value) is not None, f"{field} is not a canonical RPC quantity")
    parsed = int(value, 16)
    require(0 < parsed <= MAX_SAFE_JSON_INTEGER, f"{field} is outside the supported range")
    return parsed


def keccak(repository: Path, hex_value: str) -> str:
    value = command(["cast", "keccak", hex_value], repository)
    return exact_hex("keccak256", value, HEX32)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository-root", required=True, type=Path)
    parser.add_argument("--rpc-url", required=True)
    parser.add_argument("--policy", required=True, type=Path)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--claims-contract", required=True)
    parser.add_argument("--deployment-transaction", required=True)
    parser.add_argument("--issued-at", required=True)
    parser.add_argument("--activation-sequence", type=int, default=1)
    parser.add_argument("--previous-activation", default=None)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()

    repository = arguments.repository_root.resolve(strict=True)
    require((repository / ".git").exists(), "repository root is not a Git checkout")
    require(COMMIT.fullmatch(arguments.source_commit) is not None, "source commit must be 40 lowercase hex")
    head = command(["git", "rev-parse", "HEAD^{commit}"], repository)
    require(head == arguments.source_commit, "checkout HEAD differs from the declared source commit")
    require(command(["git", "status", "--porcelain=v1", "--untracked-files=all"], repository) == "", "source checkout must be completely clean")

    policy = load_json(arguments.policy)
    require(policy.get("schema") == POLICY_SCHEMA, f"policy schema must be {POLICY_SCHEMA}")
    policy_digest = "0x" + hashlib.sha256(canonical_json(policy)).hexdigest()

    claims_contract = exact_hex("Claims contract", arguments.claims_contract, HEX20)
    transaction_hash = exact_hex("deployment transaction", arguments.deployment_transaction, HEX32)
    require(TIMESTAMP.fullmatch(arguments.issued_at) is not None, "issued-at must be canonical UTC seconds")
    try:
        issued = dt.datetime.strptime(arguments.issued_at, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=dt.timezone.utc)
    except ValueError as error:
        raise PreparationError(f"issued-at is invalid: {error}") from error
    require(issued.timestamp() > 0, "issued-at must follow the Unix epoch")
    require(0 < arguments.activation_sequence <= MAX_SAFE_JSON_INTEGER, "activation sequence is invalid")
    if arguments.activation_sequence == 1:
        require(arguments.previous_activation is None, "activation one cannot have a predecessor")
        previous = None
    else:
        previous = exact_hex("previous activation", arguments.previous_activation, HEX32)

    rpc = Rpc(arguments.rpc_url)
    require(int(rpc.call("eth_chainId", []), 16) == CHAIN_ID, "RPC is not Robinhood Chain mainnet")
    receipt = rpc.call("eth_getTransactionReceipt", [transaction_hash])
    transaction = rpc.call("eth_getTransactionByHash", [transaction_hash])
    require(isinstance(receipt, dict) and isinstance(transaction, dict), "deployment is not canonically available")
    require(receipt.get("status") == "0x1", "Claims deployment transaction did not succeed")
    require(receipt.get("contractAddress") == claims_contract, "receipt contract address differs")
    require(transaction.get("to") is None, "Claims must be deployed by a direct creation transaction")
    require(transaction.get("hash") == transaction_hash, "deployment transaction hash differs")
    block_number_hex = receipt.get("blockNumber")
    block_number = quantity("deployment block", block_number_hex)
    block_hash = exact_hex("deployment block hash", receipt.get("blockHash"), HEX32)
    block = rpc.call("eth_getBlockByNumber", [block_number_hex, False])
    require(isinstance(block, dict) and block.get("hash") == block_hash, "deployment block is no longer canonical")

    creation_path = repository / "contracts/bytecode/TohsenoClaimsV1.creation.hex"
    require(creation_path.is_file() and not creation_path.is_symlink(), "committed Claims creation bytecode is absent")
    creation = creation_path.read_text(encoding="ascii").strip()
    require(re.fullmatch(r"0x[0-9a-f]+", creation) is not None and len(creation) % 2 == 0, "Claims creation bytecode is invalid")
    constructor = "0" * 24 + ACTIVE_REGISTRY[2:]
    require(transaction.get("input") == creation + constructor, "deployment input differs from committed creation bytecode and exact Registry constructor")

    code_at_deployment = rpc.call("eth_getCode", [claims_contract, block_number_hex])
    code_now = rpc.call("eth_getCode", [claims_contract, "latest"])
    require(isinstance(code_at_deployment, str) and code_at_deployment.startswith("0x") and len(code_at_deployment) > 2, "Claims runtime is absent at deployment block")
    require(code_now == code_at_deployment, "Claims runtime changed after deployment")
    selector = command(["cast", "sig", "shotRegistry()"], repository)
    observed_registry = rpc.call("eth_call", [{"to": claims_contract, "data": selector}, "latest"])
    require(observed_registry == "0x" + "0" * 24 + ACTIVE_REGISTRY[2:], "Claims immutable Registry differs from active generation 0.8")
    require(rpc.call("eth_getCode", [ACTIVE_REGISTRY, "latest"]) not in (None, "0x"), "active Registry runtime is absent")

    archive = command(["git", "archive", "--format=tar", arguments.source_commit], repository, binary=True)
    source_tree_sha256 = "0x" + hashlib.sha256(archive).hexdigest()
    activation = {
        "schema": ACTIVATION_SCHEMA,
        "protocol": "tohseno",
        "component": "TohsenoClaimsV1",
        "contract_version": 1,
        "activation_sequence": arguments.activation_sequence,
        "previous_activation": previous,
        "authority_policy_sha256": policy_digest,
        "chain_id": CHAIN_ID,
        "claims_contract": claims_contract,
        "shot_registry": ACTIVE_REGISTRY,
        "creation_code_keccak256": keccak(repository, creation),
        "runtime_code_keccak256": keccak(repository, code_now),
        "source_commit": arguments.source_commit,
        "source_tree_sha256": source_tree_sha256,
        "deployment": {
            "transaction_hash": transaction_hash,
            "block_number": block_number,
            "block_hash": block_hash,
        },
        "issued_at": arguments.issued_at,
    }
    encoded = canonical_json(activation)
    digest = "0x" + hashlib.sha256(ACTIVATION_DOMAIN + encoded).hexdigest()
    require(arguments.output.is_absolute(), f"output path must be absolute: {arguments.output}")
    descriptor = os.open(arguments.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(encoded)

    print(f"unsigned Claims activation written: {arguments.output}")
    print(f"source archive sha256: {source_tree_sha256}")
    print(f"creation code keccak256: {activation['creation_code_keccak256']}")
    print(f"runtime code keccak256: {activation['runtime_code_keccak256']}")
    print(f"Claims activation signing sha256: {digest}")
    print("next step: two offline release-authority custodians inspect and sign this exact file")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (PreparationError, DuplicateMember, KeyError, TypeError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
