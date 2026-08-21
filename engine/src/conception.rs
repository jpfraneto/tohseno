use crate::apple_capabilities::{AppleCapabilityCatalog, AppleCapabilityProfile};
use crate::birth_plan::{BirthExpressionPlan, BirthOrganPlan, BirthPlan, RequirementOrigin};
use crate::experience::ExperienceContract;
use crate::factory_identity::FactoryIdentity;
use crate::shot_layout::{PreparedIntentPackage, ShotLayout, ShotLayoutError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use tohseno_protocol::canonical;
use tohseno_protocol::digest::Bytes32;
use tohseno_protocol::ontology::ArtifactAvailability;

pub const CONCEPTION_INPUT_SCHEMA: &str = "tohseno.conception-input/1";
pub const CONCEPTION_OUTPUT_SCHEMA: &str = "tohseno.conception-output/1";
pub const CONCEPTION_INPUT_FILE: &str = "conception-input.json";
pub const CONCEPTION_OUTPUT_FILE: &str = "conception-output.json";
pub const ACCEPTED_CONCEPTION_OUTPUT_FILE: &str = "accepted-conception-output.json";
pub const BIRTH_PLAN_FILE: &str = "birth-plan.json";
pub const EXPERIENCE_CONTRACT_FILE: &str = "experience-contract.json";
pub const EXPERIENCE_TRIAL_FILE: &str = "experience-trial.json";
pub const APPLE_CAPABILITY_PROFILE_FILE: &str = "apple-capability-profile.json";
pub const APPLE_CAPABILITY_CATALOG_FILE: &str = "apple-capability-catalog.json";
pub const FACTORY_IDENTITY_FILE: &str = "factory-identity.json";
pub const BIRTH_PLAN_SCHEMA_FILE: &str = "birth-plan.schema.json";
pub const EXPERIENCE_CONTRACT_SCHEMA_FILE: &str = "experience-contract.schema.json";
pub const EXPERIENCE_TRIAL_SCHEMA_FILE: &str = "experience-trial.schema.json";
pub const APPLE_CAPABILITY_PROFILE_SCHEMA_FILE: &str = "apple-capability-profile.schema.json";
pub const GENOME_SCHEMA_FILE: &str = "genome.schema.json";
pub const ONTOLOGY_SCHEMA_FILE: &str = "ontology.schema.json";

const BIRTH_PLAN_SCHEMA_JSON: &str =
    include_str!("../schemas/private-planning/birth-plan.schema.json");
const EXPERIENCE_CONTRACT_SCHEMA_JSON: &str =
    include_str!("../schemas/private-planning/experience-contract.schema.json");
const EXPERIENCE_TRIAL_SCHEMA_JSON: &str =
    include_str!("../schemas/private-planning/experience-trial.schema.json");
const APPLE_CAPABILITY_PROFILE_SCHEMA_JSON: &str =
    include_str!("../schemas/private-planning/apple-capability-profile.schema.json");
const GENOME_SCHEMA_JSON: &str = include_str!("../../protocol/schemas/genome.schema.json");
const ONTOLOGY_SCHEMA_JSON: &str = include_str!("../../protocol/schemas/ontology.schema.json");

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConceptionInput {
    pub schema: String,
    pub app_name: String,
    pub intent_digest: Bytes32,
    pub intention_document_digest: Bytes32,
    pub intention_document_path: String,
    pub references: Vec<ArtifactAvailability>,
    pub apple_capability_profile: AppleCapabilityProfile,
    pub factory_identity: FactoryIdentity,
}

impl ConceptionInput {
    pub fn new(
        app_name: impl Into<String>,
        prepared: &PreparedIntentPackage,
        capability_profile: AppleCapabilityProfile,
    ) -> Result<Self, ConceptionError> {
        let profile_digest = capability_profile.digest()?;
        let input = Self {
            schema: CONCEPTION_INPUT_SCHEMA.into(),
            app_name: app_name.into(),
            intent_digest: prepared.intention_digest,
            intention_document_digest: prepared.document_digest,
            intention_document_path: prepared.document_relative_path.clone(),
            references: prepared
                .references
                .iter()
                .map(|reference| reference.availability.clone())
                .collect(),
            factory_identity: FactoryIdentity::current(None, profile_digest),
            apple_capability_profile: capability_profile,
        };
        input.validate()?;
        Ok(input)
    }

    pub fn validate(&self) -> Result<(), ConceptionError> {
        require(
            self.schema == CONCEPTION_INPUT_SCHEMA,
            "unsupported conception input schema",
        )?;
        require(
            !self.app_name.trim().is_empty(),
            "conception app name is empty",
        )?;
        require(
            self.intent_digest != Bytes32::ZERO && self.intention_document_digest != Bytes32::ZERO,
            "conception intention digests must be nonzero",
        )?;
        require(
            !self.intention_document_path.starts_with('/')
                && !self
                    .intention_document_path
                    .split('/')
                    .any(|part| part == ".."),
            "conception intention path must remain repository-relative",
        )?;
        let catalog = AppleCapabilityCatalog::embedded()?;
        self.apple_capability_profile.validate(&catalog)?;
        self.factory_identity.validate().map_err(ConceptionError)?;
        require(
            self.factory_identity.apple_capability_profile_digest
                == self.apple_capability_profile.digest()?,
            "factory identity names a different Apple capability profile",
        )?;
        require(
            self.factory_identity.accepted_shot_genome_digest.is_none(),
            "conception input cannot claim a Genome was accepted before conception",
        )?;
        for reference in &self.references {
            reference.validate().map_err(|error| {
                ConceptionError(format!("invalid conception reference: {error}"))
            })?;
        }
        Ok(())
    }

    pub fn write(&self, layout: &ShotLayout) -> Result<(), ConceptionError> {
        self.validate()?;
        preserve_json(layout, CONCEPTION_INPUT_FILE, self)?;
        preserve_json(
            layout,
            APPLE_CAPABILITY_PROFILE_FILE,
            &self.apple_capability_profile,
        )?;
        preserve_json(
            layout,
            APPLE_CAPABILITY_CATALOG_FILE,
            &AppleCapabilityCatalog::embedded()?,
        )?;
        preserve_json(layout, FACTORY_IDENTITY_FILE, &self.factory_identity)?;
        for (filename, bytes) in [
            (BIRTH_PLAN_SCHEMA_FILE, BIRTH_PLAN_SCHEMA_JSON.as_bytes()),
            (
                EXPERIENCE_CONTRACT_SCHEMA_FILE,
                EXPERIENCE_CONTRACT_SCHEMA_JSON.as_bytes(),
            ),
            (
                EXPERIENCE_TRIAL_SCHEMA_FILE,
                EXPERIENCE_TRIAL_SCHEMA_JSON.as_bytes(),
            ),
            (
                APPLE_CAPABILITY_PROFILE_SCHEMA_FILE,
                APPLE_CAPABILITY_PROFILE_SCHEMA_JSON.as_bytes(),
            ),
            (GENOME_SCHEMA_FILE, GENOME_SCHEMA_JSON.as_bytes()),
            (ONTOLOGY_SCHEMA_FILE, ONTOLOGY_SCHEMA_JSON.as_bytes()),
        ] {
            layout.preserve_private_planning_file(filename, bytes)?;
        }
        Ok(())
    }

    pub fn read(layout: &ShotLayout) -> Result<Self, ConceptionError> {
        let bytes = layout.read_private_planning_file(CONCEPTION_INPUT_FILE)?;
        let input: Self = serde_json::from_slice(&bytes)?;
        input.validate()?;
        Ok(input)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConceptionOutput {
    pub schema: String,
    pub intent_digest: Bytes32,
    pub apple_capability_profile_digest: Bytes32,
    pub birth_plan: BirthPlan,
    pub experience_contract: ExperienceContract,
    pub rationale: String,
}

impl ConceptionOutput {
    pub fn validate(
        &self,
        input: &ConceptionInput,
    ) -> Result<BirthExpressionPlan, ConceptionError> {
        require(
            self.schema == CONCEPTION_OUTPUT_SCHEMA,
            "unsupported conception output schema",
        )?;
        require(
            self.intent_digest == input.intent_digest
                && self.birth_plan.intent_digest == input.intent_digest,
            "conception output is not bound to the exact preserved intention",
        )?;
        require(
            self.apple_capability_profile_digest == input.apple_capability_profile.digest()?,
            "conception output used a different Apple capability profile",
        )?;
        require(
            self.birth_plan.product_name == input.app_name,
            "Birth Plan product name does not match the Shot",
        )?;
        require(
            !self.rationale.trim().is_empty() && self.rationale.len() <= 4000,
            "conception rationale must be concise and nonempty",
        )?;
        self.birth_plan
            .validate()
            .map_err(|error| ConceptionError(error.to_string()))?;
        let catalog = AppleCapabilityCatalog::embedded()?;
        let identifiers = self
            .birth_plan
            .capabilities
            .iter()
            .map(|capability| capability.identifier.as_str());
        input
            .apple_capability_profile
            .validate_required_capabilities(identifiers)?;
        self.experience_contract.validate(&self.birth_plan)?;
        let declared_physical = self
            .birth_plan
            .completion_contract
            .physical_verification_capabilities
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let required_physical = self
            .birth_plan
            .capabilities
            .iter()
            .filter(|planned| planned.primary)
            .filter(|planned| {
                catalog
                    .get(&planned.identifier)
                    .is_some_and(|definition| definition.physical_device_verification)
            })
            .map(|planned| planned.identifier.as_str())
            .collect::<BTreeSet<_>>();
        require(
            required_physical.is_subset(&declared_physical),
            format!(
                "completion contract omits physical verification for primary hardware capabilities: {}",
                required_physical
                    .difference(&declared_physical)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )?;
        for scenario in &self.experience_contract.scenarios {
            let exercises_physical = scenario
                .capability_ids
                .iter()
                .any(|identifier| declared_physical.contains(identifier.as_str()));
            require(
                !exercises_physical || scenario.physical_device_required,
                format!(
                    "experience scenario `{}` exercises a required physical capability but is not marked for a physical-device trial",
                    scenario.id
                ),
            )?;
        }
        for capability in &required_physical {
            require(
                self.experience_contract.scenarios.iter().any(|scenario| {
                    scenario.physical_device_required
                        && scenario
                            .capability_ids
                            .iter()
                            .any(|identifier| identifier == capability)
                }),
                format!("physical capability `{capability}` has no physical target-user scenario"),
            )?;
        }
        BirthExpressionPlan::from_birth_plan(&self.birth_plan)
            .map_err(|error| ConceptionError(error.to_string()))
    }

    pub fn read_and_validate(
        layout: &ShotLayout,
        input: &ConceptionInput,
    ) -> Result<(Self, BirthExpressionPlan), ConceptionError> {
        let bytes = layout.read_private_planning_file(CONCEPTION_OUTPUT_FILE)?;
        let output: Self = serde_json::from_slice(&bytes)?;
        let expression = output.validate(input)?;
        Ok((output, expression))
    }

    /// Proves ledger origins against the actual preserved bytes. Structural
    /// validation alone cannot tell whether an intelligence invented an
    /// "exact" excerpt or named a reference that was never supplied.
    pub fn validate_source_traceability(
        &self,
        input: &ConceptionInput,
        intention_document: &[u8],
    ) -> Result<(), ConceptionError> {
        require(
            tohseno_protocol::digest::sha256(intention_document) == input.intention_document_digest,
            "the preserved conception document digest changed",
        )?;
        let intention = std::str::from_utf8(intention_document)
            .map_err(|_| ConceptionError("the preserved intention is not UTF-8".into()))?;
        let reference_names = input
            .references
            .iter()
            .filter_map(|reference| reference.artifact.name.as_deref())
            .collect::<BTreeSet<_>>();
        for requirement in &self.birth_plan.requirements {
            match requirement.origin {
                RequirementOrigin::ExplicitIntention => {
                    let excerpt = requirement.source_excerpt.as_deref().ok_or_else(|| {
                        ConceptionError(format!(
                            "explicit requirement `{}` needs a short exact source excerpt",
                            requirement.id
                        ))
                    })?;
                    require(
                        intention.contains(excerpt),
                        format!(
                            "explicit requirement `{}` cites text absent from the exact intention",
                            requirement.id
                        ),
                    )?;
                }
                RequirementOrigin::ReferenceImage => {
                    let location = requirement.source_location.as_deref().ok_or_else(|| {
                        ConceptionError(format!(
                            "reference-image requirement `{}` needs a source location",
                            requirement.id
                        ))
                    })?;
                    require(
                        reference_names
                            .iter()
                            .any(|name| location == *name || location.ends_with(&format!("/{name}"))),
                        format!(
                            "reference-image requirement `{}` names a reference that was not supplied",
                            requirement.id
                        ),
                    )?;
                }
                RequirementOrigin::InferredToCompleteIntent
                | RequirementOrigin::ApplePlatformRequired
                | RequirementOrigin::ProtocolRequired => {}
            }
        }
        Ok(())
    }

    pub fn preserve_accepted_artifacts(&self, layout: &ShotLayout) -> Result<(), ConceptionError> {
        preserve_json(layout, ACCEPTED_CONCEPTION_OUTPUT_FILE, self)?;
        preserve_json(layout, BIRTH_PLAN_FILE, &self.birth_plan)?;
        preserve_json(layout, EXPERIENCE_CONTRACT_FILE, &self.experience_contract)?;
        Ok(())
    }
}

/// The one behavioral invariant every synthesized Genome carries. App-specific
/// organs cite it byte-for-byte, so it is a constant rather than a literal
/// repeated at each use site.
const INTENTION_IS_AUTHORITATIVE: &str =
    "The exact preserved intention is authoritative over every later interpretation of this app.";

const REQUIREMENT_ID: &str = "req_preserved_intention";
const JOURNEY_ID: &str = "journey_preserved_intention";
const SCENARIO_ID: &str = "scenario_preserved_intention";
const APP_ORGAN_ID: &str = "app_experience";

/// Derive the Shot's initial Genome, Birth Plan, and Experience Contract from
/// the preserved intention without asking an intelligence for them.
///
/// The protocol binds a `genome_digest` into every VersionRecord, so a Shot
/// still needs an accepted Genome before it can hold a Version. What it does
/// not need is a separate planning conversation: the engine composes the
/// substrate deterministically and hands the exact human intention straight to
/// the coding harness. Interpreting that intention is the harness's job, and
/// the acceptance gates that follow — Release build, install, launch, and the
/// declared trial — are where interpretation is actually checked.
pub fn synthesize(input: &ConceptionInput) -> Result<ConceptionOutput, ConceptionError> {
    input.validate()?;
    let app = input.app_name.as_str();

    let genome = tohseno_protocol::ontology::Genome {
        schema: tohseno_protocol::ontology::GENOME_SCHEMA.into(),
        revision: 1,
        purpose: format!(
            "Deliver the exact preserved intention for `{app}` as a native iPhone app."
        ),
        intended_for: vec!["The person who wrote this app's preserved intention.".into()],
        essential_experience: vec![
            "Opening the app on iPhone does what its preserved intention describes.".into(),
        ],
        behavioral_invariants: vec![INTENTION_IS_AUTHORITATIVE.into()],
        interaction_laws: Vec::new(),
        aesthetic_principles: Vec::new(),
        privacy_principles: vec![
            "App data stays on the owner's device unless the preserved intention says otherwise."
                .into(),
        ],
        ownership_principles: vec![
            "The owner owns this app, its source, and its signed lineage.".into(),
        ],
        platform_commitments: vec!["iphone".into()],
        boundaries: Vec::new(),
        non_goals: Vec::new(),
        required_capabilities: Vec::new(),
        forbidden_transformations: vec![
            "Replacing a promised experience with a placeholder, mock, or stub.".into(),
        ],
        acceptance_principles: vec![
            "The app builds in Release, installs on the owner's iPhone, and launches.".into(),
        ],
        freely_changeable: vec!["Anything the preserved intention does not fix.".into()],
    };

    let mut embodiment = crate::birth_plan::protocol_substrate_organs();
    embodiment.push(BirthOrganPlan {
        organ_id: APP_ORGAN_ID.into(),
        kind: crate::birth_plan::OrganKind::AppSpecific,
        provides: vec!["preserved_intention_experience".into()],
        owns_state: Vec::new(),
        permissions: Vec::new(),
        dependencies: Vec::new(),
        emits: Vec::new(),
        consumes: Vec::new(),
        genome_invariants: vec![INTENTION_IS_AUTHORITATIVE.into()],
        requirement_ids: vec![REQUIREMENT_ID.into()],
        capability_ids: Vec::new(),
        journey_ids: vec![JOURNEY_ID.into()],
        acceptance_criteria: vec![crate::birth_plan::OrganAcceptanceCriterion {
            id: "fulfils_preserved_intention".into(),
            assertion: format!(
                "`{app}` does what `{}` describes, with no unimplemented promise.",
                input.intention_document_path
            ),
            deterministic: false,
        }],
        platforms: vec!["iphone".into()],
    });

    let birth_plan = BirthPlan {
        schema: crate::birth_plan::BIRTH_PLAN_SCHEMA.into(),
        intent_digest: input.intent_digest,
        product_name: app.to_owned(),
        promise: format!("`{app}` does what its preserved intention describes, on iPhone."),
        target_users: vec![crate::birth_plan::TargetUser {
            id: "owner".into(),
            role: "The person who wrote this app's preserved intention.".into(),
            ability_or_age_context: None,
            environment: vec!["iphone".into()],
            goals: vec!["Use the app that the preserved intention describes.".into()],
            constraints: Vec::new(),
            understands_without_explanation: Vec::new(),
        }],
        contexts: vec!["iphone".into()],
        requirements: vec![crate::birth_plan::BirthRequirement {
            id: REQUIREMENT_ID.into(),
            statement: format!(
                "The app fulfils the exact preserved intention at `{}`.",
                input.intention_document_path
            ),
            level: crate::birth_plan::RequirementLevel::Must,
            // The engine is not claiming to have read the intention. It states
            // the one requirement that is true of every Shot and leaves the
            // reading to the harness, so nothing here can cite text the human
            // never wrote.
            origin: RequirementOrigin::InferredToCompleteIntent,
            source_excerpt: None,
            source_location: None,
        }],
        capabilities: Vec::new(),
        journeys: vec![crate::birth_plan::ProductJourney {
            id: JOURNEY_ID.into(),
            target_actor: "owner".into(),
            promise: format!("The owner opens `{app}` and it does what they asked for."),
            requirement_ids: vec![REQUIREMENT_ID.into()],
        }],
        embodiment,
        completion_contract: crate::birth_plan::CompletionContract {
            must_requirement_ids: vec![REQUIREMENT_ID.into()],
            required_scenario_ids: vec![SCENARIO_ID.into()],
            physical_verification_capabilities: Vec::new(),
            release_build_required: true,
            zero_product_gaps_required: true,
        },
        explicit_non_goals: Vec::new(),
        forbidden_substitutions: Vec::new(),
        genome,
    };

    let experience_contract = ExperienceContract {
        schema: crate::experience::EXPERIENCE_CONTRACT_SCHEMA.into(),
        intent_digest: input.intent_digest,
        birth_plan_digest: birth_plan
            .digest()
            .map_err(|error| ConceptionError(error.to_string()))?,
        scenarios: vec![crate::experience::ExperienceScenario {
            id: SCENARIO_ID.into(),
            target_actor: "owner".into(),
            initial_state: format!(
                "`{app}` is installed on the owner's iPhone and has not been opened."
            ),
            environment: vec!["iphone".into()],
            steps_or_gestures: vec![
                "Open the app.".into(),
                "Use it the way its preserved intention describes.".into(),
            ],
            expected_states: vec![
                "Every experience the preserved intention promises is present and works.".into(),
            ],
            requirement_ids: vec![REQUIREMENT_ID.into()],
            capability_ids: Vec::new(),
            completion_condition:
                "The app delivers the preserved intention with no unimplemented promise.".into(),
            evidence_required: vec![crate::experience::EvidenceKind::XcuiTest],
            physical_device_required: false,
        }],
    };

    let output = ConceptionOutput {
        schema: CONCEPTION_OUTPUT_SCHEMA.into(),
        intent_digest: input.intent_digest,
        apple_capability_profile_digest: input.apple_capability_profile.digest()?,
        birth_plan,
        experience_contract,
        rationale: format!(
            "The engine composed `{app}`'s initial Genome and Expression deterministically from the preserved intention. No planning conversation was run: the intention itself is the plan, and the Release build, install, launch, and declared trial are the gates."
        ),
    };
    output.validate(input)?;
    Ok(output)
}

fn preserve_json(
    layout: &ShotLayout,
    filename: &str,
    value: &impl Serialize,
) -> Result<Bytes32, ConceptionError> {
    let mut bytes = canonical::to_vec(value)
        .map_err(|error| ConceptionError(format!("planning artifact encoding failed: {error}")))?;
    bytes.push(b'\n');
    layout
        .preserve_private_planning_file(filename, &bytes)
        .map_err(ConceptionError::from)
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), ConceptionError> {
    if condition {
        Ok(())
    } else {
        Err(ConceptionError(message.into()))
    }
}

#[derive(Debug)]
pub struct ConceptionError(pub String);

impl fmt::Display for ConceptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConceptionError {}

impl From<std::io::Error> for ConceptionError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<serde_json::Error> for ConceptionError {
    fn from(value: serde_json::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<ShotLayoutError> for ConceptionError {
    fn from(value: ShotLayoutError) -> Self {
        Self(value.to_string())
    }
}

impl From<crate::apple_capabilities::CapabilityProfileError> for ConceptionError {
    fn from(value: crate::apple_capabilities::CapabilityProfileError) -> Self {
        Self(value.to_string())
    }
}

impl From<crate::experience::ExperienceError> for ConceptionError {
    fn from(value: crate::experience::ExperienceError) -> Self {
        Self(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTENTION: &[u8] = b"# Weather thing\n\nShow me the sky right now.\n";

    fn input(app_name: &str) -> ConceptionInput {
        let catalog = AppleCapabilityCatalog::embedded().unwrap();
        let profile = AppleCapabilityProfile {
            schema: crate::apple_capabilities::APPLE_CAPABILITY_PROFILE_SCHEMA.into(),
            catalog_digest: catalog.digest().unwrap(),
            xcode_version: "26.0".into(),
            xcode_build: "17A1".into(),
            iphoneos_sdk_version: "26.0".into(),
            simulator_runtimes: Vec::new(),
            connected_devices: Vec::new(),
            last_known_devices: Vec::new(),
            signing_team: None,
            resolutions: catalog
                .capabilities
                .iter()
                .map(
                    |capability| crate::apple_capabilities::CapabilityResolution {
                        identifier: capability.identifier.clone(),
                        state:
                            crate::apple_capabilities::CapabilityState::UnknownUntilPhysicalDevice,
                        simulator_state:
                            crate::apple_capabilities::CapabilityState::SimulatorUnavailable,
                        device_states: Vec::new(),
                        physical_device_verification: capability.physical_device_verification,
                    },
                )
                .collect(),
            observed_at_unix: 1,
        };
        let prepared = PreparedIntentPackage {
            intention_digest: Bytes32::new([0x11; 32]),
            document_digest: tohseno_protocol::digest::sha256(INTENTION),
            document_relative_path: ".tohseno/INTENTION.md".into(),
            references: Vec::new(),
        };
        ConceptionInput::new(app_name, &prepared, profile).unwrap()
    }

    /// The whole point of synthesis: the substrate a Shot needs before it can
    /// hold a Version is derivable, so no intelligence has to produce it.
    #[test]
    fn synthesis_passes_every_gate_the_engine_used_to_demand_of_an_intelligence() {
        let input = input("example");
        let output = synthesize(&input).unwrap();
        let expression = output.validate(&input).unwrap();
        output
            .validate_source_traceability(&input, INTENTION)
            .unwrap();
        expression.validate(&output.birth_plan.genome).unwrap();
        assert_eq!(output.birth_plan.genome.revision, 1);
    }

    /// Source traceability used to be the expensive rule: an intelligence could
    /// invent an "exact" excerpt and the engine had to catch it. A synthesized
    /// plan quotes nothing, so there is no citation that could be fabricated —
    /// it holds even though the plan and the intention share no wording.
    #[test]
    fn synthesis_never_cites_text_the_human_did_not_write() {
        let input = input("example");
        let output = synthesize(&input).unwrap();
        assert!(output
            .birth_plan
            .requirements
            .iter()
            .all(|requirement| requirement.source_excerpt.is_none()
                && requirement.source_location.is_none()));
        output
            .validate_source_traceability(&input, INTENTION)
            .unwrap();
    }

    #[test]
    fn synthesis_is_deterministic_for_one_intention() {
        let input = input("example");
        assert_eq!(
            synthesize(&input).unwrap().birth_plan.digest().unwrap(),
            synthesize(&input).unwrap().birth_plan.digest().unwrap()
        );
    }

    #[test]
    fn synthesis_binds_the_shot_name_and_intention() {
        let output = synthesize(&input("weather-thing")).unwrap();
        assert_eq!(output.birth_plan.product_name, "weather-thing");
        assert_eq!(output.intent_digest, Bytes32::new([0x11; 32]));
        assert!(output.birth_plan.requirements[0]
            .statement
            .contains(".tohseno/INTENTION.md"));
    }

    /// The engine still refuses a plan that drops physical verification. Only
    /// the author changed; the acceptance rules did not.
    #[test]
    fn primary_hardware_capabilities_cannot_drop_physical_verification() {
        let input = crate::anky_fixture::conception_input();
        let mut output = crate::anky_fixture::output();
        output
            .birth_plan
            .completion_contract
            .physical_verification_capabilities
            .clear();
        output.experience_contract.birth_plan_digest = output.birth_plan.digest().unwrap();
        let error = output.validate(&input).unwrap_err().to_string();
        assert!(error.contains("omits physical verification"), "{error}");
        assert!(error.contains("camera_capture"), "{error}");
    }

    #[test]
    fn explicit_requirement_excerpt_must_exist_in_preserved_intention() {
        let input = crate::anky_fixture::conception_input();
        let mut output = crate::anky_fixture::output();
        output.birth_plan.requirements[0].source_excerpt =
            Some("words the human never supplied".into());
        output.experience_contract.birth_plan_digest = output.birth_plan.digest().unwrap();
        let error = output
            .validate_source_traceability(&input, crate::anky_fixture::INTENTION.as_bytes())
            .unwrap_err()
            .to_string();
        assert!(error.contains("absent from the exact intention"), "{error}");
    }
}
