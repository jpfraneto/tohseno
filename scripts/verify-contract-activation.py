#!/usr/bin/env python3
"""Independent offline verifier for TOHSENO contract-activation evidence.

This implementation intentionally shares no Rust protocol code. It accepts an
explicit policy digest as input; it does not decide that the owner authorized
that trust root. It never reads or handles private keys.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, NoReturn


MAX_SAFE_UINT = 9_007_199_254_740_991
P256_ORDER = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
P256_HALF_ORDER = P256_ORDER // 2
RELEASE_KEY_DOMAIN = b"TOHSENO-RELEASE-AUTHORITY-KEY-V1\0"
ACTIVATION_DOMAIN = b"TOHSENO-CONTRACT-ACTIVATION-V1\0"
SOURCE_TREE_DOMAIN = b"TOHSENO-CONTRACT-SOURCE-TREE-V1\0"
SPKI_P256_PREFIX = bytes.fromhex(
    "3059301306072a8648ce3d020106082a8648ce3d03010703420004"
)
HEX32 = re.compile(r"^0x[0-9a-f]{64}$")
ADDRESS20 = re.compile(r"^0x[0-9a-f]{40}$")
VERSION = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
TIMESTAMP = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
LOWER_SHA256 = re.compile(r"^[0-9a-f]{64}$")
HEX_QUANTITY = re.compile(r"^0x(?:0|[1-9a-f][0-9a-f]*)$")
HEX_BYTES = re.compile(r"^0x(?:[0-9a-f]{2})*$")
RELATIVE_PATH = re.compile(r"^[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)*$")
ACTIVATION_KEYS = {
    "schema", "protocol", "protocol_major", "generation", "activation_sequence",
    "previous_activation", "generation_definition_sha256", "authority_policy_sha256",
    "chain_id", "builder_account_runtime_keccak256", "factory", "registry",
    "activation_block", "p256_probe_sha256", "issued_at",
}


class VerificationError(Exception):
    pass


def fail(message: str) -> NoReturn:
    raise VerificationError(message)


def reject_float(value: str) -> NoReturn:
    fail(f"floating-point JSON number is forbidden: {value}")


def reject_constant(value: str) -> NoReturn:
    fail(f"non-finite JSON number is forbidden: {value}")


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            fail(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def load_strict(path: Path) -> Any:
    try:
        raw = path.read_bytes()
    except OSError as exc:
        fail(f"cannot read {path}: {exc}")
    try:
        text = raw.decode("utf-8", "strict")
    except UnicodeDecodeError as exc:
        fail(f"{path} is not strict UTF-8: {exc}")
    try:
        return json.loads(
            text,
            object_pairs_hook=unique_object,
            parse_float=reject_float,
            parse_constant=reject_constant,
        )
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path}: {exc}")


def validate_json_domain(value: Any, field: str = "$") -> None:
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, int):
        if value < 0 or value > MAX_SAFE_UINT:
            fail(f"{field} is outside the unsigned JavaScript-safe range")
        return
    if isinstance(value, str):
        if any(0xD800 <= ord(character) <= 0xDFFF for character in value):
            fail(f"{field} contains an unpaired Unicode surrogate")
        return
    if isinstance(value, list):
        for index, member in enumerate(value):
            validate_json_domain(member, f"{field}[{index}]")
        return
    if isinstance(value, dict):
        for key, member in value.items():
            validate_json_domain(key, f"{field}.<key>")
            validate_json_domain(member, f"{field}.{key}")
        return
    fail(f"{field} has unsupported JSON type {type(value).__name__}")


def utf16_sort_key(value: str) -> bytes:
    return value.encode("utf-16-be")


def canonical_json(value: Any) -> bytes:
    validate_json_domain(value)

    def encode(member: Any) -> str:
        if member is None:
            return "null"
        if member is True:
            return "true"
        if member is False:
            return "false"
        if isinstance(member, int) and not isinstance(member, bool):
            return str(member)
        if isinstance(member, str):
            return json.dumps(member, ensure_ascii=False, separators=(",", ":"))
        if isinstance(member, list):
            return "[" + ",".join(encode(item) for item in member) + "]"
        if isinstance(member, dict):
            ordered = sorted(member, key=utf16_sort_key)
            return "{" + ",".join(encode(key) + ":" + encode(member[key]) for key in ordered) + "}"
        fail(f"unsupported canonical JSON type {type(member).__name__}")

    return encode(value).encode("utf-8")


def sha256(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def keccak256(data: bytes) -> bytes:
    result = subprocess.run(
        ["openssl", "dgst", "-KECCAK-256", "-binary"],
        input=data,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0 or len(result.stdout) != 32:
        fail("OpenSSL KECCAK-256 is unavailable or returned the wrong length")
    return result.stdout


def hex32(value: Any, field: str, *, nonzero: bool = True) -> bytes:
    if not isinstance(value, str) or HEX32.fullmatch(value) is None:
        fail(f"{field} must be a lowercase 0x-prefixed 32-byte value")
    raw = bytes.fromhex(value[2:])
    if nonzero and raw == bytes(32):
        fail(f"{field} must not be zero")
    return raw


def address20(value: Any, field: str, *, nonzero: bool = True) -> bytes:
    if not isinstance(value, str) or ADDRESS20.fullmatch(value) is None:
        fail(f"{field} must be a lowercase 0x-prefixed 20-byte address")
    raw = bytes.fromhex(value[2:])
    if nonzero and raw == bytes(20):
        fail(f"{field} must not be zero")
    return raw


def safe_uint(value: Any, field: str, *, positive: bool = False) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        fail(f"{field} must be an integer")
    minimum = 1 if positive else 0
    if value < minimum or value > MAX_SAFE_UINT:
        fail(f"{field} must be in {minimum}..={MAX_SAFE_UINT}")
    return value


def exact_object(value: Any, field: str, keys: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(f"{field} must be an object")
    observed = set(value)
    if observed != keys:
        missing = sorted(keys - observed)
        unknown = sorted(observed - keys)
        fail(f"{field} has missing={missing} unknown={unknown}")
    return value


def exact_string(value: Any, field: str, expected: str) -> None:
    if value != expected:
        fail(f"{field} must be {expected!r}")


def canonical_timestamp(value: Any, field: str) -> dt.datetime:
    if not isinstance(value, str) or TIMESTAMP.fullmatch(value) is None:
        fail(f"{field} must use YYYY-MM-DDTHH:MM:SSZ")
    try:
        parsed = dt.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=dt.timezone.utc)
    except ValueError as exc:
        fail(f"{field} is not a real UTC timestamp: {exc}")
    if parsed.timestamp() <= 0:
        fail(f"{field} must be after the Unix epoch")
    return parsed


def der_integer(value: int) -> bytes:
    encoded = value.to_bytes((value.bit_length() + 7) // 8 or 1, "big")
    encoded = encoded.lstrip(b"\0") or b"\0"
    if encoded[0] & 0x80:
        encoded = b"\0" + encoded
    return b"\x02" + bytes([len(encoded)]) + encoded


def der_signature(r: int, s: int) -> bytes:
    body = der_integer(r) + der_integer(s)
    if len(body) >= 128:
        fail("unexpected long P-256 DER signature")
    return b"\x30" + bytes([len(body)]) + body


def openssl_validate_public(public_x: bytes, public_y: bytes) -> None:
    public_der = SPKI_P256_PREFIX + public_x + public_y
    with tempfile.TemporaryDirectory(prefix="tohseno-authority-key-check.") as directory:
        public_path = Path(directory) / "public.der"
        public_path.write_bytes(public_der)
        public_check = subprocess.run(
            [
                "openssl",
                "pkey",
                "-pubin",
                "-inform",
                "DER",
                "-in",
                str(public_path),
                "-pubcheck",
                "-noout",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if public_check.returncode != 0:
            fail("release-authority public key is not a valid P-256 point")


def openssl_verify(public_x: bytes, public_y: bytes, digest: bytes, r: int, s: int) -> None:
    public_der = SPKI_P256_PREFIX + public_x + public_y
    signature_der = der_signature(r, s)
    with tempfile.TemporaryDirectory(prefix="tohseno-activation-verify.") as directory:
        root = Path(directory)
        public_path = root / "public.der"
        digest_path = root / "digest.bin"
        signature_path = root / "signature.der"
        public_path.write_bytes(public_der)
        digest_path.write_bytes(digest)
        signature_path.write_bytes(signature_der)
        verification = subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-verify",
                "-pubin",
                "-inkey",
                str(public_path),
                "-keyform",
                "DER",
                "-sigfile",
                str(signature_path),
                "-in",
                str(digest_path),
                "-pkeyopt",
                "digest:sha256",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if verification.returncode != 0:
            fail("release-authority P-256 signature verification failed")


def validate_generation(generation: Any, path: Path, repository_root: Path) -> bytes:
    generation = exact_object(
        generation,
        "generation",
        {"schema", "protocol", "protocol_major", "generation", "chain", "source", "build", "contracts", "create2"},
    )
    exact_string(generation["schema"], "generation.schema", "tohseno.contract-generation/1")
    exact_string(generation["protocol"], "generation.protocol", "tohseno")
    if generation["protocol_major"] != 2:
        fail("generation.protocol_major must be 2")
    if not isinstance(generation["generation"], str) or VERSION.fullmatch(generation["generation"]) is None:
        fail("generation.generation must be a canonical semantic version")

    chain = exact_object(generation["chain"], "generation.chain", {"chain_id", "p256_verifier"})
    safe_uint(chain["chain_id"], "generation.chain.chain_id", positive=True)
    verifier = exact_object(
        chain["p256_verifier"],
        "generation.chain.p256_verifier",
        {"standard", "address", "gas"},
    )
    exact_string(verifier["standard"], "generation.chain.p256_verifier.standard", "EIP-7951")
    exact_string(
        verifier["address"],
        "generation.chain.p256_verifier.address",
        "0x0000000000000000000000000000000000000100",
    )
    if verifier["gas"] != 6900:
        fail("generation.chain.p256_verifier.gas must be 6900")

    source = exact_object(
        generation["source"],
        "generation.source",
        {"commit", "tree_law", "tree_sha256", "files"},
    )
    if not isinstance(source["commit"], str) or COMMIT.fullmatch(source["commit"]) is None:
        fail("generation.source.commit must be lowercase 40-byte Git hex")
    exact_string(source["tree_law"], "generation.source.tree_law", "tohseno.contract-source-tree/1")
    declared_tree = hex32(source["tree_sha256"], "generation.source.tree_sha256")
    if not isinstance(source["files"], list) or not source["files"]:
        fail("generation.source.files must be a nonempty array")
    previous_path: str | None = None
    tree_preimage = bytearray(SOURCE_TREE_DOMAIN)
    for index, artifact in enumerate(source["files"]):
        artifact = validate_artifact(artifact, f"generation.source.files[{index}]")
        artifact_path = artifact["path"]
        if artifact_path != "foundry.toml" and not artifact_path.startswith("src/"):
            fail("generation source path must be foundry.toml or beneath src/")
        if previous_path is not None and previous_path >= artifact_path:
            fail("generation source paths must be unique and strictly ordered")
        previous_path = artifact_path
        declared_sha = hex32(artifact["sha256"], f"generation.source.files[{index}].sha256")
        tree_preimage.extend(artifact["sha256"].encode("ascii"))
        tree_preimage.extend(b" ")
        tree_preimage.extend(str(artifact["byte_length"]).encode("ascii"))
        tree_preimage.extend(b" ")
        tree_preimage.extend(artifact_path.encode("utf-8"))
        tree_preimage.extend(b"\n")
        validate_file(repository_root / "contracts" / artifact_path, artifact["byte_length"], declared_sha)
    if sha256(bytes(tree_preimage)) != declared_tree:
        fail("generation source-tree digest does not match its inventory")

    build = exact_object(
        generation["build"],
        "generation.build",
        {
            "profile", "solc_version", "evm_version", "optimizer", "optimizer_runs", "via_ir",
            "bytecode_hash", "cbor_metadata", "forge_version", "forge_commit",
        },
    )
    exact_string(build["profile"], "generation.build.profile", "default")
    if not isinstance(build["solc_version"], str) or VERSION.fullmatch(build["solc_version"]) is None:
        fail("generation.build.solc_version must be canonical")
    if build["optimizer"] is not True or build["cbor_metadata"] is not False or build["bytecode_hash"] != "none":
        fail("generation compiler metadata/optimizer profile is invalid")
    safe_uint(build["optimizer_runs"], "generation.build.optimizer_runs", positive=True)
    if not isinstance(build["via_ir"], bool):
        fail("generation.build.via_ir must be boolean")
    if not isinstance(build["forge_commit"], str) or COMMIT.fullmatch(build["forge_commit"]) is None:
        fail("generation.build.forge_commit is invalid")

    contracts = exact_object(
        generation["contracts"],
        "generation.contracts",
        {"builder_account", "builder_account_factory", "shot_registry"},
    )
    generation_directory = path.parent
    for contract_name, contract in contracts.items():
        contract = exact_object(
            contract,
            f"generation.contracts.{contract_name}",
            {"component_version", "abi", "creation_bytecode", "creation_code_keccak256", "runtime_code_keccak256"},
        )
        if not isinstance(contract["component_version"], str) or VERSION.fullmatch(contract["component_version"]) is None:
            fail(f"generation.contracts.{contract_name}.component_version is invalid")
        hex32(contract["creation_code_keccak256"], f"generation.contracts.{contract_name}.creation_code_keccak256")
        hex32(contract["runtime_code_keccak256"], f"generation.contracts.{contract_name}.runtime_code_keccak256")
        abi = validate_artifact(contract["abi"], f"generation.contracts.{contract_name}.abi")
        if not abi["path"].startswith("abi/") or not abi["path"].endswith(".json"):
            fail(f"generation.contracts.{contract_name}.abi.path is invalid")
        validate_file(
            generation_directory / abi["path"],
            abi["byte_length"],
            hex32(abi["sha256"], f"generation.contracts.{contract_name}.abi.sha256"),
        )
        bytecode = contract["creation_bytecode"]
        if bytecode is not None:
            bytecode = validate_artifact(bytecode, f"generation.contracts.{contract_name}.creation_bytecode")
            if not bytecode["path"].startswith("bytecode/") or not bytecode["path"].endswith(".creation.hex"):
                fail(f"generation.contracts.{contract_name}.creation_bytecode.path is invalid")
            validate_file(
                generation_directory / bytecode["path"],
                bytecode["byte_length"],
                hex32(bytecode["sha256"], f"generation.contracts.{contract_name}.creation_bytecode.sha256"),
            )
            encoded_bytecode = (generation_directory / bytecode["path"]).read_text(encoding="ascii").strip()
            if HEX_BYTES.fullmatch(encoded_bytecode) is None:
                fail(f"generation.contracts.{contract_name}.creation_bytecode is not lowercase hex")
            if keccak256(bytes.fromhex(encoded_bytecode[2:])) != hex32(
                contract["creation_code_keccak256"],
                f"generation.contracts.{contract_name}.creation_code_keccak256",
            ):
                fail(f"generation.contracts.{contract_name}.creation bytecode Keccak-256 mismatch")

    create2 = exact_object(
        generation["create2"],
        "generation.create2",
        {"deployer", "builder_account_factory", "shot_registry"},
    )
    deployer = address20(create2["deployer"], "generation.create2.deployer")
    for coordinate_name, contract_name in [
        ("builder_account_factory", "builder_account_factory"),
        ("shot_registry", "shot_registry"),
    ]:
        coordinate = exact_object(
            create2[coordinate_name],
            f"generation.create2.{coordinate_name}",
            {"salt", "init_code_keccak256", "predicted_address"},
        )
        salt = hex32(coordinate["salt"], f"generation.create2.{coordinate_name}.salt", nonzero=False)
        init_hash = hex32(coordinate["init_code_keccak256"], f"generation.create2.{coordinate_name}.init_code_keccak256")
        predicted = address20(
            coordinate["predicted_address"],
            f"generation.create2.{coordinate_name}.predicted_address",
        )
        if init_hash != hex32(
            contracts[contract_name]["creation_code_keccak256"],
            f"generation.contracts.{contract_name}.creation_code_keccak256",
        ):
            fail(f"generation.create2.{coordinate_name} does not bind the contract init-code hash")
        calculated = keccak256(b"\xff" + deployer + salt + init_hash)[12:]
        if calculated != predicted:
            fail(f"generation.create2.{coordinate_name}.predicted_address is not the CREATE2 result")

    return sha256(canonical_json(generation))


def validate_artifact(value: Any, field: str) -> dict[str, Any]:
    artifact = exact_object(value, field, {"path", "sha256", "byte_length"})
    if not isinstance(artifact["path"], str) or RELATIVE_PATH.fullmatch(artifact["path"]) is None:
        fail(f"{field}.path is not a closed relative path")
    hex32(artifact["sha256"], f"{field}.sha256")
    safe_uint(artifact["byte_length"], f"{field}.byte_length", positive=True)
    return artifact


def validate_file(path: Path, expected_length: int, expected_sha: bytes) -> None:
    try:
        stat = path.lstat()
        if path.is_symlink() or not path.is_file():
            fail(f"artifact is not a regular non-symlink file: {path}")
        raw = path.read_bytes()
    except OSError as exc:
        fail(f"cannot read artifact {path}: {exc}")
    if stat.st_size != expected_length or len(raw) != expected_length:
        fail(f"artifact byte length mismatch: {path}")
    if sha256(raw) != expected_sha:
        fail(f"artifact SHA-256 mismatch: {path}")


def validate_policy(policy: Any) -> tuple[bytes, dict[bytes, tuple[bytes, bytes]]]:
    policy = exact_object(
        policy,
        "policy",
        {"schema", "protocol", "protocol_major", "purpose", "threshold", "authorities", "issued_at"},
    )
    exact_string(policy["schema"], "policy.schema", "tohseno.release-authority-policy/1")
    exact_string(policy["protocol"], "policy.protocol", "tohseno")
    if policy["protocol_major"] != 2:
        fail("policy.protocol_major must be 2")
    exact_string(policy["purpose"], "policy.purpose", "contract_generation_activation")
    threshold = safe_uint(policy["threshold"], "policy.threshold", positive=True)
    if threshold <= 1:
        fail("production release-authority threshold must be greater than one")
    authorities = policy["authorities"]
    if not isinstance(authorities, list) or not 1 <= len(authorities) <= 32 or threshold > len(authorities):
        fail("policy authority count or threshold is invalid")
    canonical_timestamp(policy["issued_at"], "policy.issued_at")
    result: dict[bytes, tuple[bytes, bytes]] = {}
    previous: bytes | None = None
    for index, authority in enumerate(authorities):
        authority = exact_object(authority, f"policy.authorities[{index}]", {"key_id", "public_key"})
        key_id = hex32(authority["key_id"], f"policy.authorities[{index}].key_id")
        public_key = exact_object(
            authority["public_key"], f"policy.authorities[{index}].public_key", {"x", "y"}
        )
        x = hex32(public_key["x"], f"policy.authorities[{index}].public_key.x", nonzero=False)
        y = hex32(public_key["y"], f"policy.authorities[{index}].public_key.y", nonzero=False)
        if sha256(RELEASE_KEY_DOMAIN + x + y) != key_id:
            fail(f"policy.authorities[{index}].key_id uses the wrong derivation")
        openssl_validate_public(x, y)
        if previous is not None and previous >= key_id:
            fail("policy authorities must be unique and strictly ordered by key ID")
        previous = key_id
        result[key_id] = (x, y)
    return sha256(canonical_json(policy)), result


def validate_deployment(value: Any, field: str) -> dict[str, Any]:
    deployment = exact_object(value, field, {"transaction_hash", "block_number", "block_hash"})
    hex32(deployment["transaction_hash"], f"{field}.transaction_hash")
    safe_uint(deployment["block_number"], f"{field}.block_number", positive=True)
    hex32(deployment["block_hash"], f"{field}.block_hash")
    return deployment


def validate_activated_contract(value: Any, field: str) -> dict[str, Any]:
    contract = exact_object(value, field, {"address", "runtime_code_keccak256", "deployment"})
    address20(contract["address"], f"{field}.address")
    hex32(contract["runtime_code_keccak256"], f"{field}.runtime_code_keccak256")
    validate_deployment(contract["deployment"], f"{field}.deployment")
    return contract


def validate_chain_block(value: Any, field: str) -> dict[str, Any]:
    block = exact_object(value, field, {"block_number", "block_hash"})
    safe_uint(block["block_number"], f"{field}.block_number", positive=True)
    hex32(block["block_hash"], f"{field}.block_hash")
    return block


def validate_activation_shape(activation: Any, field: str) -> tuple[dict[str, Any], dt.datetime]:
    activation = exact_object(
        activation,
        field,
        ACTIVATION_KEYS,
    )
    exact_string(activation["schema"], f"{field}.schema", "tohseno.contract-activation/1")
    exact_string(activation["protocol"], f"{field}.protocol", "tohseno")
    if activation["protocol_major"] != 2:
        fail(f"{field}.protocol_major must be 2")
    if not isinstance(activation["generation"], str) or VERSION.fullmatch(activation["generation"]) is None:
        fail(f"{field}.generation must be a canonical semantic version")
    sequence = safe_uint(activation["activation_sequence"], f"{field}.activation_sequence", positive=True)
    if sequence == 1:
        if activation["previous_activation"] is not None:
            fail(f"{field} one must have a null predecessor")
    else:
        hex32(activation["previous_activation"], f"{field}.previous_activation")
    for digest_name in [
        "generation_definition_sha256",
        "authority_policy_sha256",
        "builder_account_runtime_keccak256",
        "p256_probe_sha256",
    ]:
        hex32(activation[digest_name], f"{field}.{digest_name}")
    safe_uint(activation["chain_id"], f"{field}.chain_id", positive=True)
    factory = validate_activated_contract(activation["factory"], f"{field}.factory")
    registry = validate_activated_contract(activation["registry"], f"{field}.registry")
    if factory["address"] == registry["address"]:
        fail(f"{field} factory and registry addresses must differ")
    activation_block = validate_chain_block(activation["activation_block"], f"{field}.activation_block")
    if factory["deployment"]["block_number"] > activation_block["block_number"] or registry["deployment"]["block_number"] > activation_block["block_number"]:
        fail(f"{field} block must be at or after both deployments")
    issued_at = canonical_timestamp(activation["issued_at"], f"{field}.issued_at")
    return activation, issued_at


def validate_activation(
    activation: Any,
    generation: dict[str, Any],
    generation_digest: bytes,
    policy_digest: bytes,
    probe_digest: bytes,
) -> tuple[bytes, dt.datetime]:
    activation, issued_at = validate_activation_shape(activation, "activation")
    if activation["generation"] != generation["generation"]:
        fail("activation generation does not match the supplied definition")
    if hex32(activation["generation_definition_sha256"], "activation.generation_definition_sha256") != generation_digest:
        fail("activation does not bind the exact supplied generation definition")
    if hex32(activation["authority_policy_sha256"], "activation.authority_policy_sha256") != policy_digest:
        fail("activation does not bind the exact supplied authority policy")
    if activation["chain_id"] != generation["chain"]["chain_id"]:
        fail("activation chain does not match the generation")
    factory = activation["factory"]
    registry = activation["registry"]
    for observed, generation_name, coordinate_name in [
        (factory, "builder_account_factory", "builder_account_factory"),
        (registry, "shot_registry", "shot_registry"),
    ]:
        if observed["address"] != generation["create2"][coordinate_name]["predicted_address"]:
            fail(f"activation {generation_name} address does not match the generation")
        # Generation runtime hashes are compiler deployed-bytecode templates.
        # Solidity immutables are zero placeholders there and constructor-patched
        # in BuilderAccount and ShotRegistry instances. The threshold-signed
        # activation binds their nonzero observed hashes independently. The
        # factory has no immutable references, so exact equality remains valid.
        if generation_name == "builder_account_factory" and observed["runtime_code_keccak256"] != generation["contracts"][generation_name]["runtime_code_keccak256"]:
            fail(f"activation {generation_name} runtime does not match the generation")
    if hex32(activation["p256_probe_sha256"], "activation.p256_probe_sha256") != probe_digest:
        fail("activation does not bind the exact supplied P-256 evidence bytes")
    digest = sha256(ACTIVATION_DOMAIN + canonical_json(activation))
    return digest, issued_at


def validate_probe(path: Path, expected_chain_id: int) -> bytes:
    try:
        raw = path.read_bytes()
    except OSError as exc:
        fail(f"cannot read P-256 evidence {path}: {exc}")
    probe = load_strict(path)
    probe = exact_object(
        probe,
        "p256_probe",
        {
            "schema", "checked_at", "chain_id", "rpc", "block", "precompile",
            "vector_provenance", "vectors", "gas", "valid",
        },
    )
    exact_string(probe["schema"], "p256_probe.schema", "tohseno.p256-probe/2")
    canonical_timestamp(probe["checked_at"], "p256_probe.checked_at")
    exact_string(probe["rpc"], "p256_probe.rpc", "explicit-target")
    exact_string(
        probe["precompile"],
        "p256_probe.precompile",
        "0x0000000000000000000000000000000000000100",
    )
    if probe["chain_id"] != expected_chain_id or probe["valid"] is not True:
        fail("P-256 evidence is not a valid observation for the activation chain")

    block = exact_object(
        probe["block"],
        "p256_probe.block",
        {"source_tag", "number", "hash", "require_canonical", "rechecked_after_probe"},
    )
    exact_string(block["source_tag"], "p256_probe.block.source_tag", "latest")
    if not isinstance(block["number"], str) or HEX_QUANTITY.fullmatch(block["number"]) is None:
        fail("p256_probe.block.number must be a canonical lowercase hex quantity")
    hex32(block["hash"], "p256_probe.block.hash")
    if block["require_canonical"] is not True or block["rechecked_after_probe"] is not True:
        fail("P-256 evidence is not bound to one rechecked canonical block")

    provenance = exact_object(
        probe["vector_provenance"],
        "p256_probe.vector_provenance",
        {"fixture_sha256", "upstream_sha256"},
    )
    for name in ["fixture_sha256", "upstream_sha256"]:
        if not isinstance(provenance[name], str) or LOWER_SHA256.fullmatch(provenance[name]) is None:
            fail(f"p256_probe.vector_provenance.{name} must be lowercase SHA-256")

    vectors = exact_object(probe["vectors"], "p256_probe.vectors", {"positive", "negative", "infinity"})
    for name in ["positive", "negative", "infinity"]:
        vector = exact_object(vectors[name], f"p256_probe.vectors.{name}", {"input", "output"})
        if not isinstance(vector["input"], str) or HEX_BYTES.fullmatch(vector["input"]) is None or len(vector["input"]) != 322:
            fail(f"p256_probe.vectors.{name}.input must be exactly 160 lowercase hex bytes")
    exact_string(
        vectors["positive"]["output"],
        "p256_probe.vectors.positive.output",
        "0x0000000000000000000000000000000000000000000000000000000000000001",
    )
    exact_string(vectors["negative"]["output"], "p256_probe.vectors.negative.output", "0x")
    exact_string(vectors["infinity"]["output"], "p256_probe.vectors.infinity.output", "0x")

    gas = exact_object(
        probe["gas"],
        "p256_probe.gas",
        {"meter_address", "meter_runtime", "meter_overhead", "measured_total", "measured_precompile", "outputs"},
    )
    exact_string(gas["meter_address"], "p256_probe.gas.meter_address", "0xfffffffffffffffffffffffffffffffffffffffe")
    exact_string(
        gas["meter_runtime"],
        "p256_probe.gas.meter_runtime",
        "0x5a365f5f3760205f365f6101005afa505a90035f5260205ff3",
    )
    if gas["meter_overhead"] != 157 or gas["measured_total"] != 7057 or gas["measured_precompile"] != 6900:
        fail("P-256 evidence does not prove exact 6,900-gas EIP-7951")
    gas_outputs = exact_object(gas["outputs"], "p256_probe.gas.outputs", {"positive", "negative", "infinity"})
    measured_output = "0x0000000000000000000000000000000000000000000000000000000000001b91"
    for name in ["positive", "negative", "infinity"]:
        exact_string(gas_outputs[name], f"p256_probe.gas.outputs.{name}", measured_output)
    return sha256(raw)


def validate_signed_activation(
    signed: Any,
    generation: dict[str, Any],
    generation_digest: bytes,
    policy: dict[str, Any],
    policy_digest: bytes,
    authorities: dict[bytes, tuple[bytes, bytes]],
    probe_digest: bytes,
) -> tuple[bytes, dt.datetime]:
    signed = exact_object(signed, "signed_activation", {"schema", "activation", "approvals"})
    exact_string(signed["schema"], "signed_activation.schema", "tohseno.signed-contract-activation/1")
    digest, issued_at = validate_activation(
        signed["activation"], generation, generation_digest, policy_digest, probe_digest
    )
    approvals = signed["approvals"]
    threshold = policy["threshold"]
    if not isinstance(approvals, list) or not threshold <= len(approvals) <= len(authorities):
        fail("signed activation does not satisfy the threshold")
    previous: bytes | None = None
    for index, approval in enumerate(approvals):
        approval = exact_object(approval, f"approvals[{index}]", {"key_id", "authorization"})
        key_id = hex32(approval["key_id"], f"approvals[{index}].key_id")
        if previous is not None and previous >= key_id:
            fail("approvals must be unique and strictly ordered by key ID")
        previous = key_id
        public_key = authorities.get(key_id)
        if public_key is None:
            fail("approval uses a key outside the supplied policy")
        authorization = exact_object(
            approval["authorization"],
            f"approvals[{index}].authorization",
            {"algorithm", "digest", "signature", "low_s"},
        )
        exact_string(authorization["algorithm"], f"approvals[{index}].authorization.algorithm", "p256")
        if authorization["low_s"] is not True:
            fail("approval must explicitly declare low_s true")
        if hex32(authorization["digest"], f"approvals[{index}].authorization.digest") != digest:
            fail("approval digest does not match the activation signing digest")
        signature = exact_object(
            authorization["signature"], f"approvals[{index}].authorization.signature", {"r", "s"}
        )
        r = int.from_bytes(hex32(signature["r"], f"approvals[{index}].signature.r", nonzero=False), "big")
        s = int.from_bytes(hex32(signature["s"], f"approvals[{index}].signature.s", nonzero=False), "big")
        if not 1 <= r < P256_ORDER or not 1 <= s <= P256_HALF_ORDER:
            fail("approval signature scalars are zero, out of range, or high-s")
        openssl_verify(public_key[0], public_key[1], digest, r, s)
    return digest, issued_at


def validate_successor(current: dict[str, Any], previous_path: Path | None) -> None:
    sequence = current["activation"]["activation_sequence"]
    if sequence == 1:
        if previous_path is not None:
            fail("activation one must not be supplied a predecessor")
        return
    if previous_path is None:
        fail("successor activation requires --previous-activation")
    previous_wrapper = load_strict(previous_path)
    if isinstance(previous_wrapper, dict) and set(previous_wrapper) == {"schema", "activation", "approvals"}:
        previous = previous_wrapper["activation"]
    else:
        previous = previous_wrapper
    previous, previous_issued_at = validate_activation_shape(previous, "previous_activation")
    previous_digest = sha256(ACTIVATION_DOMAIN + canonical_json(previous))
    if current["activation"]["previous_activation"] != "0x" + previous_digest.hex():
        fail("successor does not name the exact prior activation signing digest")
    if current["activation"]["activation_sequence"] != previous.get("activation_sequence", -1) + 1:
        fail("successor activation sequence is not contiguous")
    if current["activation"]["activation_block"]["block_number"] <= previous.get("activation_block", {}).get("block_number", MAX_SAFE_UINT):
        fail("successor activation block must advance")
    current_issued_at = canonical_timestamp(current["activation"]["issued_at"], "activation.issued_at")
    if current_issued_at < previous_issued_at:
        fail("successor activation time must not move backward")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository-root", required=True, type=Path)
    parser.add_argument("--generation", required=True, type=Path)
    parser.add_argument("--policy", required=True, type=Path)
    parser.add_argument("--signed-activation", required=True, type=Path)
    parser.add_argument("--p256-probe", required=True, type=Path)
    parser.add_argument("--trusted-policy-sha256", required=True)
    parser.add_argument("--previous-activation", type=Path)
    return parser.parse_args()


def main() -> int:
    arguments = parse_args()
    try:
        repository_root = arguments.repository_root.resolve(strict=True)
        if not repository_root.is_dir():
            fail("--repository-root must be a directory")
        generation_path = arguments.generation.resolve(strict=True)
        generation = load_strict(generation_path)
        generation_digest = validate_generation(generation, generation_path, repository_root)
        policy = load_strict(arguments.policy.resolve(strict=True))
        policy_digest, authorities = validate_policy(policy)
        expected_policy_digest = hex32(arguments.trusted_policy_sha256, "trusted_policy_sha256")
        if policy_digest != expected_policy_digest:
            fail("supplied policy does not match the explicitly configured policy digest")
        probe_digest = validate_probe(arguments.p256_probe.resolve(strict=True), generation["chain"]["chain_id"])
        signed = load_strict(arguments.signed_activation.resolve(strict=True))
        signing_digest, issued_at = validate_signed_activation(
            signed,
            generation,
            generation_digest,
            policy,
            policy_digest,
            authorities,
            probe_digest,
        )
        validate_successor(signed, arguments.previous_activation)
        openssl_version = subprocess.run(
            ["openssl", "version"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=True
        ).stdout.strip()
        report = {
            "schema": "tohseno.contract-activation-independent-verification/1",
            "approved_under_explicit_policy_digest": True,
            "generation_definition_sha256": "0x" + generation_digest.hex(),
            "authority_policy_sha256": "0x" + policy_digest.hex(),
            "activation_signing_sha256": "0x" + signing_digest.hex(),
            "p256_probe_sha256": "0x" + probe_digest.hex(),
            "activation_sequence": signed["activation"]["activation_sequence"],
            "activation_issued_at": issued_at.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "threshold": policy["threshold"],
            "approvals_verified": len(signed["approvals"]),
            "signature_backend": openssl_version,
            "trust_statement": "The policy matches the explicit digest input; owner authorization of that digest remains external evidence.",
        }
        print(json.dumps(report, sort_keys=True, separators=(",", ":")))
        return 0
    except (VerificationError, OSError, subprocess.SubprocessError) as exc:
        print(f"verify-contract-activation.py: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
