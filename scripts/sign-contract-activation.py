#!/usr/bin/env python3
"""Sign one governed contract-activation digest with one offline authority key.

A custodian runs this on the key's own offline device after independently
inspecting the exact activation record. It recomputes the domain-separated
signing digest from the record itself (never trusting a typed digest), signs
it once with low-s P-256 through OpenSSL, verifies its own output, and writes
one detached public approval. The private key never leaves this device.

One approval proves one custodian's decision under one policy; it activates
nothing by itself.
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
ACTIVATION_PROFILES = {
    "tohseno.contract-activation/1": b"TOHSENO-CONTRACT-ACTIVATION-V1\0",
    "tohseno.claims-activation/1": b"TOHSENO-CLAIMS-ACTIVATION-V1\0",
}
APPROVAL_SCHEMA = "tohseno.release-authority-approval/1"
MAX_SAFE_JSON_INTEGER = 9_007_199_254_740_991
P256_ORDER = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
SPKI_P256_PREFIX = bytes.fromhex(
    "3059301306072a8648ce3d020106082a8648ce3d03010703420004"
)


class SigningError(RuntimeError):
    pass


class DuplicateMember(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SigningError(message)


def reject_float(value: str) -> None:
    raise SigningError(f"floating-point JSON number is forbidden: {value}")


def reject_constant(value: str) -> None:
    raise SigningError(f"non-finite JSON number is forbidden: {value}")


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
        raise SigningError(f"invalid JSON input {path}: {error}") from error


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
        raise SigningError(f"unsupported JSON type: {type(member).__name__}")

    return encode(value).encode("utf-8")


def run_openssl(arguments: list[str]) -> bytes:
    result = subprocess.run(
        ["openssl", *arguments],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    require(
        result.returncode == 0,
        f"openssl {' '.join(arguments)} failed: {result.stderr.decode('utf-8', 'replace').strip()}",
    )
    return result.stdout


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


def parse_der_signature(der: bytes) -> tuple[int, int]:
    require(len(der) >= 8 and der[0] == 0x30 and der[1] == len(der) - 2, "malformed DER signature")
    values = []
    offset = 2
    for _ in range(2):
        require(der[offset] == 0x02, "malformed DER integer")
        length = der[offset + 1]
        values.append(int.from_bytes(der[offset + 2 : offset + 2 + length], "big"))
        offset += 2 + length
    require(offset == len(der), "trailing DER signature bytes")
    return values[0], values[1]


def public_point(key_path: Path) -> tuple[bytes, bytes]:
    der = run_openssl(
        [
            "ec",
            "-in",
            str(key_path),
            "-pubout",
            "-conv_form",
            "uncompressed",
            "-outform",
            "DER",
        ]
    )
    require(len(der) >= 65 and der[-65] == 0x04, "unexpected public key encoding")
    return der[-64:-32], der[-32:]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--key", required=True, type=Path, help="this custodian's private key PEM")
    parser.add_argument(
        "--activation",
        required=True,
        type=Path,
        help="the exact canonical activation record this custodian inspected",
    )
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()

    activation = load_json(arguments.activation)
    schema = activation.get("schema")
    require(schema in ACTIVATION_PROFILES, "activation schema is not governed by this signer")
    payload = canonical_json(activation)
    require(
        payload == arguments.activation.read_bytes(),
        "the activation file is not in canonical form; refuse to sign a non-canonical record",
    )
    digest = hashlib.sha256(ACTIVATION_PROFILES[schema] + payload).digest()

    x, y = public_point(arguments.key)
    key_id = hashlib.sha256(RELEASE_KEY_DOMAIN + x + y).digest()

    with tempfile.TemporaryDirectory(prefix="tohseno-activation-sign.") as directory:
        root = Path(directory)
        digest_path = root / "digest.bin"
        signature_path = root / "signature.der"
        public_path = root / "public.der"
        digest_path.write_bytes(digest)
        public_path.write_bytes(SPKI_P256_PREFIX + x + y)
        run_openssl(
            [
                "pkeyutl",
                "-sign",
                "-inkey",
                str(arguments.key),
                "-in",
                str(digest_path),
                "-out",
                str(signature_path),
            ]
        )
        r, s = parse_der_signature(signature_path.read_bytes())
        if s > P256_ORDER // 2:
            s = P256_ORDER - s
        require(0 < r < P256_ORDER and 0 < s <= P256_ORDER // 2, "signature outside the P-256 domain")
        # Verify the normalized signature back through OpenSSL before writing.
        normalized_path = root / "normalized.der"
        normalized_path.write_bytes(der_signature(r, s))
        run_openssl(
            [
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
                str(normalized_path),
            ]
        )

    approval = {
        "schema": APPROVAL_SCHEMA,
        "key_id": "0x" + key_id.hex(),
        "authorization": {
            "algorithm": "p256",
            "digest": "0x" + digest.hex(),
            "signature": {
                "r": "0x" + r.to_bytes(32, "big").hex(),
                "s": "0x" + s.to_bytes(32, "big").hex(),
            },
            "low_s": True,
        },
    }
    require(arguments.output.is_absolute(), f"output path must be absolute: {arguments.output}")
    descriptor = os.open(arguments.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(canonical_json(approval))

    print(f"approval written: {arguments.output}")
    print(f"release key ID: 0x{key_id.hex()}")
    print(f"signed activation digest: 0x{digest.hex()}")
    print()
    print("next step — hand only this approval file to the ceremony coordinator.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (SigningError, DuplicateMember, KeyError, TypeError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
