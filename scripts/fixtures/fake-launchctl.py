#!/usr/bin/env python3
"""Debug-only launchctl fixture that owns one isolated TOHSENO service PID."""

from __future__ import annotations

import json
import os
import plistlib
import signal
import stat
import subprocess
import sys
import time
import uuid
from pathlib import Path
from typing import BinaryIO, NoReturn


PRODUCTION_LABEL = "com.tohseno.workspace-service"
VERIFICATION_LABEL_PREFIX = f"{PRODUCTION_LABEL}.verification."
MARKER_NAME = ".tohseno-test-launchctl-v1"
MARKER_BYTES = b"TOHSENO_TEST_LAUNCHCTL_V1\n"
MAXIMUM_STATE_BYTES = 64 * 1024


class FixtureError(Exception):
    pass


def fail(message: str, status: int = 64) -> NoReturn:
    print(f"fake-launchctl.py: {message}", file=sys.stderr)
    raise SystemExit(status)


def required_absolute_directory(name: str) -> Path:
    value = os.environ.get(name, "")
    path = Path(value)
    if not value or not path.is_absolute():
        raise FixtureError(f"{name} must name an absolute directory")
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise FixtureError(f"{name} must name a real directory")
    return path


def read_regular_bounded(path: Path, maximum: int = MAXIMUM_STATE_BYTES) -> bytes:
    before = path.lstat()
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise FixtureError("managed launchctl state is not a regular file")
    if before.st_size > maximum:
        raise FixtureError("managed launchctl state exceeds its bound")
    descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_dev != before.st_dev
            or opened.st_ino != before.st_ino
            or opened.st_size != before.st_size
        ):
            raise FixtureError("managed launchctl state changed while opening")
        value = os.read(descriptor, maximum + 1)
        if len(value) != opened.st_size or len(value) > maximum:
            raise FixtureError("managed launchctl state changed while reading")
        return value
    finally:
        os.close(descriptor)


def replace_private(path: Path, value: bytes) -> None:
    if len(value) > MAXIMUM_STATE_BYTES:
        raise FixtureError("managed launchctl state exceeds its bound")
    if path.exists() or path.is_symlink():
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
            raise FixtureError("managed launchctl state target is unsafe")
    temporary = path.parent / f".{path.name}.{uuid.uuid4().hex}.tmp"
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW,
        0o600,
    )
    try:
        os.write(descriptor, value)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.replace(temporary, path)
    directory = os.open(path.parent, os.O_RDONLY | os.O_CLOEXEC)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def remove_regular(path: Path) -> None:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise FixtureError("managed launchctl state target is unsafe")
    path.unlink()


def expected_domain() -> str:
    return f"gui/{os.getuid()}"


def configured_label() -> str:
    if os.environ.get("TOHSENO_VERIFICATION_MODE") != "1":
        return PRODUCTION_LABEL
    label = os.environ.get("TOHSENO_VERIFICATION_SERVICE_LABEL", "")
    if not label.startswith(VERIFICATION_LABEL_PREFIX) or len(label) > 128:
        raise FixtureError("verification service label has the wrong namespace")
    suffix = label.removeprefix(VERIFICATION_LABEL_PREFIX)
    if not suffix or any(
        not (character.isascii() and (character.isalnum() or character in ".-"))
        for character in suffix
    ):
        raise FixtureError("verification service label is invalid")
    return label


def expected_target(label: str) -> str:
    return f"{expected_domain()}/{label}"


def load_plist(
    path: Path, install_root: Path, launch_agents: Path, label: str
) -> dict[str, object]:
    expected_path = launch_agents / f"{label}.plist"
    if path != expected_path:
        raise FixtureError("launchctl fixture received an unexpected LaunchAgent path")
    value = plistlib.loads(read_regular_bounded(path))
    if not isinstance(value, dict):
        raise FixtureError("LaunchAgent plist is not a dictionary")
    arguments = value.get("ProgramArguments")
    expected_arguments = [
        str(install_root / "bin/tohseno"),
        "--json",
        "service",
        "run",
    ]
    expected_stdout = str(install_root / "logs/workspace-service.log")
    expected_stderr = str(install_root / "logs/workspace-service.error.log")
    if (
        value.get("Label") != label
        or arguments != expected_arguments
        or value.get("RunAtLoad") is not True
        or value.get("KeepAlive") != {"SuccessfulExit": False}
        or value.get("StandardOutPath") != expected_stdout
        or value.get("StandardErrorPath") != expected_stderr
    ):
        raise FixtureError("LaunchAgent plist is not the expected TOHSENO test service")
    return value


def load_registered(path: Path, expected_plist: Path) -> None:
    try:
        registered = read_regular_bounded(path).decode("utf-8")
    except FileNotFoundError as error:
        raise FixtureError("the test service is not bootstrapped") from error
    if registered != f"{expected_plist}\n":
        raise FixtureError("registered LaunchAgent state does not match the isolated plist")


def load_pid(path: Path) -> int | None:
    try:
        value = read_regular_bounded(path, 64).decode("ascii").strip()
    except FileNotFoundError:
        return None
    if not value.isdecimal():
        raise FixtureError("service PID state is invalid")
    pid = int(value)
    if pid <= 1:
        raise FixtureError("service PID state is unsafe")
    return pid


def process_exists(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError as error:
        raise FixtureError("service PID is not owned by the test user") from error


def stop_process(pid_path: Path) -> None:
    pid = load_pid(pid_path)
    if pid is None:
        return
    if process_exists(pid):
        os.kill(pid, signal.SIGINT)
        deadline = time.monotonic() + 10
        while process_exists(pid) and time.monotonic() < deadline:
            time.sleep(0.05)
        if process_exists(pid):
            os.kill(pid, signal.SIGTERM)
            deadline = time.monotonic() + 2
            while process_exists(pid) and time.monotonic() < deadline:
                time.sleep(0.05)
        if process_exists(pid):
            os.kill(pid, signal.SIGKILL)
            deadline = time.monotonic() + 2
            while process_exists(pid) and time.monotonic() < deadline:
                time.sleep(0.05)
        if process_exists(pid):
            raise FixtureError("the exact test service PID did not stop")
    remove_regular(pid_path)


def open_safe_log(path: Path, install_root: Path) -> BinaryIO:
    if path.parent != install_root / "logs":
        raise FixtureError("test service log escaped the isolated install root")
    parent = path.parent.lstat()
    if stat.S_ISLNK(parent.st_mode) or not stat.S_ISDIR(parent.st_mode):
        raise FixtureError("test service log directory is unsafe")
    descriptor = os.open(
        path,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_APPEND
        | os.O_CLOEXEC
        | os.O_NOFOLLOW,
        0o600,
    )
    metadata = os.fstat(descriptor)
    if not stat.S_ISREG(metadata.st_mode):
        os.close(descriptor)
        raise FixtureError("test service log target is unsafe")
    return os.fdopen(descriptor, "ab", buffering=0)


def start_process(plist: dict[str, object], pid_path: Path, install_root: Path) -> None:
    existing = load_pid(pid_path)
    if existing is not None:
        if process_exists(existing):
            raise FixtureError("test service is already running")
        remove_regular(pid_path)
    arguments = plist["ProgramArguments"]
    stdout_path = Path(str(plist["StandardOutPath"]))
    stderr_path = Path(str(plist["StandardErrorPath"]))
    with open_safe_log(stdout_path, install_root) as stdout, open_safe_log(
        stderr_path, install_root
    ) as stderr:
        process = subprocess.Popen(
            arguments,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
            close_fds=True,
            start_new_session=True,
            env=os.environ.copy(),
        )
    try:
        replace_private(pid_path, f"{process.pid}\n".encode("ascii"))
    except Exception:
        try:
            os.kill(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        raise


def main() -> int:
    try:
        label = configured_label()
        state_root = required_absolute_directory("TOHSENO_TEST_LAUNCHCTL_STATE")
        install_root = required_absolute_directory("TOHSENO_INSTALL_ROOT")
        launch_agents = required_absolute_directory("TOHSENO_LAUNCH_AGENTS_DIR")
        if read_regular_bounded(state_root / MARKER_NAME) != MARKER_BYTES:
            raise FixtureError("launchctl fixture marker is missing")
        expected_plist = launch_agents / f"{label}.plist"
        registered_path = state_root / "registered-plist"
        pid_path = state_root / "service.pid"
        arguments = sys.argv[1:]

        if len(arguments) == 3 and arguments[0] == "bootstrap":
            if arguments[1] != expected_domain() or Path(arguments[2]) != expected_plist:
                raise FixtureError("bootstrap escaped the isolated user service")
            load_plist(expected_plist, install_root, launch_agents, label)
            replace_private(registered_path, f"{expected_plist}\n".encode("utf-8"))
            return 0

        if arguments == ["kickstart", "-k", expected_target(label)]:
            load_registered(registered_path, expected_plist)
            plist = load_plist(expected_plist, install_root, launch_agents, label)
            stop_process(pid_path)
            start_process(plist, pid_path, install_root)
            return 0

        if len(arguments) == 3 and arguments[0] == "bootout":
            if arguments[1] != expected_domain() or Path(arguments[2]) != expected_plist:
                raise FixtureError("bootout escaped the isolated user service")
            load_registered(registered_path, expected_plist)
            stop_process(pid_path)
            remove_regular(registered_path)
            return 0

        if arguments == ["print", expected_target(label)]:
            load_registered(registered_path, expected_plist)
            pid = load_pid(pid_path)
            if pid is None or not process_exists(pid):
                return 3
            print(json.dumps({"label": label, "pid": pid}, separators=(",", ":")))
            return 0

        raise FixtureError("unsupported launchctl operation")
    except (FixtureError, FileNotFoundError, OSError, plistlib.InvalidFileException) as error:
        fail(str(error))


if __name__ == "__main__":
    raise SystemExit(main())
