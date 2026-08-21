#!/usr/bin/env python3
"""Deterministic private-planning fixture for the ontology lifecycle smoke.

This is test infrastructure. The engine composes the Birth Plan and Experience
Contract itself, so this fixture only binds real XCUITest evidence into the
trial the engine asks a harness to return.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import sys


def fail(message: str) -> "NoReturn":
    raise SystemExit(f"prepare-birth-fixture.py: {message}")


def load_object(path: Path) -> dict:
    if path.is_symlink() or not path.is_file():
        fail(f"input must be a regular file: {path}")
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        fail(f"input is not an object: {path}")
    return value


def canonical_bytes(value: object) -> bytes:
    # The fixture contains only RFC 8785-safe strings, booleans, arrays,
    # objects, and integers. Sorted compact UTF-8 is therefore its JCS form.
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def digest_bytes(value: bytes) -> str:
    return "0x" + hashlib.sha256(value).hexdigest()


def digest_json(value: object) -> str:
    return digest_bytes(canonical_bytes(value))


def write_object(path: Path, value: object) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".fixture-tmp")
    with temporary.open("w", encoding="utf-8") as handle:
        json.dump(value, handle, ensure_ascii=False, indent=2, sort_keys=True)
        handle.write("\n")
    os.chmod(temporary, 0o600)
    temporary.replace(path)


def evidence(repository: Path, relative: str, kind: str, media_type: str) -> dict:
    path = repository / relative
    if path.is_symlink() or not path.is_file():
        fail(f"evidence must be a regular file: {path}")
    body = path.read_bytes()
    if not body:
        fail(f"evidence is empty: {path}")
    return {
        "kind": kind,
        "artifact": {
            "digest": digest_bytes(body),
            "media_type": media_type,
            "byte_length": len(body),
            "name": path.name,
        },
        "relative_path": relative,
    }


def criterion(identifier: str, proof: dict, deterministic: bool = True) -> dict:
    return {
        "id": identifier,
        "passed": True,
        "deterministic": deterministic,
        "evidence": [proof],
    }


def trial(planning: Path, repository: Path) -> None:
    plan = load_object(planning / "birth-plan.json")
    contract = load_object(planning / "experience-contract.json")
    test_relative = ".tohseno/private/birth/evidence/simulator-test.log"
    review_relative = ".tohseno/private/birth/evidence/intent-review.txt"
    xcui = evidence(repository, test_relative, "xcui_test", "text/plain")
    release = evidence(repository, test_relative, "release_build", "text/plain")
    review = evidence(repository, review_relative, "intelligent_review", "text/plain")
    scenario_results = []
    for scenario in contract.get("scenarios", []):
        scenario_results.append(
            {
                "scenario_id": scenario["id"],
                "passed": True,
                "assertions": [
                    criterion(f"{scenario['id']}_assertion", xcui)
                ],
                "evidence": [xcui, release],
            }
        )
    organ_results = []
    for organ in plan.get("embodiment", []):
        organ_results.append(
            {
                "organ_id": organ["organ_id"],
                "criteria": [
                    criterion(item["id"], xcui, item["deterministic"])
                    for item in organ.get("acceptance_criteria", [])
                ],
            }
        )
    substitutions = [
        criterion(item["id"], review, False)
        for item in plan.get("forbidden_substitutions", [])
    ]
    value = {
        "schema": "tohseno.experience-trial/1",
        "birth_plan_digest": digest_json(plan),
        "experience_contract_digest": digest_json(contract),
        "release_build_passed": True,
        "automated_tests_passed": True,
        "simulator_trial_passed": True,
        "scenario_results": scenario_results,
        "organ_results": organ_results,
        "forbidden_substitution_results": substitutions,
        "intent_review": criterion("intention_review", review, False),
        "incompleteness": [],
    }
    write_object(planning / "experience-trial.json", value)


def main(arguments: list[str]) -> None:
    if len(arguments) == 4 and arguments[1] == "trial":
        trial(Path(arguments[2]), Path(arguments[3]))
        return
    fail("usage: prepare-birth-fixture.py trial PLANNING_ROOT SHOT_ROOT")


if __name__ == "__main__":
    main(sys.argv)
