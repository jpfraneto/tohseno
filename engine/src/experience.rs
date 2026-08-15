use crate::birth_plan::{BirthExpressionPlan, BirthPlan, OrganKind, RequirementLevel};
use crate::factory_identity::FactoryIdentity;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use tohseno_protocol::canonical;
use tohseno_protocol::digest::Bytes32;
use tohseno_protocol::ontology::{
    ArtifactAvailability, ArtifactDescriptor, AvailabilityStatus, ARTIFACT_AVAILABILITY_SCHEMA,
};

pub const EXPERIENCE_CONTRACT_SCHEMA: &str = "tohseno.experience-contract/1";
pub const EXPERIENCE_TRIAL_SCHEMA: &str = "tohseno.experience-trial/1";
pub const BIRTH_RECEIPT_SCHEMA: &str = "tohseno.birth-receipt/1";

#[derive(Clone, Debug)]
pub struct BirthEvaluationEvidence {
    pub source_digest: Bytes32,
    pub factory_identity: FactoryIdentity,
    pub protocol_criteria: Vec<CriterionResult>,
    pub engine_experience_criteria: Vec<CriterionResult>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    XcuiTest,
    XcTest,
    Screenshot,
    Video,
    Log,
    PersistedState,
    ReleaseBuild,
    PhysicalDeviceTrial,
    IntelligentReview,
}

impl EvidenceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::XcuiTest => "xcui_test",
            Self::XcTest => "xc_test",
            Self::Screenshot => "screenshot",
            Self::Video => "video",
            Self::Log => "log",
            Self::PersistedState => "persisted_state",
            Self::ReleaseBuild => "release_build",
            Self::PhysicalDeviceTrial => "physical_device_trial",
            Self::IntelligentReview => "intelligent_review",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperienceScenario {
    pub id: String,
    pub target_actor: String,
    pub initial_state: String,
    pub environment: Vec<String>,
    pub steps_or_gestures: Vec<String>,
    pub expected_states: Vec<String>,
    pub requirement_ids: Vec<String>,
    pub capability_ids: Vec<String>,
    pub completion_condition: String,
    pub evidence_required: Vec<EvidenceKind>,
    pub physical_device_required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperienceContract {
    pub schema: String,
    pub intent_digest: Bytes32,
    pub birth_plan_digest: Bytes32,
    pub scenarios: Vec<ExperienceScenario>,
}

impl ExperienceContract {
    pub fn digest(&self) -> Result<Bytes32, ExperienceError> {
        let bytes = canonical::to_vec(self).map_err(|error| {
            ExperienceError(format!("experience contract encoding failed: {error}"))
        })?;
        Ok(tohseno_protocol::digest::sha256(&bytes))
    }

    pub fn validate(&self, plan: &BirthPlan) -> Result<(), ExperienceError> {
        ensure(
            self.schema == EXPERIENCE_CONTRACT_SCHEMA,
            "unsupported experience contract schema",
        )?;
        ensure(
            self.intent_digest == plan.intent_digest,
            "experience contract is bound to a different intention",
        )?;
        ensure(
            self.birth_plan_digest
                == plan
                    .digest()
                    .map_err(|error| ExperienceError(error.to_string()))?,
            "experience contract is bound to a different Birth Plan",
        )?;
        ensure(
            !self.scenarios.is_empty(),
            "experience contract has no scenarios",
        )?;
        let actors = plan
            .target_users
            .iter()
            .map(|actor| actor.id.as_str())
            .collect::<BTreeSet<_>>();
        let requirements = plan
            .requirements
            .iter()
            .map(|requirement| requirement.id.as_str())
            .collect::<BTreeSet<_>>();
        let capabilities = plan
            .capabilities
            .iter()
            .map(|capability| capability.identifier.as_str())
            .collect::<BTreeSet<_>>();
        let must_requirements = plan
            .requirements
            .iter()
            .filter(|requirement| requirement.level == RequirementLevel::Must)
            .map(|requirement| requirement.id.as_str())
            .collect::<BTreeSet<_>>();
        let required_scenarios = plan
            .completion_contract
            .required_scenario_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut ids = BTreeSet::new();
        let mut covered = BTreeSet::new();
        for scenario in &self.scenarios {
            valid_id("scenario", &scenario.id)?;
            ensure(
                ids.insert(scenario.id.as_str()),
                "scenario IDs must be unique",
            )?;
            ensure(
                actors.contains(scenario.target_actor.as_str()),
                format!("scenario `{}` has an unknown target actor", scenario.id),
            )?;
            ensure(
                !scenario.initial_state.trim().is_empty()
                    && !scenario.steps_or_gestures.is_empty()
                    && !scenario.expected_states.is_empty()
                    && !scenario.completion_condition.trim().is_empty()
                    && !scenario.evidence_required.is_empty(),
                format!("scenario `{}` is behaviorally incomplete", scenario.id),
            )?;
            ensure(
                scenario.evidence_required.iter().any(|kind| {
                    matches!(
                        kind,
                        EvidenceKind::XcuiTest | EvidenceKind::PhysicalDeviceTrial
                    )
                }),
                format!(
                    "scenario `{}` needs executable target-user journey evidence",
                    scenario.id
                ),
            )?;
            ensure(
                !scenario.physical_device_required
                    || scenario
                        .evidence_required
                        .contains(&EvidenceKind::PhysicalDeviceTrial),
                format!(
                    "physical scenario `{}` must require physical-device-trial evidence",
                    scenario.id
                ),
            )?;
            referenced(
                "scenario requirement",
                &scenario.requirement_ids,
                &requirements,
            )?;
            referenced(
                "scenario capability",
                &scenario.capability_ids,
                &capabilities,
            )?;
            covered.extend(scenario.requirement_ids.iter().map(String::as_str));
        }
        ensure(
            required_scenarios.is_subset(&ids),
            "completion contract names a missing experience scenario",
        )?;
        ensure(
            must_requirements.is_subset(&covered),
            "every must-level requirement needs scenario or deterministic evidence coverage",
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReference {
    pub kind: EvidenceKind,
    pub artifact: ArtifactDescriptor,
    /// Path relative to the Shot repository. It is useful local context, not a
    /// canonical location and is not written into the signed lineage.
    pub relative_path: String,
}

impl EvidenceReference {
    pub fn availability(&self) -> ArtifactAvailability {
        ArtifactAvailability {
            schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
            artifact: self.artifact.clone(),
            status: AvailabilityStatus::LocallyAvailable,
            locations: Vec::new(),
        }
    }

    fn validate(&self) -> Result<(), ExperienceError> {
        self.artifact
            .validate()
            .map_err(|error| ExperienceError(format!("invalid evidence artifact: {error}")))?;
        ensure(
            !self.relative_path.is_empty()
                && !self.relative_path.starts_with('/')
                && !self.relative_path.split('/').any(|part| part == ".."),
            "evidence path must be a safe repository-relative path",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionResult {
    pub id: String,
    pub passed: bool,
    pub deterministic: bool,
    pub evidence: Vec<EvidenceReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
}

impl CriterionResult {
    fn validate(&self) -> Result<(), ExperienceError> {
        valid_id("criterion result", &self.id)?;
        ensure(
            !self.passed || !self.evidence.is_empty(),
            format!("passing criterion `{}` has no evidence", self.id),
        )?;
        for evidence in &self.evidence {
            evidence.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioTrialResult {
    pub scenario_id: String,
    pub passed: bool,
    pub assertions: Vec<CriterionResult>,
    pub evidence: Vec<EvidenceReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganTrialResult {
    pub organ_id: String,
    pub criteria: Vec<CriterionResult>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalDeviceEvidence {
    pub product_type: String,
    pub os_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_build: Option<String>,
    pub exercised_capabilities: Vec<String>,
    pub passed: bool,
    pub evidence: Vec<EvidenceReference>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompletenessCategory {
    ProductGap,
    ExperienceVerificationGap,
    ExternalEnvironmentConstraint,
    FutureOpportunity,
    ExplicitNonGoal,
}

impl IncompletenessCategory {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ProductGap => "product_gap",
            Self::ExperienceVerificationGap => "experience_verification_gap",
            Self::ExternalEnvironmentConstraint => "external_environment_constraint",
            Self::FutureOpportunity => "future_opportunity",
            Self::ExplicitNonGoal => "explicit_non_goal",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedIncompleteness {
    pub id: String,
    pub category: IncompletenessCategory,
    pub description: String,
    pub requirement_ids: Vec<String>,
    pub blocks_completion: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperienceTrial {
    pub schema: String,
    pub birth_plan_digest: Bytes32,
    pub experience_contract_digest: Bytes32,
    pub release_build_passed: bool,
    pub automated_tests_passed: bool,
    pub simulator_trial_passed: bool,
    pub scenario_results: Vec<ScenarioTrialResult>,
    pub organ_results: Vec<OrganTrialResult>,
    pub forbidden_substitution_results: Vec<CriterionResult>,
    pub intent_review: CriterionResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_device: Option<PhysicalDeviceEvidence>,
    pub incompleteness: Vec<TypedIncompleteness>,
}

impl ExperienceTrial {
    pub fn validate(
        &self,
        plan: &BirthPlan,
        expression: &BirthExpressionPlan,
        contract: &ExperienceContract,
    ) -> Result<(), ExperienceError> {
        ensure(
            self.schema == EXPERIENCE_TRIAL_SCHEMA,
            "unsupported experience trial schema",
        )?;
        ensure(
            self.birth_plan_digest
                == plan
                    .digest()
                    .map_err(|error| ExperienceError(error.to_string()))?,
            "experience trial is bound to a different Birth Plan",
        )?;
        let expected_contract_digest = contract.digest()?;
        ensure(
            self.experience_contract_digest == expected_contract_digest,
            format!(
                "experience trial is bound to a different Experience Contract: trial declares {}, expected canonical digest {}",
                self.experience_contract_digest, expected_contract_digest
            ),
        )?;
        self.intent_review.validate()?;

        let requirements = plan
            .requirements
            .iter()
            .map(|requirement| requirement.id.as_str())
            .collect::<BTreeSet<_>>();
        let scenarios = contract
            .scenarios
            .iter()
            .map(|scenario| (scenario.id.as_str(), scenario))
            .collect::<BTreeMap<_, _>>();
        let mut result_ids = BTreeSet::new();
        for result in &self.scenario_results {
            valid_id("scenario result", &result.scenario_id)?;
            let scenario = scenarios.get(result.scenario_id.as_str()).ok_or_else(|| {
                ExperienceError(format!(
                    "trial result names unknown scenario `{}`",
                    result.scenario_id
                ))
            })?;
            ensure(
                result_ids.insert(result.scenario_id.as_str()),
                "scenario trial results must be unique",
            )?;
            for assertion in &result.assertions {
                assertion.validate()?;
            }
            for evidence in &result.evidence {
                evidence.validate()?;
            }
            ensure(
                !result.passed
                    || (!result.assertions.is_empty()
                        && !result.evidence.is_empty()
                        && result.assertions.iter().all(|assertion| assertion.passed)),
                format!(
                    "passing scenario `{}` needs passing assertions and evidence",
                    scenario.id
                ),
            )?;
            let evidence_kinds = result
                .evidence
                .iter()
                .map(|evidence| evidence.kind)
                .collect::<BTreeSet<_>>();
            let missing_evidence = scenario
                .evidence_required
                .iter()
                .filter(|kind| !evidence_kinds.contains(kind))
                .map(|kind| kind.as_str())
                .collect::<Vec<_>>();
            ensure(
                !result.passed || missing_evidence.is_empty(),
                format!(
                    "passing scenario `{}` lacks required evidence classes: {}",
                    scenario.id,
                    missing_evidence.join(", ")
                ),
            )?;
        }

        let required_scenarios = plan
            .completion_contract
            .required_scenario_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        ensure(
            required_scenarios.is_subset(&result_ids),
            "experience trial omits a required scenario",
        )?;

        let planned_criteria = expression.criterion_index();
        let mut observed_criteria = BTreeSet::new();
        for organ_result in &self.organ_results {
            valid_id("organ result", &organ_result.organ_id)?;
            for criterion in &organ_result.criteria {
                criterion.validate()?;
                let key = (organ_result.organ_id.clone(), criterion.id.clone());
                ensure(
                    planned_criteria.contains_key(&key),
                    format!(
                        "trial result names unknown organ criterion `{}/{}`",
                        organ_result.organ_id, criterion.id
                    ),
                )?;
                ensure(
                    observed_criteria.insert(key),
                    "organ criterion results must be independent and unique",
                )?;
            }
        }
        ensure(
            observed_criteria.len() == planned_criteria.len(),
            "every organ acceptance criterion needs its own result",
        )?;
        for result in &self.forbidden_substitution_results {
            result.validate()?;
        }
        let expected_substitutions = plan
            .forbidden_substitutions
            .iter()
            .map(|substitution| substitution.id.as_str())
            .collect::<BTreeSet<_>>();
        let observed_substitutions = self
            .forbidden_substitution_results
            .iter()
            .map(|result| result.id.as_str())
            .collect::<BTreeSet<_>>();
        ensure(
            expected_substitutions == observed_substitutions,
            "every forbidden substitution needs an explicit review result",
        )?;

        for gap in &self.incompleteness {
            valid_id("incompleteness", &gap.id)?;
            ensure(
                !gap.description.trim().is_empty(),
                "incompleteness needs a description",
            )?;
            referenced(
                "incompleteness requirement",
                &gap.requirement_ids,
                &requirements,
            )?;
            match gap.category {
                IncompletenessCategory::ProductGap => {
                    ensure(gap.blocks_completion, "a product gap must block completion")?
                }
                IncompletenessCategory::ExperienceVerificationGap => {
                    let touched_must = plan
                        .requirements
                        .iter()
                        .filter(|requirement| {
                            requirement.level == RequirementLevel::Must
                                && gap.requirement_ids.contains(&requirement.id)
                        })
                        .map(|requirement| requirement.id.as_str())
                        .collect::<Vec<_>>();
                    ensure(
                        touched_must.is_empty() || gap.blocks_completion,
                        format!(
                            "verification gap `{}` touches must requirements [{}] and must block completion",
                            gap.id,
                            touched_must.join(", ")
                        ),
                    )?;
                }
                IncompletenessCategory::FutureOpportunity => ensure(
                    !gap.blocks_completion,
                    "a future opportunity is not current incompleteness",
                )?,
                IncompletenessCategory::ExternalEnvironmentConstraint
                | IncompletenessCategory::ExplicitNonGoal => {}
            }
        }
        if let Some(device) = &self.physical_device {
            ensure(
                device.product_type.starts_with("iPhone")
                    && device.product_type.len() <= 32
                    && device
                        .product_type
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b',')
                    && !device.os_version.is_empty()
                    && device.os_version.len() <= 32
                    && device
                        .os_version
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || byte == b'.'),
                "physical-device evidence omits non-secret device facts",
            )?;
            let planned_capabilities = plan
                .capabilities
                .iter()
                .map(|capability| capability.identifier.as_str())
                .collect::<BTreeSet<_>>();
            referenced(
                "physical-device exercised capability",
                &device.exercised_capabilities,
                &planned_capabilities,
            )?;
            for evidence in &device.evidence {
                evidence.validate()?;
            }
            ensure(
                !device.passed
                    || device
                        .evidence
                        .iter()
                        .any(|evidence| evidence.kind == EvidenceKind::PhysicalDeviceTrial),
                "passing physical-device verification needs physical-device-trial evidence",
            )?;
        }
        let all_evidence = self
            .scenario_results
            .iter()
            .flat_map(|result| result.evidence.iter())
            .chain(
                self.organ_results
                    .iter()
                    .flat_map(|result| result.criteria.iter())
                    .flat_map(|criterion| criterion.evidence.iter()),
            )
            .collect::<Vec<_>>();
        ensure(
            !self.release_build_passed
                || all_evidence
                    .iter()
                    .any(|evidence| evidence.kind == EvidenceKind::ReleaseBuild),
            "a passing Release build needs Release-build evidence",
        )?;
        ensure(
            !self.automated_tests_passed
                || all_evidence.iter().any(|evidence| {
                    matches!(evidence.kind, EvidenceKind::XcuiTest | EvidenceKind::XcTest)
                }),
            "passing automated tests need XCTest or XCUITest evidence",
        )?;
        ensure(
            !self.simulator_trial_passed
                || all_evidence.iter().any(|evidence| {
                    matches!(
                        evidence.kind,
                        EvidenceKind::XcuiTest | EvidenceKind::Screenshot | EvidenceKind::Video
                    )
                }),
            "a passing Simulator trial needs journey evidence",
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceDimension {
    pub passed: bool,
    pub criteria: Vec<CriterionResult>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BirthReceipt {
    pub schema: String,
    pub birth_plan_digest: Bytes32,
    pub experience_contract_digest: Bytes32,
    pub experience_trial_digest: Bytes32,
    pub source_digest: Bytes32,
    pub factory_identity: FactoryIdentity,
    pub protocol_conformance: AcceptanceDimension,
    pub intent_fidelity: AcceptanceDimension,
    pub experience_verification: AcceptanceDimension,
    pub incompleteness: Vec<TypedIncompleteness>,
    pub accepted: bool,
}

impl BirthReceipt {
    pub fn digest(&self) -> Result<Bytes32, ExperienceError> {
        let bytes = canonical::to_vec(self)
            .map_err(|error| ExperienceError(format!("birth receipt encoding failed: {error}")))?;
        Ok(tohseno_protocol::digest::sha256(&bytes))
    }

    pub fn validate(&self) -> Result<(), ExperienceError> {
        ensure(
            self.schema == BIRTH_RECEIPT_SCHEMA,
            "unsupported birth receipt schema",
        )?;
        ensure(
            self.birth_plan_digest != Bytes32::ZERO
                && self.experience_contract_digest != Bytes32::ZERO
                && self.experience_trial_digest != Bytes32::ZERO
                && self.source_digest != Bytes32::ZERO,
            "birth receipt digests must be nonzero",
        )?;
        self.factory_identity.validate().map_err(ExperienceError)?;
        for dimension in [
            &self.protocol_conformance,
            &self.intent_fidelity,
            &self.experience_verification,
        ] {
            ensure(
                !dimension.criteria.is_empty(),
                "acceptance dimension has no criteria",
            )?;
            for criterion in &dimension.criteria {
                criterion.validate()?;
            }
            ensure(
                dimension.passed == dimension.criteria.iter().all(|criterion| criterion.passed),
                "acceptance dimension must equal the conjunction of its criteria",
            )?;
        }
        let no_blocking_gap = self.incompleteness.iter().all(|gap| !gap.blocks_completion)
            && self
                .incompleteness
                .iter()
                .all(|gap| gap.category != IncompletenessCategory::ProductGap);
        let expected = self.protocol_conformance.passed
            && self.intent_fidelity.passed
            && self.experience_verification.passed
            && no_blocking_gap;
        ensure(
            self.accepted == expected,
            "birth acceptance must equal all three dimensions and no blocking product gap",
        )?;
        if self.accepted {
            ensure(
                !self.incompleteness.iter().any(|gap| {
                    matches!(
                        gap.category,
                        IncompletenessCategory::FutureOpportunity
                            | IncompletenessCategory::ExplicitNonGoal
                    )
                }),
                "future opportunities and explicit non-goals do not belong to accepted incompleteness",
            )?;
        }
        Ok(())
    }
}

pub fn evaluate_birth(
    plan: &BirthPlan,
    expression: &BirthExpressionPlan,
    contract: &ExperienceContract,
    trial: &ExperienceTrial,
    evidence: BirthEvaluationEvidence,
) -> Result<BirthReceipt, ExperienceError> {
    let BirthEvaluationEvidence {
        source_digest,
        factory_identity,
        protocol_criteria,
        engine_experience_criteria,
    } = evidence;
    plan.validate()
        .map_err(|error| ExperienceError(error.to_string()))?;
    expression
        .validate(&plan.genome)
        .map_err(|error| ExperienceError(error.to_string()))?;
    contract.validate(plan)?;
    trial.validate(plan, expression, contract)?;
    ensure(
        source_digest != Bytes32::ZERO,
        "candidate source digest is zero",
    )?;
    factory_identity.validate().map_err(ExperienceError)?;
    ensure(
        factory_identity.accepted_shot_genome_digest
            == Some(
                plan.genome
                    .digest()
                    .map_err(|error| ExperienceError(error.to_string()))?,
            ),
        "birth receipt factory identity names a different accepted Genome",
    )?;
    ensure(
        !protocol_criteria.is_empty(),
        "protocol conformance has no criteria",
    )?;
    for criterion in &protocol_criteria {
        criterion.validate()?;
    }
    ensure(
        !engine_experience_criteria.is_empty(),
        "the engine did not independently execute birth experience tests",
    )?;
    for criterion in &engine_experience_criteria {
        criterion.validate()?;
    }
    let plan_bytes = canonical::to_vec(plan)
        .map_err(|error| ExperienceError(format!("Birth Plan encoding failed: {error}")))?;
    let trial_bytes = canonical::to_vec(trial)
        .map_err(|error| ExperienceError(format!("trial encoding failed: {error}")))?;
    let trial_digest = tohseno_protocol::digest::sha256(&trial_bytes);

    let mut fidelity = Vec::new();
    fidelity.push(CriterionResult {
        id: "must_requirements_mapped".into(),
        passed: true,
        deterministic: true,
        evidence: vec![synthetic_evidence(
            "birth-plan.json",
            tohseno_protocol::digest::sha256(&plan_bytes),
            plan_bytes.len(),
            EvidenceKind::Log,
        )],
        observation: Some("every must requirement maps to app-specific organs and journeys".into()),
    });
    fidelity.push(trial.intent_review.clone());
    fidelity.extend(trial.forbidden_substitution_results.clone());
    for organ_result in &trial.organ_results {
        let kind = expression
            .organs
            .iter()
            .find(|organ| organ.organ_id == organ_result.organ_id)
            .map(|organ| organ.kind);
        if kind == Some(OrganKind::AppSpecific) {
            fidelity.extend(organ_result.criteria.clone());
        }
    }

    let scenario_by_id = trial
        .scenario_results
        .iter()
        .map(|scenario| (scenario.scenario_id.as_str(), scenario))
        .collect::<BTreeMap<_, _>>();
    let scenario_evidence = trial
        .scenario_results
        .iter()
        .flat_map(|result| result.evidence.iter());
    let no_blocking_incompleteness = trial
        .incompleteness
        .iter()
        .all(|gap| !gap.blocks_completion && gap.category != IncompletenessCategory::ProductGap);
    let mut experience = vec![
        CriterionResult {
            id: "release_build".into(),
            passed: trial.release_build_passed,
            deterministic: true,
            evidence: scenario_evidence
                .clone()
                .find(|evidence| evidence.kind == EvidenceKind::ReleaseBuild)
                .cloned()
                .into_iter()
                .collect(),
            observation: None,
        },
        CriterionResult {
            id: "automated_tests".into(),
            passed: trial.automated_tests_passed,
            deterministic: true,
            evidence: scenario_evidence
                .clone()
                .filter(|evidence| {
                    matches!(evidence.kind, EvidenceKind::XcuiTest | EvidenceKind::XcTest)
                })
                .cloned()
                .collect(),
            observation: None,
        },
        CriterionResult {
            id: "simulator_target_user_trial".into(),
            passed: trial.simulator_trial_passed,
            deterministic: false,
            evidence: scenario_evidence
                .filter(|evidence| {
                    matches!(
                        evidence.kind,
                        EvidenceKind::XcuiTest | EvidenceKind::Screenshot | EvidenceKind::Video
                    )
                })
                .cloned()
                .collect(),
            observation: None,
        },
        CriterionResult {
            id: "no_blocking_product_or_experience_gap".into(),
            passed: no_blocking_incompleteness,
            deterministic: true,
            evidence: no_blocking_incompleteness
                .then(|| {
                    synthetic_evidence(
                        "experience-trial.json",
                        trial_digest,
                        trial_bytes.len(),
                        EvidenceKind::Log,
                    )
                })
                .into_iter()
                .collect(),
            observation: (!no_blocking_incompleteness)
                .then(|| "typed incompleteness blocks the completion contract".into()),
        },
    ];
    for scenario_id in &plan.completion_contract.required_scenario_ids {
        let result = scenario_by_id.get(scenario_id.as_str()).ok_or_else(|| {
            ExperienceError(format!("required scenario `{scenario_id}` has no result"))
        })?;
        experience.push(CriterionResult {
            id: format!("scenario.{}", result.scenario_id),
            passed: result.passed,
            deterministic: result
                .assertions
                .iter()
                .all(|assertion| assertion.deterministic),
            evidence: result.evidence.clone(),
            observation: None,
        });
    }
    if !plan
        .completion_contract
        .physical_verification_capabilities
        .is_empty()
    {
        let physical = trial.physical_device.as_ref();
        let required = plan
            .completion_contract
            .physical_verification_capabilities
            .iter()
            .all(|required| {
                physical.is_some_and(|device| device.exercised_capabilities.contains(required))
            });
        experience.push(CriterionResult {
            id: "physical_device_experience".into(),
            passed: physical.is_some_and(|device| device.passed) && required,
            deterministic: false,
            evidence: physical
                .into_iter()
                .flat_map(|device| device.evidence.clone())
                .collect(),
            observation: if physical.is_none() {
                Some("implementation_complete; acceptance_pending_physical_experience".into())
            } else {
                None
            },
        });
    }
    experience.extend(engine_experience_criteria);
    let protocol = AcceptanceDimension {
        passed: protocol_criteria.iter().all(|criterion| criterion.passed),
        criteria: protocol_criteria,
    };
    let intent_fidelity = AcceptanceDimension {
        passed: fidelity.iter().all(|criterion| criterion.passed),
        criteria: fidelity,
    };
    let experience_verification = AcceptanceDimension {
        passed: experience.iter().all(|criterion| criterion.passed),
        criteria: experience,
    };
    let no_blocking_gap = no_blocking_incompleteness;
    let accepted = protocol.passed
        && intent_fidelity.passed
        && experience_verification.passed
        && no_blocking_gap;
    let receipt = BirthReceipt {
        schema: BIRTH_RECEIPT_SCHEMA.into(),
        birth_plan_digest: plan
            .digest()
            .map_err(|error| ExperienceError(error.to_string()))?,
        experience_contract_digest: contract.digest()?,
        experience_trial_digest: trial_digest,
        source_digest,
        factory_identity,
        protocol_conformance: protocol,
        intent_fidelity,
        experience_verification,
        incompleteness: trial.incompleteness.clone(),
        accepted,
    };
    receipt.validate()?;
    Ok(receipt)
}

fn synthetic_evidence(
    name: &str,
    digest: Bytes32,
    byte_length: usize,
    kind: EvidenceKind,
) -> EvidenceReference {
    EvidenceReference {
        kind,
        artifact: ArtifactDescriptor {
            digest,
            media_type: "application/json".into(),
            byte_length: byte_length.try_into().unwrap_or(u64::MAX),
            name: Some(name.into()),
        },
        relative_path: format!(".tohseno/private/birth/{name}"),
    }
}

fn referenced(
    field: &str,
    references: &[String],
    known: &BTreeSet<&str>,
) -> Result<(), ExperienceError> {
    let mut unique = BTreeSet::new();
    for reference in references {
        ensure(
            known.contains(reference.as_str()),
            format!("{field} `{reference}` is not declared"),
        )?;
        ensure(
            unique.insert(reference),
            format!("{field} `{reference}` repeats"),
        )?;
    }
    Ok(())
}

fn valid_id(field: &str, value: &str) -> Result<(), ExperienceError> {
    ensure(
        !value.is_empty()
            && value.len() <= 255
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.')
            }),
        format!("{field} `{value}` must be a bounded lower-case token"),
    )
}

fn ensure(condition: bool, message: impl Into<String>) -> Result<(), ExperienceError> {
    if condition {
        Ok(())
    } else {
        Err(ExperienceError(message.into()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExperienceError(pub String);

impl fmt::Display for ExperienceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ExperienceError {}
