#!/usr/bin/env python3
"""Generate one release-authority P-256 key on this device.

Run this on the custodian's offline device. It writes the private key PEM to
one owner-chosen path with mode 0600 and refuses existing files. Only the
public coordinates and derived release key ID are printed for hand-carry to
the ceremony workstation; the private key must never leave this device.

Generating a key does not approve a policy digest, install a trust root, or
authorize an activation.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile

RELEASE_KEY_DOMAIN = b"TOHSENO-RELEASE-AUTHORITY-KEY-V1\0"
PUBLIC_KEY_SCHEMA = "tohseno.release-authority-public-key/1"

# secp256r1 / NIST P-256 domain parameters, checked independently of OpenSSL.
P256_FIELD = 0xFFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF
P256_A = P256_FIELD - 3
P256_B = 0x5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B


class GenerationError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GenerationError(message)


def run_openssl(arguments: list[str], stdin: bytes | None = None) -> bytes:
    result = subprocess.run(
        ["openssl", *arguments],
        input=stdin,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    require(
        result.returncode == 0,
        f"openssl {' '.join(arguments)} failed: {result.stderr.decode('utf-8', 'replace').strip()}",
    )
    return result.stdout


def exclusive_write(path: Path, contents: bytes, mode: int) -> None:
    require(path.is_absolute(), f"output path must be absolute: {path}")
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(contents)


def uncompressed_point(private_pem: bytes) -> tuple[bytes, bytes]:
    with tempfile.TemporaryDirectory(prefix="tohseno-authority-keygen.") as directory:
        key_path = Path(directory) / "key.pem"
        key_path.write_bytes(private_pem)
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


def check_curve_membership(x: bytes, y: bytes) -> None:
    x_value = int.from_bytes(x, "big")
    y_value = int.from_bytes(y, "big")
    require(0 < x_value < P256_FIELD and 0 < y_value < P256_FIELD, "coordinate outside the field")
    require(
        pow(y_value, 2, P256_FIELD)
        == (pow(x_value, 3, P256_FIELD) + P256_A * x_value + P256_B) % P256_FIELD,
        "public point is not on P-256",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-key",
        required=True,
        type=Path,
        help="absolute path for the private key PEM; must not exist; stays on this device",
    )
    arguments = parser.parse_args()

    private_pem = run_openssl(["ecparam", "-name", "prime256v1", "-genkey", "-noout"])
    exclusive_write(arguments.output_key, private_pem, 0o600)

    x, y = uncompressed_point(private_pem)
    check_curve_membership(x, y)
    key_id = hashlib.sha256(RELEASE_KEY_DOMAIN + x + y).hexdigest()

    # The runbook requires displaying the public coordinates twice.
    for display in (1, 2):
        print(f"public x (display {display}): 0x{x.hex()}")
        print(f"public y (display {display}): 0x{y.hex()}")
    print(f"release key ID: 0x{key_id}")
    print(f"private key written: {arguments.output_key} (mode 0600; never export it)")
    print()
    print("next step — add this object to the ceremony public-keys.json:")
    print(f'  {{ "x": "0x{x.hex()}", "y": "0x{y.hex()}" }}')
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except GenerationError as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
