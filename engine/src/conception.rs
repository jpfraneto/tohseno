use crate::apple_capabilities::{AppleCapabilityCatalog, AppleCapabilityProfile};
use crate::birth_plan::{BirthExpressionPlan, BirthPlan, RequirementOrigin};
use crate::experience::ExperienceContract;
use crate::factory_identity::FactoryIdentity;
use crate::shot_layout::{PreparedIntentPackage, ShotLayout, ShotLayoutError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
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

pub trait ConceptionHarness {
    fn conceive(&self, input: &ConceptionInput) -> Result<ConceptionOutput, ConceptionError>;
}

/// Deterministic test harness. It proves the orchestration boundary without
/// invoking or pretending to be a live intelligence.
#[derive(Clone)]
pub struct FakeConceptionHarness {
    output: ConceptionOutput,
}

impl FakeConceptionHarness {
    pub fn returning(output: ConceptionOutput) -> Self {
        Self { output }
    }
}

impl ConceptionHarness for FakeConceptionHarness {
    fn conceive(&self, input: &ConceptionInput) -> Result<ConceptionOutput, ConceptionError> {
        self.output.validate(input)?;
        Ok(self.output.clone())
    }
}

pub struct JsonFileConceptionHarness {
    path: PathBuf,
}

impl JsonFileConceptionHarness {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl ConceptionHarness for JsonFileConceptionHarness {
    fn conceive(&self, input: &ConceptionInput) -> Result<ConceptionOutput, ConceptionError> {
        let metadata = fs::symlink_metadata(&self.path)?;
        require(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "conception output must be a regular file",
        )?;
        require(
            metadata.len() <= 4 * 1024 * 1024,
            "conception output exceeds 4 MiB",
        )?;
        let output: ConceptionOutput = serde_json::from_slice(&fs::read(&self.path)?)?;
        output.validate(input)?;
        Ok(output)
    }
}

pub fn render_conception_task(input: &ConceptionInput) -> Result<String, ConceptionError> {
    input.validate()?;
    let identity = &input.factory_identity;
    Ok(format!(
        r#"# TOHSENO Conception

This phase interprets the exact human intention before any app-specific Genome is accepted. Do not materialize an app in this phase.

Read `{intent_path}` and inspect every supplied reference. The exact intention is authoritative; your interpretation supplements it and never replaces it.

Resolve unspecified implementation details in service of the product promise, target users, explicit requirements, app-specific Genome, and actual Apple constraints. A consequential reduction of the product promise is a deviation, not an implementation decision.

The Apple capability profile is available material and verification context, not a denylist. `simulator_unavailable` and `unknown_until_physical_device` never mean a capability is forbidden in the Release product. A runtime fallback is valid only under its declared runtime condition and cannot become the primary product for factory convenience.

Produce exactly one strict JSON object at `.tohseno/private/planning/{output_file}` using this closed wrapper (replace the values and objects; do not add fields):

```json
{{
  "schema": "{output_schema}",
  "intent_digest": "<exact intent_digest from {input_file}>",
  "apple_capability_profile_digest": "<exact profile digest from {identity_file}>",
  "birth_plan": {{"schema": "{birth_schema}"}},
  "experience_contract": {{"schema": "{experience_schema}"}},
  "rationale": "<concise rationale for meaningful ambiguity resolution>"
}}
```

The complete nested field definitions are in `{birth_schema_file}`, `{experience_schema_file}`, `{profile_schema_file}`, `{genome_schema_file}`, and `{ontology_schema_file}` in the same planning directory. Derive specific target actors, stable requirement IDs with origins, app-specific organs, forbidden substitutions, and target-user scenarios. Protocol substrate cannot count as product fulfillment.

Before writing output, read:

- `.tohseno/private/planning/{input_file}`
- `.tohseno/private/planning/{profile_file}`
- `.tohseno/private/planning/{catalog_file}`
- `.tohseno/private/planning/{identity_file}`
- `genome/LAWS.md`

Factory identity governing this conception:

- TOHSENO engine version: `{version}`
- TOHSENO source commit: `{commit}`
- static Constitution/Genome bundle digest: `{constitution}`
- accepted Shot Genome digest: not accepted yet
- Apple capability profile digest: `{profile}`

The output is a proposal. The engine validates it deterministically and owns acceptance.

## Compiled Factory Constitution

{constitution_text}
"#,
        intent_path = input.intention_document_path,
        output_file = CONCEPTION_OUTPUT_FILE,
        output_schema = CONCEPTION_OUTPUT_SCHEMA,
        birth_schema = crate::birth_plan::BIRTH_PLAN_SCHEMA,
        experience_schema = crate::experience::EXPERIENCE_CONTRACT_SCHEMA,
        input_file = CONCEPTION_INPUT_FILE,
        profile_file = APPLE_CAPABILITY_PROFILE_FILE,
        identity_file = FACTORY_IDENTITY_FILE,
        catalog_file = APPLE_CAPABILITY_CATALOG_FILE,
        birth_schema_file = BIRTH_PLAN_SCHEMA_FILE,
        experience_schema_file = EXPERIENCE_CONTRACT_SCHEMA_FILE,
        profile_schema_file = APPLE_CAPABILITY_PROFILE_SCHEMA_FILE,
        genome_schema_file = GENOME_SCHEMA_FILE,
        ontology_schema_file = ONTOLOGY_SCHEMA_FILE,
        version = identity.engine_version,
        commit = identity.source_commit,
        constitution = identity.static_constitution_digest,
        profile = identity.apple_capability_profile_digest,
        constitution_text = crate::genome::Genome::constitution_text().trim(),
    ))
}

pub fn write_conception_task(
    root: &Path,
    input: &ConceptionInput,
) -> Result<PathBuf, ConceptionError> {
    let task = render_conception_task(input)?;
    let path = root.join(".tohseno/CONCEPTION.md");
    let parent = path
        .parent()
        .ok_or_else(|| ConceptionError("conception task has no parent".into()))?;
    fs::create_dir_all(parent)?;
    fs::write(&path, task)?;
    Ok(path)
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

    #[test]
    fn conception_task_exposes_current_compiled_policy_and_matching_digest() {
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
            document_digest: Bytes32::new([0x12; 32]),
            document_relative_path: ".tohseno/EVOLUTION_INTENT.md".into(),
            references: Vec::new(),
        };
        let input = ConceptionInput::new("example", &prepared, profile).unwrap();
        let task = render_conception_task(&input).unwrap();
        assert!(task.contains(crate::genome::Genome::constitution_text().trim()));
        assert!(task.contains(&crate::genome::Genome::bundle_digest().to_string()));
        assert!(task.contains(env!("TOHSENO_SOURCE_COMMIT")));
        assert!(task.contains("simulator_unavailable"));
        assert!(task.contains("not a denylist"));
        assert!(task.contains(BIRTH_PLAN_SCHEMA_FILE));
        assert!(task.contains(EXPERIENCE_CONTRACT_SCHEMA_FILE));
        assert!(task.contains(APPLE_CAPABILITY_CATALOG_FILE));

        let directory = tempfile::tempdir().unwrap();
        let layout = ShotLayout::at(directory.path());
        input.write(&layout).unwrap();
        for filename in [
            BIRTH_PLAN_SCHEMA_FILE,
            EXPERIENCE_CONTRACT_SCHEMA_FILE,
            EXPERIENCE_TRIAL_SCHEMA_FILE,
            APPLE_CAPABILITY_PROFILE_SCHEMA_FILE,
            GENOME_SCHEMA_FILE,
            ONTOLOGY_SCHEMA_FILE,
            APPLE_CAPABILITY_CATALOG_FILE,
        ] {
            let bytes = layout.read_private_planning_file(filename).unwrap();
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .unwrap_or_else(|error| panic!("{filename} is not JSON: {error}"));
        }
    }

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
