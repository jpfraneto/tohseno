use crate::apple_capabilities::{
    AppleCapabilityCatalog, AppleCapabilityProfile, CapabilityResolution, CapabilityState,
    APPLE_CAPABILITY_PROFILE_SCHEMA,
};
use crate::birth_plan::{
    protocol_substrate_organs, BirthOrganPlan, BirthPlan, BirthRequirement, CompletionContract,
    ForbiddenSubstitution, OrganAcceptanceCriterion, OrganKind, PlannedCapability, ProductJourney,
    RequirementLevel, RequirementOrigin, TargetUser, BIRTH_PLAN_SCHEMA,
};
use crate::conception::{ConceptionInput, ConceptionOutput, CONCEPTION_OUTPUT_SCHEMA};
use crate::experience::{
    BirthEvaluationEvidence, CriterionResult, EvidenceKind, EvidenceReference, ExperienceContract,
    ExperienceScenario, ExperienceTrial, IncompletenessCategory, OrganTrialResult,
    PhysicalDeviceEvidence, ScenarioTrialResult, TypedIncompleteness, EXPERIENCE_CONTRACT_SCHEMA,
    EXPERIENCE_TRIAL_SCHEMA,
};
use crate::factory_identity::FactoryIdentity;
use crate::shot_layout::PreparedIntentPackage;
use tohseno_protocol::digest::{sha256, Bytes32};
use tohseno_protocol::ontology::{ArtifactDescriptor, Genome, GENOME_SCHEMA};

pub(crate) const INTENTION: &str = include_str!("../fixtures/anky-intention.md");
pub(crate) const PROMISE: &str = "There is a small creature hiding in the child’s real home. Find it, meet it, and begin teaching it language.";

pub(crate) fn profile() -> AppleCapabilityProfile {
    let catalog = AppleCapabilityCatalog::embedded().expect("test catalog");
    AppleCapabilityProfile {
        schema: APPLE_CAPABILITY_PROFILE_SCHEMA.into(),
        catalog_digest: catalog.digest().expect("catalog digest"),
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
            .map(|capability| CapabilityResolution {
                identifier: capability.identifier.clone(),
                state: if capability.physical_device_verification {
                    CapabilityState::HardwareSpecific
                } else {
                    CapabilityState::Supported
                },
                simulator_state: match capability.simulator_support {
                    crate::apple_capabilities::SimulatorSupport::Supported => {
                        CapabilityState::Supported
                    }
                    crate::apple_capabilities::SimulatorSupport::Partial => {
                        CapabilityState::HardwareSpecific
                    }
                    crate::apple_capabilities::SimulatorSupport::FixtureOnly
                    | crate::apple_capabilities::SimulatorSupport::Unavailable => {
                        CapabilityState::SimulatorUnavailable
                    }
                },
                device_states: Vec::new(),
                physical_device_verification: capability.physical_device_verification,
            })
            .collect(),
        observed_at_unix: 1,
    }
}

pub(crate) fn conception_input() -> ConceptionInput {
    let prepared = PreparedIntentPackage {
        intention_digest: sha256(INTENTION.as_bytes()),
        document_digest: sha256(INTENTION.as_bytes()),
        document_relative_path: ".tohseno/EVOLUTION_INTENT.md".into(),
        references: Vec::new(),
    };
    ConceptionInput::new("Anky", &prepared, profile()).expect("valid Anky conception input")
}

pub(crate) fn output() -> ConceptionOutput {
    let input = conception_input();
    let birth_plan = plan(input.intent_digest);
    let experience_contract = contract(&birth_plan);
    let output = ConceptionOutput {
        schema: CONCEPTION_OUTPUT_SCHEMA.into(),
        intent_digest: input.intent_digest,
        apple_capability_profile_digest: input
            .apple_capability_profile
            .digest()
            .expect("profile digest"),
        birth_plan,
        experience_contract,
        rationale: "Resolve the two-actor, camera-first spatial relationship exactly as requested; preserve Tier C only as a runtime failure fallback.".into(),
    };
    output
        .validate(&input)
        .expect("valid Anky conception output");
    output
}

pub(crate) fn plan(intent_digest: Bytes32) -> BirthPlan {
    let requirements = vec![
        requirement("parent_setup", "A parent owns consent, permissions, privacy, and diagnostics.", "a parent who handles setup, consent, permissions, privacy, and diagnostics"),
        requirement("camera_window", "The child experience keeps the rear camera open as a window into the real home.", "The rear camera is open during the child experience."),
        requirement("spatial_creature", "ARKit and RealityKit place a creature at a plausible spatial hiding location.", "Use ARKit and RealityKit to place a small creature at a plausible spatial hiding location."),
        requirement("warm_cold_search", "Movement, audio, haptics, particles, light, and visual traces communicate warmer and colder.", "communicate warmer and colder as the child physically searches"),
        requirement("voice_mimicry", "After discovery Anky listens and performs one constrained imperfect mimicry act.", "listens through the microphone, reacts to the child’s voice, attempts one constrained imperfect act of mimicry"),
        requirement("relationship_memory", "One sparse local relationship memory changes a later encounter.", "stores one sparse relationship memory locally, and reflects that memory in a later encounter"),
        requirement("tiered_spatial", "LiDAR strengthens reconstruction and occlusion; standard phones retain world tracking; Tier C activates only after genuine spatial failure.", "A motion-relative Tier C experience exists only as a runtime fallback when spatial tracking genuinely fails"),
        requirement("finite_safety_privacy", "The experience is finite, movement-safe, and private by default.", "The experience is finite, safe for child movement, private by default"),
        requirement("second_encounter", "The complete journey reaches a second memory-bearing encounter.", "complete from parent setup through the second memory-bearing encounter"),
    ];
    let capabilities = vec![
        capability("camera_capture", "Keep the real rear-camera environment visible.", &["camera_window"], true, None),
        capability("ar_world_tracking", "Track the child’s real home and anchor the hiding place.", &["spatial_creature", "tiered_spatial"], true, Some("Motion-relative Tier C may activate only after world tracking genuinely fails at runtime.")),
        capability("realitykit_rendering", "Embody the creature and traces in the real environment.", &["spatial_creature"], true, None),
        capability("scene_reconstruction", "Improve occlusion on compatible LiDAR phones.", &["tiered_spatial"], false, Some("Standard ARKit plane and world tracking on non-LiDAR hardware.")),
        capability("lidar", "Detect the compatible high-spatial-fidelity device tier.", &["tiered_spatial"], false, Some("Standard ARKit plane and world tracking on non-LiDAR hardware.")),
        capability("motion_orientation", "Make search cues react to physical movement.", &["warm_cold_search", "tiered_spatial"], true, None),
        capability("haptics", "Give embodied warmer and colder feedback.", &["warm_cold_search"], true, None),
        capability("spatial_audio", "Give reactive spatial search and creature cues.", &["warm_cold_search"], true, None),
        capability("microphone_input", "Hear the child only during the bounded post-discovery moment.", &["voice_mimicry"], true, None),
        capability("speech_recognition", "Perform constrained phonetic analysis for one imperfect mimicry act.", &["voice_mimicry"], true, Some("On-device constrained phonetic analysis that preserves the same mimicry promise.")),
        capability("local_persistence", "Persist one sparse relationship memory on device with SwiftData.", &["relationship_memory", "second_encounter"], true, None),
        capability("notifications", "Optionally let the parent request a local return reminder.", &["parent_setup"], false, Some("No reminder when the parent declines it.")),
    ];
    let journeys = vec![
        journey(
            "parent_setup",
            "parent",
            &["parent_setup", "finite_safety_privacy"],
        ),
        journey(
            "permission_grant_and_transition",
            "parent",
            &["parent_setup", "camera_window", "voice_mimicry"],
        ),
        journey(
            "permission_denial_and_recovery",
            "parent",
            &["parent_setup", "camera_window", "voice_mimicry"],
        ),
        journey("camera_first_transition", "young_child", &["camera_window"]),
        journey(
            "environment_initialization",
            "young_child",
            &["spatial_creature", "tiered_spatial"],
        ),
        journey(
            "spatial_hiding_place_selection",
            "young_child",
            &["spatial_creature"],
        ),
        journey(
            "warm_cold_search",
            "young_child",
            &["warm_cold_search", "finite_safety_privacy"],
        ),
        journey(
            "glimpses",
            "young_child",
            &["spatial_creature", "warm_cold_search"],
        ),
        journey("discovery", "young_child", &["spatial_creature"]),
        journey(
            "voice_listening",
            "young_child",
            &["voice_mimicry", "finite_safety_privacy"],
        ),
        journey("mimicry", "young_child", &["voice_mimicry"]),
        journey("session_ending", "young_child", &["finite_safety_privacy"]),
        journey(
            "interruption_and_resume",
            "young_child",
            &["camera_window", "spatial_creature", "finite_safety_privacy"],
        ),
        journey(
            "tracking_failure_runtime_fallback",
            "young_child",
            &["tiered_spatial", "finite_safety_privacy"],
        ),
        journey("memory_persistence", "parent", &["relationship_memory"]),
        journey(
            "second_encounter_recognition",
            "young_child",
            &["relationship_memory", "second_encounter"],
        ),
    ];
    let invariants = vec![
        "The rear camera and spatially anchored creature remain the primary child experience.".to_owned(),
        "The parent controls permission and privacy context before the child experience.".to_owned(),
        "Anky begins without language and performs one bounded imperfect mimicry act after discovery.".to_owned(),
        "One sparse local relationship memory visibly changes the second encounter.".to_owned(),
        "Tier C activates only after genuine runtime spatial-tracking failure.".to_owned(),
        "The experience remains finite, movement-safe, and private by default.".to_owned(),
    ];
    let genome = Genome {
        schema: GENOME_SCHEMA.into(),
        revision: 1,
        purpose: PROMISE.into(),
        intended_for: vec!["A parent who owns setup and privacy context".into(), "A young child searching safely in their real home".into()],
        essential_experience: vec!["Parent setup leads into a camera-first spatial search, discovery, mimicry, and a later memory-bearing encounter.".into()],
        behavioral_invariants: invariants.clone(),
        interaction_laws: vec!["Every primary control acts, explains a recoverable denial, or is absent.".into()],
        aesthetic_principles: vec!["Anky and its traces feel magical while the real home remains legible.".into()],
        privacy_principles: vec!["Camera and microphone data stay bounded to their declared live purposes; sparse memory remains local.".into()],
        ownership_principles: vec!["The parent controls permissions and the family owns local relationship state.".into()],
        platform_commitments: vec!["Release uses real ARKit, RealityKit, Core Motion, Core Haptics, audio input, and SwiftData paths.".into()],
        boundaries: vec!["The experience does not turn child context into analytics, tracking, or published identity.".into()],
        non_goals: vec!["Unbounded conversation and social accounts are outside this intention.".into()],
        required_capabilities: capabilities.iter().map(|item| item.identifier.clone()).collect(),
        forbidden_transformations: vec!["Do not replace the camera environment, spatial hiding, voice, or relationship memory with screen-space imitations.".into()],
        acceptance_principles: vec!["Accept only after every must requirement and target-user scenario has independent evidence, including physical evidence for sensor-critical paths.".into()],
        freely_changeable: vec!["Nonessential copy rhythm and decorative particle parameters may change while the invariants remain true.".into()],
    };
    let mut embodiment = protocol_substrate_organs();
    embodiment.extend([
        organ(
            "parent_privacy_gate",
            &["parent_setup", "finite_safety_privacy"],
            &["notifications"],
            &[
                "parent_setup",
                "permission_grant_and_transition",
                "permission_denial_and_recovery",
            ],
            &invariants[1],
            &[],
        ),
        organ(
            "spatial_environment_sensing",
            &["camera_window", "spatial_creature", "tiered_spatial"],
            &[
                "camera_capture",
                "ar_world_tracking",
                "scene_reconstruction",
                "lidar",
                "motion_orientation",
            ],
            &[
                "camera_first_transition",
                "environment_initialization",
                "permission_grant_and_transition",
                "permission_denial_and_recovery",
                "interruption_and_resume",
                "tracking_failure_runtime_fallback",
            ],
            &invariants[0],
            &["parent_privacy_gate"],
        ),
        organ(
            "hiding_place_selection",
            &["spatial_creature", "tiered_spatial"],
            &[],
            &[
                "spatial_hiding_place_selection",
                "tracking_failure_runtime_fallback",
            ],
            &invariants[4],
            &["spatial_environment_sensing"],
        ),
        organ(
            "warm_cold_search_field",
            &["warm_cold_search"],
            &["spatial_audio", "haptics"],
            &["warm_cold_search", "glimpses"],
            &invariants[0],
            &["hiding_place_selection"],
        ),
        organ(
            "creature_embodiment",
            &["spatial_creature"],
            &["realitykit_rendering"],
            &["glimpses", "discovery"],
            &invariants[0],
            &["hiding_place_selection"],
        ),
        organ(
            "voice_listening_and_mimicry",
            &["voice_mimicry"],
            &["microphone_input", "speech_recognition"],
            &["voice_listening", "mimicry"],
            &invariants[2],
            &["creature_embodiment"],
        ),
        organ(
            "relationship_memory",
            &["relationship_memory", "second_encounter"],
            &["local_persistence"],
            &["memory_persistence", "second_encounter_recognition"],
            &invariants[3],
            &["voice_listening_and_mimicry"],
        ),
        organ(
            "finite_encounter_arc",
            &["finite_safety_privacy", "second_encounter"],
            &[],
            &["session_ending", "second_encounter_recognition"],
            &invariants[5],
            &["relationship_memory"],
        ),
        organ(
            "child_movement_safety",
            &["finite_safety_privacy"],
            &[],
            &["warm_cold_search", "session_ending"],
            &invariants[5],
            &["warm_cold_search_field"],
        ),
    ]);
    let must_requirement_ids = requirements.iter().map(|item| item.id.clone()).collect();
    let required_scenario_ids = journeys.iter().map(|item| item.id.clone()).collect();
    let plan = BirthPlan {
        schema: BIRTH_PLAN_SCHEMA.into(),
        intent_digest,
        product_name: "Anky".into(),
        promise: PROMISE.into(),
        target_users: vec![
            TargetUser {
                id: "parent".into(),
                role: "parent".into(),
                ability_or_age_context: None,
                environment: vec!["family home during setup".into()],
                goals: vec!["understand and control safety, privacy, and permissions".into()],
                constraints: vec!["child context must remain private".into()],
                understands_without_explanation: vec![
                    "why each permission is requested and how to recover a denial".into(),
                ],
            },
            TargetUser {
                id: "young_child".into(),
                role: "young child".into(),
                ability_or_age_context: Some("young child; reading cannot be required".into()),
                environment: vec!["moving through the real home with a parent nearby".into()],
                goals: vec!["find, meet, and begin teaching Anky language".into()],
                constraints: vec!["movement must remain finite and safe".into()],
                understands_without_explanation: vec![
                    "where to move and whether Anky feels nearer".into(),
                ],
            },
        ],
        contexts: vec!["a parent-supervised child experience in the family’s real home".into()],
        requirements,
        capabilities,
        journeys,
        embodiment,
        completion_contract: CompletionContract {
            must_requirement_ids,
            required_scenario_ids,
            physical_verification_capabilities: vec![
                "camera_capture".into(),
                "ar_world_tracking".into(),
                "motion_orientation".into(),
                "haptics".into(),
                "spatial_audio".into(),
                "microphone_input".into(),
                "speech_recognition".into(),
            ],
            release_build_required: true,
            zero_product_gaps_required: true,
        },
        explicit_non_goals: vec![
            "unbounded conversational AI".into(),
            "remote child analytics".into(),
        ],
        forbidden_substitutions: vec![
            substitution(
                "camera_to_dark_background",
                "real camera environment",
                "static dark or decorative background",
                &["camera_window"],
            ),
            substitution(
                "spatial_to_screen_target",
                "spatially anchored hiding",
                "screen-coordinate target",
                &["spatial_creature"],
            ),
            substitution(
                "voice_to_tapping",
                "child voice listening",
                "tap rhythm",
                &["voice_mimicry"],
            ),
            substitution(
                "simulator_to_no_arkit",
                "ARKit in the Release product",
                "removing ARKit because Simulator lacks sensor input",
                &["spatial_creature", "tiered_spatial"],
            ),
            substitution(
                "memory_to_counter",
                "relationship-bearing sparse memory",
                "generic session counter",
                &["relationship_memory", "second_encounter"],
            ),
        ],
        genome,
    };
    plan.validate().expect("valid Anky Birth Plan");
    plan
}

pub(crate) fn contract(plan: &BirthPlan) -> ExperienceContract {
    let scenarios = plan
        .journeys
        .iter()
        .map(|journey| {
            let capability_ids = plan
                .capabilities
                .iter()
                .filter(|capability| {
                    capability
                        .requirement_ids
                        .iter()
                        .any(|id| journey.requirement_ids.contains(id))
                })
                .map(|capability| capability.identifier.clone())
                .collect::<Vec<_>>();
            let physical_device_required = capability_ids.iter().any(|capability| {
                plan.completion_contract
                    .physical_verification_capabilities
                    .contains(capability)
            });
            ExperienceScenario {
                id: journey.id.clone(),
                target_actor: journey.target_actor.clone(),
                initial_state: format!("The app is ready to begin {}.", journey.id),
                environment: vec![
                    "controlled Simulator fixture and the accepted device tier".into()
                ],
                steps_or_gestures: vec![format!("Traverse {} as the target actor.", journey.id)],
                expected_states: vec![journey.promise.clone()],
                requirement_ids: journey.requirement_ids.clone(),
                capability_ids,
                completion_condition: format!(
                    "{} completes without a mock, dead control, or unexplained state.",
                    journey.id
                ),
                evidence_required: {
                    let mut evidence = vec![EvidenceKind::XcuiTest, EvidenceKind::Screenshot];
                    if physical_device_required {
                        evidence.push(EvidenceKind::PhysicalDeviceTrial);
                    }
                    evidence
                },
                physical_device_required,
            }
        })
        .collect();
    let contract = ExperienceContract {
        schema: EXPERIENCE_CONTRACT_SCHEMA.into(),
        intent_digest: plan.intent_digest,
        birth_plan_digest: plan.digest().expect("plan digest"),
        scenarios,
    };
    contract
        .validate(plan)
        .expect("valid Anky Experience Contract");
    contract
}

pub(crate) fn trial(
    plan: &BirthPlan,
    contract: &ExperienceContract,
    failed_scenario: Option<&str>,
    failed_organ: Option<&str>,
    physical: bool,
    gaps: Vec<TypedIncompleteness>,
) -> ExperienceTrial {
    let expression =
        crate::birth_plan::BirthExpressionPlan::from_birth_plan(plan).expect("expression plan");
    let scenario_results = contract
        .scenarios
        .iter()
        .enumerate()
        .map(|(index, scenario)| {
            let passed = failed_scenario != Some(scenario.id.as_str())
                && (!scenario.physical_device_required || physical);
            let mut scenario_evidence = vec![
                evidence(EvidenceKind::XcuiTest, &format!("{}.xcresult", scenario.id)),
                evidence(EvidenceKind::Screenshot, &format!("{}.png", scenario.id)),
            ];
            if index == 0 {
                scenario_evidence.push(evidence(
                    EvidenceKind::ReleaseBuild,
                    "Anky-release-build.log",
                ));
            }
            if physical && scenario.physical_device_required {
                scenario_evidence.push(evidence(
                    EvidenceKind::PhysicalDeviceTrial,
                    &format!("{}.physical.log", scenario.id),
                ));
            }
            ScenarioTrialResult {
                scenario_id: scenario.id.clone(),
                passed,
                assertions: vec![criterion(
                    &format!("{}_assertion", scenario.id),
                    passed,
                    true,
                )],
                evidence: scenario_evidence,
            }
        })
        .collect();
    let organ_results = expression
        .organs
        .iter()
        .map(|organ| OrganTrialResult {
            organ_id: organ.organ_id.clone(),
            criteria: organ
                .acceptance_criteria
                .iter()
                .map(|planned| {
                    criterion(
                        &planned.id,
                        failed_organ != Some(organ.organ_id.as_str()),
                        planned.deterministic,
                    )
                })
                .collect(),
        })
        .collect();
    let forbidden_substitution_results = plan
        .forbidden_substitutions
        .iter()
        .map(|substitution| criterion(&substitution.id, true, false))
        .collect();
    let trial = ExperienceTrial {
        schema: EXPERIENCE_TRIAL_SCHEMA.into(),
        birth_plan_digest: plan.digest().expect("plan digest"),
        experience_contract_digest: contract.digest().expect("contract digest"),
        release_build_passed: true,
        automated_tests_passed: true,
        simulator_trial_passed: failed_scenario.is_none(),
        scenario_results,
        organ_results,
        forbidden_substitution_results,
        intent_review: criterion("intention_review", true, false),
        physical_device: physical.then(|| PhysicalDeviceEvidence {
            product_type: "iPhone15,4".into(),
            os_version: "26.0".into(),
            os_build: Some("23A1".into()),
            exercised_capabilities: plan
                .completion_contract
                .physical_verification_capabilities
                .clone(),
            passed: true,
            evidence: vec![evidence(
                EvidenceKind::PhysicalDeviceTrial,
                "physical-trial.log",
            )],
        }),
        incompleteness: gaps,
    };
    trial
        .validate(plan, &expression, contract)
        .expect("structurally valid Anky trial");
    trial
}

pub(crate) fn factory_identity(plan: &BirthPlan) -> FactoryIdentity {
    FactoryIdentity::current(
        Some(plan.genome.digest().expect("genome digest")),
        profile().digest().expect("profile digest"),
    )
}

fn evaluation_evidence(
    plan: &BirthPlan,
    source_digest: Bytes32,
    protocol_criteria: Vec<CriterionResult>,
) -> BirthEvaluationEvidence {
    BirthEvaluationEvidence {
        source_digest,
        factory_identity: factory_identity(plan),
        protocol_criteria,
        engine_experience_criteria: vec![criterion("engine_simulator_test_execution", true, true)],
    }
}

pub(crate) fn product_gap() -> TypedIncompleteness {
    TypedIncompleteness {
        id: "missing_camera_experience".into(),
        category: IncompletenessCategory::ProductGap,
        description: "The required camera-first child experience is absent.".into(),
        requirement_ids: vec!["camera_window".into()],
        blocks_completion: true,
    }
}

fn requirement(id: &str, statement: &str, excerpt: &str) -> BirthRequirement {
    BirthRequirement {
        id: id.into(),
        statement: statement.into(),
        level: RequirementLevel::Must,
        origin: RequirementOrigin::ExplicitIntention,
        source_excerpt: Some(excerpt.into()),
        source_location: Some("anky-intention.md".into()),
    }
}

fn capability(
    identifier: &str,
    purpose: &str,
    requirement_ids: &[&str],
    primary: bool,
    runtime_fallback: Option<&str>,
) -> PlannedCapability {
    PlannedCapability {
        identifier: identifier.into(),
        purpose: purpose.into(),
        requirement_ids: requirement_ids
            .iter()
            .map(|value| (*value).into())
            .collect(),
        primary,
        runtime_fallback: runtime_fallback.map(str::to_owned),
    }
}

fn journey(id: &str, actor: &str, requirement_ids: &[&str]) -> ProductJourney {
    ProductJourney {
        id: id.into(),
        target_actor: actor.into(),
        promise: format!(
            "The target actor completes {id} as part of the accepted product promise."
        ),
        requirement_ids: requirement_ids
            .iter()
            .map(|value| (*value).into())
            .collect(),
    }
}

fn organ(
    id: &str,
    requirement_ids: &[&str],
    capability_ids: &[&str],
    journey_ids: &[&str],
    invariant: &str,
    dependencies: &[&str],
) -> BirthOrganPlan {
    let mut provides = capability_ids
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    if provides.is_empty() {
        provides.push(id.into());
    }
    BirthOrganPlan {
        organ_id: id.into(),
        kind: OrganKind::AppSpecific,
        provides,
        owns_state: vec![format!("{id}_state")],
        permissions: capability_ids.iter().map(|value| (*value).into()).collect(),
        dependencies: dependencies.iter().map(|value| (*value).into()).collect(),
        emits: vec![format!("{id}_completed")],
        consumes: Vec::new(),
        genome_invariants: vec![invariant.into()],
        requirement_ids: requirement_ids
            .iter()
            .map(|value| (*value).into())
            .collect(),
        capability_ids: capability_ids.iter().map(|value| (*value).into()).collect(),
        journey_ids: journey_ids.iter().map(|value| (*value).into()).collect(),
        acceptance_criteria: vec![OrganAcceptanceCriterion {
            id: format!("{id}_verified"),
            assertion: format!("{id} fulfills its mapped requirements independently."),
            deterministic: !matches!(id, "creature_embodiment" | "voice_listening_and_mimicry"),
        }],
        platforms: vec!["iphone".into()],
    }
}

fn substitution(
    id: &str,
    requested: &str,
    replacement: &str,
    requirement_ids: &[&str],
) -> ForbiddenSubstitution {
    ForbiddenSubstitution {
        id: id.into(),
        requested_experience: requested.into(),
        forbidden_replacement: replacement.into(),
        requirement_ids: requirement_ids
            .iter()
            .map(|value| (*value).into())
            .collect(),
        allowed_runtime_condition: None,
    }
}

fn criterion(id: &str, passed: bool, deterministic: bool) -> CriterionResult {
    CriterionResult {
        id: id.into(),
        passed,
        deterministic,
        evidence: vec![evidence(EvidenceKind::Log, &format!("{id}.log"))],
        observation: None,
    }
}

fn evidence(kind: EvidenceKind, name: &str) -> EvidenceReference {
    EvidenceReference {
        kind,
        artifact: ArtifactDescriptor {
            digest: sha256(name.as_bytes()),
            media_type: "application/octet-stream".into(),
            byte_length: 1,
            name: Some(name.into()),
        },
        relative_path: format!(".tohseno/private/birth/evidence/{name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::birth_plan::{BirthExpressionPlan, OrganKind};
    use crate::conception::{ConceptionHarness, FakeConceptionHarness};
    use crate::experience::evaluate_birth;

    #[test]
    fn anky_conception_derives_the_intention_instead_of_a_generic_factory_shape() {
        let input = conception_input();
        let harness = FakeConceptionHarness::returning(output());
        let proposed = harness.conceive(&input).unwrap();
        let expression = proposed.validate(&input).unwrap();
        assert_eq!(
            proposed
                .birth_plan
                .target_users
                .iter()
                .map(|actor| actor.role.as_str())
                .collect::<Vec<_>>(),
            ["parent", "young child"]
        );
        assert_eq!(proposed.birth_plan.promise, PROMISE);
        for capability in [
            "camera_capture",
            "ar_world_tracking",
            "realitykit_rendering",
            "scene_reconstruction",
            "motion_orientation",
            "haptics",
            "spatial_audio",
            "microphone_input",
            "speech_recognition",
            "local_persistence",
        ] {
            assert!(proposed
                .birth_plan
                .capabilities
                .iter()
                .any(|planned| planned.identifier == capability));
        }
        let ar = proposed
            .birth_plan
            .capabilities
            .iter()
            .find(|capability| capability.identifier == "ar_world_tracking")
            .unwrap();
        assert!(ar.primary);
        assert!(ar
            .runtime_fallback
            .as_deref()
            .unwrap()
            .contains("only after world tracking genuinely fails"));
        assert!(expression
            .organs
            .iter()
            .any(|organ| organ.kind == OrganKind::AppSpecific
                && organ.organ_id == "spatial_environment_sensing"));
        assert!(expression
            .organs
            .iter()
            .filter(|organ| organ.kind == OrganKind::ProtocolSubstrate)
            .all(|organ| organ.requirement_ids.is_empty()));
    }

    #[test]
    fn materialization_task_exposes_factory_identity_and_forbids_silent_substitution() {
        let proposed = output();
        let expression = BirthExpressionPlan::from_birth_plan(&proposed.birth_plan).unwrap();
        let factory = factory_identity(&proposed.birth_plan);
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join(".tohseno")).unwrap();
        crate::genome::Genome
            .write_birth_task(
                directory.path(),
                "Anky",
                "org.tohseno.genesis.fixture.anky",
                &proposed,
                &expression,
                &factory,
            )
            .unwrap();
        let task = std::fs::read_to_string(directory.path().join(".tohseno/TASK.md")).unwrap();
        assert!(task.contains(env!("CARGO_PKG_VERSION")));
        assert!(task.contains(env!("TOHSENO_SOURCE_COMMIT")));
        assert!(task.contains(&factory.static_constitution_digest.to_string()));
        assert!(task.contains(&factory.accepted_shot_genome_digest.unwrap().to_string()));
        assert!(task.contains(&factory.apple_capability_profile_digest.to_string()));
        assert!(task.contains(
            &proposed
                .experience_contract
                .digest()
                .expect("experience contract digest")
                .to_string()
        ));
        assert!(task.contains("not a denylist"));
        assert!(task.contains("engine-selected development team"));
        assert!(task.contains("Simulator sensor input is absent"));
        assert!(task.contains("silently replace accepted persistence"));
        assert!(task.contains(".tohseno/private/planning/birth-plan.json"));
        assert!(task.contains(".tohseno/private/planning/birth-expression-plan.json"));
        assert!(task.contains(".tohseno/private/planning/experience-contract.json"));
        assert!(!task.contains("\"forbidden_substitutions\""));
        assert!(!task.contains("\"camera_to_dark_background\""));
        assert!(task.contains("A DEBUG fixture may prove individual"));
        assert!(task.contains("relative to the Shot repository root"));
        assert!(task.contains("Set `experience_contract_digest` to the authoritative digest"));
        assert!(task.contains("exactly one real\n  Xcode project at `./Anky.xcodeproj`"));
        assert!(task.contains("repository-root `TohsenoFascia/`"));
        assert!(task.contains("repository-root\n`TOHSENO/fascia.json`"));
        assert!(task.contains("Do not call `tohseno evolve`"));
    }

    #[test]
    fn independent_acceptance_dimensions_block_failed_organs_and_camera_journeys() {
        let plan = plan(sha256(INTENTION.as_bytes()));
        let contract = contract(&plan);
        let expression = BirthExpressionPlan::from_birth_plan(&plan).unwrap();
        let protocol = vec![criterion("protocol_conformance", true, true)];

        let failed_organ_trial = trial(
            &plan,
            &contract,
            None,
            Some("spatial_environment_sensing"),
            true,
            Vec::new(),
        );
        let receipt = evaluate_birth(
            &plan,
            &expression,
            &contract,
            &failed_organ_trial,
            evaluation_evidence(&plan, Bytes32::new([0x71; 32]), protocol.clone()),
        )
        .unwrap();
        assert!(receipt.protocol_conformance.passed);
        assert!(!receipt.intent_fidelity.passed);
        assert!(!receipt.accepted);

        let failed_camera_trial = trial(
            &plan,
            &contract,
            Some("camera_first_transition"),
            None,
            true,
            Vec::new(),
        );
        let receipt = evaluate_birth(
            &plan,
            &expression,
            &contract,
            &failed_camera_trial,
            evaluation_evidence(&plan, Bytes32::new([0x72; 32]), protocol),
        )
        .unwrap();
        assert!(receipt.protocol_conformance.passed);
        assert!(!receipt.experience_verification.passed);
        assert!(!receipt.accepted);
    }

    #[test]
    fn build_only_or_missing_physical_evidence_cannot_be_accepted() {
        let plan = plan(sha256(INTENTION.as_bytes()));
        let contract = contract(&plan);
        let expression = BirthExpressionPlan::from_birth_plan(&plan).unwrap();
        let mut build_only = trial(&plan, &contract, None, None, true, Vec::new());
        build_only.scenario_results.clear();
        assert!(build_only.validate(&plan, &expression, &contract).is_err());

        let mut missing_class = trial(&plan, &contract, None, None, true, Vec::new());
        let scenario_id = contract.scenarios[0].id.clone();
        missing_class
            .scenario_results
            .iter_mut()
            .find(|result| result.scenario_id == scenario_id)
            .unwrap()
            .evidence
            .retain(|evidence| evidence.kind != EvidenceKind::XcuiTest);
        let diagnostic = missing_class
            .validate(&plan, &expression, &contract)
            .unwrap_err()
            .to_string();
        assert!(diagnostic.contains(&format!(
            "passing scenario `{scenario_id}` lacks required evidence classes: xcui_test"
        )));

        let no_phone = trial(&plan, &contract, None, None, false, Vec::new());
        let receipt = evaluate_birth(
            &plan,
            &expression,
            &contract,
            &no_phone,
            evaluation_evidence(
                &plan,
                Bytes32::new([0x73; 32]),
                vec![criterion("protocol_conformance", true, true)],
            ),
        )
        .unwrap();
        assert!(!receipt.experience_verification.passed);
        assert!(receipt
            .experience_verification
            .criteria
            .iter()
            .any(|criterion| criterion.id == "physical_device_experience"
                && criterion.observation.as_deref()
                    == Some("implementation_complete; acceptance_pending_physical_experience")));
        assert!(!receipt.accepted);
    }

    #[test]
    fn product_gap_blocks_an_otherwise_passing_birth() {
        let plan = plan(sha256(INTENTION.as_bytes()));
        let contract = contract(&plan);
        let expression = BirthExpressionPlan::from_birth_plan(&plan).unwrap();
        let with_gap = trial(&plan, &contract, None, None, true, vec![product_gap()]);
        let receipt = evaluate_birth(
            &plan,
            &expression,
            &contract,
            &with_gap,
            evaluation_evidence(
                &plan,
                Bytes32::new([0x74; 32]),
                vec![criterion("protocol_conformance", true, true)],
            ),
        )
        .unwrap();
        assert!(receipt.protocol_conformance.passed);
        assert!(receipt.intent_fidelity.passed);
        assert!(!receipt.experience_verification.passed);
        assert!(!receipt.accepted);
    }

    #[test]
    fn verification_gap_diagnostic_names_the_gap_and_must_requirement() {
        let plan = plan(sha256(INTENTION.as_bytes()));
        let contract = contract(&plan);
        let expression = BirthExpressionPlan::from_birth_plan(&plan).unwrap();
        let must_requirement = plan
            .requirements
            .iter()
            .find(|requirement| requirement.level == RequirementLevel::Must)
            .unwrap()
            .id
            .clone();
        let mut incomplete = trial(&plan, &contract, None, None, true, Vec::new());
        incomplete.incompleteness.push(TypedIncompleteness {
            id: "missing_visual_review".into(),
            category: IncompletenessCategory::ExperienceVerificationGap,
            description: "The final visual review has not run.".into(),
            requirement_ids: vec![must_requirement.clone()],
            blocks_completion: false,
        });
        let diagnostic = incomplete
            .validate(&plan, &expression, &contract)
            .unwrap_err()
            .to_string();
        assert!(diagnostic.contains(&format!(
            "verification gap `missing_visual_review` touches must requirements [{must_requirement}] and must block completion"
        )));
    }

    #[test]
    fn tests_and_simulator_trial_are_independent_acceptance_criteria() {
        let plan = plan(sha256(INTENTION.as_bytes()));
        let contract = contract(&plan);
        let expression = BirthExpressionPlan::from_birth_plan(&plan).unwrap();
        let mut failed_tests = trial(&plan, &contract, None, None, true, Vec::new());
        failed_tests.automated_tests_passed = false;
        failed_tests.simulator_trial_passed = false;
        let receipt = evaluate_birth(
            &plan,
            &expression,
            &contract,
            &failed_tests,
            evaluation_evidence(
                &plan,
                Bytes32::new([0x75; 32]),
                vec![criterion("protocol_conformance", true, true)],
            ),
        )
        .unwrap();
        assert!(!receipt.experience_verification.passed);
        assert!(receipt
            .experience_verification
            .criteria
            .iter()
            .any(|criterion| criterion.id == "automated_tests" && !criterion.passed));
        assert!(receipt
            .experience_verification
            .criteria
            .iter()
            .any(|criterion| {
                criterion.id == "simulator_target_user_trial" && !criterion.passed
            }));
        assert!(!receipt.accepted);
    }

    #[test]
    fn every_organ_criterion_requires_an_independent_result() {
        let plan = plan(sha256(INTENTION.as_bytes()));
        let contract = contract(&plan);
        let expression = BirthExpressionPlan::from_birth_plan(&plan).unwrap();
        let mut incomplete = trial(&plan, &contract, None, None, true, Vec::new());
        incomplete.organ_results[0].criteria.clear();
        assert!(incomplete.validate(&plan, &expression, &contract).is_err());
    }
}
