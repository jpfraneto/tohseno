#!/usr/bin/env python3
"""Reject duplicate or malformed JSON-RPC success responses and canonicalize them."""

from __future__ import annotations

import argparse
import json
import sys
from typing import Any


class DuplicateMember(ValueError):
    pass


def reject_nonstandard_number(value: str) -> None:
    raise ValueError(f"non-standard JSON number: {value}")


def closed_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, member in pairs:
        if key in value:
            raise DuplicateMember(f"duplicate JSON member: {key}")
        value[key] = member
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected-id", type=int, required=True)
    args = parser.parse_args()

    try:
        value = json.load(
            sys.stdin,
            object_pairs_hook=closed_object,
            parse_constant=reject_nonstandard_number,
        )
    except (json.JSONDecodeError, UnicodeDecodeError, ValueError) as error:
        print(f"strict-jsonrpc.py: {error}", file=sys.stderr)
        return 1

    if not isinstance(value, dict):
        print("strict-jsonrpc.py: response must be one object", file=sys.stderr)
        return 1
    if set(value) != {"jsonrpc", "id", "result"}:
        print(
            "strict-jsonrpc.py: success response must contain exactly jsonrpc, id, and result",
            file=sys.stderr,
        )
        return 1
    if value["jsonrpc"] != "2.0":
        print("strict-jsonrpc.py: jsonrpc must equal 2.0", file=sys.stderr)
        return 1
    if type(value["id"]) is not int or value["id"] != args.expected_id:
        print("strict-jsonrpc.py: response id did not match request", file=sys.stderr)
        return 1

    json.dump(value, sys.stdout, ensure_ascii=False, separators=(",", ":"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
