#!/bin/sh
set -eu

script_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
fascia_root=$(dirname "$script_directory")

python3 - "$fascia_root" <<'PY'
import json
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
manifest = json.loads((root / "FASCIA.json").read_text(encoding="utf-8"))
schema = json.loads((root / "FASCIA.schema.json").read_text(encoding="utf-8"))

def resolve(reference):
    assert reference.startswith("#/")
    value = schema
    for token in reference[2:].split("/"):
        value = value[token.replace("~1", "/").replace("~0", "~")]
    return value

def validate(specification, value, path="$"):
    if "$ref" in specification:
        validate(resolve(specification["$ref"]), value, path)
        return
    if "const" in specification:
        assert value == specification["const"], f"{path}: const mismatch"
    if "enum" in specification:
        assert value in specification["enum"], f"{path}: not in enum"

    expected_type = specification.get("type")
    if expected_type == "object":
        assert isinstance(value, dict), f"{path}: expected object"
    elif expected_type == "array":
        assert isinstance(value, list), f"{path}: expected array"
    elif expected_type == "string":
        assert isinstance(value, str), f"{path}: expected string"
    elif expected_type == "boolean":
        assert isinstance(value, bool), f"{path}: expected boolean"

    if isinstance(value, str):
        assert len(value) >= specification.get("minLength", 0), (
            f"{path}: string is too short"
        )
        if "pattern" in specification:
            assert re.search(specification["pattern"], value), (
                f"{path}: pattern mismatch"
            )

    if isinstance(value, list):
        assert len(value) >= specification.get("minItems", 0), (
            f"{path}: too few items"
        )
        if "maxItems" in specification:
            assert len(value) <= specification["maxItems"], (
                f"{path}: too many items"
            )
        if specification.get("uniqueItems"):
            encoded = [
                json.dumps(item, sort_keys=True, separators=(",", ":"))
                for item in value
            ]
            assert len(encoded) == len(set(encoded)), f"{path}: duplicate items"
        if "items" in specification:
            for index, item in enumerate(value):
                validate(specification["items"], item, f"{path}[{index}]")

    if isinstance(value, dict):
        for name in specification.get("required", []):
            assert name in value, f"{path}: missing {name}"
        properties = specification.get("properties", {})
        if specification.get("additionalProperties") is False:
            extras = set(value) - set(properties)
            assert not extras, f"{path}: unexpected {sorted(extras)}"
        for name, item in value.items():
            if name in properties:
                validate(properties[name], item, f"{path}.{name}")

    for subschema in specification.get("allOf", []):
        validate(subschema, value, path)
    if "if" in specification:
        try:
            validate(specification["if"], value, path)
        except AssertionError:
            if "else" in specification:
                validate(specification["else"], value, path)
        else:
            if "then" in specification:
                validate(specification["then"], value, path)

validate(schema, manifest)

assert manifest["protocol"] == "tohseno"
assert manifest["schema"] == "tohseno.apple-fascia-definition/1"
assert manifest["fascia"] == "tohseno.apple/1"
assert schema["$schema"] == "https://json-schema.org/draft/2020-12/schema"
assert schema["properties"]["schema"]["const"] == manifest["schema"]
assert set(schema["required"]) == set(schema["properties"])
assert schema["additionalProperties"] is False
assert manifest["candidate"] == {
    "version": "0.7.0",
    "codename": "GENESIS",
    "status": "protocol_candidate_not_canonical",
}

expected_docs = {
    "FASCIA.md",
    "IDENTITY.md",
    "STORAGE.md",
    "CONTINUITY.md",
    "PRIVACY.md",
    "PROVENANCE.md",
    "DISTRIBUTION.md",
}
assert expected_docs == {
    pathlib.PurePosixPath(path).name
    for path in manifest["required_files"]["documentation"]
}
for name in expected_docs:
    assert (root / name).is_file(), name

expected_sources = {
    "InstallationIdentity.swift",
    "ContinuityEnvelope.swift",
    "LocalPersistence.swift",
    "Provenance.swift",
    "TohsenoMetadata.swift",
}
assert expected_sources == {
    pathlib.PurePosixPath(path).name
    for path in manifest["required_files"]["swift_sources"]
}
for name in expected_sources:
    assert (root / "swift" / name).is_file(), name
observed_source_entries = list((root / "swift").iterdir())
assert all(path.is_file() and not path.is_symlink() for path in observed_source_entries)
assert {path.name for path in observed_source_entries} == expected_sources

expected_capabilities = [
    "local_storage",
    "network_access",
    "private_cloudkit_sync",
    "storekit",
    "notifications",
    "camera",
    "microphone",
    "location",
    "contacts",
    "health",
    "bluetooth",
    "other_apple_entitlements",
]
observed_capabilities = [
    entry["id"] for entry in manifest["capability_vocabulary"]
]
assert observed_capabilities == expected_capabilities
assert len(observed_capabilities) == len(set(observed_capabilities))

gate_ids = [entry["id"] for entry in manifest["conformance_gates"]]
assert len(gate_ids) == len(set(gate_ids))
assert {
    "required_files",
    "target_membership",
    "installation_identity_bootstrap",
    "third_party_dependencies",
    "capability_declarations",
    "privacy_boundary",
    "bundle_identity",
    "bundle_version",
    "provenance",
    "offline_build",
}.issubset(gate_ids)

print("FASCIA.json and normative file inventory are valid.")
PY
