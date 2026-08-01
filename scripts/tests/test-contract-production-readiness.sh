#!/bin/sh
set -eu

script_directory="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_directory/../.." && pwd)"

python3 - "$repository_root" <<'PY'
import hashlib
import json
from pathlib import Path
import re
import sys


class DuplicateMember(ValueError):
    pass


def closed_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateMember(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def load(path):
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=closed_object)


root = Path(sys.argv[1]).resolve()
readiness_path = root / "release/CONTRACT_0_8_0_PRODUCTION_READINESS.json"
readiness = load(readiness_path)

assert readiness["schema"] == "tohseno.contract-candidate-readiness/1"
assert readiness["record_kind"] == "production_readiness_evidence"
assert readiness["ready"] is False
assert readiness["deployment_authorized"] is False
assert readiness["activation_authorized"] is False

pairs = [
    (readiness["security_review"]["internal_pre_audit"], readiness["security_review"]["internal_pre_audit_sha256"]),
    (readiness["security_review"]["independent_ai_audits"][0]["report"], readiness["security_review"]["independent_ai_audits"][0]["report_sha256"]),
    (readiness["security_review"]["independent_ai_audits"][1]["report"], readiness["security_review"]["independent_ai_audits"][1]["report_sha256"]),
    (readiness["security_review"]["independent_ai_disposition"], readiness["security_review"]["independent_ai_disposition_sha256"]),
    (readiness["security_review"]["manual_audit_brief"], readiness["security_review"]["manual_audit_brief_sha256"]),
    (readiness["external_audit"]["request_record"], readiness["external_audit"]["request_record_sha256"]),
    (readiness["external_audit"]["provider_recovery_request"], readiness["external_audit"]["provider_recovery_request_sha256"]),
    (readiness["external_audit"]["provider_recovery_chain_evidence"], readiness["external_audit"]["provider_recovery_chain_evidence_sha256"]),
    (readiness["release_authority"]["proposed_runbook"], readiness["release_authority"]["proposed_runbook_sha256"]),
    (readiness["release_authority"]["independent_verifier"], readiness["release_authority"]["independent_verifier_sha256"]),
    (readiness["release_authority"]["public_policy_preparer"], readiness["release_authority"]["public_policy_preparer_sha256"]),
    (readiness["release_authority"]["independent_policy_digest_verifier"], readiness["release_authority"]["independent_policy_digest_verifier_sha256"]),
    (readiness["release_authority"]["public_policy_preparation_safety_test"], readiness["release_authority"]["public_policy_preparation_safety_test_sha256"]),
    (readiness["deployment_authority"]["proposal"], readiness["deployment_authority"]["proposal_sha256"]),
    (readiness["deployment_authority"]["prepared_operator_runbook"], readiness["deployment_authority"]["prepared_operator_runbook_sha256"]),
    (readiness["deployment_authority"]["create2_risk_policy_proposal"], readiness["deployment_authority"]["create2_risk_policy_proposal_sha256"]),
    (readiness["deployment_authority"]["read_only_preflight_verifier"], readiness["deployment_authority"]["read_only_preflight_verifier_sha256"]),
    (readiness["deployment_authority"]["deployment_evidence"], readiness["deployment_authority"]["deployment_evidence_sha256"]),
    (readiness["deployment_authority"]["activation_semantics_adr"], readiness["deployment_authority"]["activation_semantics_adr_sha256"]),
    (readiness["read_only_robinhood_preflight"]["p256_evidence"], readiness["read_only_robinhood_preflight"]["p256_evidence_sha256"]),
    (readiness["read_only_robinhood_preflight"]["candidate_evidence"], readiness["read_only_robinhood_preflight"]["candidate_evidence_sha256"]),
    (readiness["production_canary_preparation"]["runbook"], readiness["production_canary_preparation"]["runbook_sha256"]),
    (readiness["downstream_public_lifecycle_readiness"]["gap_audit"], readiness["downstream_public_lifecycle_readiness"]["gap_audit_sha256"]),
]

for relative, expected in pairs:
    assert isinstance(relative, str) and not relative.startswith("/") and ".." not in Path(relative).parts
    assert isinstance(expected, str) and re.fullmatch(r"[0-9a-f]{64}", expected)
    path = root / relative
    assert path.is_file() and not path.is_symlink(), f"missing or unsafe evidence: {relative}"
    observed = hashlib.sha256(path.read_bytes()).hexdigest()
    assert observed == expected, f"stale evidence digest for {relative}: {observed} != {expected}"

external = readiness["external_audit"]
assert external["prior_payment_created_job"] is False
assert external["fresh_payment_authorized"] is False
assert external["job_id"] is None
assert external["report_sha256"] is None
recovery = load(root / external["provider_recovery_chain_evidence"])
assert recovery["schema"] == "tohseno.external-audit-recovery-evidence/1"
assert recovery["payment"]["status"] == 1
assert recovery["payment"]["amount_base_units"] == "1000000"
assert recovery["job_contract"]["get_jobs_by_client"] == []
assert recovery["job_contract"]["delayed_job_found"] is False
assert recovery["fresh_payment_authorized"] is False

authority = readiness["release_authority"]
assert authority["owner_accepted_design"] is False
assert authority["production_policy_exists"] is False
assert authority["trusted_policy_digest"] is None
assert authority["production_private_keys_generated"] is False
assert authority["production_public_policy_constructed"] is False

deployment = readiness["deployment_authority"]
assert deployment["owner_decision"] == "accepted_inactive_deployment_only_and_consumed"
assert deployment["accepted_policy_or_adr_exists"] is True
assert deployment["ceremony_implementation_exists"] is False
assert deployment["broadcast_authorized"] is False
assert deployment["broadcast_authorization_consumed"] is True
assert deployment["deployment_completed"] is True

preflight = readiness["read_only_robinhood_preflight"]
assert preflight["authorization_value"] == "historical_consumed_ceremony_gate"
assert preflight["reusable_for_deployment"] is False
assert preflight["private_key_accessed"] is False
assert preflight["transaction_signed"] is False
assert preflight["transaction_broadcast"] is False
candidate = load(root / preflight["candidate_evidence"])
assert candidate["reusable_for_broadcast"] is False
assert candidate["safety"]["deployment_authorized"] is False
assert candidate["safety"]["private_key_accessed"] is False
assert candidate["safety"]["transaction_signed"] is False
assert candidate["safety"]["transaction_broadcast"] is False

canary = readiness["production_canary_preparation"]
assert canary["transactions_authorized"] is False
assert canary["started_at"] is None
assert canary["completed_at"] is None
assert canary["result"] == "not_started"

public = readiness["downstream_public_lifecycle_readiness"]
assert public["private_creation_evolution_lifecycle_available"] is True
for key in (
    "activated_generation_resolver_implemented",
    "app_metadata_v3_schema_accepted",
    "successor_apple_fascia_implemented",
    "registry_publication_workflow_implemented",
    "two_node_publication_discovery_verified",
    "bounded_remote_feedback_transport_implemented",
    "public_lifecycle_complete",
):
    assert public[key] is False, f"unsupported completion claim: {key}"

bankr = readiness["downstream_bankr_readiness"]
assert bankr["credential_persistence_forbidden"] is True
assert bankr["simulation_before_deploy_required"] is True
assert bankr["single_use_confirmation_required"] is True
assert bankr["broadcast_unlocked"] is False
assert bankr["token_deployed"] is False

phase_status = {phase["phase"]: phase["status"] for phase in readiness["phases"]}
assert phase_status[1] == "pass" and phase_status[2] == "pass"
assert phase_status[3] == "pass_for_inactive_deployment"
assert phase_status[4] == "blocked"
assert phase_status[5] == "pass_inactive_deployed"
assert all(phase_status[number] == "not_started" for number in range(6, 10))
PY

printf '%s\n' "Contract-production readiness evidence is internally consistent."
