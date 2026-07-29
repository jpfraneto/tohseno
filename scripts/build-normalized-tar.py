#!/usr/bin/env python3
"""Build a byte-reproducible, metadata-normalized tar.gz archive."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import os
import secrets
import stat
import tarfile
from pathlib import Path, PurePosixPath
from typing import BinaryIO


def contains_ascii_control(value: str) -> bool:
    return any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)


def parse_mtime(value: str) -> int:
    try:
        parsed = int(value, 10)
    except ValueError as error:
        raise argparse.ArgumentTypeError("mtime must be an unsigned integer") from error
    if parsed < 0 or parsed > 0xFFFFFFFF:
        raise argparse.ArgumentTypeError(
            "mtime must fit in the portable gzip timestamp field"
        )
    return parsed


def parse_root_name(value: str) -> str:
    path = PurePosixPath(value)
    if (
        value in {"", ".", ".."}
        or path.is_absolute()
        or len(path.parts) != 1
        or "\\" in value
    ):
        raise argparse.ArgumentTypeError(
            "root name must be one non-empty relative path component"
        )
    if contains_ascii_control(value):
        raise argparse.ArgumentTypeError(
            "root name must not contain ASCII control characters"
        )
    return value


def archive_name(root_name: str, source: Path, path: Path) -> str:
    if path == source:
        name = root_name
    else:
        name = f"{root_name}/{path.relative_to(source).as_posix()}"
    try:
        name.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ValueError(f"archive path is not valid UTF-8: {path}") from error
    if contains_ascii_control(name):
        raise ValueError(
            f"archive path contains an ASCII control character: {path}"
        )
    return name


def raise_walk_error(error: OSError) -> None:
    raise error


def collect_entries(
    source: Path, root_name: str
) -> list[tuple[str, Path, os.stat_result]]:
    source_stat = source.lstat()
    if not stat.S_ISDIR(source_stat.st_mode):
        raise ValueError(f"archive source is no longer a directory: {source}")
    entries = [(archive_name(root_name, source, source), source, source_stat)]
    for directory, directory_names, file_names in os.walk(
        source,
        topdown=True,
        followlinks=False,
        onerror=raise_walk_error,
    ):
        directory_path = Path(directory)
        for name in directory_names + file_names:
            path = directory_path / name
            source_stat = path.lstat()
            if stat.S_ISLNK(source_stat.st_mode):
                raise ValueError(f"symlinks are not allowed in archive input: {path}")
            if not (
                stat.S_ISDIR(source_stat.st_mode)
                or stat.S_ISREG(source_stat.st_mode)
            ):
                raise ValueError(f"unsupported archive input type: {path}")
            entries.append(
                (archive_name(root_name, source, path), path, source_stat)
            )
    entries.sort(key=lambda entry: entry[0].encode("utf-8"))
    return entries


def stat_signature(source_stat: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (
        stat.S_IFMT(source_stat.st_mode),
        source_stat.st_dev,
        source_stat.st_ino,
        source_stat.st_size,
        source_stat.st_mtime_ns,
        source_stat.st_ctime_ns,
    )


def require_same_stat(
    path: Path,
    expected_stat: os.stat_result,
    observed_stat: os.stat_result,
    phase: str,
) -> None:
    if stat_signature(observed_stat) != stat_signature(expected_stat):
        raise ValueError(f"archive input changed {phase}: {path}")


def tar_info(
    name: str,
    path: Path,
    mtime: int,
    collected_stat: os.stat_result,
) -> tuple[tarfile.TarInfo, os.stat_result]:
    source_stat = path.lstat()
    require_same_stat(path, collected_stat, source_stat, "after collection")
    info = tarfile.TarInfo(name)
    info.uid = 0
    info.gid = 0
    info.uname = "root"
    info.gname = "root"
    info.mtime = mtime
    if stat.S_ISDIR(source_stat.st_mode):
        info.type = tarfile.DIRTYPE
        info.mode = 0o755
        info.size = 0
    elif stat.S_ISREG(source_stat.st_mode):
        info.type = tarfile.REGTYPE
        info.mode = 0o755 if source_stat.st_mode & 0o111 else 0o644
        info.size = source_stat.st_size
    elif stat.S_ISLNK(source_stat.st_mode):
        raise ValueError(f"symlinks are not allowed in archive input: {path}")
    else:
        raise ValueError(f"unsupported archive input type: {path}")
    return info, source_stat


def open_verified_regular(
    path: Path, expected_stat: os.stat_result
) -> tuple[BinaryIO, os.stat_result]:
    no_follow = getattr(os, "O_NOFOLLOW", None)
    if no_follow is None:
        raise OSError("this platform cannot open archive inputs without following links")
    flags = (
        os.O_RDONLY
        | no_follow
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )
    descriptor = os.open(path, flags)
    try:
        opened_stat = os.fstat(descriptor)
        require_same_stat(path, expected_stat, opened_stat, "before it could be read")
        if not stat.S_ISREG(opened_stat.st_mode):
            raise ValueError(f"archive input is no longer a regular file: {path}")
        return os.fdopen(descriptor, "rb"), opened_stat
    except BaseException:
        os.close(descriptor)
        raise


def require_unchanged_after_read(
    path: Path, source_file: BinaryIO, opened_stat: os.stat_result
) -> None:
    require_same_stat(
        path,
        opened_stat,
        os.fstat(source_file.fileno()),
        "while it was being read",
    )


def parse_files_manifest(contents: bytes) -> dict[str, str]:
    expected: dict[str, str] = {}
    try:
        lines = contents.decode("ascii").splitlines()
    except UnicodeDecodeError as error:
        raise ValueError("FILES.sha256 is not ASCII") from error
    if not lines:
        raise ValueError("FILES.sha256 is empty")
    for line in lines:
        if len(line) < 67 or line[64:66] != "  ":
            raise ValueError("FILES.sha256 contains a malformed line")
        digest, relative_name = line[:64], line[66:]
        path = PurePosixPath(relative_name)
        if (
            any(character not in "0123456789abcdef" for character in digest)
            or relative_name in {"", "FILES.sha256"}
            or path.is_absolute()
            or relative_name.startswith("/")
            or relative_name.endswith("/")
            or "//" in relative_name
            or "\\" in relative_name
            or any(part in {"", ".", ".."} for part in path.parts)
            or contains_ascii_control(relative_name)
            or any(
                not (
                    character.isascii()
                    and (
                        character.isalnum()
                        or character in "._/-"
                    )
                )
                for character in relative_name
            )
            or relative_name in expected
        ):
            raise ValueError("FILES.sha256 contains an unsafe or duplicate entry")
        expected[relative_name] = digest
    return expected


def verify_embedded_files_manifest(
    archive_file: BinaryIO, root_name: str
) -> None:
    """Verify a Genesis FILES.sha256 against bytes in the completed archive."""

    file_digests: dict[str, str] = {}
    manifest = None
    directories: set[str] = set()
    manifest_name = f"{root_name}/FILES.sha256"
    archive_file.seek(0)
    with tarfile.open(fileobj=archive_file, mode="r:gz") as archive:
        for member in archive:
            name = PurePosixPath(member.name)
            if (
                member.name == ""
                or name.is_absolute()
                or not name.parts
                or name.parts[0] != root_name
                or "//" in member.name
                or "\\" in member.name
                or any(part in {"", ".", ".."} for part in name.parts)
                or contains_ascii_control(member.name)
            ):
                raise ValueError("completed archive contains an unsafe path")
            if member.name in file_digests or member.name in directories:
                raise ValueError("completed archive contains a duplicate path")
            if member.isdir():
                directories.add(member.name)
                continue
            if not member.isfile():
                raise ValueError("completed archive contains an unsupported entry")
            extracted = archive.extractfile(member)
            if extracted is None:
                raise ValueError("completed archive file could not be read")
            digest = hashlib.sha256()
            captured_manifest = bytearray()
            while chunk := extracted.read(1024 * 1024):
                digest.update(chunk)
                if member.name == manifest_name:
                    captured_manifest.extend(chunk)
                    if len(captured_manifest) > 16 * 1024 * 1024:
                        raise ValueError("completed archive FILES.sha256 is too large")
            file_digests[member.name] = digest.hexdigest()
            if member.name == manifest_name:
                manifest = bytes(captured_manifest)

    if manifest is None:
        raise ValueError("completed archive is missing FILES.sha256")
    expected = parse_files_manifest(manifest)
    observed_names = {
        name.removeprefix(f"{root_name}/")
        for name in file_digests
        if name != manifest_name
    }
    if set(expected) != observed_names:
        raise ValueError(
            "completed archive FILES.sha256 does not cover exactly its files"
        )
    for relative_name, expected_digest in expected.items():
        if file_digests[f"{root_name}/{relative_name}"] != expected_digest:
            raise ValueError(
                f"completed archive FILES.sha256 mismatch: {relative_name}"
            )


def build_archive(
    source: Path,
    output_parent_descriptor: int,
    output_name: str,
    root_name: str,
    mtime: int,
) -> None:
    entries = collect_entries(source, root_name)
    stage_name = ""
    stage_descriptor = -1
    descriptor = -1
    try:
        for _attempt in range(128):
            stage_name = f".{output_name}.{secrets.token_hex(16)}.stage"
            try:
                os.mkdir(
                    stage_name,
                    0o700,
                    dir_fd=output_parent_descriptor,
                )
                break
            except FileExistsError:
                continue
        else:
            raise OSError("could not allocate a private archive staging directory")

        stage_descriptor = os.open(
            stage_name,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_CLOEXEC", 0),
            dir_fd=output_parent_descriptor,
        )
        descriptor = os.open(
            "archive.tmp",
            os.O_RDWR
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_CLOEXEC", 0),
            0o600,
            dir_fd=stage_descriptor,
        )
        with os.fdopen(os.dup(descriptor), "wb") as raw_output:
            with gzip.GzipFile(
                filename="",
                mode="wb",
                compresslevel=9,
                fileobj=raw_output,
                mtime=mtime,
            ) as compressed_output:
                with tarfile.open(
                    fileobj=compressed_output,
                    mode="w|",
                    format=tarfile.GNU_FORMAT,
                ) as archive:
                    for name, path, collected_stat in entries:
                        info, source_stat = tar_info(
                            name, path, mtime, collected_stat
                        )
                        if info.isreg():
                            source_file, opened_stat = open_verified_regular(
                                path, source_stat
                            )
                            with source_file:
                                archive.addfile(info, source_file)
                                require_unchanged_after_read(
                                    path, source_file, opened_stat
                                )
                        else:
                            archive.addfile(info)
            raw_output.flush()
            os.fsync(raw_output.fileno())
        os.fchmod(descriptor, 0o644)
        os.fsync(descriptor)
        with os.fdopen(os.dup(descriptor), "rb") as archive_input:
            verify_embedded_files_manifest(archive_input, root_name)
        os.fsync(stage_descriptor)
        os.replace(
            "archive.tmp",
            output_name,
            src_dir_fd=stage_descriptor,
            dst_dir_fd=output_parent_descriptor,
        )
        os.fsync(output_parent_descriptor)
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        if stage_descriptor >= 0:
            try:
                os.unlink("archive.tmp", dir_fd=stage_descriptor)
            except FileNotFoundError:
                pass
            os.fsync(stage_descriptor)
            stage_stat = os.fstat(stage_descriptor)
            os.close(stage_descriptor)
            try:
                named_stage_stat = os.stat(
                    stage_name,
                    dir_fd=output_parent_descriptor,
                    follow_symlinks=False,
                )
            except FileNotFoundError:
                named_stage_stat = None
            if (
                named_stage_stat is not None
                and stat.S_ISDIR(named_stage_stat.st_mode)
                and named_stage_stat.st_dev == stage_stat.st_dev
                and named_stage_stat.st_ino == stage_stat.st_ino
            ):
                os.rmdir(stage_name, dir_fd=output_parent_descriptor)
                os.fsync(output_parent_descriptor)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--root-name", required=True, type=parse_root_name)
    parser.add_argument("--mtime", required=True, type=parse_mtime)
    arguments = parser.parse_args()

    if arguments.source.is_symlink():
        parser.error("--source must be a real directory, not a symlink")
    source = arguments.source.resolve(strict=True)
    if not source.is_dir():
        parser.error("--source must be a real directory, not a symlink")
    output_name = arguments.output.name
    if (
        output_name in {"", ".", ".."}
        or "/" in output_name
        or "\\" in output_name
        or contains_ascii_control(output_name)
    ):
        parser.error("--output must name one safe file")
    if arguments.output.is_symlink():
        parser.error("--output must not be a symlink")
    preliminary_output = arguments.output.resolve(strict=False)
    if preliminary_output == source or source in preliminary_output.parents:
        parser.error("--output must be outside --source")
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    output_parent = arguments.output.parent.resolve(strict=True)
    if not output_parent.is_dir():
        parser.error("--output parent must be a real directory")
    output = output_parent / output_name
    if output == source or source in output.parents:
        parser.error("--output must be outside --source")

    output_parent_descriptor = -1
    try:
        output_parent_descriptor = os.open(
            output_parent,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_CLOEXEC", 0),
        )
        build_archive(
            source,
            output_parent_descriptor,
            output_name,
            arguments.root_name,
            arguments.mtime,
        )
    except (OSError, ValueError) as error:
        parser.error(str(error))
    finally:
        if output_parent_descriptor >= 0:
            os.close(output_parent_descriptor)


if __name__ == "__main__":
    main()
