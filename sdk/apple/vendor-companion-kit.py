#!/usr/bin/env python3
"""Vendor the exact released CompanionKit into a Shot without mutable paths."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys


FIXED_MTIME = 946684800  # 2000-01-01T00:00:00Z
VECTOR_SCHEMA = "tohseno.companion-test-vectors/1"


def fail(message: str) -> "NoReturn":
    raise SystemExit(f"vendor-companion-kit: {message}")


def safe_existing_directory(path: Path, label: str) -> Path:
    absolute = path.absolute()
    if not absolute.exists() or not absolute.is_dir() or absolute.is_symlink():
        fail(f"{label} must be an existing non-symlink directory: {absolute}")
    if absolute.resolve() != absolute:
        fail(f"{label} must not traverse symbolic links: {absolute}")
    return absolute


def source_files(package: Path) -> list[Path]:
    excluded = {".build", ".swiftpm", "VENDORED-MANIFEST.sha256"}
    values: list[Path] = []
    for path in package.rglob("*"):
        relative = path.relative_to(package)
        if any(part in excluded for part in relative.parts):
            continue
        if path.is_symlink():
            fail(f"source package contains a symbolic link: {relative}")
        if path.is_file():
            values.append(relative)
    return sorted(values, key=lambda value: value.as_posix().encode("utf-8"))


def publish(arguments: argparse.Namespace) -> None:
    script = Path(__file__).absolute()
    repository = script.parents[2]
    package = safe_existing_directory(
        repository / "sdk" / "apple" / "TohsenoCompanionKit",
        "source package",
    )
    shot = safe_existing_directory(Path(arguments.into), "Shot directory")
    vendor_root = shot / "Vendor"
    destination = vendor_root / "TohsenoCompanionKit"
    if vendor_root.exists() and (vendor_root.is_symlink() or vendor_root.resolve() != vendor_root.absolute()):
        fail(f"Vendor directory is unsafe: {vendor_root}")
    if destination.exists() or destination.is_symlink():
        fail(f"destination already exists; refusing to overwrite: {destination}")

    vector = repository / "companion" / "test-vectors" / "companion-v1.json"
    try:
        vector_value = json.loads(vector.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"shared vector is unreadable: {error}")
    if set(vector_value) != {
        "schema", "test_only", "bip39_official", "companion_identity",
        "workspace_service_identity", "pairing", "capability", "command",
        "snapshot_request_command", "pairing_acceptance", "icon_blob", "envelope", "relay",
        "negative",
    } or vector_value.get("schema") != VECTOR_SCHEMA or vector_value.get("test_only") is not True:
        fail("shared vector does not have the exact released schema")

    vendor_root.mkdir(mode=0o755, parents=True, exist_ok=True)
    destination.mkdir(mode=0o755)
    try:
        for relative in source_files(package):
            source = package / relative
            target = destination / relative
            target.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            target.chmod(0o644)
        copied_vector = (
            destination / "Tests" / "TohsenoCompanionKitTests"
            / "TestVectors" / "companion-v1.json"
        )
        shutil.copyfile(vector, copied_vector)
        copied_vector.chmod(0o644)

        entries: list[str] = []
        for file in source_files(destination):
            digest = hashlib.sha256((destination / file).read_bytes()).hexdigest()
            entries.append(f"{digest}  {file.as_posix()}\n")
        manifest = destination / "VENDORED-MANIFEST.sha256"
        manifest.write_text("".join(entries), encoding="ascii", newline="\n")
        manifest.chmod(0o644)
        for path in sorted(destination.rglob("*"), reverse=True):
            os.utime(path, (FIXED_MTIME, FIXED_MTIME), follow_symlinks=False)
            if path.is_dir():
                path.chmod(0o755)
        os.utime(destination, (FIXED_MTIME, FIXED_MTIME))
    except Exception:
        shutil.rmtree(destination, ignore_errors=True)
        raise

    manifest_digest = hashlib.sha256(
        (destination / "VENDORED-MANIFEST.sha256").read_bytes()
    ).hexdigest()
    print(json.dumps({
        "schema": "tohseno.companion-sdk-vendored/1",
        "version": (package / "VERSION").read_text(encoding="utf-8").strip(),
        "destination": str(destination),
        "manifest_sha256": manifest_digest,
    }, sort_keys=True, separators=(",", ":")))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--into", required=True, help="existing Shot repository directory")
    publish(parser.parse_args())


if __name__ == "__main__":
    main()
