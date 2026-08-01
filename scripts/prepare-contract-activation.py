#!/usr/bin/env python3
"""Construct the canonical contract-activation record for custodian review.

The coordinator runs this on the ceremony workstation. It binds the exact
generation definition, the approved policy digest, the observed inactive
deployment evidence, the canary-established BuilderAccount instance hash, a
fresh activation block observation, and the fresh EIP-7951 probe evidence into
one closed `tohseno.contract-activation/1` record, then reports the
domain-separated signing digest custodians must independently inspect and
sign.

This tool holds no private keys and produces no signature. Writing an
activation record does not approve it, and a signed record activates nothing
until a client separately pins the policy digest as its trust root.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import sys
from typing import Any

ACTIVATION_SCHEMA = "tohseno.contract-activation/1"
ACTIVATION_DOMAIN = b"TOHSENO-CONTRACT-ACTIVATION-V1\0"
POLICY_SCHEMA = "tohseno.release-authority-policy/1"
DEPLOYMENT_EVIDENCE_SCHEMA = "tohseno.inactive-contract-deployment-evidence/1"
P256_PROBE_SCHEMA = "tohseno.p256-probe/2"
MAX_SAFE_JSON_INTEGER = 9_007_199_254_740_991
HEX32 = re.compile(r"^0x[0-9a-f]{64}$")
HEX20 = re.compile(r"^0x[0-9a-f]{40}$")
TIMESTAMP = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")


class PreparationError(RuntimeError):
    pass


class DuplicateMember(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PreparationError(message)


def reject_float(value: str) -> None:
    raise PreparationError(f"floating-point JSON number is forbidden: {value}")


def reject_constant(value: str) -> None:
    raise PreparationError(f"non-finite JSON number is forbidden: {value}")


def closed_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateMember(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> Any:
    require(
        path.is_file() and not path.is_symlink(),
        f"input is not a regular non-symlink file: {path}",
    )
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
            require(
                0 <= member <= MAX_SAFE_JSON_INTEGER,
                "JSON integer is outside the protocol domain",
            )
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
            require(
                all(isinstance(key, str) for key in member),
                "JSON object key is not a string",
            )
            keys = sorted(member, key=lambda key: key.encode("utf-16-be"))
            return "{" + ",".join(encode(key) + ":" + encode(member[key]) for key in keys) + "}"
        raise PreparationError(f"unsupported JSON type: {type(member).__name__}")

    return encode(value).encode("utf-8")


def sha256_hex(payload: bytes) -> str:
    return "0x" + hashlib.sha256(payload).hexdigest()


def hex32(field: str, value: Any) -> str:
    require(isinstance(value, str) and HEX32.match(value), f"{field} must be 0x + 64 lowercase hex")
    return value


def timestamp(field: str, value: str) -> str:
    require(bool(TIMESTAMP.match(value)), f"{field} must be a canonical UTC timestamp")
    try:
        parsed = dt.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise PreparationError(f"{field} is not a real UTC instant: {error}") from error
    require(parsed > dt.datetime(1970, 1, 1), f"{field} must be after the Unix epoch")
    return value


def deployment_observation(field: str, transaction: dict[str, Any]) -> dict[str, Any]:
    block_number = transaction["block_number"]
    require(
        isinstance(block_number, int) and 0 < block_number <= MAX_SAFE_JSON_INTEGER,
        f"{field} block number must be a positive safe integer",
    )
    return {
        "transaction_hash": hex32(f"{field}.transaction_hash", transaction["hash"]),
        "block_number": block_number,
        "block_hash": hex32(f"{field}.block_hash", transaction["block_hash"]),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--generation", required=True, type=Path)
    parser.add_argument("--policy", required=True, type=Path)
    parser.add_argument(
        "--deployment-evidence",
        required=True,
        type=Path,
        help="the published inactive-deployment evidence record",
    )
    parser.add_argument(
        "--p256-probe",
        required=True,
        type=Path,
        help="the fresh ceremony-bound EIP-7951 probe evidence file",
    )
    parser.add_argument(
        "--builder-account-runtime-keccak256",
        required=True,
        help="the canary-established instantiated BuilderAccount runtime hash (never the compiler template)",
    )
    parser.add_argument("--activation-block-number", required=True, type=int)
    parser.add_argument("--activation-block-hash", required=True)
    parser.add_argument("--issued-at", required=True)
    parser.add_argument("--activation-sequence", type=int, default=1)
    parser.add_argument("--previous-activation", default=None)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()

    generation = load_json(arguments.generation)
    policy = load_json(arguments.policy)
    evidence = load_json(arguments.deployment_evidence)

    require(policy["schema"] == POLICY_SCHEMA, f"policy schema must be {POLICY_SCHEMA}")
    require(
        evidence["schema"] == DEPLOYMENT_EVIDENCE_SCHEMA,
        f"deployment evidence schema must be {DEPLOYMENT_EVIDENCE_SCHEMA}",
    )
    require(
        evidence["chain"]["chain_id"] == generation["chain"]["chain_id"],
        "deployment evidence chain does not match the generation",
    )

    generation_digest = sha256_hex(canonical_json(generation))
    recorded_digest = evidence["generation"]["definition_sha256"]
    require(
        generation_digest == recorded_digest,
        f"generation digest {generation_digest} does not match the deployment evidence {recorded_digest}",
    )
    policy_digest = sha256_hex(canonical_json(policy))

    factory_evidence = evidence["contracts"]["builder_account_factory"]
    registry_evidence = evidence["contracts"]["shot_registry"]
    require(
        factory_evidence["address"]
        == generation["create2"]["builder_account_factory"]["predicted_address"],
        "observed factory address does not match the generation prediction",
    )
    require(
        registry_evidence["address"]
        == generation["create2"]["shot_registry"]["predicted_address"],
        "observed registry address does not match the generation prediction",
    )
    # ADR 0010: the factory has no immutable references, so its observed
    # runtime must equal the compiler template exactly.
    require(
        factory_evidence["observed_runtime_keccak256"]
        == generation["contracts"]["builder_account_factory"]["runtime_code_keccak256"],
        "observed factory runtime does not equal its compiler template",
    )
    # ADR 0010: BuilderAccount and ShotRegistry carry constructor-patched
    # immutables; the activation approves observed instance hashes and must
    # never substitute a zero-placeholder template hash.
    builder_runtime = hex32(
        "builder_account_runtime_keccak256", arguments.builder_account_runtime_keccak256
    )
    require(
        builder_runtime != generation["contracts"]["builder_account"]["runtime_code_keccak256"],
        "the BuilderAccount hash equals the compiler template; supply the instantiated instance hash",
    )
    registry_runtime = hex32(
        "registry runtime", registry_evidence["observed_runtime_keccak256"]
    )
    require(
        registry_runtime != generation["contracts"]["shot_registry"]["runtime_code_keccak256"],
        "the observed registry runtime equals the compiler template; expected a constructor-patched instance",
    )

    factory_deployment = deployment_observation("factory", factory_evidence["transaction"])
    registry_deployment = deployment_observation("registry", registry_evidence["transaction"])
    require(
        arguments.activation_block_number
        >= max(factory_deployment["block_number"], registry_deployment["block_number"]),
        "the activation block must be at or after both deployment observations",
    )

    probe_bytes = arguments.p256_probe.read_bytes()
    probe = load_json(arguments.p256_probe)
    require(
        probe.get("schema") == P256_PROBE_SCHEMA,
        f"probe evidence schema must be {P256_PROBE_SCHEMA}",
    )

    previous = arguments.previous_activation
    if arguments.activation_sequence == 1:
        require(previous is None, "activation one must not claim a predecessor")
    else:
        require(
            previous is not None,
            "a successor activation must name its predecessor signing digest",
        )
        previous = hex32("previous_activation", previous)

    activation = {
        "schema": ACTIVATION_SCHEMA,
        "protocol": "tohseno",
        "protocol_major": generation["protocol_major"],
        "generation": generation["generation"],
        "activation_sequence": arguments.activation_sequence,
        "previous_activation": previous,
        "generation_definition_sha256": generation_digest,
        "authority_policy_sha256": policy_digest,
        "chain_id": generation["chain"]["chain_id"],
        "builder_account_runtime_keccak256": builder_runtime,
        "factory": {
            "address": factory_evidence["address"],
            "runtime_code_keccak256": factory_evidence["observed_runtime_keccak256"],
            "deployment": factory_deployment,
        },
        "registry": {
            "address": registry_evidence["address"],
            "runtime_code_keccak256": registry_runtime,
            "deployment": registry_deployment,
        },
        "activation_block": {
            "block_number": arguments.activation_block_number,
            "block_hash": hex32("activation_block.block_hash", arguments.activation_block_hash),
        },
        "p256_probe_sha256": sha256_hex(probe_bytes),
        "issued_at": timestamp("issued_at", arguments.issued_at),
    }

    payload = canonical_json(activation)
    require(arguments.output.is_absolute(), f"output path must be absolute: {arguments.output}")
    descriptor = os.open(arguments.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(payload)

    signing_digest = sha256_hex(ACTIVATION_DOMAIN + payload)
    print(f"activation record written: {arguments.output}")
    print(f"generation definition sha256: {generation_digest}")
    print(f"authority policy sha256: {policy_digest}")
    print(f"activation signing digest: {signing_digest}")
    print()
    print("next step — each signing custodian independently inspects the record and runs:")
    print(
        f"  python3 scripts/sign-contract-activation.py --key <offline-key.pem> \\\n"
        f"    --activation {arguments.output} --output <custodian-approval.json>"
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (PreparationError, DuplicateMember, KeyError, TypeError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
