#!/usr/bin/env python3
"""Assemble custodian approvals into one signed contract-activation envelope.

The coordinator runs this after collecting detached approvals. It recomputes
the activation signing digest, checks every approval against the supplied
policy (member key, exact digest, low-s signature verified through OpenSSL),
orders approvals strictly by key ID, enforces the threshold, and writes the
closed `tohseno.signed-contract-activation/1` envelope.

A valid envelope proves threshold approval under the supplied policy only.
No client trusts it until a separately authorized release pins the same
policy digest as its trust root.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import Any

RELEASE_KEY_DOMAIN = b"TOHSENO-RELEASE-AUTHORITY-KEY-V1\0"
ACTIVATION_DOMAIN = b"TOHSENO-CONTRACT-ACTIVATION-V1\0"
ACTIVATION_SCHEMA = "tohseno.contract-activation/1"
APPROVAL_SCHEMA = "tohseno.release-authority-approval/1"
POLICY_SCHEMA = "tohseno.release-authority-policy/1"
SIGNED_SCHEMA = "tohseno.signed-contract-activation/1"
MAX_SAFE_JSON_INTEGER = 9_007_199_254_740_991
P256_ORDER = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
SPKI_P256_PREFIX = bytes.fromhex(
    "3059301306072a8648ce3d020106082a8648ce3d03010703420004"
)


class AssemblyError(RuntimeError):
    pass


class DuplicateMember(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssemblyError(message)


def reject_float(value: str) -> None:
    raise AssemblyError(f"floating-point JSON number is forbidden: {value}")


def reject_constant(value: str) -> None:
    raise AssemblyError(f"non-finite JSON number is forbidden: {value}")


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
        raise AssemblyError(f"invalid JSON input {path}: {error}") from error


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
        raise AssemblyError(f"unsupported JSON type: {type(member).__name__}")

    return encode(value).encode("utf-8")


def der_integer(value: int) -> bytes:
    encoded = value.to_bytes((value.bit_length() + 7) // 8 or 1, "big")
    encoded = encoded.lstrip(b"\0") or b"\0"
    if encoded[0] & 0x80:
        encoded = b"\0" + encoded
    return b"\x02" + bytes([len(encoded)]) + encoded


def der_signature(r: int, s: int) -> bytes:
    body = der_integer(r) + der_integer(s)
    require(len(body) < 128, "unexpected long P-256 DER signature")
    return b"\x30" + bytes([len(body)]) + body


def openssl_verify(x: bytes, y: bytes, digest: bytes, r: int, s: int) -> None:
    with tempfile.TemporaryDirectory(prefix="tohseno-activation-assemble.") as directory:
        root = Path(directory)
        public_path = root / "public.der"
        digest_path = root / "digest.bin"
        signature_path = root / "signature.der"
        public_path.write_bytes(SPKI_P256_PREFIX + x + y)
        digest_path.write_bytes(digest)
        signature_path.write_bytes(der_signature(r, s))
        result = subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-verify",
                "-pubin",
                "-inkey",
                str(public_path),
                "-keyform",
                "DER",
                "-in",
                str(digest_path),
                "-sigfile",
                str(signature_path),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        require(result.returncode == 0, "an approval signature failed OpenSSL verification")


def field_bytes(field: str, value: Any, length: int) -> bytes:
    require(
        isinstance(value, str)
        and value.startswith("0x")
        and len(value) == 2 + 2 * length
        and value == value.lower(),
        f"{field} must be 0x + {2 * length} lowercase hex",
    )
    return bytes.fromhex(value[2:])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--activation", required=True, type=Path)
    parser.add_argument("--policy", required=True, type=Path)
    parser.add_argument(
        "--approval",
        action="append",
        required=True,
        type=Path,
        help="one custodian approval file; repeat for each approval",
    )
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()

    activation = load_json(arguments.activation)
    require(
        activation.get("schema") == ACTIVATION_SCHEMA,
        f"activation schema must be {ACTIVATION_SCHEMA}",
    )
    policy = load_json(arguments.policy)
    require(policy.get("schema") == POLICY_SCHEMA, f"policy schema must be {POLICY_SCHEMA}")
    policy_digest = "0x" + hashlib.sha256(canonical_json(policy)).hexdigest()
    require(
        activation["authority_policy_sha256"] == policy_digest,
        "the activation does not bind the supplied policy",
    )
    digest = hashlib.sha256(ACTIVATION_DOMAIN + canonical_json(activation)).digest()

    authorities = {
        authority["key_id"]: (
            field_bytes("authority.x", authority["public_key"]["x"], 32),
            field_bytes("authority.y", authority["public_key"]["y"], 32),
        )
        for authority in policy["authorities"]
    }

    approvals = []
    for path in arguments.approval:
        approval = load_json(path)
        require(
            approval.get("schema") == APPROVAL_SCHEMA,
            f"{path}: approval schema must be {APPROVAL_SCHEMA}",
        )
        key_id = approval["key_id"]
        require(key_id in authorities, f"{path}: approval key is outside the bound policy")
        authorization = approval["authorization"]
        require(authorization["algorithm"] == "p256", f"{path}: algorithm must be p256")
        require(authorization["low_s"] is True, f"{path}: low_s must be true")
        require(
            field_bytes("approval.digest", authorization["digest"], 32) == digest,
            f"{path}: approval signs a different digest than this activation",
        )
        r = int.from_bytes(field_bytes("signature.r", authorization["signature"]["r"], 32), "big")
        s = int.from_bytes(field_bytes("signature.s", authorization["signature"]["s"], 32), "big")
        require(0 < r < P256_ORDER and 0 < s <= P256_ORDER // 2, f"{path}: signature outside the low-s domain")
        x, y = authorities[key_id]
        require(
            hashlib.sha256(RELEASE_KEY_DOMAIN + x + y).hexdigest() == key_id[2:],
            f"{path}: approval key ID does not derive from the policy public key",
        )
        openssl_verify(x, y, digest, r, s)
        approvals.append(
            {
                "key_id": key_id,
                "authorization": {
                    "algorithm": "p256",
                    "digest": authorization["digest"],
                    "signature": {
                        "r": authorization["signature"]["r"],
                        "s": authorization["signature"]["s"],
                    },
                    "low_s": True,
                },
            }
        )

    approvals.sort(key=lambda approval: bytes.fromhex(approval["key_id"][2:]))
    key_ids = [approval["key_id"] for approval in approvals]
    require(len(set(key_ids)) == len(key_ids), "duplicate approval keys")
    require(
        policy["threshold"] <= len(approvals) <= len(policy["authorities"]),
        f"approvals must satisfy threshold {policy['threshold']} without unknown keys",
    )

    envelope = {
        "schema": SIGNED_SCHEMA,
        "activation": activation,
        "approvals": approvals,
    }
    require(arguments.output.is_absolute(), f"output path must be absolute: {arguments.output}")
    descriptor = os.open(arguments.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(canonical_json(envelope))

    print(f"signed activation written: {arguments.output}")
    print(f"authority policy sha256: {policy_digest}")
    print(f"activation signing digest: 0x{digest.hex()}")
    print(f"approvals: {len(approvals)} of threshold {policy['threshold']}")
    print()
    print("next step — verify with both independent implementations before any client change:")
    print(
        "  python3 scripts/verify-contract-activation.py --repository-root \"$PWD\" \\\n"
        "    --generation contracts/generations/0.8.0/generation.json \\\n"
        f"    --policy {arguments.policy} --signed-activation {arguments.output} \\\n"
        "    --p256-probe <ceremony-probe.json> --trusted-policy-sha256 " + policy_digest
    )
    print(
        "  cargo run --quiet --locked -p tohseno-protocol --example verify_signed_contract_activation -- \\\n"
        f"    contracts/generations/0.8.0/generation.json {arguments.policy} {arguments.output} {policy_digest}"
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssemblyError, DuplicateMember, KeyError, TypeError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
