use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{Bytes32, ExpressionId};
use tohseno_protocol::ontology::{Organ, ORGAN_SCHEMA};

pub const BIRTH_PLAN_SCHEMA: &str = "tohseno.birth-plan/1";
pub const BIRTH_EXPRESSION_PLAN_SCHEMA: &str = "tohseno.birth-expression-plan/1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementLevel {
    Must,
    Should,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementOrigin {
    ExplicitIntention,
    ReferenceImage,
    InferredToCompleteIntent,
    ApplePlatformRequired,
    ProtocolRequired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetUser {
    pub id: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability_or_age_context: Option<String>,
    pub environment: Vec<String>,
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
    pub understands_without_explanation: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BirthRequirement {
    pub id: String,
    pub statement: String,
    pub level: RequirementLevel,
    pub origin: RequirementOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedCapability {
    pub identifier: String,
    pub purpose: String,
    pub requirement_ids: Vec<String>,
    pub primary: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_fallback: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductJourney {
    pub id: String,
    pub target_actor: String,
    pub promise: String,
    pub requirement_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganKind {
    ProtocolSubstrate,
    AppSpecific,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganAcceptanceCriterion {
    pub id: String,
    pub assertion: String,
    pub deterministic: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BirthOrganPlan {
    pub organ_id: String,
    pub kind: OrganKind,
    pub provides: Vec<String>,
    pub owns_state: Vec<String>,
    pub permissions: Vec<String>,
    pub dependencies: Vec<String>,
    pub emits: Vec<String>,
    pub consumes: Vec<String>,
    pub genome_invariants: Vec<String>,
    pub requirement_ids: Vec<String>,
    pub capability_ids: Vec<String>,
    pub journey_ids: Vec<String>,
    pub acceptance_criteria: Vec<OrganAcceptanceCriterion>,
    pub platforms: Vec<String>,
}

impl BirthOrganPlan {
    pub fn to_protocol_organ(&self, expression_id: ExpressionId) -> Organ {
        Organ {
            schema: ORGAN_SCHEMA.into(),
            expression_id,
            organ_id: self.organ_id.clone(),
            provides: self.provides.clone(),
            owns_state: self.owns_state.clone(),
            permissions: self.permissions.clone(),
            dependencies: self.dependencies.clone(),
            emits: self.emits.clone(),
            consumes: self.consumes.clone(),
            satisfies_genome_constraints: self.genome_invariants.clone(),
            acceptance_tests: self
                .acceptance_criteria
                .iter()
                .map(|criterion| format!("{}: {}", criterion.id, criterion.assertion))
                .collect(),
            platforms: self.platforms.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionContract {
    pub must_requirement_ids: Vec<String>,
    pub required_scenario_ids: Vec<String>,
    pub physical_verification_capabilities: Vec<String>,
    pub release_build_required: bool,
    pub zero_product_gaps_required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForbiddenSubstitution {
    pub id: String,
    pub requested_experience: String,
    pub forbidden_replacement: String,
    pub requirement_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_runtime_condition: Option<String>,
}

/// The intelligence-produced, app-specific interpretation of one exact human
/// intention. It supplements rather than replaces the canonical Intention.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BirthPlan {
    pub schema: String,
    pub intent_digest: Bytes32,
    pub product_name: String,
    pub promise: String,
    pub target_users: Vec<TargetUser>,
    pub contexts: Vec<String>,
    pub requirements: Vec<BirthRequirement>,
    pub capabilities: Vec<PlannedCapability>,
    pub journeys: Vec<ProductJourney>,
    pub embodiment: Vec<BirthOrganPlan>,
    pub completion_contract: CompletionContract,
    pub explicit_non_goals: Vec<String>,
    pub forbidden_substitutions: Vec<ForbiddenSubstitution>,
    pub genome: tohseno_protocol::Genome,
}

impl BirthPlan {
    pub fn digest(&self) -> Result<Bytes32, BirthPlanError> {
        self.validate()?;
        let bytes = canonical::to_vec(self)
            .map_err(|error| BirthPlanError(format!("birth plan encoding failed: {error}")))?;
        Ok(tohseno_protocol::digest::sha256(&bytes))
    }

    pub fn validate(&self) -> Result<(), BirthPlanError> {
        require(
            self.schema == BIRTH_PLAN_SCHEMA,
            "unsupported birth plan schema",
        )?;
        require(
            self.intent_digest != Bytes32::ZERO,
            "intent digest must be nonzero",
        )?;
        require_text("product_name", &self.product_name)?;
        require_text("promise", &self.promise)?;
        require(
            !self.target_users.is_empty(),
            "target users must not be empty",
        )?;
        require(
            !self.requirements.is_empty(),
            "requirements must not be empty",
        )?;
        require(!self.journeys.is_empty(), "journeys must not be empty")?;
        self.genome
            .validate()
            .map_err(|error| BirthPlanError(format!("app-specific Genome is invalid: {error}")))?;
        require(
            self.genome.revision == 1,
            "an initial birth plan must propose Genome revision 1",
        )?;
        let generic_actor = "the owner and the people identified by the preserved intention";
        let mut actor_ids = BTreeSet::new();
        for actor in &self.target_users {
            require_id("target user", &actor.id)?;
            require_text("target user role", &actor.role)?;
            require(
                !actor.role.to_ascii_lowercase().contains(generic_actor),
                "target users must be derived from the intention rather than the generic factory fallback",
            )?;
            require(
                actor_ids.insert(actor.id.as_str()),
                "target user IDs must be unique",
            )?;
        }

        let mut requirement_ids = BTreeSet::new();
        let mut must_ids = BTreeSet::new();
        for requirement in &self.requirements {
            require_id("requirement", &requirement.id)?;
            require_text("requirement statement", &requirement.statement)?;
            require(
                requirement_ids.insert(requirement.id.as_str()),
                "requirement IDs must be unique",
            )?;
            if requirement.level == RequirementLevel::Must {
                must_ids.insert(requirement.id.as_str());
            }
            if matches!(
                requirement.origin,
                RequirementOrigin::ExplicitIntention | RequirementOrigin::ReferenceImage
            ) {
                require(
                    requirement
                        .source_excerpt
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty())
                        || requirement
                            .source_location
                            .as_deref()
                            .is_some_and(|value| !value.trim().is_empty()),
                    "explicit and reference-image requirements need a source excerpt or location",
                )?;
            }
        }
        require(
            !must_ids.is_empty(),
            "a birth plan needs at least one must requirement",
        )?;

        let mut capability_ids = BTreeSet::new();
        for capability in &self.capabilities {
            require_id("capability", &capability.identifier)?;
            require_text("capability purpose", &capability.purpose)?;
            require(
                capability_ids.insert(capability.identifier.as_str()),
                "planned capability identifiers must be unique",
            )?;
            references_exist(
                "capability requirement",
                &capability.requirement_ids,
                &requirement_ids,
            )?;
            require(
                self.genome
                    .required_capabilities
                    .iter()
                    .any(|value| value == &capability.identifier),
                format!(
                    "planned capability `{}` is absent from the app-specific Genome",
                    capability.identifier
                ),
            )?;
        }

        let mut journey_ids = BTreeSet::new();
        let mut scenario_requirement_coverage = BTreeSet::new();
        for journey in &self.journeys {
            require_id("journey", &journey.id)?;
            require_text("journey promise", &journey.promise)?;
            require(
                actor_ids.contains(journey.target_actor.as_str()),
                format!("journey `{}` has an unknown target actor", journey.id),
            )?;
            require(
                journey_ids.insert(journey.id.as_str()),
                "journey IDs must be unique",
            )?;
            references_exist(
                "journey requirement",
                &journey.requirement_ids,
                &requirement_ids,
            )?;
            scenario_requirement_coverage
                .extend(journey.requirement_ids.iter().map(String::as_str));
        }

        let mut organ_ids = BTreeSet::new();
        let mut product_requirement_coverage = BTreeSet::new();
        let genome_invariants = self
            .genome
            .behavioral_invariants
            .iter()
            .chain(self.genome.interaction_laws.iter())
            .chain(self.genome.aesthetic_principles.iter())
            .chain(self.genome.privacy_principles.iter())
            .chain(self.genome.ownership_principles.iter())
            .chain(self.genome.platform_commitments.iter())
            .chain(self.genome.boundaries.iter())
            .chain(self.genome.acceptance_principles.iter())
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut app_specific_count = 0_usize;
        for organ in &self.embodiment {
            validate_organ(
                organ,
                &requirement_ids,
                &capability_ids,
                &journey_ids,
                &organ_ids,
            )?;
            if organ.kind == OrganKind::ProtocolSubstrate {
                require(
                    organ.requirement_ids.is_empty()
                        && organ.capability_ids.is_empty()
                        && organ.journey_ids.is_empty(),
                    format!(
                        "protocol substrate `{}` cannot claim product requirements, capabilities, or journeys",
                        organ.organ_id
                    ),
                )?;
            } else {
                for invariant in &organ.genome_invariants {
                    require(
                        genome_invariants.contains(invariant.as_str()),
                        format!(
                            "app-specific organ `{}` cites an invariant absent from the accepted Genome",
                            organ.organ_id
                        ),
                    )?;
                }
                app_specific_count += 1;
                product_requirement_coverage
                    .extend(organ.requirement_ids.iter().map(String::as_str));
            }
            organ_ids.insert(organ.organ_id.as_str());
        }
        require(
            app_specific_count > 0,
            "birth embodiment needs app-specific organs",
        )?;

        for required in &must_ids {
            require(
                product_requirement_coverage.contains(required),
                format!("must requirement `{required}` has no app-specific organ"),
            )?;
            require(
                scenario_requirement_coverage.contains(required),
                format!("must requirement `{required}` has no target-user journey"),
            )?;
        }

        references_exist(
            "completion must requirement",
            &self.completion_contract.must_requirement_ids,
            &requirement_ids,
        )?;
        let completion_must = self
            .completion_contract
            .must_requirement_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        require(
            must_ids.is_subset(&completion_must),
            "completion contract must contain every must-level requirement",
        )?;
        references_exist(
            "physical verification capability",
            &self.completion_contract.physical_verification_capabilities,
            &capability_ids,
        )?;
        require(
            self.completion_contract.release_build_required,
            "birth completion must require a Release build",
        )?;
        require(
            self.completion_contract.zero_product_gaps_required,
            "birth completion must reject product gaps",
        )?;

        let mut substitution_ids = BTreeSet::new();
        for substitution in &self.forbidden_substitutions {
            require_id("forbidden substitution", &substitution.id)?;
            require_text("requested experience", &substitution.requested_experience)?;
            require_text("forbidden replacement", &substitution.forbidden_replacement)?;
            references_exist(
                "forbidden substitution requirement",
                &substitution.requirement_ids,
                &requirement_ids,
            )?;
            require(
                substitution_ids.insert(substitution.id.as_str()),
                "forbidden substitution IDs must be unique",
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BirthExpressionPlan {
    pub schema: String,
    pub kind: String,
    pub name: String,
    pub platforms: Vec<String>,
    pub genome_revision: u64,
    pub genome_digest: Bytes32,
    pub birth_plan_digest: Bytes32,
    pub organs: Vec<BirthOrganPlan>,
}

impl BirthExpressionPlan {
    pub fn from_birth_plan(plan: &BirthPlan) -> Result<Self, BirthPlanError> {
        let birth_plan_digest = plan.digest()?;
        let expression = Self {
            schema: BIRTH_EXPRESSION_PLAN_SCHEMA.into(),
            kind: "native_apple_application".into(),
            name: plan.product_name.clone(),
            platforms: vec!["iphone".into()],
            genome_revision: plan.genome.revision,
            genome_digest: plan
                .genome
                .digest()
                .map_err(|error| BirthPlanError(error.to_string()))?,
            birth_plan_digest,
            organs: plan.embodiment.clone(),
        };
        expression.validate(&plan.genome)?;
        Ok(expression)
    }

    pub fn validate(&self, genome: &tohseno_protocol::Genome) -> Result<(), BirthPlanError> {
        require(
            self.schema == BIRTH_EXPRESSION_PLAN_SCHEMA,
            "unsupported birth expression plan schema",
        )?;
        require(
            self.kind == "native_apple_application",
            "unsupported expression kind",
        )?;
        require_text("expression name", &self.name)?;
        require(
            self.platforms == ["iphone"],
            "birth expression must target iPhone",
        )?;
        require(
            self.genome_revision == genome.revision
                && self.genome_digest
                    == genome
                        .digest()
                        .map_err(|error| BirthPlanError(error.to_string()))?,
            "birth expression does not bind the proposed app-specific Genome",
        )?;
        require(
            self.birth_plan_digest != Bytes32::ZERO,
            "birth plan digest is zero",
        )?;
        require(!self.organs.is_empty(), "birth expression has no organs")?;
        let provided = self
            .organs
            .iter()
            .filter(|organ| organ.kind == OrganKind::AppSpecific)
            .flat_map(|organ| organ.provides.iter().map(String::as_str))
            .collect::<BTreeSet<_>>();
        for capability in &genome.required_capabilities {
            require(
                provided.contains(capability.as_str()),
                format!("app-specific organs do not provide Genome capability `{capability}`"),
            )?;
        }
        Ok(())
    }

    pub fn protocol_organs(&self, expression_id: ExpressionId) -> Vec<Organ> {
        self.organs
            .iter()
            .map(|organ| organ.to_protocol_organ(expression_id))
            .collect()
    }

    pub fn criterion_index(&self) -> BTreeMap<(String, String), &OrganAcceptanceCriterion> {
        self.organs
            .iter()
            .flat_map(|organ| {
                organ.acceptance_criteria.iter().map(move |criterion| {
                    ((organ.organ_id.clone(), criterion.id.clone()), criterion)
                })
            })
            .collect()
    }
}

/// Universal protocol and provenance substrate. It is intentionally excluded
/// from product requirement, capability, and journey coverage.
pub fn protocol_substrate_organs() -> Vec<BirthOrganPlan> {
    vec![
        BirthOrganPlan {
            organ_id: "substrate_installation_identity".into(),
            kind: OrganKind::ProtocolSubstrate,
            provides: vec!["embedded_shot_identity".into()],
            owns_state: vec!["installation_key_reference".into()],
            permissions: Vec::new(),
            dependencies: Vec::new(),
            emits: vec!["installation_identity_ready".into()],
            consumes: Vec::new(),
            genome_invariants: vec![
                "factory_substrate: installation identity remains app-specific and non-exportable"
                    .into(),
            ],
            requirement_ids: Vec::new(),
            capability_ids: Vec::new(),
            journey_ids: Vec::new(),
            acceptance_criteria: vec![OrganAcceptanceCriterion {
                id: "installation_identity_embedded".into(),
                assertion: "Release artifact embeds valid app-installation identity support".into(),
                deterministic: true,
            }],
            platforms: vec!["iphone".into()],
        },
        BirthOrganPlan {
            organ_id: "substrate_signed_continuity".into(),
            kind: OrganKind::ProtocolSubstrate,
            provides: vec!["signed_continuity".into(), "embedded_provenance".into()],
            owns_state: vec!["continuity_envelope".into()],
            permissions: Vec::new(),
            dependencies: vec!["substrate_installation_identity".into()],
            emits: vec!["continuity_verified".into()],
            consumes: vec!["installation_identity_ready".into()],
            genome_invariants: vec![
                "factory_substrate: signed identity and exact provenance remain truthful".into(),
            ],
            requirement_ids: Vec::new(),
            capability_ids: Vec::new(),
            journey_ids: Vec::new(),
            acceptance_criteria: vec![OrganAcceptanceCriterion {
                id: "signed_continuity_embedded".into(),
                assertion: "Release artifact embeds the exact signed continuity and provenance"
                    .into(),
                deterministic: true,
            }],
            platforms: vec!["iphone".into()],
        },
    ]
}

fn validate_organ<'a>(
    organ: &'a BirthOrganPlan,
    requirements: &BTreeSet<&str>,
    capabilities: &BTreeSet<&str>,
    journeys: &BTreeSet<&str>,
    earlier_organs: &BTreeSet<&'a str>,
) -> Result<(), BirthPlanError> {
    require_id("organ", &organ.organ_id)?;
    require(
        !organ.provides.is_empty(),
        format!("organ `{}` provides nothing", organ.organ_id),
    )?;
    require(
        !organ.genome_invariants.is_empty(),
        format!("organ `{}` has no invariant binding", organ.organ_id),
    )?;
    require(
        !organ.acceptance_criteria.is_empty(),
        format!(
            "organ `{}` has no independently verifiable acceptance criteria",
            organ.organ_id
        ),
    )?;
    require(
        organ.platforms.iter().any(|platform| platform == "iphone"),
        format!("organ `{}` does not support iPhone", organ.organ_id),
    )?;
    for dependency in &organ.dependencies {
        require(
            earlier_organs.contains(dependency.as_str()),
            format!(
                "organ `{}` depends on undeclared or later organ `{dependency}`",
                organ.organ_id
            ),
        )?;
    }
    references_exist("organ requirement", &organ.requirement_ids, requirements)?;
    references_exist("organ capability", &organ.capability_ids, capabilities)?;
    references_exist("organ journey", &organ.journey_ids, journeys)?;
    let mut criterion_ids = BTreeSet::new();
    for criterion in &organ.acceptance_criteria {
        require_id("acceptance criterion", &criterion.id)?;
        require_text("acceptance assertion", &criterion.assertion)?;
        require(
            criterion_ids.insert(criterion.id.as_str()),
            format!("organ `{}` repeats an acceptance criterion", organ.organ_id),
        )?;
    }
    Ok(())
}

fn references_exist(
    field: &str,
    references: &[String],
    known: &BTreeSet<&str>,
) -> Result<(), BirthPlanError> {
    let mut unique = BTreeSet::new();
    for reference in references {
        require_id(field, reference)?;
        require(
            known.contains(reference.as_str()),
            format!("{field} `{reference}` is not declared"),
        )?;
        require(
            unique.insert(reference),
            format!("{field} `{reference}` is repeated"),
        )?;
    }
    Ok(())
}

fn require_id(field: &str, value: &str) -> Result<(), BirthPlanError> {
    require(
        !value.is_empty()
            && value.len() <= 128
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.')
            }),
        format!("{field} ID `{value}` must be a bounded lower-case token"),
    )
}

fn require_text(field: &str, value: &str) -> Result<(), BirthPlanError> {
    require(
        !value.trim().is_empty() && value.len() <= 4000,
        format!("{field} must be nonempty and bounded"),
    )
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), BirthPlanError> {
    if condition {
        Ok(())
    } else {
        Err(BirthPlanError(message.into()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BirthPlanError(pub String);

impl fmt::Display for BirthPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for BirthPlanError {}

#[cfg(test)]
mod tests {
    use tohseno_protocol::digest::sha256;

    #[test]
    fn completion_scenario_ids_are_owned_by_the_experience_contract() {
        let mut plan = crate::anky_fixture::plan(sha256(b"scenario ownership"));
        let mut contract = crate::anky_fixture::contract(&plan);
        plan.completion_contract.required_scenario_ids = vec!["scenario_only".into()];
        plan.validate().unwrap();

        contract.birth_plan_digest = plan.digest().unwrap();
        let error = contract.validate(&plan).unwrap_err().to_string();
        assert!(error.contains("missing experience scenario"), "{error}");
    }

    #[test]
    fn organ_dependencies_must_name_earlier_organs_not_provided_tokens() {
        let mut plan = crate::anky_fixture::plan(sha256(b"organ dependency"));
        let provided_token = plan.embodiment[0].provides[0].clone();
        assert_ne!(provided_token, plan.embodiment[0].organ_id);
        plan.embodiment[1].dependencies = vec![provided_token];

        let error = plan.validate().unwrap_err().to_string();
        assert!(error.contains("undeclared or later organ"), "{error}");
    }
}
