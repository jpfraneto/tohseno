#!/usr/bin/env python3
"""Deterministic private-planning fixture for the ontology lifecycle smoke.

This is test infrastructure, not a product conception fallback. It consumes
the exact engine-produced conception input, emits one deliberately bounded
app-specific proposal, and later binds real XCUITest evidence into a trial.
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


def substrate_organs() -> list[dict]:
    return [
        {
            "organ_id": "substrate_installation_identity",
            "kind": "protocol_substrate",
            "provides": ["embedded_shot_identity"],
            "owns_state": ["installation_key_reference"],
            "permissions": [],
            "dependencies": [],
            "emits": ["installation_identity_ready"],
            "consumes": [],
            "genome_invariants": [
                "factory_substrate: installation identity remains app-specific and non-exportable"
            ],
            "requirement_ids": [],
            "capability_ids": [],
            "journey_ids": [],
            "acceptance_criteria": [
                {
                    "id": "installation_identity_embedded",
                    "assertion": "Release artifact embeds valid app-installation identity support",
                    "deterministic": True,
                }
            ],
            "platforms": ["iphone"],
        },
        {
            "organ_id": "substrate_signed_continuity",
            "kind": "protocol_substrate",
            "provides": ["signed_continuity", "embedded_provenance"],
            "owns_state": ["continuity_envelope"],
            "permissions": [],
            "dependencies": ["substrate_installation_identity"],
            "emits": ["continuity_verified"],
            "consumes": ["installation_identity_ready"],
            "genome_invariants": [
                "factory_substrate: signed identity and exact provenance remain truthful"
            ],
            "requirement_ids": [],
            "capability_ids": [],
            "journey_ids": [],
            "acceptance_criteria": [
                {
                    "id": "signed_continuity_embedded",
                    "assertion": "Release artifact embeds the exact signed continuity and provenance",
                    "deterministic": True,
                }
            ],
            "platforms": ["iphone"],
        },
    ]


def conception(input_path: Path, output_path: Path) -> None:
    source = load_object(input_path)
    if source.get("schema") != "tohseno.conception-input/1":
        fail("unsupported conception input")
    app_name = source.get("app_name")
    intent_digest = source.get("intent_digest")
    factory = source.get("factory_identity")
    if not isinstance(app_name, str) or not isinstance(intent_digest, str):
        fail("conception input omits app name or intention digest")
    if not isinstance(factory, dict):
        fail("conception input omits factory identity")
    profile_digest = factory.get("apple_capability_profile_digest")
    if not isinstance(profile_digest, str):
        fail("conception input omits capability-profile digest")

    visible_invariant = (
        "A lifecycle verifier reaches the complete visible expression and sees both bounded "
        "confirmation statements without explanation."
    )
    requirements = [
        {
            "id": "visible_launch",
            "statement": "Show a complete, quiet native iPhone expression with a clear visible identity.",
            "level": "must",
            "origin": "explicit_intention",
            "source_excerpt": "Show a complete, quiet native iPhone expression with a clear visible identity.",
        },
        {
            "id": "materialization_confirmation",
            "statement": "State that this is a TOHSENO expression and that the Apple materialization gates passed.",
            "level": "must",
            "origin": "explicit_intention",
            "source_excerpt": "The primary screen must state that it is a TOHSENO expression and that the Apple materialization gates passed.",
        },
    ]
    organs = substrate_organs()
    organs.append(
        {
            "organ_id": "visible_lifecycle_expression",
            "kind": "app_specific",
            "provides": ["primary_expression"],
            "owns_state": ["visible_expression_state"],
            "permissions": [],
            "dependencies": ["substrate_signed_continuity"],
            "emits": ["primary_expression_visible"],
            "consumes": ["continuity_verified"],
            "genome_invariants": [visible_invariant],
            "requirement_ids": ["visible_launch", "materialization_confirmation"],
            "capability_ids": [],
            "journey_ids": ["primary_continuity_journey"],
            "acceptance_criteria": [
                {
                    "id": "bounded_promise_visible",
                    "assertion": "XCUITest observes both exact bounded confirmation statements on first launch",
                    "deterministic": True,
                }
            ],
            "platforms": ["iphone"],
        }
    )
    plan = {
        "schema": "tohseno.birth-plan/1",
        "intent_digest": intent_digest,
        "product_name": app_name,
        "promise": "A lifecycle verifier can launch a complete native expression and immediately see that its bounded Apple materialization promise is alive.",
        "target_users": [
            {
                "id": "lifecycle_verifier",
                "role": "person verifying a local TOHSENO Shot",
                "environment": ["an iPhone Simulator during the isolated lifecycle smoke"],
                "goals": ["see the bounded native expression launch completely"],
                "constraints": ["the fixture must not need network access or private user input"],
                "understands_without_explanation": [
                    "that the native expression is present and the bounded materialization gates passed"
                ],
            }
        ],
        "contexts": ["isolated source-built ontology lifecycle verification"],
        "requirements": requirements,
        "capabilities": [],
        "journeys": [
            {
                "id": "primary_continuity_journey",
                "target_actor": "lifecycle_verifier",
                "promise": "Launch once and observe both exact bounded confirmation statements.",
                "requirement_ids": ["visible_launch", "materialization_confirmation"],
            }
        ],
        "embodiment": organs,
        "completion_contract": {
            "must_requirement_ids": ["visible_launch", "materialization_confirmation"],
            "required_scenario_ids": ["primary_continuity_journey"],
            "physical_verification_capabilities": [],
            "release_build_required": True,
            "zero_product_gaps_required": True,
        },
        "explicit_non_goals": [
            "This deterministic factory smoke does not stand in for conception of a human product."
        ],
        "forbidden_substitutions": [
            {
                "id": "visible_app_to_build_only",
                "requested_experience": "a launched native screen with both exact confirmation statements",
                "forbidden_replacement": "a build-only artifact, blank screen, or console-only claim",
                "requirement_ids": ["visible_launch", "materialization_confirmation"],
            }
        ],
        "genome": {
            "schema": "tohseno.genome/2",
            "revision": 1,
            "purpose": "Keep the isolated lifecycle fixture a complete visible native expression rather than a build-only claim.",
            "intended_for": ["A person verifying a local TOHSENO Shot"],
            "essential_experience": [
                "Launch the app and see both bounded confirmation statements on the primary screen."
            ],
            "behavioral_invariants": [visible_invariant],
            "interaction_laws": ["First launch directly reveals the complete bounded promise."],
            "aesthetic_principles": ["The verification expression remains quiet, legible, and visually coherent."],
            "privacy_principles": ["The fixture collects and transmits no target-user data."],
            "ownership_principles": ["The local Builder retains the signed Shot and source lineage."],
            "platform_commitments": ["The expression is a buildable native iPhone application."],
            "boundaries": ["No network, tracking, analytics, or sensitive product capability is introduced."],
            "non_goals": ["The fixture is not a general note-taking product."],
            "required_capabilities": [],
            "forbidden_transformations": [
                "Do not replace the visible launched expression with a build-only or console-only result."
            ],
            "acceptance_principles": [
                "Accept only after Release build, XCUITest journey, intent review, and protocol conformance pass independently."
            ],
            "freely_changeable": ["Decorative symbol size and spacing may change without hiding the bounded promise."],
        },
    }
    plan_digest = digest_json(plan)
    contract = {
        "schema": "tohseno.experience-contract/1",
        "intent_digest": intent_digest,
        "birth_plan_digest": plan_digest,
        "scenarios": [
            {
                "id": "primary_continuity_journey",
                "target_actor": "lifecycle_verifier",
                "initial_state": "the application is not running",
                "environment": ["a booted iPhone Simulator"],
                "steps_or_gestures": ["launch the application", "read the primary screen"],
                "expected_states": [
                    "A TOHSENO expression is visible",
                    "This fixture passes the real Apple materialization gates is visible",
                ],
                "requirement_ids": ["visible_launch", "materialization_confirmation"],
                "capability_ids": [],
                "completion_condition": "both exact confirmation statements exist in the accessibility tree",
                "evidence_required": ["xcui_test"],
                "physical_device_required": False,
            }
        ],
    }
    output = {
        "schema": "tohseno.conception-output/1",
        "intent_digest": intent_digest,
        "apple_capability_profile_digest": profile_digest,
        "birth_plan": plan,
        "experience_contract": contract,
        "rationale": "The isolated smoke intention is deliberately bounded to one visible first-launch journey; protocol substrate remains separate from the app-specific visible-expression organ.",
    }
    write_object(output_path, output)


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
    if len(arguments) == 4 and arguments[1] == "conception":
        conception(Path(arguments[2]), Path(arguments[3]))
        return
    if len(arguments) == 4 and arguments[1] == "trial":
        trial(Path(arguments[2]), Path(arguments[3]))
        return
    fail("usage: prepare-birth-fixture.py conception INPUT OUTPUT | trial PLANNING_ROOT SHOT_ROOT")


if __name__ == "__main__":
    main(sys.argv)
