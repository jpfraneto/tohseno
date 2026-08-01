#!/usr/bin/env python3
"""Construct the proposed 2-of-3 release policy from public P-256 keys only.

This tool never generates, reads, signs with, or stores private keys. Running it
does not approve the resulting policy digest or install a client trust root.
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


KEY_DOMAIN = b"TOHSENO-RELEASE-AUTHORITY-KEY-V1\0"
PUBLIC_KEYS_SCHEMA = "tohseno.release-authority-public-keys/1"
POLICY_SCHEMA = "tohseno.release-authority-policy/1"
REPORT_SCHEMA = "tohseno.release-authority-policy-preparation/1"
PRODUCTION_AUTHORITY_COUNT = 3
PRODUCTION_THRESHOLD = 2
HEX32 = re.compile(r"^0x[0-9a-f]{64}$")
TIMESTAMP = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")

# secp256r1 / NIST P-256 domain parameters.
P256_FIELD = 0xFFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF
P256_B = 0x5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B


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
    require(path.is_file() and not path.is_symlink(), f"input is not a regular non-symlink file: {path}")
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=closed_object,
            parse_float=reject_float,
            parse_constant=reject_constant,
        )
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise PreparationError(f"invalid public-key input: {error}") from error


def canonical_json(value: Any) -> bytes:
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
            keys = sorted(member, key=lambda key: key.encode("utf-16-be"))
            return "{" + ",".join(encode(key) + ":" + encode(member[key]) for key in keys) + "}"
        raise PreparationError(f"unsupported JSON type: {type(member).__name__}")

    return encode(value).encode("utf-8")


def public_coordinate(value: Any, field: str) -> bytes:
    require(isinstance(value, str) and bool(HEX32.fullmatch(value)), f"{field} must be lowercase 32-byte hex")
    return bytes.fromhex(value[2:])


def validate_public_key(value: Any, index: int) -> tuple[bytes, bytes]:
    field = f"keys[{index}]"
    require(isinstance(value, dict), f"{field} must be an object")
    require(set(value) == {"x", "y"}, f"{field} must contain exactly x and y")
    x_bytes = public_coordinate(value["x"], f"{field}.x")
    y_bytes = public_coordinate(value["y"], f"{field}.y")
    x = int.from_bytes(x_bytes, "big")
    y = int.from_bytes(y_bytes, "big")
    require(0 <= x < P256_FIELD and 0 <= y < P256_FIELD, f"{field} coordinate is outside the P-256 field")
    left = pow(y, 2, P256_FIELD)
    right = (pow(x, 3, P256_FIELD) - 3 * x + P256_B) % P256_FIELD
    require(left == right, f"{field} is not an affine P-256 point")
    return x_bytes, y_bytes


def canonical_timestamp(value: str) -> str:
    require(bool(TIMESTAMP.fullmatch(value)), "issued-at must be YYYY-MM-DDTHH:MM:SSZ")
    try:
        parsed = dt.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=dt.timezone.utc)
    except ValueError as error:
        raise PreparationError(f"issued-at is not a real UTC timestamp: {error}") from error
    require(parsed.timestamp() > 0, "issued-at must be after the Unix epoch")
    return value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--public-keys", required=True, type=Path)
    parser.add_argument("--issued-at", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    source = load_json(args.public_keys)
    require(isinstance(source, dict), "public-key input must be an object")
    require(set(source) == {"schema", "keys"}, "public-key input must contain exactly schema and keys")
    require(source["schema"] == PUBLIC_KEYS_SCHEMA, f"public-key input schema must be {PUBLIC_KEYS_SCHEMA}")
    require(isinstance(source["keys"], list), "keys must be an array")
    require(
        len(source["keys"]) == PRODUCTION_AUTHORITY_COUNT,
        f"the proposed production policy requires exactly {PRODUCTION_AUTHORITY_COUNT} keys",
    )

    authorities: list[dict[str, Any]] = []
    for index, key in enumerate(source["keys"]):
        x, y = validate_public_key(key, index)
        key_id = hashlib.sha256(KEY_DOMAIN + x + y).hexdigest()
        authorities.append(
            {
                "key_id": "0x" + key_id,
                "public_key": {"x": "0x" + x.hex(), "y": "0x" + y.hex()},
            }
        )
    authorities.sort(key=lambda authority: authority["key_id"])
    require(
        len({authority["key_id"] for authority in authorities}) == PRODUCTION_AUTHORITY_COUNT,
        "release-authority keys must be unique",
    )

    policy = {
        "schema": POLICY_SCHEMA,
        "protocol": "tohseno",
        "protocol_major": 2,
        "purpose": "contract_generation_activation",
        "threshold": PRODUCTION_THRESHOLD,
        "authorities": authorities,
        "issued_at": canonical_timestamp(args.issued_at),
    }
    policy_digest = "0x" + hashlib.sha256(canonical_json(policy)).hexdigest()
    require(args.output.parent.is_dir(), "output parent directory does not exist")
    descriptor = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        handle.write(json.dumps(policy, indent=2, ensure_ascii=False) + "\n")

    report = {
        "schema": REPORT_SCHEMA,
        "policy_sha256": policy_digest,
        "authority_count": PRODUCTION_AUTHORITY_COUNT,
        "threshold": PRODUCTION_THRESHOLD,
        "authorities_strictly_ordered": True,
        "curve_membership_valid": True,
        "private_key_accessed": False,
        "owner_approved": False,
        "trust_root_installed": False,
    }
    print(json.dumps(report, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, PreparationError) as error:
        print(f"prepare-release-authority-policy.py: {error}", file=sys.stderr)
        raise SystemExit(1)
