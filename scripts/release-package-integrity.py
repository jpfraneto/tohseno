#!/usr/bin/env python3
"""Validate, checksum, and atomically publish release-candidate packages."""

from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import os
import re
import secrets
import stat
import subprocess
import sys
from pathlib import Path, PurePosixPath
from typing import BinaryIO


RELEASE_CHECKSUM_NAME = "CHECKSUMS.sha256"
GENESIS_CHECKSUM_NAME = "FILES.sha256"
PUBLIC_CHECKSUM_NAME = "SHA256SUMS"
SAFE_RELEASE_PATH = re.compile(r"^[A-Za-z0-9._/-]+$")
SAFE_COMPONENT = re.compile(r"^[A-Za-z0-9._-]+$")
SUPPORTED_TARGETS = {"aarch64-apple-darwin", "x86_64-apple-darwin"}


def stat_signature(value: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (
        stat.S_IFMT(value.st_mode),
        value.st_dev,
        value.st_ino,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def require_same_stat(
    path: Path,
    expected: os.stat_result,
    observed: os.stat_result,
    phase: str,
) -> None:
    if stat_signature(expected) != stat_signature(observed):
        raise ValueError(f"release input changed {phase}: {path}")


def validate_relative_name(relative: Path) -> str:
    name = relative.as_posix()
    path = PurePosixPath(name)
    if (
        name in {"", ".", ".."}
        or path.is_absolute()
        or name.startswith("/")
        or name.endswith("/")
        or "//" in name
        or "\\" in name
        or any(part in {"", ".", ".."} for part in path.parts)
        or SAFE_RELEASE_PATH.fullmatch(name) is None
    ):
        raise ValueError(f"release package contains an unsafe path: {name!r}")
    return name


def raise_walk_error(error: OSError) -> None:
    raise error


def scan_tree(
    root: Path,
    *,
    excluded_directory_names: frozenset[str] = frozenset(),
    excluded_names: frozenset[str] = frozenset(),
) -> list[tuple[str, Path, os.stat_result]]:
    root_stat = root.lstat()
    if not stat.S_ISDIR(root_stat.st_mode):
        raise ValueError(f"release input must be a real directory: {root}")

    entries: list[tuple[str, Path, os.stat_result]] = []
    for directory, directory_names, file_names in os.walk(
        root,
        topdown=True,
        followlinks=False,
        onerror=raise_walk_error,
    ):
        directory_path = Path(directory)
        included_directories: list[str] = []
        for name in sorted(directory_names, key=os.fsencode):
            if name in excluded_directory_names or name in excluded_names:
                continue
            path = directory_path / name
            source_stat = path.lstat()
            if stat.S_ISLNK(source_stat.st_mode):
                raise ValueError(f"symlink is not allowed in release input: {path}")
            if not stat.S_ISDIR(source_stat.st_mode):
                raise ValueError(f"unsupported release input type: {path}")
            relative_name = validate_relative_name(path.relative_to(root))
            entries.append((relative_name, path, source_stat))
            included_directories.append(name)
        directory_names[:] = included_directories

        for name in sorted(file_names, key=os.fsencode):
            if name in excluded_names:
                continue
            path = directory_path / name
            source_stat = path.lstat()
            if stat.S_ISLNK(source_stat.st_mode):
                raise ValueError(f"symlink is not allowed in release input: {path}")
            if not stat.S_ISREG(source_stat.st_mode):
                raise ValueError(f"unsupported release input type: {path}")
            relative_name = validate_relative_name(path.relative_to(root))
            entries.append((relative_name, path, source_stat))

    entries.sort(key=lambda entry: entry[0].encode("ascii"))
    return entries


def open_verified_regular(
    path: Path, expected_stat: os.stat_result | None = None
) -> tuple[BinaryIO, os.stat_result]:
    no_follow = getattr(os, "O_NOFOLLOW", None)
    if no_follow is None:
        raise OSError("this platform cannot open release inputs without following links")
    descriptor = os.open(
        path,
        os.O_RDONLY | no_follow | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        opened_stat = os.fstat(descriptor)
        if not stat.S_ISREG(opened_stat.st_mode):
            raise ValueError(f"release input is not a regular file: {path}")
        if expected_stat is not None:
            require_same_stat(
                path, expected_stat, opened_stat, "before it could be read"
            )
        return os.fdopen(descriptor, "rb"), opened_stat
    except BaseException:
        os.close(descriptor)
        raise


def digest_regular(path: Path, expected_stat: os.stat_result | None = None) -> str:
    digest = hashlib.sha256()
    source_file, opened_stat = open_verified_regular(path, expected_stat)
    with source_file:
        while chunk := source_file.read(1024 * 1024):
            digest.update(chunk)
        require_same_stat(
            path,
            opened_stat,
            os.fstat(source_file.fileno()),
            "while it was being read",
        )
    return digest.hexdigest()


def parse_checksum_manifest(contents: bytes, manifest_name: str) -> dict[str, str]:
    if not contents.endswith(b"\n"):
        raise ValueError(f"{manifest_name} must end with a newline")
    try:
        lines = contents.decode("ascii").splitlines()
    except UnicodeDecodeError as error:
        raise ValueError(f"{manifest_name} must be ASCII") from error
    if not lines:
        raise ValueError(f"{manifest_name} is empty")

    expected: dict[str, str] = {}
    for line in lines:
        if len(line) < 67 or line[64:66] != "  ":
            raise ValueError(f"{manifest_name} contains a malformed line")
        digest, relative_name = line[:64], line[66:]
        if (
            len(digest) != 64
            or any(character not in "0123456789abcdef" for character in digest)
            or relative_name == manifest_name
            or relative_name in expected
        ):
            raise ValueError(
                f"{manifest_name} contains an unsafe or duplicate entry"
            )
        validate_relative_name(Path(relative_name))
        expected[relative_name] = digest

    canonical_names = sorted(expected, key=lambda name: name.encode("ascii"))
    if list(expected) != canonical_names:
        raise ValueError(f"{manifest_name} entries are not canonically sorted")
    return expected


def read_regular(path: Path, expected_stat: os.stat_result | None = None) -> bytes:
    source_file, opened_stat = open_verified_regular(path, expected_stat)
    with source_file:
        contents = source_file.read()
        require_same_stat(
            path,
            opened_stat,
            os.fstat(source_file.fileno()),
            "while it was being read",
        )
    return contents


def fsync_directory(path: Path) -> None:
    descriptor = os.open(
        path,
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def write_checksum_manifest(root: Path, manifest_name: str) -> None:
    entries = scan_tree(root)
    lines: list[str] = []
    for relative_name, path, source_stat in entries:
        if stat.S_ISDIR(source_stat.st_mode) or relative_name == manifest_name:
            continue
        lines.append(f"{digest_regular(path, source_stat)}  {relative_name}\n")

    temporary_name = f".{manifest_name}.{secrets.token_hex(12)}.tmp"
    temporary_path = root / temporary_name
    descriptor = os.open(
        temporary_path,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write("".join(lines).encode("ascii"))
            output.flush()
            os.fchmod(output.fileno(), 0o644)
            os.fsync(output.fileno())
        os.replace(temporary_path, root / manifest_name)
        fsync_directory(root)
    except BaseException:
        try:
            os.unlink(temporary_path)
        except FileNotFoundError:
            pass
        raise


def verify_checksum_manifest(root: Path, manifest_name: str) -> None:
    entries = scan_tree(root)
    by_name = {relative_name: (path, source_stat) for relative_name, path, source_stat in entries}
    manifest_entry = by_name.get(manifest_name)
    if manifest_entry is None:
        raise ValueError(f"release package is missing {manifest_name}")
    manifest_path, manifest_stat = manifest_entry
    expected = parse_checksum_manifest(
        read_regular(manifest_path, manifest_stat), manifest_name
    )
    observed = {
        relative_name
        for relative_name, _path, source_stat in entries
        if stat.S_ISREG(source_stat.st_mode) and relative_name != manifest_name
    }
    if set(expected) != observed:
        raise ValueError(f"{manifest_name} does not cover exactly the package files")
    for relative_name, expected_digest in expected.items():
        path, source_stat = by_name[relative_name]
        if digest_regular(path, source_stat) != expected_digest:
            raise ValueError(f"{manifest_name} mismatch: {relative_name}")


def validate_file(path: Path) -> None:
    source_stat = path.lstat()
    if not stat.S_ISREG(source_stat.st_mode):
        raise ValueError(f"release input must be a real regular file: {path}")
    digest_regular(path, source_stat)


def git_output(repository: Path, arguments: list[str]) -> bytes:
    completed = subprocess.run(
        ["git", "-C", os.fspath(repository), *arguments],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return completed.stdout


def add_digest_field(digest: hashlib._Hash, label: bytes, contents: bytes) -> None:
    digest.update(len(label).to_bytes(8, "big"))
    digest.update(label)
    digest.update(len(contents).to_bytes(8, "big"))
    digest.update(contents)


def source_state_digest(repository: Path) -> str:
    digest = hashlib.sha256()
    add_digest_field(
        digest,
        b"head",
        git_output(repository, ["rev-parse", "--verify", "HEAD^{commit}"]),
    )
    add_digest_field(
        digest,
        b"status",
        git_output(
            repository,
            [
                "status",
                "--porcelain=v2",
                "-z",
                "--untracked-files=all",
                "--ignore-submodules=none",
            ],
        ),
    )
    add_digest_field(
        digest,
        b"tracked-diff",
        git_output(
            repository,
            [
                "diff",
                "--binary",
                "--full-index",
                "--no-ext-diff",
                "--submodule=diff",
                "HEAD",
                "--",
            ],
        ),
    )

    untracked = git_output(
        repository,
        ["ls-files", "--others", "--exclude-standard", "-z"],
    ).split(b"\0")
    for encoded_name in sorted(
        (name for name in untracked if name), key=lambda name: name
    ):
        path = repository / os.fsdecode(encoded_name)
        source_stat = path.lstat()
        metadata = (
            stat.S_IFMT(source_stat.st_mode).to_bytes(8, "big")
            + stat.S_IMODE(source_stat.st_mode).to_bytes(8, "big")
        )
        if stat.S_ISREG(source_stat.st_mode):
            contents = digest_regular(path, source_stat).encode("ascii")
        elif stat.S_ISLNK(source_stat.st_mode):
            contents = os.fsencode(os.readlink(path))
        else:
            contents = stat_signature(source_stat).__repr__().encode("ascii")
        add_digest_field(digest, b"untracked-name", encoded_name)
        add_digest_field(digest, b"untracked-metadata", metadata)
        add_digest_field(digest, b"untracked-contents", contents)
    return digest.hexdigest()


def open_directory_at(parent_descriptor: int, name: str) -> int:
    return os.open(
        name,
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        dir_fd=parent_descriptor,
    )


def open_repository_subdirectory(
    repository_root: Path, components: tuple[str, ...]
) -> tuple[Path, int]:
    if repository_root.is_symlink():
        raise ValueError("repository root must not be a symlink")
    repository = repository_root.resolve(strict=True)
    repository_descriptor = os.open(
        repository,
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
    )
    current_descriptor = repository_descriptor
    try:
        for component in components:
            try:
                os.mkdir(component, 0o755, dir_fd=current_descriptor)
                os.fsync(current_descriptor)
            except FileExistsError:
                pass
            child_descriptor = open_directory_at(current_descriptor, component)
            if current_descriptor != repository_descriptor:
                os.close(current_descriptor)
            current_descriptor = child_descriptor
        os.close(repository_descriptor)
        return repository.joinpath(*components), current_descriptor
    except BaseException:
        if current_descriptor != repository_descriptor:
            os.close(current_descriptor)
        os.close(repository_descriptor)
        raise


def create_stage(
    repository_root: Path,
    components: tuple[str, ...],
    prefix: str,
) -> Path:
    output_root, output_descriptor = open_repository_subdirectory(
        repository_root, components
    )
    try:
        for _attempt in range(128):
            name = f"{prefix}{secrets.token_hex(16)}"
            try:
                os.mkdir(name, 0o700, dir_fd=output_descriptor)
                os.fsync(output_descriptor)
                return output_root / name
            except FileExistsError:
                continue
        raise OSError("could not allocate a unique release staging directory")
    finally:
        os.close(output_descriptor)


def remove_tree_at(parent_descriptor: int, name: str) -> None:
    try:
        child_descriptor = open_directory_at(parent_descriptor, name)
    except FileNotFoundError:
        return
    try:
        for child_name in os.listdir(child_descriptor):
            child_stat = os.stat(
                child_name,
                dir_fd=child_descriptor,
                follow_symlinks=False,
            )
            if stat.S_ISDIR(child_stat.st_mode):
                remove_tree_at(child_descriptor, child_name)
            else:
                os.unlink(child_name, dir_fd=child_descriptor)
    finally:
        os.close(child_descriptor)
    os.rmdir(name, dir_fd=parent_descriptor)


def cleanup_stage(
    repository_root: Path,
    components: tuple[str, ...],
    stage_name: str,
    stage_prefix: str,
) -> None:
    if (
        SAFE_COMPONENT.fullmatch(stage_name) is None
        or not stage_name.startswith(stage_prefix)
    ):
        raise ValueError("invalid release staging directory name")
    _output_root, output_descriptor = open_repository_subdirectory(
        repository_root, components
    )
    try:
        remove_tree_at(output_descriptor, stage_name)
        os.fsync(output_descriptor)
    finally:
        os.close(output_descriptor)


def rename_exchange(
    source_directory_descriptor: int,
    source_name: str,
    destination_directory_descriptor: int,
    destination_name: str,
) -> None:
    library = ctypes.CDLL(None, use_errno=True)
    source = os.fsencode(source_name)
    destination = os.fsencode(destination_name)
    if sys.platform == "darwin":
        rename = library.renameatx_np
        rename.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        rename.restype = ctypes.c_int
        result = rename(
            source_directory_descriptor,
            source,
            destination_directory_descriptor,
            destination,
            0x00000002,  # RENAME_SWAP
        )
    elif sys.platform.startswith("linux") and hasattr(library, "renameat2"):
        rename = library.renameat2
        rename.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        rename.restype = ctypes.c_int
        result = rename(
            source_directory_descriptor,
            source,
            destination_directory_descriptor,
            destination,
            0x00000002,  # RENAME_EXCHANGE
        )
    else:
        raise OSError(
            errno.ENOTSUP,
            "atomic directory exchange is unavailable on this platform",
        )
    if result != 0:
        error_number = ctypes.get_errno()
        raise OSError(error_number, os.strerror(error_number))


def fsync_package_tree(package_descriptor: int) -> None:
    original_directory = os.open(
        ".",
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        os.fchdir(package_descriptor)
        root = Path(".")
        entries = scan_tree(root)
        directories: list[Path] = []
        for _relative_name, path, source_stat in entries:
            if stat.S_ISREG(source_stat.st_mode):
                descriptor = os.open(
                    path,
                    os.O_RDONLY
                    | getattr(os, "O_NOFOLLOW", 0)
                    | getattr(os, "O_CLOEXEC", 0),
                )
                try:
                    os.fsync(descriptor)
                finally:
                    os.close(descriptor)
            else:
                directories.append(path)
        for directory in sorted(
            directories,
            key=lambda path: len(path.parts),
            reverse=True,
        ):
            fsync_directory(directory)
        os.fsync(package_descriptor)
    finally:
        os.fchdir(original_directory)
        os.close(original_directory)


def verify_package_at_descriptor(
    package_descriptor: int, manifest_name: str
) -> None:
    original_directory = os.open(
        ".",
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        os.fchdir(package_descriptor)
        verify_checksum_manifest(Path("."), manifest_name)
    finally:
        os.fchdir(original_directory)
        os.close(original_directory)


def publish_stage(
    repository_root: Path,
    *,
    components: tuple[str, ...],
    stage_name: str,
    stage_prefix: str,
    target: str,
    allowed_targets: frozenset[str],
    manifest_name: str,
) -> Path:
    if (
        SAFE_COMPONENT.fullmatch(stage_name) is None
        or not stage_name.startswith(stage_prefix)
    ):
        raise ValueError("invalid release staging directory name")
    if target not in allowed_targets:
        raise ValueError("invalid release target")

    output_root, output_descriptor = open_repository_subdirectory(
        repository_root, components
    )
    stage_descriptor = open_directory_at(output_descriptor, stage_name)
    package_descriptor = open_directory_at(stage_descriptor, target)
    try:
        verify_package_at_descriptor(package_descriptor, manifest_name)
        fsync_package_tree(package_descriptor)
        verify_package_at_descriptor(package_descriptor, manifest_name)
        try:
            destination_stat = os.stat(
                target,
                dir_fd=output_descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            destination_stat = None

        if destination_stat is None:
            os.rename(
                target,
                target,
                src_dir_fd=stage_descriptor,
                dst_dir_fd=output_descriptor,
            )
        else:
            if not stat.S_ISDIR(destination_stat.st_mode):
                raise ValueError(
                    "existing release output must be a real directory"
                )
            rename_exchange(
                stage_descriptor,
                target,
                output_descriptor,
                target,
            )
        os.fsync(output_descriptor)
        os.fsync(stage_descriptor)
        return output_root / target
    finally:
        os.close(package_descriptor)
        os.close(stage_descriptor)
        os.close(output_descriptor)


def resolved_real_directory(value: Path, label: str) -> Path:
    if value.is_symlink():
        raise ValueError(f"{label} must be a real directory")
    resolved = value.resolve(strict=True)
    if not resolved.is_dir():
        raise ValueError(f"{label} must be a real directory")
    return resolved


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    source_state_parser = subparsers.add_parser("source-state")
    source_state_parser.add_argument("--repository-root", required=True, type=Path)

    validate_file_parser = subparsers.add_parser("validate-file")
    validate_file_parser.add_argument("--path", required=True, type=Path)

    validate_tree_parser = subparsers.add_parser("validate-tree")
    validate_tree_parser.add_argument("--root", required=True, type=Path)
    validate_tree_parser.add_argument("--exclude-dir-name", action="append", default=[])
    validate_tree_parser.add_argument("--exclude-name", action="append", default=[])

    write_manifest_parser = subparsers.add_parser("write-manifest")
    write_manifest_parser.add_argument("--root", required=True, type=Path)
    write_manifest_parser.add_argument(
        "--manifest-name",
        choices=(
            RELEASE_CHECKSUM_NAME,
            GENESIS_CHECKSUM_NAME,
            PUBLIC_CHECKSUM_NAME,
        ),
        default=RELEASE_CHECKSUM_NAME,
    )

    verify_manifest_parser = subparsers.add_parser("verify-manifest")
    verify_manifest_parser.add_argument("--root", required=True, type=Path)
    verify_manifest_parser.add_argument(
        "--manifest-name",
        choices=(
            RELEASE_CHECKSUM_NAME,
            GENESIS_CHECKSUM_NAME,
            PUBLIC_CHECKSUM_NAME,
        ),
        default=RELEASE_CHECKSUM_NAME,
    )

    create_stage_parser = subparsers.add_parser("create-stage")
    create_stage_parser.add_argument("--repository-root", required=True, type=Path)

    publish_parser = subparsers.add_parser("publish")
    publish_parser.add_argument("--repository-root", required=True, type=Path)
    publish_parser.add_argument("--stage-name", required=True)
    publish_parser.add_argument("--target", required=True)

    cleanup_stage_parser = subparsers.add_parser("cleanup-stage")
    cleanup_stage_parser.add_argument("--repository-root", required=True, type=Path)
    cleanup_stage_parser.add_argument("--stage-name", required=True)

    create_genesis_stage_parser = subparsers.add_parser("create-genesis-stage")
    create_genesis_stage_parser.add_argument(
        "--repository-root", required=True, type=Path
    )

    publish_genesis_parser = subparsers.add_parser("publish-genesis")
    publish_genesis_parser.add_argument(
        "--repository-root", required=True, type=Path
    )
    publish_genesis_parser.add_argument("--stage-name", required=True)

    cleanup_genesis_stage_parser = subparsers.add_parser(
        "cleanup-genesis-stage"
    )
    cleanup_genesis_stage_parser.add_argument(
        "--repository-root", required=True, type=Path
    )
    cleanup_genesis_stage_parser.add_argument("--stage-name", required=True)

    arguments = parser.parse_args()
    try:
        if arguments.command == "source-state":
            repository = resolved_real_directory(
                arguments.repository_root, "repository root"
            )
            print(source_state_digest(repository))
        elif arguments.command == "validate-file":
            validate_file(arguments.path)
        elif arguments.command == "validate-tree":
            root = resolved_real_directory(arguments.root, "release input root")
            scan_tree(
                root,
                excluded_directory_names=frozenset(arguments.exclude_dir_name),
                excluded_names=frozenset(arguments.exclude_name),
            )
        elif arguments.command == "write-manifest":
            root = resolved_real_directory(arguments.root, "release package root")
            write_checksum_manifest(root, arguments.manifest_name)
            verify_checksum_manifest(root, arguments.manifest_name)
        elif arguments.command == "verify-manifest":
            root = resolved_real_directory(arguments.root, "release package root")
            verify_checksum_manifest(root, arguments.manifest_name)
        elif arguments.command == "create-stage":
            print(
                create_stage(
                    arguments.repository_root,
                    ("dist", "release-candidate"),
                    ".stage.",
                )
            )
        elif arguments.command == "publish":
            print(
                publish_stage(
                    arguments.repository_root,
                    components=("dist", "release-candidate"),
                    stage_name=arguments.stage_name,
                    stage_prefix=".stage.",
                    target=arguments.target,
                    allowed_targets=frozenset(SUPPORTED_TARGETS),
                    manifest_name=RELEASE_CHECKSUM_NAME,
                )
            )
        elif arguments.command == "cleanup-stage":
            cleanup_stage(
                arguments.repository_root,
                ("dist", "release-candidate"),
                arguments.stage_name,
                ".stage.",
            )
        elif arguments.command == "create-genesis-stage":
            print(
                create_stage(
                    arguments.repository_root,
                    ("dist",),
                    ".genesis-stage.",
                )
            )
        elif arguments.command == "publish-genesis":
            print(
                publish_stage(
                    arguments.repository_root,
                    components=("dist",),
                    stage_name=arguments.stage_name,
                    stage_prefix=".genesis-stage.",
                    target="genesis",
                    allowed_targets=frozenset({"genesis"}),
                    manifest_name=GENESIS_CHECKSUM_NAME,
                )
            )
        elif arguments.command == "cleanup-genesis-stage":
            cleanup_stage(
                arguments.repository_root,
                ("dist",),
                arguments.stage_name,
                ".genesis-stage.",
            )
        else:
            raise AssertionError(f"unhandled command: {arguments.command}")
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
