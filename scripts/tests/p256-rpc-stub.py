#!/usr/bin/env python3
"""Deterministic local JSON-RPC peer for the P256 deployment-gate tests."""

from __future__ import annotations

import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


PRECOMPILE = "0x0000000000000000000000000000000000000100"
METER_ADDRESS = "0xfffffffffffffffffffffffffffffffffffffffe"
METER_RUNTIME = "0x5a365f5f3760205f365f6101005afa505a90035f5260205ff3"
BLOCK_HASH = "0x" + ("ab" * 32)
BLOCK_NUMBER = "0x1690abc"
WORD_ONE = "0x" + ("00" * 31) + "01"
METER_7057 = "0x" + ("00" * 30) + "1b91"
METER_3607 = "0x" + ("00" * 30) + "0e17"


def load_vectors() -> list[dict[str, Any]]:
    repository = Path(__file__).resolve().parents[2]
    fixture = repository / "contracts" / "test-vectors" / "eip-7951.json"
    return json.loads(fixture.read_text(encoding="utf-8"))["vectors"]


class ProbeServer(ThreadingHTTPServer):
    scenario: str
    vectors: list[dict[str, Any]]


class Handler(BaseHTTPRequestHandler):
    server: ProbeServer

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def send_rpc(self, payload: dict[str, Any]) -> None:
        encoded = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        self.send_raw(encoded)

    def send_raw(self, encoded: bytes) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        try:
            self.wfile.write(encoded)
        except BrokenPipeError:
            pass

    def send_http_error(self) -> None:
        self.send_response(500)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def invalid_request(self, request_id: object, message: str) -> None:
        self.send_rpc(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {"code": -32602, "message": message},
            }
        )

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        if self.path != "/":
            self.send_error(404)
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            self.send_error(400)
            return
        if length <= 0 or length > 100_000:
            self.send_error(413)
            return
        try:
            request = json.loads(self.rfile.read(length))
        except (UnicodeDecodeError, json.JSONDecodeError):
            self.send_error(400)
            return
        if not isinstance(request, dict) or request.get("jsonrpc") != "2.0":
            self.invalid_request(None, "expected one JSON-RPC 2.0 object")
            return

        request_id = request.get("id")
        method = request.get("method")
        params = request.get("params")
        block_reference = {"blockHash": BLOCK_HASH, "requireCanonical": True}

        if self.server.scenario == "http-error" and request_id == 1:
            self.send_http_error()
            return
        if request_id == 1 and method == "eth_chainId" and params == []:
            result = "0x1" if self.server.scenario == "wrong-chain" else "0x1237"
            self.send_rpc({"jsonrpc": "2.0", "id": request_id, "result": result})
            return
        if (
            request_id == 2
            and method == "eth_getBlockByNumber"
            and params == ["latest", False]
        ):
            if self.server.scenario == "null-block":
                self.send_rpc({"jsonrpc": "2.0", "id": request_id, "result": None})
                return
            if self.server.scenario == "short-block-hash":
                self.send_rpc(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "result": {"number": BLOCK_NUMBER, "hash": "0xab"},
                    }
                )
                return
            if self.server.scenario == "duplicate-block-member":
                self.send_raw(
                    (
                        '{"jsonrpc":"2.0","id":2,"result":'
                        f'{{"number":"{BLOCK_NUMBER}","hash":"{BLOCK_HASH}",'
                        f'"hash":"{BLOCK_HASH}"}}}}'
                    ).encode("ascii")
                )
                return
            self.send_rpc(
                {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {"number": BLOCK_NUMBER, "hash": BLOCK_HASH},
                }
            )
            return
        if (
            request_id == 3
            and method == "eth_getCode"
            and params == [METER_ADDRESS, block_reference]
        ):
            result = "0x6000" if self.server.scenario == "nonempty-meter" else "0x"
            self.send_rpc({"jsonrpc": "2.0", "id": request_id, "result": result})
            return

        if isinstance(request_id, int) and 4 <= request_id <= 6:
            vector = self.server.vectors[request_id - 4]
            expected_params = [
                {"to": PRECOMPILE, "data": vector["input"], "gas": "0x100000"},
                block_reference,
            ]
            if method != "eth_call" or params != expected_params:
                self.invalid_request(request_id, "malformed pinned direct call")
                return
            result: object = vector["expected_output"]
            if request_id == 4 and self.server.scenario == "wrong-positive":
                result = "0x"
            elif request_id == 4 and self.server.scenario == "zero-positive":
                result = "0x" + ("00" * 32)
            elif request_id == 4 and self.server.scenario == "short-positive":
                result = "0x01"
            elif request_id == 4 and self.server.scenario == "malformed-result":
                result = 1
            elif request_id == 4 and self.server.scenario == "duplicate-result":
                self.send_raw(
                    (
                        '{"jsonrpc":"2.0","id":4,'
                        f'"result":"{WORD_ONE}","result":"{WORD_ONE}"}}'
                    ).encode("ascii")
                )
                return
            elif request_id == 5 and self.server.scenario == "nonempty-negative":
                result = WORD_ONE
            elif request_id == 6 and self.server.scenario == "nonempty-infinity":
                result = WORD_ONE
            self.send_rpc({"jsonrpc": "2.0", "id": request_id, "result": result})
            return

        if isinstance(request_id, int) and 7 <= request_id <= 9:
            vector = self.server.vectors[request_id - 7]
            expected_params = [
                {"to": METER_ADDRESS, "data": vector["input"], "gas": "0x100000"},
                block_reference,
                {METER_ADDRESS: {"code": METER_RUNTIME}},
            ]
            if method != "eth_call" or params != expected_params:
                self.invalid_request(request_id, "malformed pinned state-override call")
                return
            if self.server.scenario == "no-state-override":
                self.send_rpc(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "error": {
                            "code": -32602,
                            "message": "state override unsupported",
                        },
                    }
                )
                return
            if self.server.scenario == "legacy-gas":
                result = METER_3607
            elif self.server.scenario == "off-by-one-gas":
                result = "0x" + ("00" * 30) + "1b92"
            elif self.server.scenario == "short-gas-word":
                result = "0x1b91"
            elif self.server.scenario == "nonhex-gas-word":
                result = "0x" + ("00" * 30) + "1bzz"
            else:
                result = METER_7057
            self.send_rpc({"jsonrpc": "2.0", "id": request_id, "result": result})
            return

        if (
            request_id == 10
            and method == "eth_getBlockByNumber"
            and params == [BLOCK_NUMBER, False]
        ):
            hash_value = (
                "0x" + ("cd" * 32)
                if self.server.scenario == "reorg-after-probe"
                else BLOCK_HASH
            )
            self.send_rpc(
                {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {"number": BLOCK_NUMBER, "hash": hash_value},
                }
            )
            return

        self.invalid_request(request_id, "unexpected method, id, or parameters")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--scenario",
        required=True,
        choices=[
            "pass",
            "http-error",
            "wrong-chain",
            "null-block",
            "short-block-hash",
            "wrong-positive",
            "zero-positive",
            "short-positive",
            "malformed-result",
            "duplicate-result",
            "duplicate-block-member",
            "nonempty-negative",
            "nonempty-infinity",
            "legacy-gas",
            "off-by-one-gas",
            "short-gas-word",
            "nonhex-gas-word",
            "nonempty-meter",
            "no-state-override",
            "reorg-after-probe",
        ],
    )
    parser.add_argument("--port-file", type=Path, required=True)
    args = parser.parse_args()

    server = ProbeServer(("127.0.0.1", 0), Handler)
    server.scenario = args.scenario
    server.vectors = load_vectors()
    args.port_file.write_text(str(server.server_port), encoding="ascii")
    server.serve_forever()


if __name__ == "__main__":
    main()
