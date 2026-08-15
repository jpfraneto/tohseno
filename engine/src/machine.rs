use crate::apple_capabilities::{AppleCapabilityProfile, CapabilityProfileError};
use crate::birth_plan::{BirthExpressionPlan, BirthOrganPlan, BirthPlanError};
use crate::builder_identity::{BuilderIdentity, BuilderIdentityError, BuilderIdentityManager};
use crate::conception::{ConceptionError, ConceptionInput, ConceptionOutput};
use crate::config::{Config, ConfigError};
use crate::events::{Event, EventBus};
use crate::experience::{
    evaluate_birth, BirthEvaluationEvidence, BirthReceipt, CriterionResult, EvidenceKind,
    EvidenceReference, ExperienceContract, ExperienceError, ExperienceTrial,
    IncompletenessCategory,
};
use crate::gates::apple_signing::{self, AppleSigningState};
use crate::gates::device::{self, DeviceState};
use crate::gates::intent::{Intent, IntentError};
use crate::gates::toolchain::{self, ToolchainState};
use crate::gates::{build, install, preview, sign};
use crate::genome::{Genome, GenomeError};
use crate::harness::{default_selection, discover_harnesses, resolve_selection, HarnessOption};
use crate::ledger::{sanitize_component, AppRecord, Evolution, Ledger, LedgerError};
use crate::protocol_lifecycle::{self, ProtocolLifecycleError};
use crate::shot_layout::{
    describe_feedback_attachment, ImportedShot, PortableShotManifest, PortableVisibility,
    ShotBodyVerification, ShotLayout, ShotLayoutError, StoredFeedback, StoredReference,
};
use crate::workshop::WorkshopFeedbackPacket;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_protocol::conformance::CheckStatus;
use tohseno_protocol::digest::{Bytes32, ExpressionId, ShotId, VersionId};
use tohseno_protocol::lineage::{LineageAction, LineagePayload, SignedLineageAction};
use tohseno_protocol::ontology::{
    canonical_capability_graph_bytes, capability_graph_digest, organ_acceptance_gate_name,
    ArtifactAvailability, ArtifactDescriptor, AvailabilityStatus, ChangeScope, DesiredChange,
    Evolution as ProtocolEvolution, EvolutionaryIntent, Expression, Feedback, FeedbackAuthor,
    GenomeAcceptance, GenomeProposal, IntentionRecord, MaterializationProvenance, Organ,
    OriginalMaterial, ShotCommitment, TokenAssociation, TokenAssociationOperation,
    VerificationGate, VerificationResult, VersionRecord, Visibility, ARTIFACT_AVAILABILITY_SCHEMA,
    EVOLUTIONARY_INTENT_SCHEMA, EVOLUTION_SCHEMA, EXPRESSION_SCHEMA, FEEDBACK_SCHEMA,
    GENOME_ACCEPTANCE_SCHEMA, GENOME_PROPOSAL_SCHEMA, VERIFICATION_RESULT_SCHEMA, VERSION_SCHEMA,
};
use tohseno_protocol::record::CanonicalTimestamp;
use tohseno_protocol::record::ShotOrigin;

#[derive(Clone, Debug)]
pub struct ShotRequest {
    pub app_name: String,
    pub intent: Intent,
    /// Exact signed Feedback action commitments selected for the next
    /// Evolutionary Intent. Payload-only feedback IDs are not accepted.
    pub selected_feedback_actions: Vec<Bytes32>,
}

/// The outcome of `evolve`: either the folder's state became history, or the
/// builder's own agent was handed the intent to work on.
pub enum Evolved {
    Recorded(Evolution),
    NothingNew(Evolution),
    Conducted(ConductedCreation),
}

pub struct Engine {
    ledger: Ledger,
    events: EventBus,
    config: Config,
    genome: Genome,
}

/// Conducted work: the folder is ready and the builder's own agent takes it
/// from here, launched with this exact instruction.
pub struct ConductedCreation {
    pub folder: PathBuf,
    pub agent_command: Option<String>,
    pub instruction: String,
    pub phase: ConductionPhase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConductionPhase {
    Conception,
    BirthMaterialization,
    EvolutionMaterialization,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedGenomeRevision {
    pub genome: tohseno_protocol::Genome,
    pub proposal_action: tohseno_protocol::digest::Bytes32,
    pub acceptance_action: tohseno_protocol::digest::Bytes32,
    pub lineage_head: tohseno_protocol::digest::Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenAssociationReceipt {
    pub action: SignedLineageAction,
    pub action_commitment: Bytes32,
    pub lineage_head: Bytes32,
    pub outbox_path: Option<PathBuf>,
}

pub type InitialExpressionPlan = BirthExpressionPlan;
pub type InitialOrganPlan = BirthOrganPlan;

#[derive(Clone, Debug)]
struct MaterializationLineageInput {
    shot_id: ShotId,
    expression_id: ExpressionId,
    version_ordinal: u64,
    genome_revision: u64,
    genome_digest: Bytes32,
    lineage_sequence: u64,
    lineage_head: Bytes32,
    last_timestamp: CanonicalTimestamp,
    template_digest: Bytes32,
    capability_graph: Vec<Organ>,
    capability_graph_digest: Bytes32,
    from_version: Option<VersionRecord>,
    evolutionary_intent_action: Option<Bytes32>,
    genome_acceptance_action: Option<Bytes32>,
    preserved_invariants: Vec<String>,
}

struct BirthContext {
    plan: crate::birth_plan::BirthPlan,
    expression: BirthExpressionPlan,
    contract: ExperienceContract,
    trial: ExperienceTrial,
    factory_identity: crate::factory_identity::FactoryIdentity,
}

impl Engine {
    pub fn discover(events: EventBus) -> Result<Self, EngineError> {
        let ledger = Ledger::discover()?;
        ledger.initialize()?;
        let config = Config::load_or_create(ledger.machine_root())?;
        Ok(Self {
            ledger,
            events,
            config,
            genome: Genome,
        })
    }

    pub fn at(ledger: Ledger, events: EventBus, config: Config) -> Self {
        Self {
            ledger,
            events,
            config,
            genome: Genome,
        }
    }

    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    pub fn harnesses(&self) -> Vec<HarnessOption> {
        discover_harnesses(&self.config.harness)
    }

    /// Preserve the exact intention and prepare the selected intelligence for
    /// conception. No app-specific Genome exists or can be accepted before
    /// the structured conception output passes deterministic validation.
    pub fn create(&self, request: &ShotRequest) -> Result<ConductedCreation, EngineError> {
        crate::ledger::validate_app_name(&request.app_name)?;
        if request.intent.images.len() > crate::gates::intent::MAX_IMAGES {
            return Err(EngineError::ProtocolBodyIncomplete(
                "a Shot accepts at most eight reference images; no attachment was staged".into(),
            ));
        }
        if !request.selected_feedback_actions.is_empty() {
            return Err(EngineError::ProtocolBodyIncomplete(
                "feedback can be selected only for an evolution from an accepted Version".into(),
            ));
        }
        let _app_lock = self.ledger.lock_app(&request.app_name)?;
        // Prove every reference source and its deterministic `image_N` origin
        // descriptor before the folder or identity exists. This description
        // is read-only and lets a retry compare origin lineage before it
        // replaces any prepared reference aliases.
        let prospective_layout = ShotLayout::at(self.ledger.working_tree(&request.app_name));
        let source_materials =
            prospective_layout.describe_prepared_intent_references(&request.intent.images)?;
        self.emit_upsell_once(
            "welcome",
            "first shot: Xcode + Apple ID now · iPhone later · free Apple IDs refresh weekly.",
        )?;
        self.events
            .emit(Event::status("preparing your TOHSENO identity…"));
        let identity_manager = BuilderIdentityManager::for_ledger(&self.ledger);
        let builder = identity_manager.ensure()?;
        let proposed_bundle_id = bundle_id(&request.app_name)?;
        let mut app = match self.ledger.load_app(&request.app_name) {
            Ok(existing) if existing.latest_evolution.is_none() => existing,
            Ok(_) => return Err(LedgerError::AppExists(request.app_name.clone()).into()),
            Err(LedgerError::AppMissing(_)) => self
                .ledger
                .create_app(&request.app_name, &proposed_bundle_id)?,
            Err(error) => return Err(error.into()),
        };
        if app.shot_id.is_none() && app.builder_id.is_none() {
            app = self.ledger.bind_protocol_identity(
                &request.app_name,
                tohseno_protocol::digest::ShotId::random(),
                builder.builder_id,
            )?;
        }
        if app.builder_id != Some(builder.builder_id) {
            return Err(EngineError::BuilderMismatch(request.app_name.clone()));
        }
        if self.working_tree_has_user_content(&request.app_name)? {
            return Err(EngineError::FolderInProgress(request.app_name.clone()));
        }
        let layout = ShotLayout::at(self.ledger.working_tree(&request.app_name));
        layout.initialize_directories()?;
        layout.preserve_exact_intention(request.intent.prompt.as_bytes())?;
        let reference_sources = self.validated_reference_sources(&request.intent);
        self.ensure_origin_lineage(
            &layout,
            &app,
            &builder,
            &identity_manager,
            request.intent.prompt.as_bytes(),
            &source_materials,
        )?;
        let (prepared_intention, source_references) =
            layout.prepare_intent_package(request.intent.prompt.as_bytes(), &reference_sources)?;
        if source_references
            .iter()
            .map(|reference| &reference.availability)
            .ne(source_materials.iter())
        {
            return Err(EngineError::ProtocolBodyIncomplete(
                "reference image bytes changed between origin preflight and intention staging"
                    .into(),
            ));
        }
        self.genome.compose_briefing(
            &self.ledger,
            &request.app_name,
            &app.bundle_id,
            &request.intent,
            &source_references,
        )?;
        let conception_input_path = self
            .ledger
            .briefing_dir(&request.app_name)
            .join("private/planning")
            .join(crate::conception::CONCEPTION_INPUT_FILE);
        let conception_input = if conception_input_path.is_file() {
            let existing = ConceptionInput::read(&layout)?;
            if existing.intent_digest != prepared_intention.intention_digest {
                return Err(EngineError::ProtocolBodyIncomplete(
                    "the pending conception is bound to a different exact intention".into(),
                ));
            }
            existing
        } else {
            let capability_profile = AppleCapabilityProfile::discover(&self.ledger)?;
            let input =
                ConceptionInput::new(&request.app_name, &prepared_intention, capability_profile)?;
            input.write(&layout)?;
            input
        };
        crate::conception::write_conception_task(
            &self.ledger.working_tree(&request.app_name),
            &conception_input,
        )?;
        Ok(ConductedCreation {
            folder: self.ledger.working_tree(&request.app_name),
            agent_command: self.preferred_agent_command(),
            instruction: "Read .tohseno/CONCEPTION.md and produce the strict app-specific conception output. No Genome is accepted and no app is materialized before that output validates.".into(),
            phase: ConductionPhase::Conception,
        })
    }

    pub fn conception_input(&self, app_name: &str) -> Result<ConceptionInput, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        self.ledger.load_app(app_name)?;
        ConceptionInput::read(&ShotLayout::at(self.ledger.working_tree(app_name)))
            .map_err(EngineError::from)
    }

    pub fn stage_conception_output(
        &self,
        app_name: &str,
        output: &ConceptionOutput,
    ) -> Result<(), EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let input = self.conception_input(app_name)?;
        output.validate(&input)?;
        validate_conception_source_traceability(
            &ShotLayout::at(self.ledger.working_tree(app_name)),
            &input,
            output,
        )?;
        let mut bytes = tohseno_protocol::canonical::to_vec(output)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        bytes.push(b'\n');
        ShotLayout::at(self.ledger.working_tree(app_name))
            .preserve_private_planning_file(crate::conception::CONCEPTION_OUTPUT_FILE, &bytes)?;
        Ok(())
    }

    pub fn pending_conception(
        &self,
        app_name: &str,
    ) -> Result<(ConceptionOutput, BirthExpressionPlan), EngineError> {
        let input = self.conception_input(app_name)?;
        let layout = ShotLayout::at(self.ledger.working_tree(app_name));
        let (output, expression) = ConceptionOutput::read_and_validate(&layout, &input)?;
        validate_conception_source_traceability(&layout, &input, &output)?;
        Ok((output, expression))
    }

    /// Accept the actual app-specific proposal returned by the selected
    /// intelligence, declare its app-specific organs, and prepare
    /// materialization. This is the only initial Genome acceptance path.
    pub fn accept_pending_conception(
        &self,
        app_name: &str,
    ) -> Result<ConductedCreation, EngineError> {
        let input = self.conception_input(app_name)?;
        let layout = ShotLayout::at(self.ledger.working_tree(app_name));
        let (output, expression) = ConceptionOutput::read_and_validate(&layout, &input)?;
        validate_conception_source_traceability(&layout, &input, &output)?;
        let app = self.ledger.load_app(app_name)?;
        let (expression_plan_bytes, _, _) =
            prepare_initial_expression_parts(&app, &expression, &output.birth_plan.genome)?;
        // Every deterministic artifact and protocol conversion that follows
        // Genome acceptance must succeed first. A malformed app-specific
        // Organ graph must never leave a Genome accepted on its own.
        output.preserve_accepted_artifacts(&layout)?;
        layout
            .preserve_private_planning_file("birth-expression-plan.json", &expression_plan_bytes)?;
        let genome_digest = output
            .birth_plan
            .genome
            .digest()
            .map_err(ShotLayoutError::from)?;
        let factory = crate::factory_identity::FactoryIdentity::current(
            Some(genome_digest),
            input.apple_capability_profile.digest()?,
        );
        factory
            .validate()
            .map_err(EngineError::ProtocolBodyIncomplete)?;
        let mut factory_bytes = tohseno_protocol::canonical::to_vec(&factory)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        factory_bytes.push(b'\n');
        layout.preserve_private_planning_file(
            "materialization-factory-identity.json",
            &factory_bytes,
        )?;
        self.accept_genome_from_validated_conception(
            app_name,
            &output.birth_plan.genome,
            &output.rationale,
        )?;
        self.declare_initial_expression(app_name, &expression)?;
        self.genome.write_birth_task(
            &self.ledger.working_tree(app_name),
            app_name,
            &app.bundle_id,
            &output,
            &expression,
            &factory,
        )?;
        self.genome
            .write_standing_orders(&self.ledger.working_tree(app_name), app_name)?;
        self.conduct_accepted_creation(app_name)
    }

    /// Hand the accepted initial expression to the configured builder agent.
    /// This is deliberately separate from origin capture and Genome proposal.
    pub fn conduct_accepted_creation(
        &self,
        app_name: &str,
    ) -> Result<ConductedCreation, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        let expression_id = app.expression_id.ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "the initial ExpressionID has not been established".into(),
            )
        })?;
        let layout = ShotLayout::at(self.ledger.working_tree(app_name));
        let lineage = layout.read_lineage()?;
        let state = tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
        if state.accepted_genome.is_none() || state.expression(expression_id).is_none() {
            return Err(EngineError::ProtocolBodyIncomplete(
                "materialization requires an explicitly accepted Genome and declared Expression"
                    .into(),
            ));
        }
        layout.verify_shot_body(Some(expression_id))?;
        Ok(ConductedCreation {
            folder: self.ledger.working_tree(app_name),
            agent_command: self.preferred_agent_command(),
            instruction: "Read AGENTS.md and .tohseno/TASK.md, materialize the accepted Birth Plan, execute every required target-user scenario, and return a strict Experience Trial. Do not call tohseno evolve; the engine owns acceptance and sealing.".into(),
            phase: ConductionPhase::BirthMaterialization,
        })
    }

    pub fn verify_shot_body(&self, app_name: &str) -> Result<ShotBodyVerification, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        ShotLayout::at(self.ledger.working_tree(app_name))
            .verify_shot_body(app.expression_id)
            .map_err(EngineError::from)
    }

    pub fn export_shot(
        &self,
        app_name: &str,
        destination: &Path,
        visibility: PortableVisibility,
    ) -> Result<PortableShotManifest, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        self.ledger.load_app(app_name)?;
        ShotLayout::at(self.ledger.working_tree(app_name))
            .export_bundle(destination, visibility)
            .map_err(EngineError::from)
    }

    pub fn import_shot(bundle: &Path, destination: &Path) -> Result<ImportedShot, EngineError> {
        ShotLayout::import_bundle(bundle, destination).map_err(EngineError::from)
    }

    /// Verify and project a frozen v1 lineage without rewriting or resigning
    /// any historical record.
    pub fn migrate_legacy_shot(
        &self,
        app_name: &str,
    ) -> Result<tohseno_protocol::lineage::AdaptedV1Lineage, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let adapted = self.ledger.migrate_v1_identity(app_name)?;
        ShotLayout::at(self.ledger.working_tree(app_name)).write_v1_migration(&adapted)?;
        Ok(adapted)
    }

    /// Refuse any Shot execution that would not land attributed to the local
    /// Builder identity. Running is a recording act: an app with no recorded
    /// Builder, or one recorded under a different Builder, must be refused
    /// before the harness is allowed to touch the folder.
    pub fn verify_builder_binding(&self, app_name: &str) -> Result<(), EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        let manager = BuilderIdentityManager::for_ledger(&self.ledger);
        let builder = manager.ensure()?;
        let shot_id =
            verify_recorded_builder(app_name, app.shot_id, app.builder_id, builder.builder_id)?;
        let layout = ShotLayout::at(self.ledger.working_tree(app_name));
        let lineage = layout.read_lineage()?;
        let state = tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
        if state.shot_id != shot_id
            || state.controller != builder.builder_id
            || state.controller_key != builder.device.public_key
        {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        Ok(())
    }

    /// Append an optional economic relationship without changing Shot,
    /// Expression, Version, or ownership identity.
    ///
    /// Token Associations remain private until an ancestry-free public
    /// relation projection is defined. Publishing an ordinary lineage action
    /// would also publish a commitment to every private predecessor.
    pub fn record_token_association(
        &self,
        app_name: &str,
        association: TokenAssociation,
        availability: AvailabilityStatus,
    ) -> Result<TokenAssociationReceipt, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        association.validate().map_err(ShotLayoutError::from)?;
        if availability == AvailabilityStatus::PubliclyAvailable {
            return Err(EngineError::ProtocolBodyIncomplete(
                "public Token Association export is disabled: ordinary lineage actions may commit private predecessors; record it privately until a dedicated ancestry-free public relation exists"
                    .into(),
            ));
        }
        if availability != AvailabilityStatus::IntentionallyPrivate {
            return Err(EngineError::ProtocolBodyIncomplete(
                "a signed Token Association must currently be intentionally private".into(),
            ));
        }

        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        let shot_id = app
            .shot_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
        let expected_builder = app
            .builder_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
        let manager = BuilderIdentityManager::for_ledger(&self.ledger);
        let builder = manager.ensure()?;
        if builder.builder_id != expected_builder {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }

        let layout = ShotLayout::at(self.ledger.working_tree(app_name));
        let lineage = layout.read_lineage()?;
        let state = tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
        if state.shot_id != shot_id
            || state.controller != builder.builder_id
            || state.controller_key != builder.device.public_key
        {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }

        // Retrying the same still-current private relation is idempotent.
        let effect_is_current = match association.operation {
            TokenAssociationOperation::Associate => {
                state.token_association.as_ref() == Some(&association)
            }
            TokenAssociationOperation::Remove => {
                state.token_association.is_none()
                    && state
                        .token_history
                        .last()
                        .is_some_and(|entry| entry.record == association)
            }
        };
        if effect_is_current {
            if let Some(existing) = state
                .token_history
                .last()
                .filter(|entry| entry.record == association)
                .and_then(|entry| {
                    lineage.iter().find(|action| {
                        action.commitment().ok().as_ref() == Some(&entry.action)
                            && action.action.availability == availability
                    })
                })
            {
                let action_commitment = existing.commitment().map_err(ShotLayoutError::from)?;
                return Ok(TokenAssociationReceipt {
                    action: existing.clone(),
                    action_commitment,
                    lineage_head: state.head,
                    outbox_path: None,
                });
            }
        }

        let timestamp = canonical_now_at_least(&state.last_timestamp)?;
        let action = LineageAction::new(
            state.sequence.checked_add(1).ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete("lineage sequence overflowed".into())
            })?,
            Some(state.head),
            shot_id,
            builder.builder_id,
            timestamp,
            availability,
            LineagePayload::TokenAssociation(association),
        )
        .map_err(ShotLayoutError::from)?;
        let signed = sign_lineage_action(&manager, &builder, action)?;
        let action_commitment = layout.append_lineage(&signed)?;
        Ok(TokenAssociationReceipt {
            action: signed,
            action_commitment,
            lineage_head: action_commitment,
            outbox_path: None,
        })
    }

    /// Attach private text feedback to one exact accepted Version of this
    /// Shot. Ordinals are resolved only within the app's stable ExpressionID;
    /// this API never falls back to Shot-level or "latest" feedback.
    pub fn record_feedback(
        &self,
        app_name: &str,
        version_ordinal: u64,
        text: &str,
    ) -> Result<StoredFeedback, EngineError> {
        self.record_feedback_with_attachments(app_name, version_ordinal, text, &[])
    }

    /// The attachment-capable feedback entry point used by CLI automation.
    /// Exact bytes are bounded, read without following symlinks, described by
    /// digest and length, and then checked again while they are stored.
    pub fn record_feedback_with_attachments(
        &self,
        app_name: &str,
        version_ordinal: u64,
        text: &str,
        attachments: &[PathBuf],
    ) -> Result<StoredFeedback, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        let shot_id = app
            .shot_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
        let expected_builder = app
            .builder_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
        let expression_id = app.expression_id.ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(format!("{app_name} has no stable ExpressionID"))
        })?;
        let manager = BuilderIdentityManager::for_ledger(&self.ledger);
        let builder = manager.ensure()?;
        if builder.builder_id != expected_builder {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        let layout = ShotLayout::at(self.ledger.working_tree(app_name));
        let lineage = layout.read_lineage()?;
        let state = tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
        if state.shot_id != shot_id
            || state.controller != builder.builder_id
            || state.controller_key != builder.device.public_key
        {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        let version = state
            .expression(expression_id)
            .and_then(|expression| {
                expression
                    .versions
                    .iter()
                    .find(|version| version.ordinal == version_ordinal)
            })
            .cloned()
            .ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete(format!(
                    "{app_name} has no accepted version {version_ordinal:04} for expression {expression_id}"
                ))
            })?;
        let timestamp = canonical_now_at_least(&state.last_timestamp)?;
        let attachment_records = attachments
            .iter()
            .map(|path| describe_feedback_attachment(path))
            .collect::<Result<Vec<_>, _>>()?;
        let feedback = Feedback {
            schema: FEEDBACK_SCHEMA.into(),
            expression_id,
            version_id: version.version_id,
            build_identity: version.build_identity.clone(),
            author: Some(FeedbackAuthor {
                identity: builder.builder_id.to_string(),
                display_name: None,
            }),
            visibility: Visibility::Private,
            text: (!text.is_empty()).then(|| text.to_owned()),
            observations: Vec::new(),
            attachments: attachment_records,
            observed_at: timestamp.clone(),
        };
        feedback.validate().map_err(ShotLayoutError::from)?;
        let action = LineageAction::new(
            state.sequence.checked_add(1).ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete("lineage sequence overflowed".into())
            })?,
            Some(state.head),
            shot_id,
            builder.builder_id,
            timestamp,
            AvailabilityStatus::IntentionallyPrivate,
            LineagePayload::Feedback(feedback.clone()),
        )
        .map_err(ShotLayoutError::from)?;
        let signed = sign_lineage_action(&manager, &builder, action)?;
        layout
            .record_feedback_action(shot_id, &version, &feedback, &signed, attachments)
            .map_err(EngineError::from)
    }

    /// Accept one reviewed workshop feedback packet into the Builder's private
    /// lineage. The packet binds an exact public Version but is not itself an
    /// authority signature; the current Builder decides whether to admit it.
    pub fn record_workshop_feedback(
        &self,
        app_name: &str,
        packet: &WorkshopFeedbackPacket,
    ) -> Result<StoredFeedback, EngineError> {
        packet
            .validate()
            .map_err(|error| EngineError::ProtocolBodyIncomplete(error.to_string()))?;
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        let shot_id = app
            .shot_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
        let expected_builder = app
            .builder_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
        let expression_id = app.expression_id.ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(format!("{app_name} has no stable ExpressionID"))
        })?;
        if packet.shot_id != shot_id || packet.expression_id != expression_id {
            return Err(EngineError::ProtocolBodyIncomplete(
                "workshop feedback names a different Shot or Expression".into(),
            ));
        }

        let manager = BuilderIdentityManager::for_ledger(&self.ledger);
        let builder = manager.ensure()?;
        if builder.builder_id != expected_builder {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        let layout = ShotLayout::at(self.ledger.working_tree(app_name));
        let lineage = layout.read_lineage()?;
        let state = tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
        if state.shot_id != shot_id
            || state.controller != builder.builder_id
            || state.controller_key != builder.device.public_key
        {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        let version = state
            .expression(expression_id)
            .and_then(|expression| {
                expression
                    .versions
                    .iter()
                    .find(|version| version.ordinal == packet.version_ordinal)
            })
            .cloned()
            .ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete(format!(
                    "{app_name} has no accepted version {:04} for this workshop feedback",
                    packet.version_ordinal
                ))
            })?;
        if version.version_id != packet.version_id {
            return Err(EngineError::ProtocolBodyIncomplete(
                "workshop feedback VersionID does not match the accepted ordinal".into(),
            ));
        }

        let action_timestamp = canonical_now_at_least(&state.last_timestamp)?;
        let feedback = Feedback {
            schema: FEEDBACK_SCHEMA.into(),
            expression_id,
            version_id: version.version_id,
            build_identity: version.build_identity.clone(),
            author: Some(FeedbackAuthor {
                identity: "workshop:self-declared".into(),
                display_name: packet.author_display_name.clone(),
            }),
            visibility: Visibility::Private,
            text: Some(packet.text.clone()),
            observations: Vec::new(),
            attachments: Vec::new(),
            observed_at: packet.observed_at.clone(),
        };
        feedback.validate().map_err(ShotLayoutError::from)?;
        let action = LineageAction::new(
            state.sequence.checked_add(1).ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete("lineage sequence overflowed".into())
            })?,
            Some(state.head),
            shot_id,
            builder.builder_id,
            action_timestamp,
            AvailabilityStatus::IntentionallyPrivate,
            LineagePayload::Feedback(feedback.clone()),
        )
        .map_err(ShotLayoutError::from)?;
        let signed = sign_lineage_action(&manager, &builder, action)?;
        layout
            .record_feedback_action(shot_id, &version, &feedback, &signed, &[])
            .map_err(EngineError::from)
    }

    /// Explicitly accept a reviewed Genome proposal.
    ///
    /// Calling this method is the acceptance boundary: create/evolve never
    /// invokes it implicitly. Initial revision 1 requires no mutation summary;
    /// every later revision requires explicit nonempty mutation statements.
    pub fn accept_genome(
        &self,
        app_name: &str,
        proposed: &tohseno_protocol::Genome,
        rationale: &str,
        mutation_summary: &[String],
    ) -> Result<AcceptedGenomeRevision, EngineError> {
        self.accept_genome_with_source(app_name, proposed, rationale, mutation_summary, false)
    }

    fn accept_genome_from_validated_conception(
        &self,
        app_name: &str,
        proposed: &tohseno_protocol::Genome,
        rationale: &str,
    ) -> Result<AcceptedGenomeRevision, EngineError> {
        self.accept_genome_with_source(app_name, proposed, rationale, &[], true)
    }

    fn accept_genome_with_source(
        &self,
        app_name: &str,
        proposed: &tohseno_protocol::Genome,
        rationale: &str,
        mutation_summary: &[String],
        validated_initial_conception: bool,
    ) -> Result<AcceptedGenomeRevision, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        proposed.validate().map_err(ShotLayoutError::from)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        let shot_id = app
            .shot_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
        let expected_builder = app
            .builder_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
        let manager = BuilderIdentityManager::for_ledger(&self.ledger);
        let builder = manager.ensure()?;
        if builder.builder_id != expected_builder {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        let layout = ShotLayout::at(self.ledger.working_tree(app_name));
        let lineage = layout.read_lineage()?;
        let state = tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
        if state.shot_id != shot_id
            || state.controller != builder.builder_id
            || state.controller_key != builder.device.public_key
        {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        if let Some(current) = &state.accepted_genome {
            if &current.genome == proposed {
                layout.write_accepted_genome(proposed)?;
                return Ok(AcceptedGenomeRevision {
                    genome: proposed.clone(),
                    proposal_action: current.proposal_action,
                    acceptance_action: current.acceptance_action,
                    lineage_head: state.head,
                });
            }
        }

        let proposal = match &state.accepted_genome {
            None => {
                if !validated_initial_conception {
                    return Err(EngineError::ProtocolBodyIncomplete(
                        "revision 1 can be accepted only from the strict app-specific conception output produced after the intelligence reads the exact intention and Apple capability profile"
                            .into(),
                    ));
                }
                if proposed.revision != 1 || !mutation_summary.is_empty() {
                    return Err(EngineError::ProtocolBodyIncomplete(
                        "the initial Genome must be revision 1 without mutation claims".into(),
                    ));
                }
                GenomeProposal::initial(proposed.clone(), rationale.to_owned())
            }
            Some(current) => GenomeProposal {
                schema: GENOME_PROPOSAL_SCHEMA.into(),
                base_revision: Some(current.genome.revision),
                base_genome_digest: Some(current.genome.digest().map_err(ShotLayoutError::from)?),
                proposed: proposed.clone(),
                rationale: rationale.to_owned(),
                mutation_summary: mutation_summary.to_vec(),
            },
        };
        proposal.validate().map_err(ShotLayoutError::from)?;
        let timestamp = canonical_now_at_least(&state.last_timestamp)?;
        let proposal_action = LineageAction::new(
            state.sequence.checked_add(1).ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete("lineage sequence overflowed".into())
            })?,
            Some(state.head),
            shot_id,
            builder.builder_id,
            timestamp.clone(),
            AvailabilityStatus::IntentionallyPrivate,
            LineagePayload::GenomeProposal(proposal),
        )
        .map_err(ShotLayoutError::from)?;
        let signed_proposal = sign_lineage_action(&manager, &builder, proposal_action)?;
        let proposal_commitment = signed_proposal
            .commitment()
            .map_err(ShotLayoutError::from)?;
        let acceptance = GenomeAcceptance {
            schema: GENOME_ACCEPTANCE_SCHEMA.into(),
            proposal_action: proposal_commitment,
            revision: proposed.revision,
            genome_digest: proposed.digest().map_err(ShotLayoutError::from)?,
            accepted_at: timestamp.clone(),
        };
        let acceptance_action = LineageAction::new(
            state.sequence.checked_add(2).ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete("lineage sequence overflowed".into())
            })?,
            Some(proposal_commitment),
            shot_id,
            builder.builder_id,
            timestamp,
            AvailabilityStatus::IntentionallyPrivate,
            LineagePayload::GenomeAcceptance(acceptance),
        )
        .map_err(ShotLayoutError::from)?;
        let signed_acceptance = sign_lineage_action(&manager, &builder, acceptance_action)?;
        let acceptance_commitment = signed_acceptance
            .commitment()
            .map_err(ShotLayoutError::from)?;
        layout.append_lineage_batch(&[signed_proposal, signed_acceptance])?;
        layout.write_accepted_genome(proposed)?;
        Ok(AcceptedGenomeRevision {
            genome: proposed.clone(),
            proposal_action: proposal_commitment,
            acceptance_action: acceptance_commitment,
            lineage_head: acceptance_commitment,
        })
    }

    /// Declare the reviewed initial Apple Expression and its bounded Organs.
    /// The reducer requires the referenced Genome to be accepted first.
    pub fn declare_initial_expression(
        &self,
        app_name: &str,
        plan: &InitialExpressionPlan,
    ) -> Result<Expression, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        let shot_id = app
            .shot_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
        let expected_builder = app
            .builder_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
        let manager = BuilderIdentityManager::for_ledger(&self.ledger);
        let builder = manager.ensure()?;
        if builder.builder_id != expected_builder {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        let layout = ShotLayout::at(self.ledger.working_tree(app_name));
        let lineage = layout.read_lineage()?;
        let state = tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
        if state.shot_id != shot_id
            || state.controller != builder.builder_id
            || state.controller_key != builder.device.public_key
        {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        let accepted = state.accepted_genome.as_ref().ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "an Expression cannot be declared before explicit Genome acceptance".into(),
            )
        })?;
        if accepted.genome.revision != plan.genome_revision
            || accepted.genome.digest().map_err(ShotLayoutError::from)? != plan.genome_digest
        {
            return Err(EngineError::ProtocolBodyIncomplete(
                "expression plan does not bind the current accepted Genome".into(),
            ));
        }
        let (plan_bytes, expression, organs) =
            prepare_initial_expression_parts(&app, plan, &accepted.genome)?;
        let expression_id = expression.expression_id;
        layout.preserve_private_planning_file("birth-expression-plan.json", &plan_bytes)?;
        if let Some(existing) = state.expression(expression_id) {
            let exact_organs = existing.organs.len() == organs.len()
                && organs.iter().all(|organ| {
                    existing
                        .organs
                        .get(&organ.organ_id)
                        .is_some_and(|value| value == organ)
                });
            if existing.expression != expression || !exact_organs {
                return Err(EngineError::ProtocolBodyIncomplete(
                    "the stable ExpressionID is already declared with different facts".into(),
                ));
            }
            layout.write_metadata_json("expression.json", &expression, false)?;
            layout.write_metadata_json(
                "capabilities.lock",
                &canonical_organ_view(&organs),
                false,
            )?;
            return Ok(expression);
        }

        let timestamp = canonical_now_at_least(&state.last_timestamp)?;
        let mut sequence = state.sequence;
        let mut previous = state.head;
        let mut signed_actions = Vec::with_capacity(1 + organs.len());
        for payload in std::iter::once(LineagePayload::Expression(expression.clone()))
            .chain(organs.iter().cloned().map(LineagePayload::Organ))
        {
            sequence = sequence.checked_add(1).ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete("lineage sequence overflowed".into())
            })?;
            let action = LineageAction::new(
                sequence,
                Some(previous),
                shot_id,
                builder.builder_id,
                timestamp.clone(),
                AvailabilityStatus::IntentionallyPrivate,
                payload,
            )
            .map_err(ShotLayoutError::from)?;
            let signed = sign_lineage_action(&manager, &builder, action)?;
            previous = signed.commitment().map_err(ShotLayoutError::from)?;
            signed_actions.push(signed);
        }
        layout.append_lineage_batch(&signed_actions)?;
        layout.write_metadata_json("expression.json", &expression, false)?;
        layout.write_metadata_json("capabilities.lock", &canonical_organ_view(&organs), false)?;
        Ok(expression)
    }

    fn ensure_origin_lineage(
        &self,
        layout: &ShotLayout,
        app: &AppRecord,
        builder: &BuilderIdentity,
        manager: &BuilderIdentityManager,
        original_intention: &[u8],
        source_materials: &[ArtifactAvailability],
    ) -> Result<(), EngineError> {
        let shot_id = app.shot_id.ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete("new Shot has no persistent ShotID".into())
        })?;
        let materials = original_materials(original_intention, source_materials)?;
        let existing = layout.read_lineage()?;
        if !existing.is_empty() {
            let state =
                tohseno_protocol::reduce_lineage(&existing).map_err(ShotLayoutError::from)?;
            let intention_matches = state
                .intention
                .as_ref()
                .is_some_and(|record| record.materials == materials);
            if state.shot_id != shot_id
                || state.controller != builder.builder_id
                || state.controller_key != builder.device.public_key
                || !intention_matches
            {
                return Err(EngineError::ProtocolBodyIncomplete(
                    "existing origin lineage does not match this Shot identity and exact intention"
                        .into(),
                ));
            }
            return Ok(());
        }

        let timestamp = canonical_now()?;
        let intention = IntentionRecord::new(materials, timestamp.clone());
        intention.validate().map_err(ShotLayoutError::from)?;
        let commitment = ShotCommitment::new(
            intention.commitment().map_err(ShotLayoutError::from)?,
            builder.builder_id,
            builder.device.public_key.clone(),
            timestamp.clone(),
        );
        let commitment_action = LineageAction::new(
            1,
            None,
            shot_id,
            builder.builder_id,
            timestamp.clone(),
            AvailabilityStatus::IntentionallyPrivate,
            LineagePayload::Commitment(commitment),
        )
        .map_err(ShotLayoutError::from)?;
        let signed_commitment = sign_lineage_action(manager, builder, commitment_action)?;
        let intention_action = LineageAction::new(
            2,
            Some(
                signed_commitment
                    .commitment()
                    .map_err(ShotLayoutError::from)?,
            ),
            shot_id,
            builder.builder_id,
            timestamp,
            AvailabilityStatus::IntentionallyPrivate,
            LineagePayload::Intention(intention),
        )
        .map_err(ShotLayoutError::from)?;
        let signed_intention = sign_lineage_action(manager, builder, intention_action)?;
        layout.append_lineage_batch(&[signed_commitment, signed_intention])?;
        Ok(())
    }

    /// Evolves the Shot. Whatever the folder holds becomes history first;
    /// with an intent, the builder's own agent is then handed the work.
    pub async fn evolve(&self, request: &ShotRequest) -> Result<Evolved, EngineError> {
        crate::ledger::validate_app_name(&request.app_name)?;
        if request.intent.images.len() > crate::gates::intent::MAX_IMAGES {
            return Err(EngineError::ProtocolBodyIncomplete(
                "an Evolution accepts at most eight reference images; no attachment was staged"
                    .into(),
            ));
        }
        if request.intent.prompt.trim().is_empty()
            && (!request.selected_feedback_actions.is_empty() || !request.intent.images.is_empty())
        {
            return Err(EngineError::ProtocolBodyIncomplete(
                "selected Feedback actions and references require a nonempty evolutionary instruction"
                    .into(),
            ));
        }
        let _app_lock = self.ledger.lock_app(&request.app_name)?;
        let app = self.ledger.load_app(&request.app_name)?;
        if app.shot_id.is_none() || app.builder_id.is_none() {
            return Err(EngineError::LegacyRequiresAdoption(
                request.app_name.clone(),
            ));
        }
        let builder = BuilderIdentityManager::for_ledger(&self.ledger).ensure()?;
        if app.builder_id != Some(builder.builder_id) {
            return Err(EngineError::BuilderMismatch(request.app_name.clone()));
        }
        let latest = self.ledger.latest_evolution(&request.app_name)?;
        // The builder's selected feedback binds the exact Version they
        // experienced. Prove the selection can survive BEFORE any recording
        // side effect: otherwise a drifted folder silently seals a surprise
        // Version and the feedback becomes permanently unselectable.
        if !request.selected_feedback_actions.is_empty() {
            let previous = latest.as_ref().ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete(
                    "feedback can be selected only for an evolution from an accepted Version"
                        .into(),
                )
            })?;
            if !self.working_tree_matches(previous)? {
                return Err(EngineError::ProtocolBodyIncomplete(format!(
                    "the folder changed after evolution {} was accepted, so the next Evolution would begin from a new Version while the selected feedback is bound to the one you experienced; record the folder first with `tohseno evolve {}`, attach feedback to the new Version, then retry",
                    previous.number, request.app_name
                )));
            }
            let layout = ShotLayout::at(self.ledger.working_tree(&request.app_name));
            let lineage = layout.read_lineage()?;
            let state =
                tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
            let expression_id = app.expression_id.ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete("the Shot has no stable ExpressionID".into())
            })?;
            let expression = state.expression(expression_id).ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete(
                    "the Shot has no declared Expression for feedback selection".into(),
                )
            })?;
            let current_version_id = expression.current_version.ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete(
                    "an evolutionary instruction requires an accepted source Version".into(),
                )
            })?;
            for action in &request.selected_feedback_actions {
                let feedback = state.feedback.get(action).ok_or_else(|| {
                    EngineError::ProtocolBodyIncomplete(format!(
                        "selected Feedback action {action} is unavailable"
                    ))
                })?;
                if feedback.expression_id != expression_id
                    || feedback.version_id != current_version_id
                {
                    return Err(EngineError::ProtocolBodyIncomplete(format!(
                        "selected Feedback action {action} is not bound to the current exact expression Version"
                    )));
                }
            }
        }
        let recorded = match &latest {
            Some(previous) => {
                protocol_lifecycle::verify_completed_evolution(previous)?;
                if self.working_tree_matches(previous)? {
                    None
                } else {
                    Some(
                        self.record_locked(&request.app_name, &app, &builder, None, None, false)
                            .await?,
                    )
                }
            }
            None if self.working_tree_has_content(&request.app_name)? => Some(
                self.record_locked(&request.app_name, &app, &builder, None, None, false)
                    .await?,
            ),
            None => return Err(EngineError::NoCompleteShot(request.app_name.clone())),
        };
        if request.intent.prompt.trim().is_empty() {
            return Ok(match recorded {
                Some(shot) => Evolved::Recorded(shot),
                None => {
                    let previous = latest.expect("clean tree implies a recorded evolution");
                    self.events.emit(Event::result(format!(
                        "nothing new — the folder already matches evolution {}.",
                        previous.number
                    )));
                    Evolved::NothingNew(previous)
                }
            });
        }
        let layout = ShotLayout::at(self.ledger.working_tree(&request.app_name));
        let lineage = layout.read_lineage()?;
        let state = tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
        let expression_id = app.expression_id.ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete("the Shot has no stable ExpressionID".into())
        })?;
        let expression = state.expression(expression_id).ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "the Shot has no declared Expression for feedback selection".into(),
            )
        })?;
        let current_version_id = expression.current_version.ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "an evolutionary instruction requires an accepted source Version".into(),
            )
        })?;
        let current_version = expression
            .versions
            .iter()
            .find(|version| version.version_id == current_version_id)
            .ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete(
                    "the current Version is unavailable in canonical lineage".into(),
                )
            })?;
        let mut selected_feedback_actions = request.selected_feedback_actions.clone();
        if selected_feedback_actions.len() > 256
            || selected_feedback_actions.contains(&Bytes32::ZERO)
        {
            return Err(EngineError::ProtocolBodyIncomplete(
                "feedback selection accepts at most 256 nonzero action commitments".into(),
            ));
        }
        selected_feedback_actions.sort_unstable();
        if selected_feedback_actions
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(EngineError::ProtocolBodyIncomplete(
                "feedback selection must not repeat an action commitment".into(),
            ));
        }
        for action in &selected_feedback_actions {
            let feedback = state.feedback.get(action).ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete(format!(
                    "selected Feedback action {action} is unavailable"
                ))
            })?;
            if feedback.expression_id != expression_id || feedback.version_id != current_version_id
            {
                return Err(EngineError::ProtocolBodyIncomplete(format!(
                    "selected Feedback action {action} is not bound to the current exact expression Version"
                )));
            }
        }

        // The exact instruction, selected signed Feedback actions, and exact
        // private reference descriptors wait together until a successful
        // accepted Version.
        let reference_sources = self.validated_reference_sources(&request.intent);
        let staged_references = layout.stage_evolution_inputs(
            request.intent.prompt.as_bytes(),
            &selected_feedback_actions,
            &reference_sources,
        )?;
        let accepted_genome = state.accepted_genome.as_ref().ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "an evolutionary instruction requires an accepted Genome".into(),
            )
        })?;
        let accepted_digest = accepted_genome
            .genome
            .digest()
            .map_err(ShotLayoutError::from)?;
        let genome_mutation = (accepted_digest != current_version.genome_digest).then(|| {
            let summary = state
                .genome_proposals
                .get(&accepted_genome.proposal_action)
                .map(|proposal| proposal.mutation_summary.as_slice())
                .unwrap_or(&[]);
            (accepted_genome.proposal_action, summary)
        });
        layout.write_pending_evolution_document(&render_pending_evolution_document(
            current_version,
            &selected_feedback_actions,
            &staged_references,
            &request.intent.prompt,
            &accepted_genome.genome.behavioral_invariants,
            genome_mutation,
        ))?;
        let instruction = format!(
            "Read AGENTS.md, .tohseno/TASK.md, and the staged evolutionary intention. The builder asks: {}\nReturn a complete candidate and independently inspectable experience evidence. Do not call tohseno evolve; the engine owns final acceptance and sealing.",
            request.intent.prompt.trim()
        );
        Ok(Evolved::Conducted(ConductedCreation {
            folder: self.ledger.working_tree(&request.app_name),
            agent_command: self.preferred_agent_command(),
            instruction,
            phase: ConductionPhase::EvolutionMaterialization,
        }))
    }

    /// Adopts an existing plain folder as a Shot: it gains its `.tohseno/`
    /// ledger, its standing orders, and its first recorded Evolution —
    /// without changing a byte of the app itself before the record.
    pub async fn adopt(&self, app_name: &str) -> Result<Evolution, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        // Adoption must fail before any side effect: the folder needs an
        // Xcode project named after itself and the fascia anatomy in place.
        let working = self.ledger.working_tree(app_name);
        if !working.join(format!("{app_name}.xcodeproj")).is_dir() {
            return Err(EngineError::NotAdoptable(app_name.into()));
        }
        if !working.join("TohsenoFascia").is_dir() {
            return Err(EngineError::NotAdoptable(app_name.into()));
        }
        self.events
            .emit(Event::status("preparing your TOHSENO identity…"));
        let builder = BuilderIdentityManager::for_ledger(&self.ledger).ensure()?;
        let proposed_bundle_id = bundle_id(app_name)?;
        let mut app = match self.ledger.load_app(app_name) {
            Ok(existing) => existing,
            Err(LedgerError::AppMissing(_)) => {
                self.ledger.adopt_app(app_name, &proposed_bundle_id)?
            }
            Err(error) => return Err(error.into()),
        };
        if app.shot_id.is_none() && app.builder_id.is_none() {
            app = self.ledger.bind_protocol_identity(
                app_name,
                tohseno_protocol::digest::ShotId::random(),
                builder.builder_id,
            )?;
        }
        if app.builder_id != Some(builder.builder_id) {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        self.genome
            .write_standing_orders(&self.ledger.working_tree(app_name), app_name)?;
        // A latest evolution that no longer verifies (a stranded pre-repin
        // world, or an unsigned past) is honest legacy: the adoption records
        // a fresh root that names it without inventing history for it.
        let origin = match self.ledger.latest_evolution(app_name)? {
            Some(previous)
                if protocol_lifecycle::verify_completed_evolution(&previous).is_err() =>
            {
                let legacy_source_sha256 =
                    tohseno_protocol::tree_hash::hash_source_tree(&previous.source_path())
                        .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?
                        .digest;
                Some(ShotOrigin::LegacyAdoption {
                    legacy_latest_shot: previous.number,
                    legacy_source_sha256,
                })
            }
            _ => None,
        };
        self.record_locked(
            app_name,
            &app,
            &builder,
            Some("adopted: this folder becomes a Shot without changing its purpose."),
            origin,
            false,
        )
        .await
    }

    fn validated_reference_sources(&self, intent: &Intent) -> Vec<PathBuf> {
        debug_assert!(intent.images.len() <= crate::gates::intent::MAX_IMAGES);
        intent.images.clone()
    }

    fn copy_initial_intention_images(
        &self,
        layout: &ShotLayout,
        shot: &Evolution,
    ) -> Result<(), EngineError> {
        let lineage = layout.read_lineage()?;
        let state = tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
        let intention = state.intention.as_ref().ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "the first materialization has no canonical source intention".into(),
            )
        })?;
        let images = intention
            .materials
            .iter()
            .filter(|material| {
                material.inline_text.is_none()
                    && material.artifact.artifact.media_type.starts_with("image/")
            })
            .collect::<Vec<_>>();
        if images.len() > crate::gates::intent::MAX_IMAGES {
            return Err(EngineError::ProtocolBodyIncomplete(
                "the source intention exceeds the Apple factory's 8-image surface".into(),
            ));
        }
        let mut names = std::collections::BTreeSet::new();
        for material in images {
            let name = material.artifact.artifact.name.as_deref().ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete(
                    "an intention image is missing its safe original name".into(),
                )
            })?;
            if !names.insert(name.to_ascii_lowercase()) {
                return Err(EngineError::ProtocolBodyIncomplete(
                    "intention image names collide on Apple filesystems".into(),
                ));
            }
            let bytes = layout.read_private_reference(&material.artifact)?;
            self.ledger
                .write_evolution_file(shot, Path::new("images").join(name), &bytes)?;
        }
        Ok(())
    }

    /// Compatibility handoff for callers that have not adopted prepared
    /// executions yet. Native execution uses the adapter's argument vector.
    fn preferred_agent_command(&self) -> Option<String> {
        let selection = default_selection(&self.config.harness)?;
        let (_, command) = resolve_selection(&selection).ok()?;
        Some(command.program.to_string_lossy().into_owned())
    }

    /// Resolve the exact authorized lineage head that a candidate
    /// materialization consumes. A later version first records (or reuses
    /// after a failed attempt) its private Evolutionary Intent. Nothing in
    /// this method accepts a Version.
    fn prepare_materialization_lineage_input(
        &self,
        shot: &Evolution,
        app: &AppRecord,
        builder: &BuilderIdentity,
    ) -> Result<MaterializationLineageInput, EngineError> {
        let shot_id = app
            .shot_id
            .ok_or_else(|| EngineError::LegacyRequiresAdoption(shot.app_name.clone()))?;
        let expression_id = app.expression_id.ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "the Shot has no stable ExpressionID for this materialization".into(),
            )
        })?;
        let layout = ShotLayout::at(self.ledger.working_tree(&shot.app_name));
        let manager = BuilderIdentityManager::for_ledger(&self.ledger);
        let mut lineage = layout.read_lineage()?;
        let mut state =
            tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
        if state.shot_id != shot_id
            || state.controller != builder.builder_id
            || state.controller_key != builder.device.public_key
        {
            return Err(EngineError::BuilderMismatch(shot.app_name.clone()));
        }

        let accepted = state.accepted_genome.clone().ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "materialization requires an explicitly accepted Genome".into(),
            )
        })?;
        let expression = state.expression(expression_id).cloned().ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "materialization requires the reviewed Expression declaration".into(),
            )
        })?;
        let version_ordinal = u64::try_from(expression.versions.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete("expression version sequence overflowed".into())
            })?;
        if version_ordinal != u64::from(shot.number) {
            return Err(EngineError::ProtocolBodyIncomplete(format!(
                "Apple bundle evolution {} cannot become expression version {version_ordinal:04}; migrate the legacy lineage before recording",
                shot.number
            )));
        }
        let genome_digest = accepted.genome.digest().map_err(ShotLayoutError::from)?;
        let genome_revision = accepted.genome.revision;
        if version_ordinal == 1
            && (expression.expression.genome_revision != accepted.genome.revision
                || expression.expression.genome_digest != genome_digest)
        {
            return Err(EngineError::ProtocolBodyIncomplete(
                "the first materialization no longer matches its reviewed Expression plan; declare a compatible expression before building".into(),
            ));
        }

        let mut evolutionary_intent_action = None;
        let mut preserved_for_evolution = accepted.genome.behavioral_invariants.clone();
        if version_ordinal > 1 {
            let from_version = expression.versions.last().ok_or_else(|| {
                EngineError::ProtocolBodyIncomplete(
                    "a later version has no accepted source Version".into(),
                )
            })?;
            if expression.current_version != Some(from_version.version_id) {
                return Err(EngineError::ProtocolBodyIncomplete(
                    "the expression head does not identify its last accepted Version".into(),
                ));
            }

            let mut preserved_invariants = accepted.genome.behavioral_invariants.clone();
            let genome_changed = from_version.genome_digest != genome_digest;
            if genome_changed {
                let previous_genome = state
                    .genome_proposals
                    .values()
                    .map(|proposal| &proposal.proposed)
                    .find(|genome| {
                        genome.revision == from_version.genome_revision
                            && genome.digest().ok() == Some(from_version.genome_digest)
                    })
                    .ok_or_else(|| {
                        EngineError::ProtocolBodyIncomplete(
                            "the source Version's accepted Genome is unavailable".into(),
                        )
                    })?;
                preserved_invariants
                    .retain(|invariant| previous_genome.behavioral_invariants.contains(invariant));
                if preserved_invariants.is_empty() {
                    return Err(EngineError::ProtocolBodyIncomplete(
                        "a Genome mutation must preserve at least one explicit behavioral invariant"
                            .into(),
                    ));
                }
            }

            let prompt = fs::read_to_string(shot.prompt_path())?;
            let description = intention_excerpt(&prompt, 4000);
            if description.is_empty() {
                return Err(EngineError::ProtocolBodyIncomplete(
                    "an Evolutionary Intent requires a nonempty human instruction".into(),
                ));
            }
            let (selected_feedback_actions, selected_references) =
                layout.pending_evolution_inputs(prompt.as_bytes())?;
            let mut desired_changes = vec![DesiredChange {
                scope: ChangeScope::Expression,
                description,
            }];
            let current_capability_graph = expression.organs.values().cloned().collect::<Vec<_>>();
            let current_capability_graph_digest =
                capability_graph_digest(&current_capability_graph)
                    .map_err(ShotLayoutError::from)?;
            if from_version.capability_graph_digest != current_capability_graph_digest {
                desired_changes.push(DesiredChange {
                    scope: ChangeScope::Organ,
                    description: "Adopt the exact newly declared Organ graph for this Expression."
                        .into(),
                });
            }
            let proposed_genome_action = if genome_changed {
                let proposal = state
                    .genome_proposals
                    .get(&accepted.proposal_action)
                    .ok_or_else(|| {
                        EngineError::ProtocolBodyIncomplete(
                            "the accepted Genome proposal is unavailable".into(),
                        )
                    })?;
                desired_changes.push(DesiredChange {
                    scope: ChangeScope::Genome,
                    description: if proposal.mutation_summary.is_empty() {
                        format!(
                            "Adopt explicitly accepted Genome revision {}.",
                            accepted.genome.revision
                        )
                    } else {
                        proposal.mutation_summary.join("; ")
                    },
                });
                Some(accepted.proposal_action)
            } else {
                None
            };
            let intent = EvolutionaryIntent {
                schema: EVOLUTIONARY_INTENT_SCHEMA.into(),
                expression_id,
                from_version_id: from_version.version_id,
                preserved_invariants: preserved_invariants.clone(),
                desired_changes,
                feedback_actions: selected_feedback_actions,
                references: selected_references,
                proposed_genome_action,
            };
            intent.validate().map_err(ShotLayoutError::from)?;
            preserved_for_evolution = preserved_invariants;
            let used_intents = state
                .evolutions
                .iter()
                .map(|evolution| evolution.record.evolutionary_intent_action)
                .collect::<std::collections::BTreeSet<_>>();
            for action in lineage.iter().rev() {
                if let LineagePayload::EvolutionaryIntent(existing) = &action.action.payload {
                    let commitment = action.commitment().map_err(ShotLayoutError::from)?;
                    if existing == &intent && !used_intents.contains(&commitment) {
                        evolutionary_intent_action = Some(commitment);
                        break;
                    }
                }
            }
            if evolutionary_intent_action.is_none() {
                let timestamp = canonical_now_at_least(&state.last_timestamp)?;
                let action = LineageAction::new(
                    state.sequence.checked_add(1).ok_or_else(|| {
                        EngineError::ProtocolBodyIncomplete("lineage sequence overflowed".into())
                    })?,
                    Some(state.head),
                    shot_id,
                    builder.builder_id,
                    timestamp,
                    AvailabilityStatus::IntentionallyPrivate,
                    LineagePayload::EvolutionaryIntent(intent),
                )
                .map_err(ShotLayoutError::from)?;
                let signed = sign_lineage_action(&manager, builder, action)?;
                evolutionary_intent_action =
                    Some(signed.commitment().map_err(ShotLayoutError::from)?);
                layout.append_lineage_batch(&[signed])?;
                lineage = layout.read_lineage()?;
                state =
                    tohseno_protocol::reduce_lineage(&lineage).map_err(ShotLayoutError::from)?;
            }
        }

        let accepted = state.accepted_genome.as_ref().ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "the accepted Genome disappeared before materialization".into(),
            )
        })?;
        let expression = state.expression(expression_id).ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete(
                "the Expression disappeared before materialization".into(),
            )
        })?;
        let current_genome_digest = accepted.genome.digest().map_err(ShotLayoutError::from)?;
        if accepted.genome.revision != genome_revision
            || current_genome_digest != genome_digest
            || u64::try_from(expression.versions.len())
                .ok()
                .and_then(|value| value.checked_add(1))
                != Some(version_ordinal)
        {
            return Err(EngineError::ProtocolBodyIncomplete(
                "authorized materialization inputs changed while they were being resolved".into(),
            ));
        }
        let from_version = expression.versions.last().cloned();
        let capability_graph = expression.organs.values().cloned().collect::<Vec<_>>();
        let capability_graph_digest =
            capability_graph_digest(&capability_graph).map_err(ShotLayoutError::from)?;
        let genome_acceptance_action = from_version
            .as_ref()
            .filter(|version| version.genome_digest != current_genome_digest)
            .map(|_| accepted.acceptance_action);
        Ok(MaterializationLineageInput {
            shot_id,
            expression_id,
            version_ordinal,
            genome_revision: accepted.genome.revision,
            genome_digest: current_genome_digest,
            lineage_sequence: state.sequence,
            lineage_head: state.head,
            last_timestamp: state.last_timestamp.clone(),
            template_digest: expression.expression.definition.artifact.digest,
            capability_graph,
            capability_graph_digest,
            from_version,
            evolutionary_intent_action,
            genome_acceptance_action,
            preserved_invariants: preserved_for_evolution,
        })
    }

    fn load_birth_context(&self, app_name: &str) -> Result<BirthContext, EngineError> {
        let root = self.ledger.working_tree(app_name);
        let layout = ShotLayout::at(&root);
        let plan: crate::birth_plan::BirthPlan =
            read_private_planning_json(&layout, crate::conception::BIRTH_PLAN_FILE)?;
        let expression: BirthExpressionPlan =
            read_private_planning_json(&layout, "birth-expression-plan.json")?;
        let contract: ExperienceContract =
            read_private_planning_json(&layout, crate::conception::EXPERIENCE_CONTRACT_FILE)?;
        let trial: ExperienceTrial =
            read_private_planning_json(&layout, crate::conception::EXPERIENCE_TRIAL_FILE)?;
        let factory_identity: crate::factory_identity::FactoryIdentity =
            read_private_planning_json(&layout, "materialization-factory-identity.json")?;
        plan.validate()?;
        expression.validate(&plan.genome)?;
        contract.validate(&plan)?;
        trial.validate(&plan, &expression, &contract)?;
        let current_factory = crate::factory_identity::FactoryIdentity::current(
            Some(plan.genome.digest().map_err(ShotLayoutError::from)?),
            factory_identity.apple_capability_profile_digest,
        );
        if current_factory != factory_identity {
            return Err(EngineError::ProtocolBodyIncomplete(format!(
                "factory identity changed after conception: task used engine {} at commit {} with Constitution digest {}, but this engine is {} at commit {} with digest {}; regenerate the materialization task before acceptance",
                factory_identity.engine_version,
                factory_identity.source_commit,
                factory_identity.static_constitution_digest,
                current_factory.engine_version,
                current_factory.source_commit,
                current_factory.static_constitution_digest,
            )));
        }
        validate_trial_evidence_files(&root, &trial)?;
        Ok(BirthContext {
            plan,
            expression,
            contract,
            trial,
            factory_identity,
        })
    }

    /// The shared recording path for a candidate Version. Initial birth adds
    /// independent intent and experience acceptance, including a required
    /// physical-device build and trial for hardware-critical completion
    /// contracts. Later Evolutions retain the historical recording semantics.
    #[allow(clippy::too_many_arguments)]
    async fn finish_evolution(
        &self,
        shot: &Evolution,
        app: &AppRecord,
        builder: &BuilderIdentity,
        genesis_input_sha256: tohseno_protocol::digest::Bytes32,
        origin: Option<ShotOrigin>,
        app_name: &str,
        bundle_id: &str,
        working_digest_at_start: Option<tohseno_protocol::digest::Bytes32>,
        require_device_delivery: bool,
    ) -> Result<Evolution, EngineError> {
        let requires_birth_acceptance = shot.number == 1 && origin.is_none();
        self.events.emit(Event::status(format!(
            "committing evolution {}…",
            shot.number
        )));
        let lineage_input = self.prepare_materialization_lineage_input(shot, app, builder)?;
        let mut prepared = protocol_lifecycle::prepare_evolution(
            &self.ledger,
            shot,
            app,
            builder,
            genesis_input_sha256,
            origin,
        )?;
        let candidate_version_id = VersionId::derive(
            lineage_input.shot_id,
            lineage_input.expression_id,
            lineage_input.version_ordinal,
            lineage_input.genome_digest,
            prepared.record.source_tree_sha256,
        );
        let embedded_metadata = protocol_lifecycle::bind_v2_app_metadata(
            &self.ledger,
            shot,
            &mut prepared,
            lineage_input.expression_id,
            candidate_version_id,
            lineage_input.version_ordinal,
            lineage_input.genome_revision,
            lineage_input.genome_digest,
            lineage_input.lineage_sequence,
            lineage_input.lineage_head,
            None,
        )?;

        self.events.emit(Event::status(format!(
            "materializing evolution {}…",
            shot.number
        )));
        let artifact = match build::materialize_artifact(&self.ledger, shot, app.target_name())? {
            Ok(artifact) => artifact,
            Err(failure) => return Err(EngineError::ArtifactUnbuildable(failure.output)),
        };
        self.events.emit(Event::status(format!(
            "looking at evolution {}…",
            shot.number
        )));
        if let Err(reason) = preview::capture(&artifact, bundle_id, &shot.path.join("preview.png"))
        {
            self.events
                .emit(Event::status(preview::failure_diagnostic(&reason)));
        }
        let birth_context = requires_birth_acceptance
            .then(|| self.load_birth_context(app_name))
            .transpose()?;
        let physical_was_required = birth_context.as_ref().is_some_and(|context| {
            !context
                .plan
                .completion_contract
                .physical_verification_capabilities
                .is_empty()
        });
        let mut engine_experience_criteria = Vec::new();
        if birth_context.is_some() {
            self.events.emit(Event::status(
                "running the birth test suite independently in Release on Simulator…",
            ));
            let simulator_udid = preview::ensure_iphone_simulator().map_err(|reason| {
                EngineError::ProtocolBodyIncomplete(format!(
                    "experience_verification_gap · external_environment_constraint · acceptance_pending_simulator_environment · {reason}"
                ))
            })?;
            if let Err(failure) =
                build::test_simulator(&self.ledger, shot, app.target_name(), &simulator_udid)?
            {
                let tail = failure
                    .output
                    .lines()
                    .filter(|line| {
                        line.contains("error:")
                            || line.contains("failed")
                            || line.contains("Test Suite")
                    })
                    .rev()
                    .take(12)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(EngineError::ProtocolBodyIncomplete(format!(
                    "experience_verification_gap · engine Simulator tests failed; repair the app or its target-user tests and rerun; evidence=.tohseno/evolutions/{:04}/test.log\n{tail}",
                    shot.number
                )));
            }
            let test_log = shot.path.join("test.log");
            engine_experience_criteria.push(CriterionResult {
                id: "engine_simulator_test_execution".into(),
                passed: true,
                deterministic: true,
                evidence: vec![evidence_reference_for_file(
                    &self.ledger.working_tree(app_name),
                    &test_log,
                    EvidenceKind::XcuiTest,
                    "text/plain",
                )?],
                observation: Some(
                    "the engine independently reran the checked-in Release XCTest/XCUITest action"
                        .into(),
                ),
            });
        }
        self.events.emit(Event::status(format!(
            "verifying evolution {}…",
            shot.number
        )));
        let completed =
            protocol_lifecycle::complete_evolution(&self.ledger, shot, builder, prepared)?;
        if completed.app_metadata_v2.as_ref() != Some(&embedded_metadata) {
            return Err(EngineError::ProtocolBodyIncomplete(
                "the verified artifact did not retain its exact v2 expression identity".into(),
            ));
        }

        if let Some(context) = &birth_context {
            if !context
                .plan
                .completion_contract
                .physical_verification_capabilities
                .is_empty()
            {
                let device = if require_device_delivery {
                    DevicePipeline::new(self.events.clone())
                        .wait_for_device()
                        .await?
                } else {
                    match device::check()? {
                        DeviceState::Ready(device) => device,
                        state => {
                            return Err(EngineError::ProtocolBodyIncomplete(format!(
                                "physical_device_experience · acceptance_pending_physical_experience · {state:?}"
                            )));
                        }
                    }
                };
                let declared_device = context.trial.physical_device.as_ref().ok_or_else(|| {
                    EngineError::ProtocolBodyIncomplete(
                        "physical_device_experience · acceptance_pending_physical_experience · the trial contains no physical-device evidence"
                            .into(),
                    )
                })?;
                let detected_product_type = device.product_type.as_deref().ok_or_else(|| {
                    EngineError::ProtocolBodyIncomplete(
                        "physical_device_experience · acceptance_pending_physical_experience · devicectl did not expose a product type"
                            .into(),
                    )
                })?;
                let detected_os = device.os_version.as_deref().ok_or_else(|| {
                    EngineError::ProtocolBodyIncomplete(
                        "physical_device_experience · acceptance_pending_physical_experience · devicectl did not expose an OS version"
                            .into(),
                    )
                })?;
                if detected_product_type != declared_device.product_type
                    || detected_os != declared_device.os_version
                    || declared_device
                        .os_build
                        .as_deref()
                        .is_some_and(|build| device.os_build.as_deref() != Some(build))
                {
                    return Err(EngineError::ProtocolBodyIncomplete(format!(
                        "physical_device_experience · acceptance_pending_physical_experience · current sanitized device facts ({detected_product_type}, iOS {detected_os}) do not match the target-user trial evidence ({}, iOS {})",
                        declared_device.product_type, declared_device.os_version
                    )));
                }
                let artifact_directory = temporary_path("birth-device-verification");
                DevicePipeline::new(self.events.clone())
                    .build_install(
                        shot.number,
                        app.target_name(),
                        bundle_id,
                        &shot.source_path(),
                        &artifact_directory,
                    )
                    .await?;
                let evidence_value = serde_json::json!({
                    "schema": "tohseno.engine-physical-verification/1",
                    "source_digest": completed.record.source_tree_sha256,
                    "product_type": detected_product_type,
                    "os_version": detected_os,
                    "os_build": device.os_build,
                    "transport": device.transport,
                    "exercised_capabilities": context
                        .plan
                        .completion_contract
                        .physical_verification_capabilities,
                    "build_install_launch_passed": true
                });
                let mut evidence_bytes =
                    serde_json::to_vec_pretty(&evidence_value).map_err(|error| {
                        EngineError::ProtocolBodyIncomplete(format!(
                            "physical verification evidence encoding failed: {error}"
                        ))
                    })?;
                evidence_bytes.push(b'\n');
                self.ledger.write_evolution_file(
                    shot,
                    "physical-verification.json",
                    &evidence_bytes,
                )?;
                engine_experience_criteria.push(CriterionResult {
                    id: "engine_physical_build_install_launch".into(),
                    passed: true,
                    deterministic: true,
                    evidence: vec![evidence_reference_for_file(
                        &self.ledger.working_tree(app_name),
                        &shot.path.join("physical-verification.json"),
                        EvidenceKind::PhysicalDeviceTrial,
                        "application/json",
                    )?],
                    observation: Some(
                        "the engine rebuilt, installed, and launched the exact candidate snapshot on the device whose sanitized facts match the harness trial"
                            .into(),
                    ),
                });
            }
        }

        let conformance_path = shot.path.join("TOHSENO/conformance.json");
        let conformance_evidence = evidence_reference_for_file(
            &self.ledger.working_tree(app_name),
            &conformance_path,
            EvidenceKind::Log,
            "application/json",
        )?;
        let protocol_criteria = completed
            .conformance
            .checks
            .iter()
            .map(|check| CriterionResult {
                id: format!("protocol.{}", check.id),
                passed: check.status == CheckStatus::Pass,
                deterministic: true,
                evidence: (check.status == CheckStatus::Pass)
                    .then(|| conformance_evidence.clone())
                    .into_iter()
                    .collect(),
                observation: Some(format!(
                    "expected {}; observed {}",
                    check.expected, check.observed
                )),
            })
            .collect::<Vec<_>>();
        let birth_receipt = if let Some(context) = &birth_context {
            let receipt = evaluate_birth(
                &context.plan,
                &context.expression,
                &context.contract,
                &context.trial,
                BirthEvaluationEvidence {
                    source_digest: completed.record.source_tree_sha256,
                    factory_identity: context.factory_identity.clone(),
                    protocol_criteria,
                    engine_experience_criteria,
                },
            )?;
            let bytes = tohseno_protocol::canonical::to_vec(&receipt)
                .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
            self.ledger
                .write_evolution_file(shot, "TOHSENO/birth-receipt.json", &bytes)?;
            if !receipt.accepted {
                return Err(EngineError::ProtocolBodyIncomplete(format!(
                    "birth candidate remains unsealed: protocol_conformance={}, intent_fidelity={}, experience_verification={}; repair the failed independent criteria and rerun",
                    receipt.protocol_conformance.passed,
                    receipt.intent_fidelity.passed,
                    receipt.experience_verification.passed,
                )));
            }
            Some(receipt)
        } else {
            None
        };

        let mut gates = completed
            .conformance
            .checks
            .iter()
            .map(|check| VerificationGate {
                name: check.id.clone(),
                passed: check.status == CheckStatus::Pass,
                deterministic: true,
                evidence: Some(conformance_evidence.availability()),
            })
            .collect::<Vec<_>>();
        let capability_graph_bytes =
            canonical_capability_graph_bytes(&lineage_input.capability_graph)
                .map_err(ShotLayoutError::from)?;
        let capability_graph_evidence = ArtifactAvailability {
            schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
            artifact: ArtifactDescriptor {
                digest: lineage_input.capability_graph_digest,
                media_type: "application/vnd.tohseno.capability-graph+json".into(),
                byte_length: u64::try_from(capability_graph_bytes.len()).map_err(|_| {
                    EngineError::ProtocolBodyIncomplete("capability graph length overflowed".into())
                })?,
                name: Some("capabilities.lock".into()),
            },
            status: AvailabilityStatus::LocallyAvailable,
            locations: Vec::new(),
        };
        for organ in &lineage_input.capability_graph {
            for index in 0..organ.acceptance_tests.len() {
                let independent = birth_context.as_ref().and_then(|context| {
                    let planned = context
                        .expression
                        .organs
                        .iter()
                        .find(|planned| planned.organ_id == organ.organ_id)?;
                    let criterion = planned.acceptance_criteria.get(index)?;
                    context
                        .trial
                        .organ_results
                        .iter()
                        .find(|result| result.organ_id == organ.organ_id)?
                        .criteria
                        .iter()
                        .find(|result| result.id == criterion.id)
                });
                gates.push(VerificationGate {
                    name: organ_acceptance_gate_name(organ, index)
                        .map_err(ShotLayoutError::from)?,
                    passed: independent
                        .map(|criterion| criterion.passed)
                        .unwrap_or(completed.conformance.conformant),
                    deterministic: independent
                        .map(|criterion| criterion.deterministic)
                        .unwrap_or(true),
                    evidence: independent
                        .and_then(|criterion| criterion.evidence.first())
                        .map(EvidenceReference::availability)
                        .or_else(|| Some(capability_graph_evidence.clone())),
                });
            }
        }
        let receipt_evidence = birth_receipt
            .as_ref()
            .map(birth_receipt_availability)
            .transpose()?;
        if let (Some(receipt), Some(evidence)) = (&birth_receipt, &receipt_evidence) {
            gates.extend([
                VerificationGate {
                    name: "acceptance.protocol_conformance".into(),
                    passed: receipt.protocol_conformance.passed,
                    deterministic: true,
                    evidence: Some(evidence.clone()),
                },
                VerificationGate {
                    name: "acceptance.intent_fidelity".into(),
                    passed: receipt.intent_fidelity.passed,
                    deterministic: false,
                    evidence: Some(evidence.clone()),
                },
                VerificationGate {
                    name: "acceptance.experience_verification".into(),
                    passed: receipt.experience_verification.passed,
                    deterministic: false,
                    evidence: Some(evidence.clone()),
                },
            ]);
        }
        let known_incompleteness = birth_receipt
            .as_ref()
            .into_iter()
            .flat_map(|receipt| receipt.incompleteness.iter())
            .filter(|gap| {
                gap.category == IncompletenessCategory::ExternalEnvironmentConstraint
                    && !gap.blocks_completion
            })
            .map(|gap| {
                format!(
                    "external_environment_constraint:{}:{}",
                    gap.id, gap.description
                )
            })
            .collect::<Vec<_>>();
        let verification_passed = gates.iter().all(|gate| gate.passed);
        if require_device_delivery && !verification_passed {
            let failed = gates
                .iter()
                .filter(|gate| !gate.passed)
                .map(|gate| gate.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(EngineError::ProtocolBodyIncomplete(format!(
                "candidate verification failed before device delivery: {failed}"
            )));
        }

        // A one-shot factory run includes delivery. After every non-device
        // gate passes, install and launch the exact candidate before signing
        // any acceptance action. Hardware-critical births already performed
        // this step above because build/install/launch is itself evidence.
        if require_device_delivery && !physical_was_required {
            let artifact_directory = temporary_path("shot-delivery");
            DevicePipeline::new(self.events.clone())
                .build_install(
                    shot.number,
                    app.target_name(),
                    bundle_id,
                    &shot.source_path(),
                    &artifact_directory,
                )
                .await?;
        }

        let manager = BuilderIdentityManager::for_ledger(&self.ledger);
        let accepted_at = canonical_now_at_least(&lineage_input.last_timestamp)?;
        let verification = VerificationResult {
            schema: VERIFICATION_RESULT_SCHEMA.into(),
            expression_id: lineage_input.expression_id,
            candidate_version_id,
            genome_revision: lineage_input.genome_revision,
            genome_digest: lineage_input.genome_digest,
            source_digest: completed.record.source_tree_sha256,
            capability_graph_digest: lineage_input.capability_graph_digest,
            gates,
            passed: verification_passed,
            known_incompleteness: known_incompleteness.clone(),
            verified_at: accepted_at.clone(),
        };
        verification.validate().map_err(ShotLayoutError::from)?;
        let verification_action = LineageAction::new(
            lineage_input
                .lineage_sequence
                .checked_add(1)
                .ok_or_else(|| {
                    EngineError::ProtocolBodyIncomplete("lineage sequence overflowed".into())
                })?,
            Some(lineage_input.lineage_head),
            lineage_input.shot_id,
            builder.builder_id,
            accepted_at.clone(),
            AvailabilityStatus::IntentionallyPrivate,
            LineagePayload::VerificationResult(verification),
        )
        .map_err(ShotLayoutError::from)?;
        let signed_verification = sign_lineage_action(&manager, builder, verification_action)?;
        let verification_commitment = signed_verification
            .commitment()
            .map_err(ShotLayoutError::from)?;
        let version = VersionRecord {
            schema: VERSION_SCHEMA.into(),
            version_id: candidate_version_id,
            expression_id: lineage_input.expression_id,
            ordinal: lineage_input.version_ordinal,
            genome_revision: lineage_input.genome_revision,
            genome_digest: lineage_input.genome_digest,
            source_digest: completed.record.source_tree_sha256,
            provenance: MaterializationProvenance {
                factory: completed.record.factory.implementation.clone(),
                factory_version: completed.record.factory.version.clone(),
                factory_source_commit: Some(completed.record.factory.source_commit.clone()),
                template_digest: lineage_input.template_digest,
                input_action: lineage_input.lineage_head,
                deterministic: false,
            },
            capability_graph_digest: lineage_input.capability_graph_digest,
            verification_action: verification_commitment,
            known_incompleteness,
            build_identity: Some(format!("{bundle_id}:{}", shot.number)),
            build_digest: None,
            accepted_at: accepted_at.clone(),
        };
        version
            .validate(lineage_input.shot_id)
            .map_err(ShotLayoutError::from)?;
        let version_action = LineageAction::new(
            lineage_input
                .lineage_sequence
                .checked_add(2)
                .ok_or_else(|| {
                    EngineError::ProtocolBodyIncomplete("lineage sequence overflowed".into())
                })?,
            Some(verification_commitment),
            lineage_input.shot_id,
            builder.builder_id,
            accepted_at.clone(),
            AvailabilityStatus::IntentionallyPrivate,
            LineagePayload::Version(version),
        )
        .map_err(ShotLayoutError::from)?;
        let signed_version = sign_lineage_action(&manager, builder, version_action)?;
        let version_commitment = signed_version.commitment().map_err(ShotLayoutError::from)?;

        let signed_evolution = match (
            &lineage_input.from_version,
            lineage_input.evolutionary_intent_action,
        ) {
            (None, None) => None,
            (Some(from), Some(intent_action)) => {
                let evolution = ProtocolEvolution {
                    schema: EVOLUTION_SCHEMA.into(),
                    evolutionary_intent_action: intent_action,
                    expression_id: lineage_input.expression_id,
                    from_version_id: from.version_id,
                    to_version_id: candidate_version_id,
                    from_genome_digest: from.genome_digest,
                    to_genome_digest: lineage_input.genome_digest,
                    genome_acceptance_action: lineage_input.genome_acceptance_action,
                    preserved_invariants: lineage_input.preserved_invariants.clone(),
                    completed_at: accepted_at.clone(),
                };
                evolution.validate().map_err(ShotLayoutError::from)?;
                let action = LineageAction::new(
                    lineage_input
                        .lineage_sequence
                        .checked_add(3)
                        .ok_or_else(|| {
                            EngineError::ProtocolBodyIncomplete(
                                "lineage sequence overflowed".into(),
                            )
                        })?,
                    Some(version_commitment),
                    lineage_input.shot_id,
                    builder.builder_id,
                    accepted_at,
                    AvailabilityStatus::IntentionallyPrivate,
                    LineagePayload::Evolution(evolution),
                )
                .map_err(ShotLayoutError::from)?;
                Some(sign_lineage_action(&manager, builder, action)?)
            }
            _ => {
                return Err(EngineError::ProtocolBodyIncomplete(
                    "a later Version and its Evolutionary Intent must be present together".into(),
                ))
            }
        };
        let layout = ShotLayout::at(self.ledger.working_tree(app_name));
        layout.record_accepted_materialization(
            &embedded_metadata,
            &signed_verification,
            &signed_version,
            signed_evolution.as_ref(),
        )?;
        if let Some(receipt) = &birth_receipt {
            layout.write_metadata_json("birth-receipt.json", receipt, false)?;
        }
        self.ledger.finalize_evolution(shot)?;
        protocol_lifecycle::verify_completed_evolution(shot)?;
        self.ledger.set_retired(app_name, false)?;
        if self.working_digest(app_name) == working_digest_at_start {
            self.ledger.checkout_working_tree(shot)?;
        } else {
            self.events.emit(Event::status(
                "the folder changed while recording; your newer edits stay in place for the next evolution.",
            ));
        }
        if let (Some(context), Some(receipt)) = (&birth_context, &birth_receipt) {
            let target_users = context
                .plan
                .target_users
                .iter()
                .map(|actor| actor.role.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let journeys = context
                .plan
                .completion_contract
                .required_scenario_ids
                .join(", ");
            let capabilities = context
                .plan
                .capabilities
                .iter()
                .filter(|capability| capability.primary)
                .map(|capability| capability.identifier.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let physical = if context
                .plan
                .completion_contract
                .physical_verification_capabilities
                .is_empty()
            {
                "not required"
            } else {
                "passed"
            };
            self.events.emit(Event::result(format!(
                "Birth accepted: {app_name}\nTarget users: {target_users}\nProduct promise: {}\nPrimary journeys verified: {journeys}\nNative capabilities exercised: {capabilities}\nPhysical device verification: {physical}\nProtocol conformance: {}\nIntent fidelity: {}\nExperience verification: {}\nKnown product incompleteness: none",
                context.plan.promise,
                if receipt.protocol_conformance.passed { "passed" } else { "failed" },
                if receipt.intent_fidelity.passed { "passed" } else { "failed" },
                if receipt.experience_verification.passed { "passed" } else { "failed" },
            )));
        } else {
            self.events.emit(Event::result(format!(
                "Version {} of {} was recorded after its declared verification gates passed; this result alone does not claim renewed intent or target-user review.",
                shot.number, app_name
            )));
        }
        self.events.emit(Event::status(format!(
            "folder: {}",
            self.ledger.working_tree(app_name).display()
        )));

        if require_device_delivery {
            self.events.emit(Event::result(format!(
                "Version {} of {} is installed and running on your iPhone.",
                shot.number, app_name
            )));
        } else {
            match device::check() {
                Ok(DeviceState::Ready(_)) if physical_was_required => {
                    self.events.emit(Event::result(format!(
                        "accepted birth of {app_name} is on the verified iPhone."
                    )));
                }
                Ok(DeviceState::Ready(_)) => {
                    let artifact_directory = temporary_path("install");
                    DevicePipeline::new(self.events.clone())
                        .build_install(
                            shot.number,
                            app.target_name(),
                            bundle_id,
                            &shot.source_path(),
                            &artifact_directory,
                        )
                        .await?;
                    self.events.emit(Event::result(format!(
                        "Version {} of {} is on your phone.",
                        shot.number, app_name
                    )));
                }
                _ => {
                    self.events.emit(Event::handoff(format!(
                        "Plug in your iPhone anytime, then run `tohseno refresh {app_name}`.",
                    )));
                }
            }
        }
        Ok(shot.clone())
    }

    /// Records the working tree — however it got there — as the next
    /// Evolution of this one Shot. Editing is never a tohseno operation;
    /// recording is.
    pub async fn record(
        &self,
        app_name: &str,
        note: Option<&str>,
    ) -> Result<Evolution, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        if app.shot_id.is_none() || app.builder_id.is_none() {
            return Err(EngineError::LegacyRequiresAdoption(app_name.into()));
        }
        let builder = BuilderIdentityManager::for_ledger(&self.ledger).ensure()?;
        if app.builder_id != Some(builder.builder_id) {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        self.record_locked(app_name, &app, &builder, note, None, false)
            .await
    }

    /// Records, installs, and launches one exact candidate as a single Shot.
    /// Unlike the lower-level `record` operation, this waits for the paired
    /// iPhone and does not sign acceptance until delivery succeeds.
    pub async fn record_and_deliver(
        &self,
        app_name: &str,
        note: Option<&str>,
    ) -> Result<Evolution, EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        if app.shot_id.is_none() || app.builder_id.is_none() {
            return Err(EngineError::LegacyRequiresAdoption(app_name.into()));
        }
        let builder = BuilderIdentityManager::for_ledger(&self.ledger).ensure()?;
        if app.builder_id != Some(builder.builder_id) {
            return Err(EngineError::BuilderMismatch(app_name.into()));
        }
        self.record_locked(app_name, &app, &builder, note, None, true)
            .await
    }

    /// The recording body, run while the app lock is already held.
    async fn record_locked(
        &self,
        app_name: &str,
        app: &AppRecord,
        builder: &BuilderIdentity,
        note: Option<&str>,
        origin: Option<ShotOrigin>,
        require_device_delivery: bool,
    ) -> Result<Evolution, EngineError> {
        let working = self.ledger.working_tree(app_name);
        let has_project = fs::read_dir(&working)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "xcodeproj")
                })
            })
            .unwrap_or(false);
        if !has_project {
            return Err(EngineError::NothingToSeal(app_name.into()));
        }
        let previous = self.ledger.latest_evolution(app_name)?;
        if let Some(previous) = &previous {
            if origin.is_none() {
                protocol_lifecycle::verify_completed_evolution(previous)?;
                if self.working_tree_matches(previous)? {
                    self.events.emit(Event::result(format!(
                        "nothing new — the folder already matches evolution {}.",
                        previous.number
                    )));
                    return Ok(previous.clone());
                }
            }
        }
        let is_initial_birth = previous.is_none() && origin.is_none();
        if is_initial_birth {
            // Fail before reserving or building when the harness returned only
            // a build, a generic plan, or incomplete target-user evidence.
            validate_canonical_birth_project_layout(&working, app_name)?;
            let birth = self.load_birth_context(app_name)?;
            let blockers = birth_candidate_blockers(&birth);
            if !blockers.is_empty() {
                return Err(EngineError::ProtocolBodyIncomplete(format!(
                    "birth candidate remains unsealed before recording: {}",
                    summarize_birth_candidate_blockers(&blockers)
                )));
            }
            protocol_lifecycle::reconcile_birth_capability_declaration(&working, &birth.plan)?;
        }
        self.wait_for_apple_prerequisites().await?;
        let working_digest_at_start = self.working_digest(app_name);
        let shot = self
            .ledger
            .reserve_evolution(app_name, previous.as_ref().map(|shot| shot.number))?;
        self.events.emit(Event::status(format!(
            "preparing evolution {}…",
            shot.number
        )));
        // An evolve-conducted intent remains staged through failed attempts.
        // It is cleared only after a passing Version is accepted.
        let layout = ShotLayout::at(&working);
        let pending_intent = layout
            .pending_evolution_prompt()?
            .filter(|text| !text.trim().is_empty());
        let consumed_pending_intent = note.is_none().then(|| pending_intent.clone()).flatten();
        let briefing_intent = if shot.number == 1 {
            fs::read_to_string(self.ledger.briefing_dir(app_name).join("intent.md")).ok()
        } else {
            None
        };
        let prompt = note
            .map(str::to_owned)
            .or(pending_intent)
            .or(briefing_intent)
            .unwrap_or_else(|| "recorded from the working tree.".into());
        self.ledger
            .write_evolution_file(&shot, "prompt.md", prompt.as_bytes())?;
        if shot.number == 1 {
            // The same exact image bytes and original safe filenames that
            // entered the signed Intention also enter the frozen v1 genesis
            // input commitment. Every read and write is checked.
            self.copy_initial_intention_images(&layout, &shot)?;
        }
        let genesis_input_sha256 = protocol_lifecycle::capture_input_commitment(&shot)?;
        self.genome
            .compose(&self.ledger, &shot, app_name, &app.bundle_id, &[], None)?;
        self.events.emit(Event::status(format!(
            "recording evolution {}…",
            shot.number
        )));
        self.ledger.snapshot_working_tree(&shot)?;
        self.genome
            .write_standing_orders(&shot.source_path(), app_name)?;
        // The engine-owned provenance placeholder never travels through a
        // snapshot; recreate it so the anatomy gate and prepare can run.
        let placeholder = shot.source_path().join("TOHSENO/embedded-provenance.json");
        if !placeholder.exists() {
            if let Some(parent) = placeholder.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&placeholder, b"{}")?;
        }
        if let Err(error) = build::validate_complete_source(&shot.source_path()) {
            return Err(EngineError::WorkingTreeIncomplete(error.to_string()));
        }
        self.events.emit(Event::status(format!(
            "building evolution {}…",
            shot.number
        )));
        if build::compile(&self.ledger, &shot, app.target_name())?.is_err() {
            return Err(EngineError::WorkingTreeUnbuildable {
                app: app_name.into(),
                shot: shot.number,
            });
        }
        let bundle_id = app.bundle_id.clone();
        install::require_candidate_namespace(&bundle_id).map_err(EngineError::Install)?;
        let completed = self
            .finish_evolution(
                &shot,
                app,
                builder,
                genesis_input_sha256,
                origin,
                app_name,
                &bundle_id,
                working_digest_at_start,
                require_device_delivery,
            )
            .await?;
        // Sealing substitutes engine-owned values into the SNAPSHOT (shot
        // token, pbxproj CURRENT_PROJECT_VERSION) and materializes the real
        // Fascia and provenance sidecars there. Mirror those exact bytes into
        // the living folder so a landed folder equals its accepted Version;
        // otherwise every folder drifts the moment it lands, the next evolve
        // seals a surprise Version, and the builder's version-bound feedback
        // can never seed an Evolution.
        if let Err(error) = self.align_working_tree_with_seal(app_name, &completed) {
            self.events.emit(Event::status(format!(
                "evolution accepted; aligning the folder with its sealed version needs attention: {error}"
            )));
        }
        if let Some(pending) = consumed_pending_intent {
            if let Err(error) = layout
                .clear_evolution_feedback_selection(pending.as_bytes())
                .and_then(|_| layout.clear_evolution_prompt(pending.as_bytes()))
            {
                self.events.emit(Event::status(format!(
                    "accepted evolution, but private pending-intent cleanup needs attention: {error}"
                )));
            }
        }
        Ok(completed)
    }

    /// Mirrors seal-time engine substitutions from the accepted snapshot back
    /// into the living folder: the shot-number substitution and the two
    /// engine-owned identity sidecars. After this, an untouched landed folder
    /// hashes identically to its accepted Version.
    fn align_working_tree_with_seal(
        &self,
        app_name: &str,
        sealed: &Evolution,
    ) -> Result<(), EngineError> {
        let working = self.ledger.working_tree(app_name);
        if !working.is_dir() {
            return Ok(());
        }
        build::substitute_shot_number(&working, sealed.number)?;
        for sidecar in ["TOHSENO/fascia.json", "TOHSENO/embedded-provenance.json"] {
            let source = sealed.source_path().join(sidecar);
            if !source.is_file() {
                continue;
            }
            let destination = working.join(sidecar);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source, &destination)?;
        }
        Ok(())
    }

    /// The lenient digest of the working tree, or None when the folder is
    /// absent or unhashable.
    fn working_digest(&self, app_name: &str) -> Option<tohseno_protocol::digest::Bytes32> {
        let working = self.ledger.working_tree(app_name);
        if !working.is_dir() {
            return None;
        }
        crate::shot_layout::hash_expression_working_tree(&working)
            .ok()
            .map(|commitment| commitment.digest)
    }

    /// Whether the folder holds anything a seal would include.
    fn working_tree_has_content(&self, app_name: &str) -> Result<bool, EngineError> {
        let working = self.ledger.working_tree(app_name);
        if !working.is_dir() {
            return Ok(false);
        }
        let commitment = crate::shot_layout::hash_expression_working_tree(&working)
            .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
        Ok(!commitment.entries.is_empty())
    }

    /// Whether the folder holds anything the BUILDER put there. The engine's
    /// own standing orders do not count: a prepared-but-never-run Shot must
    /// remain re-creatable, or a failed first handoff wedges the folder
    /// forever.
    fn working_tree_has_user_content(&self, app_name: &str) -> Result<bool, EngineError> {
        let working = self.ledger.working_tree(app_name);
        if !working.is_dir() {
            return Ok(false);
        }
        let commitment = crate::shot_layout::hash_expression_working_tree(&working)
            .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
        Ok(commitment
            .entries
            .iter()
            .any(|entry| entry.path != "AGENTS.md" && entry.path != "CLAUDE.md"))
    }

    fn working_tree_matches(&self, sealed: &Evolution) -> Result<bool, EngineError> {
        let working = self.ledger.working_tree(&sealed.app_name);
        if !working.is_dir() {
            return Ok(true);
        }
        let working_hash = crate::shot_layout::hash_expression_working_tree(&working)
            .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
        let sealed_hash = tohseno_protocol::tree_hash::hash_source_tree(&sealed.source_path())
            .map_err(|error| ProtocolLifecycleError::Protocol(error.to_string()))?;
        Ok(working_hash.digest == sealed_hash.digest)
    }

    pub async fn refresh(&self, app_name: Option<&str>) -> Result<(), EngineError> {
        self.wait_for_apple_prerequisites().await?;
        let apps = if let Some(app_name) = app_name {
            let app = self.ledger.load_app(app_name)?;
            vec![app]
        } else {
            self.ledger
                .list_apps()?
                .into_iter()
                .filter(|app| !app.retired)
                .collect()
        };
        for app in apps
            .into_iter()
            .filter(|app| app.latest_evolution.is_some())
        {
            let _app_lock = self.ledger.lock_app(&app.name)?;
            let app = self.ledger.load_app(&app.name)?;
            if app.latest_evolution.is_none() {
                continue;
            }
            let shot = self.ledger.latest_evolution(&app.name)?.unwrap();
            protocol_lifecycle::verify_completed_evolution(&shot)?;
            let recorded_artifact = shot
                .artifact_path()
                .join(format!("{}.app", app.target_name()));
            if sign::days_until_expiry(&recorded_artifact).is_some_and(|days| days <= 0)
                && sign::development_team_profile()
                    .is_ok_and(|team| team.provisioning == sign::ProvisioningKind::Free)
            {
                self.emit_upsell_once(
                    "expiry",
                    "A paid Apple Developer membership removes weekly expiry: developer.apple.com.",
                )?;
            }
            let artifact_directory = temporary_path("refresh");
            self.events.emit(Event::status(format!(
                "refreshing evolution {} of {}…",
                shot.number, app.name
            )));
            DevicePipeline::new(self.events.clone())
                .build_install(
                    shot.number,
                    app.target_name(),
                    &app.bundle_id,
                    &shot.source_path(),
                    &artifact_directory,
                )
                .await?;
            self.ledger.set_retired(&app.name, false)?;
            self.events.emit(Event::result(format!(
                "evolution {} of {} is refreshed on your phone.",
                shot.number, app.name
            )));
        }
        Ok(())
    }

    pub async fn retire(&self, app_name: &str, local: bool) -> Result<(), EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        install::require_candidate_namespace(&app.bundle_id).map_err(EngineError::Install)?;
        if local {
            self.ledger.set_retired(app_name, true)?;
            self.events.emit(Event::result(format!(
                "{app_name} is retired in your ledger. If it is installed on a phone, it stays there until you remove it."
            )));
            return Ok(());
        }
        // One honest check instead of an endless wait: a loop that never
        // touched a phone must not block forever on a cable.
        if matches!(device::check(), Ok(DeviceState::CableMissing)) {
            return Err(EngineError::ProtocolBodyIncomplete(format!(
                "no iPhone is connected. Plug one in to remove {app_name} from it, or run `tohseno retire {app_name} --local` to retire without a phone."
            )));
        }
        self.wait_for_apple_prerequisites().await?;
        let device = DevicePipeline::new(self.events.clone())
            .wait_for_device()
            .await?;
        self.events
            .emit(Event::status(format!("retiring {app_name}…")));
        install::retire(&device, &app.bundle_id).map_err(EngineError::Install)?;
        self.ledger.set_retired(app_name, true)?;
        self.events.emit(Event::result(format!(
            "{app_name} is off your phone and remains in your ledger."
        )));
        Ok(())
    }

    pub fn doctor_once(&self) -> Result<bool, EngineError> {
        let mut ready = true;
        match toolchain::check() {
            ToolchainState::Ready => {
                self.events.emit(Event::status("Xcode is ready."));
            }
            ToolchainState::Missing => {
                ready = false;
                let _ = toolchain::trigger_install();
                self.events.emit(Event::handoff(
                    "Install Xcode from the App Store, then open it once.",
                ));
            }
        }
        // Recording waits on Apple Development signing; say so now instead
        // of letting the first evolve poll silently.
        match apple_signing::check() {
            AppleSigningState::Ready {
                team_name,
                provisioning,
                ..
            } => {
                self.events.emit(Event::status(format!(
                    "Apple Development signing is ready with the {} team {}.",
                    provisioning.as_str(),
                    team_name
                        .as_deref()
                        .unwrap_or("selected in Xcode")
                        .trim_end_matches('.')
                )));
            }
            AppleSigningState::Missing => {
                ready = false;
                self.events.emit(Event::handoff(
                    "Add an Apple Development identity: Xcode → Settings → Accounts → Manage Certificates → + → Apple Development.",
                ));
            }
        }
        let harnesses = self.harnesses();
        let usable = harnesses
            .iter()
            .filter(|harness| {
                harness.installed && harness.routes.iter().any(|route| route.available)
            })
            .map(|harness| harness.label.as_str())
            .collect::<Vec<_>>();
        if usable.is_empty() {
            ready = false;
            self.events.emit(Event::handoff(
                "Install and sign in to a coding harness (Codex or Claude Code); `tohseno shot harnesses` shows what this Mac detects.",
            ));
        } else {
            self.events.emit(Event::status(format!(
                "coding harness ready: {}.",
                usable.join(", ")
            )));
        }
        let identity_path = self.ledger.machine_root().join("identity/builder.json");
        if identity_path.is_file() {
            self.events
                .emit(Event::status("local Builder identity is present."));
        } else {
            self.events.emit(Event::status(
                "no local Builder identity yet — the first Shot creates a local, test-only one.",
            ));
        }
        Ok(ready)
    }

    /// Starts Apple's installer before the user begins describing the app so
    /// toolchain download time overlaps with the intent gate.
    pub fn prime_toolchain(&self) {
        if toolchain::check() == ToolchainState::Missing {
            let _ = toolchain::trigger_install();
            self.events
                .emit(Event::status("starting the Apple toolchain installation…"));
        }
    }

    fn emit_upsell_once(&self, wall: &str, message: &str) -> Result<(), EngineError> {
        let directory = self.ledger.machine_root().join("walls");
        fs::create_dir_all(&directory)?;
        let marker = directory.join(wall);
        if !marker.exists() {
            fs::write(marker, b"shown\n")?;
            self.events.emit(Event::status(message));
        }
        Ok(())
    }

    async fn wait_for_apple_prerequisites(&self) -> Result<(), EngineError> {
        let mut toolchain_announced = false;
        loop {
            match toolchain::check() {
                ToolchainState::Ready => break,
                ToolchainState::Missing => {
                    if !toolchain_announced {
                        let _ = toolchain::trigger_install();
                        self.events.emit(Event::handoff(
                            "Install Xcode from the App Store, then open it once.",
                        ));
                        toolchain_announced = true;
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }

        let mut apple_signing_announced = false;
        loop {
            match apple_signing::check() {
                AppleSigningState::Ready { .. } => return Ok(()),
                AppleSigningState::Missing => {
                    if !apple_signing_announced {
                        self.events.emit(Event::handoff(
                            "Open Xcode → Settings → Accounts and sign in with your Apple ID.",
                        ));
                        apple_signing_announced = true;
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
}

/// Birth has one canonical source root. A project copied under `src/` can look
/// buildable to a harness while the engine later signs a separately repaired
/// root project, leaving two divergent descriptions of the app. Inspect the
/// source tree independently and reject that ambiguity before any Version is
/// reserved. Private lineage and ordinary derived-data roots are not source.
fn validate_canonical_birth_project_layout(root: &Path, app_name: &str) -> Result<(), EngineError> {
    let expected = PathBuf::from(format!("{app_name}.xcodeproj"));
    let mut projects = Vec::new();
    let mut pending = vec![PathBuf::new()];

    while let Some(relative_directory) = pending.pop() {
        let directory = root.join(&relative_directory);
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let relative = relative_directory.join(entry.file_name());
            let is_project = entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "xcodeproj");

            if file_type.is_symlink() {
                if is_project {
                    return Err(EngineError::ProtocolBodyIncomplete(format!(
                        "birth source layout contains a symlinked Xcode project at `./{}`",
                        relative.display()
                    )));
                }
                continue;
            }
            if !file_type.is_dir() {
                continue;
            }
            if is_project {
                projects.push(relative);
                continue;
            }
            if matches!(
                entry.file_name().to_str(),
                Some(".tohseno" | ".git" | ".build" | "build" | "DerivedData")
            ) {
                continue;
            }
            pending.push(relative);
        }
    }

    projects.sort();
    if projects.as_slice() != [expected.clone()] {
        let observed = if projects.is_empty() {
            "none".into()
        } else {
            projects
                .iter()
                .map(|path| format!("`./{}`", path.display()))
                .collect::<Vec<_>>()
                .join(", ")
        };
        return Err(EngineError::ProtocolBodyIncomplete(format!(
            "birth source layout must contain exactly one real Xcode project at `./{}` and no nested duplicate; observed {observed}",
            expected.display()
        )));
    }
    Ok(())
}

fn birth_candidate_blockers(context: &BirthContext) -> Vec<String> {
    let mut blockers = Vec::new();
    if context.plan.completion_contract.release_build_required
        && !context.trial.release_build_passed
    {
        blockers.push("release_build: failed or absent".into());
    }
    if !context.trial.automated_tests_passed {
        blockers.push("automated_tests: failed or absent".into());
    }
    if !context.trial.simulator_trial_passed {
        blockers.push("simulator_target_user_trial: failed or absent".into());
    }
    for scenario_id in &context.plan.completion_contract.required_scenario_ids {
        if let Some(result) = context
            .trial
            .scenario_results
            .iter()
            .find(|result| &result.scenario_id == scenario_id)
        {
            if !result.passed {
                blockers.push(format!("experience_scenario.{scenario_id}: failed"));
            }
        }
    }
    for organ in &context.trial.organ_results {
        for criterion in &organ.criteria {
            if !criterion.passed {
                blockers.push(format!(
                    "organ.{}/{}: failed independently",
                    organ.organ_id, criterion.id
                ));
            }
        }
    }
    for substitution in &context.trial.forbidden_substitution_results {
        if !substitution.passed {
            blockers.push(format!(
                "forbidden_substitution.{}: observed",
                substitution.id
            ));
        }
    }
    if !context.trial.intent_review.passed {
        blockers.push("intent_fidelity.intelligent_review: failed".into());
    }
    for gap in &context.trial.incompleteness {
        if gap.blocks_completion || gap.category == IncompletenessCategory::ProductGap {
            blockers.push(format!(
                "incompleteness.{} [{}]: {}",
                gap.id,
                gap.category.as_str(),
                gap.description
            ));
        } else if matches!(
            gap.category,
            IncompletenessCategory::FutureOpportunity | IncompletenessCategory::ExplicitNonGoal
        ) {
            blockers.push(format!(
                "incompleteness.{}: {:?} belongs in the Birth Plan or product memory, not an accepted Version's incompleteness",
                gap.id, gap.category
            ));
        }
    }
    let required_physical = &context
        .plan
        .completion_contract
        .physical_verification_capabilities;
    if !required_physical.is_empty() {
        match &context.trial.physical_device {
            None => blockers.push(
                "physical_device_experience: implementation_complete; acceptance_pending_physical_experience"
                    .into(),
            ),
            Some(device) if !device.passed => {
                blockers.push("physical_device_experience: failed".into())
            }
            Some(device) => {
                let missing = required_physical
                    .iter()
                    .filter(|required| !device.exercised_capabilities.contains(required))
                    .cloned()
                    .collect::<Vec<_>>();
                if !missing.is_empty() {
                    blockers.push(format!(
                        "physical_device_experience: missing required capabilities {}",
                        missing.join(", ")
                    ));
                }
            }
        }
    }
    blockers
}

/// Keep the repair diagnostic actionable without repeating the same verdict
/// phrase for every criterion. The strict Experience Trial remains the full
/// evidence record; this summary groups stable semantic identifiers and keeps
/// typed incompleteness visible so the runner can stop on external blocks.
fn summarize_birth_candidate_blockers(blockers: &[String]) -> String {
    let mut gates = Vec::new();
    let mut scenarios = Vec::new();
    let mut organs = Vec::new();
    let mut substitutions = Vec::new();
    let mut incompleteness = Vec::new();
    let mut other = Vec::new();

    for blocker in blockers {
        if blocker.starts_with("release_build:")
            || blocker.starts_with("automated_tests:")
            || blocker.starts_with("simulator_target_user_trial:")
        {
            gates.push(blocker.clone());
        } else if let Some(value) = blocker
            .strip_prefix("experience_scenario.")
            .and_then(|value| value.strip_suffix(": failed"))
        {
            scenarios.push(value.to_owned());
        } else if let Some(value) = blocker
            .strip_prefix("organ.")
            .and_then(|value| value.strip_suffix(": failed independently"))
        {
            organs.push(value.to_owned());
        } else if let Some(value) = blocker
            .strip_prefix("forbidden_substitution.")
            .and_then(|value| value.strip_suffix(": observed"))
        {
            substitutions.push(value.to_owned());
        } else if blocker.starts_with("incompleteness.") {
            incompleteness.push(compact_diagnostic(blocker, 280));
        } else {
            other.push(blocker.clone());
        }
    }

    let mut groups = Vec::new();
    if !gates.is_empty() {
        groups.push(format!(
            "failed gates ({})={}",
            gates.len(),
            gates.join(", ")
        ));
    }
    if !scenarios.is_empty() {
        groups.push(format!(
            "failed scenarios ({})={}",
            scenarios.len(),
            scenarios.join(", ")
        ));
    }
    if !organs.is_empty() {
        groups.push(format!(
            "failed organ criteria ({})={}",
            organs.len(),
            organs.join(", ")
        ));
    }
    if !substitutions.is_empty() {
        groups.push(format!(
            "forbidden substitutions observed ({})={}",
            substitutions.len(),
            substitutions.join(", ")
        ));
    }
    if !incompleteness.is_empty() {
        groups.push(format!(
            "blocking incompleteness ({})={}",
            incompleteness.len(),
            incompleteness.join(" | ")
        ));
    }
    if !other.is_empty() {
        groups.push(format!(
            "other blockers ({})={}",
            other.len(),
            other.join(", ")
        ));
    }
    groups.join("; ")
}

fn compact_diagnostic(value: &str, maximum_characters: usize) -> String {
    let mut characters = value.chars();
    let compact = characters
        .by_ref()
        .take(maximum_characters)
        .collect::<String>();
    if characters.next().is_some() {
        format!("{compact}…")
    } else {
        compact
    }
}

/// Gates 6–8, reusable by create/evolve and refresh.
pub struct DevicePipeline {
    events: EventBus,
    poll_interval: Duration,
}

impl DevicePipeline {
    pub fn new(events: EventBus) -> Self {
        Self {
            events,
            poll_interval: Duration::from_secs(2),
        }
    }

    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    pub async fn run(
        &self,
        shot: &Evolution,
        app_name: &str,
        bundle_id: &str,
        source: &Path,
    ) -> Result<(), EngineError> {
        self.build_install(
            shot.number,
            app_name,
            bundle_id,
            source,
            &shot.artifact_path(),
        )
        .await?;
        self.events.emit(Event::result(format!(
            "evolution {} of {} is on your phone.",
            shot.number, app_name
        )));
        Ok(())
    }

    pub async fn build_install(
        &self,
        shot_number: u32,
        app_name: &str,
        bundle_id: &str,
        source: &Path,
        artifact_directory: &Path,
    ) -> Result<(), EngineError> {
        install::require_candidate_namespace(bundle_id).map_err(EngineError::Install)?;
        let device = self.wait_for_device().await?;
        let team = sign::development_team_profile().map_err(EngineError::Sign)?;
        if team.provisioning == sign::ProvisioningKind::Free {
            let installed =
                install::installed_candidate_apps(&device).map_err(EngineError::Install)?;
            if let Some(blocker) = install::free_team_slot_blocker(&installed, bundle_id) {
                let candidate = blocker
                    .bundle_id
                    .rsplit('.')
                    .next()
                    .unwrap_or(&blocker.bundle_id);
                self.events.emit(Event::status(
                    "Xcode selected a free Personal Team, whose connected-iPhone development profile is limited to three TOHSENO app bundles.",
                ));
                self.events.emit(Event::handoff(format!(
                    "The connected iPhone actually contains {} TOHSENO apps. Run `tohseno retire {candidate}` to remove one before installing this new bundle, or select a paid Apple team in Xcode.",
                    installed.len()
                )));
                return Err(EngineError::SlotLimit);
            }
        }
        self.events.emit(Event::status(format!(
            "signing evolution {shot_number} with {} Apple team {}…",
            team.provisioning.as_str(),
            team.team_name.as_deref().unwrap_or(&team.team_id)
        )));
        let app = sign::build_signed(sign::SignRequest {
            source,
            artifact_directory,
            app_name,
            bundle_id,
            shot_number,
            device: &device,
            team_id: &team.team_id,
        })
        .map_err(EngineError::Sign)?;
        self.events.emit(Event::status(format!(
            "installing evolution {shot_number}…"
        )));
        install::install(&device, &app, bundle_id).map_err(EngineError::Install)?;
        install::launch(&device, bundle_id).map_err(EngineError::Install)?;
        Ok(())
    }

    pub async fn wait_for_device(&self) -> Result<device::Device, EngineError> {
        let mut last_handoff: Option<&'static str> = None;
        loop {
            let state = device::check().map_err(EngineError::Device)?;
            let (handoff, ready) = match state {
                DeviceState::Ready(device) => (None, Some(device)),
                DeviceState::CableMissing => {
                    (Some("Connect a paired iPhone by cable or local network."), None)
                }
                DeviceState::TrustRequired => (Some("Tap Trust on your iPhone."), None),
                DeviceState::DeveloperModeRequired => (
                    Some("Enable Developer Mode: Settings → Privacy & Security → Developer Mode, then let your phone restart."),
                    None,
                ),
            };
            if let Some(device) = ready {
                self.events.emit(Event::status(format!(
                    "found {} over {}.",
                    device.name, device.transport
                )));
                return Ok(device);
            }
            if handoff != last_handoff {
                self.events.emit(Event::handoff(handoff.unwrap()));
                last_handoff = handoff;
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }
}

fn intention_excerpt(source: &str, maximum_bytes: usize) -> String {
    let mut output = String::new();
    let mut pending_space = false;
    let mut truncated = false;
    for character in source.trim().chars() {
        if character.is_whitespace() || character.is_control() {
            pending_space = !output.is_empty();
            continue;
        }
        let added = character.len_utf8() + usize::from(pending_space);
        if output.len().saturating_add(added) > maximum_bytes {
            truncated = true;
            break;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        output.push(character);
    }
    if truncated && output.len().saturating_add(3) <= maximum_bytes {
        output.push_str("...");
    }
    output
}

fn original_materials(
    original_intention: &[u8],
    source_materials: &[ArtifactAvailability],
) -> Result<Vec<OriginalMaterial>, EngineError> {
    let text = std::str::from_utf8(original_intention)
        .map_err(|_| EngineError::ProtocolBodyIncomplete("original intention is not UTF-8".into()))?
        .to_owned();
    let mut materials = vec![OriginalMaterial {
        artifact: ArtifactAvailability {
            schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
            artifact: ArtifactDescriptor {
                digest: tohseno_protocol::digest::sha256(original_intention),
                media_type: "text/plain; charset=utf-8".into(),
                byte_length: u64::try_from(original_intention.len()).map_err(|_| {
                    EngineError::ProtocolBodyIncomplete(
                        "original intention length overflowed".into(),
                    )
                })?,
                name: Some("INTENTION.md".into()),
            },
            status: AvailabilityStatus::IntentionallyPrivate,
            locations: Vec::new(),
        },
        inline_text: Some(text),
    }];
    materials.extend(
        source_materials
            .iter()
            .cloned()
            .map(|artifact| OriginalMaterial {
                artifact,
                inline_text: None,
            }),
    );
    Ok(materials)
}

fn render_pending_evolution_document(
    current_version: &VersionRecord,
    feedback_actions: &[Bytes32],
    references: &[StoredReference],
    desired_change: &str,
    preserved_invariants: &[String],
    genome_mutation: Option<(Bytes32, &[String])>,
) -> String {
    let mut document = format!(
        "# Evolutionary Intent\n\nCurrent version: {:04} (`{}`)\n\nFeedback action references:\n",
        current_version.ordinal, current_version.version_id
    );
    if feedback_actions.is_empty() {
        document.push_str("- None selected.\n");
    } else {
        for action in feedback_actions {
            document.push_str(&format!("- `{action}`\n"));
        }
    }
    document.push_str("\nPrivate artifact references:\n");
    if references.is_empty() {
        document.push_str("- None selected.\n");
    } else {
        for reference in references {
            let name = reference
                .availability
                .artifact
                .name
                .as_deref()
                .unwrap_or("unnamed reference");
            document.push_str(&format!(
                "- `{}` — `{}` ({} bytes)\n",
                name,
                reference.availability.artifact.digest,
                reference.availability.artifact.byte_length
            ));
        }
    }
    document.push_str("\nDesired changes:\n\n");
    document.push_str(desired_change.trim());
    document.push_str("\n\nInvariants to preserve:\n");
    for invariant in preserved_invariants {
        document.push_str(&format!("- {invariant}\n"));
    }
    document.push_str("\nProposed genome mutations:\n");
    match genome_mutation {
        None => document.push_str("- None.\n"),
        Some((proposal_action, summary)) => {
            document.push_str(&format!("- Accepted proposal `{proposal_action}`.\n"));
            for mutation in summary {
                document.push_str(&format!("- {mutation}\n"));
            }
        }
    }
    document
}

fn read_private_planning_json<T>(layout: &ShotLayout, filename: &str) -> Result<T, EngineError>
where
    T: serde::de::DeserializeOwned,
{
    let bytes = layout
        .read_private_planning_file(filename)
        .map_err(|error| {
            EngineError::ProtocolBodyIncomplete(format!(
                "required structured birth artifact `{filename}` is unavailable: {error}"
            ))
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        EngineError::ProtocolBodyIncomplete(format!(
            "required structured birth artifact `{filename}` is invalid: {error}"
        ))
    })
}

fn validate_conception_source_traceability(
    layout: &ShotLayout,
    input: &ConceptionInput,
    output: &ConceptionOutput,
) -> Result<(), EngineError> {
    let path = layout.root().join(&input.intention_document_path);
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 4 * 1024 * 1024
    {
        return Err(EngineError::ProtocolBodyIncomplete(format!(
            "preserved conception document is not a bounded regular file: {}",
            path.display()
        )));
    }
    output.validate_source_traceability(input, &fs::read(path)?)?;
    Ok(())
}

fn validate_trial_evidence_files(
    repository: &Path,
    trial: &ExperienceTrial,
) -> Result<(), EngineError> {
    let mut references = Vec::new();
    references.extend(trial.intent_review.evidence.iter());
    for result in &trial.forbidden_substitution_results {
        references.extend(result.evidence.iter());
    }
    for result in &trial.scenario_results {
        references.extend(result.evidence.iter());
        for assertion in &result.assertions {
            references.extend(assertion.evidence.iter());
        }
    }
    for result in &trial.organ_results {
        for criterion in &result.criteria {
            references.extend(criterion.evidence.iter());
        }
    }
    if let Some(device) = &trial.physical_device {
        references.extend(device.evidence.iter());
    }
    let canonical_root = fs::canonicalize(repository)?;
    for reference in references {
        let path = repository.join(&reference.relative_path);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            EngineError::ProtocolBodyIncomplete(format!(
                "experience evidence repository-root-relative path `{}` is unavailable under `{}`: {error}",
                reference.relative_path,
                repository.display()
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(EngineError::ProtocolBodyIncomplete(format!(
                "experience evidence `{}` must be a regular file",
                reference.relative_path
            )));
        }
        let canonical = fs::canonicalize(&path)?;
        if !canonical.starts_with(&canonical_root) {
            return Err(EngineError::ProtocolBodyIncomplete(format!(
                "experience evidence `{}` escapes the Shot repository",
                reference.relative_path
            )));
        }
        if metadata.len() != reference.artifact.byte_length {
            return Err(EngineError::ProtocolBodyIncomplete(format!(
                "experience evidence `{}` length differs from its declaration",
                reference.relative_path
            )));
        }
        let bytes = fs::read(&canonical)?;
        if tohseno_protocol::digest::sha256(&bytes) != reference.artifact.digest {
            return Err(EngineError::ProtocolBodyIncomplete(format!(
                "experience evidence `{}` digest differs from its declaration",
                reference.relative_path
            )));
        }
    }
    Ok(())
}

fn evidence_reference_for_file(
    repository: &Path,
    path: &Path,
    kind: EvidenceKind,
    media_type: &str,
) -> Result<EvidenceReference, EngineError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(EngineError::ProtocolBodyIncomplete(format!(
            "engine evidence `{}` is not a regular file",
            path.display()
        )));
    }
    let relative_path = path
        .strip_prefix(repository)
        .map_err(|_| {
            EngineError::ProtocolBodyIncomplete(format!(
                "engine evidence `{}` is outside the Shot repository",
                path.display()
            ))
        })?
        .to_string_lossy()
        .into_owned();
    let bytes = fs::read(path)?;
    Ok(EvidenceReference {
        kind,
        artifact: ArtifactDescriptor {
            digest: tohseno_protocol::digest::sha256(&bytes),
            media_type: media_type.into(),
            byte_length: u64::try_from(bytes.len()).map_err(|_| {
                EngineError::ProtocolBodyIncomplete("evidence length overflowed".into())
            })?,
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned),
        },
        relative_path,
    })
}

fn birth_receipt_availability(receipt: &BirthReceipt) -> Result<ArtifactAvailability, EngineError> {
    let bytes = tohseno_protocol::canonical::to_vec(receipt)
        .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
    Ok(ArtifactAvailability {
        schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
        artifact: ArtifactDescriptor {
            digest: tohseno_protocol::digest::sha256(&bytes),
            media_type: "application/vnd.tohseno.birth-receipt+json".into(),
            byte_length: u64::try_from(bytes.len()).map_err(|_| {
                EngineError::ProtocolBodyIncomplete("birth receipt length overflowed".into())
            })?,
            name: Some("birth-receipt.json".into()),
        },
        status: AvailabilityStatus::LocallyAvailable,
        locations: Vec::new(),
    })
}

fn bundle_id(app_name: &str) -> Result<String, EngineError> {
    let output = Command::new("whoami").output().map_err(EngineError::Io)?;
    if !output.status.success() {
        return Err(EngineError::IdentityName);
    }
    Ok(candidate_bundle_id(
        String::from_utf8_lossy(&output.stdout).trim(),
        app_name,
    ))
}

fn candidate_bundle_id(local_username: &str, app_name: &str) -> String {
    let username = sanitize_component(local_username);
    let username = if username.is_empty() {
        "user".to_owned()
    } else {
        username
    };
    format!("{}{username}.{app_name}", install::CANDIDATE_BUNDLE_PREFIX)
}

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("tohseno-{label}-{}-{nonce}", std::process::id()));
    let _ = fs::create_dir_all(&path);
    path
}

fn canonical_now() -> Result<CanonicalTimestamp, EngineError> {
    let value = OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .map_err(|error| EngineError::ProtocolBodyIncomplete(error.to_string()))?
        .format(&Rfc3339)
        .map_err(|error| EngineError::ProtocolBodyIncomplete(error.to_string()))?;
    CanonicalTimestamp::parse(value)
        .map_err(ShotLayoutError::from)
        .map_err(EngineError::from)
}

fn canonical_now_at_least(
    previous: &CanonicalTimestamp,
) -> Result<CanonicalTimestamp, EngineError> {
    let now = canonical_now()?;
    if now.unix_timestamp() < previous.unix_timestamp() {
        Ok(previous.clone())
    } else {
        Ok(now)
    }
}

fn sign_lineage_action(
    manager: &BuilderIdentityManager,
    builder: &BuilderIdentity,
    action: LineageAction,
) -> Result<SignedLineageAction, EngineError> {
    let digest = action.signing_digest().map_err(ShotLayoutError::from)?;
    let signature = manager.sign_record_digest(builder, digest)?;
    SignedLineageAction::new(action, signature)
        .map_err(ShotLayoutError::from)
        .map_err(EngineError::from)
}

/// Construct and validate every deterministic protocol object needed for the
/// initial Expression before Genome acceptance mutates lineage. Birth-plan
/// validation is intentionally not enough: protocol Organ token constraints
/// are stricter than some agent-facing planning fields.
fn prepare_initial_expression_parts(
    app: &AppRecord,
    plan: &InitialExpressionPlan,
    genome: &tohseno_protocol::Genome,
) -> Result<(Vec<u8>, Expression, Vec<Organ>), EngineError> {
    let expression_id = app.expression_id.ok_or_else(|| {
        EngineError::ProtocolBodyIncomplete("the Shot has no stable ExpressionID".into())
    })?;
    if plan.schema != crate::birth_plan::BIRTH_EXPRESSION_PLAN_SCHEMA
        || plan.kind != "native_apple_application"
        || plan.name != app.target_name()
        || plan.platforms != ["iphone"]
        || plan.organs.is_empty()
    {
        return Err(EngineError::ProtocolBodyIncomplete(
            "initial expression plan is not the reviewed Apple plan for this Shot".into(),
        ));
    }
    plan.validate(genome)?;
    let plan_bytes = tohseno_protocol::canonical::to_vec(plan)
        .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
    let expression = Expression {
        schema: EXPRESSION_SCHEMA.into(),
        expression_id,
        kind: plan.kind.clone(),
        name: plan.name.clone(),
        platforms: plan.platforms.clone(),
        genome_revision: plan.genome_revision,
        genome_digest: plan.genome_digest,
        definition: ArtifactAvailability {
            schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
            artifact: ArtifactDescriptor {
                digest: tohseno_protocol::digest::sha256(&plan_bytes),
                media_type: "application/json".into(),
                byte_length: u64::try_from(plan_bytes.len()).map_err(|_| {
                    EngineError::ProtocolBodyIncomplete("expression plan length overflowed".into())
                })?,
                name: Some("birth-expression-plan.json".into()),
            },
            status: AvailabilityStatus::LocallyAvailable,
            locations: Vec::new(),
        },
    };
    expression.validate().map_err(ShotLayoutError::from)?;
    let organs = plan.protocol_organs(expression_id);
    for organ in &organs {
        organ.validate().map_err(ShotLayoutError::from)?;
    }
    canonical_capability_graph_bytes(&organs).map_err(ShotLayoutError::from)?;
    Ok((plan_bytes, expression, organs))
}

/// The protocol reducer stores an Expression's organs in a `BTreeMap`, so
/// every derived capability view must use organ-ID order rather than the
/// Birth Plan's topological declaration order.
fn canonical_organ_view(organs: &[Organ]) -> Vec<Organ> {
    let mut canonical = organs.to_vec();
    canonical.sort_by(|left, right| left.organ_id.cmp(&right.organ_id));
    canonical
}

/// The recording law behind `Engine::verify_builder_binding`, kept pure so
/// every refusal branch is provable without a Keychain: an app is runnable
/// only when its recorded Shot and Builder bindings exist and the recorded
/// Builder is the local one.
fn verify_recorded_builder(
    app_name: &str,
    recorded_shot: Option<tohseno_protocol::digest::ShotId>,
    recorded_builder: Option<tohseno_protocol::identity::BuilderId>,
    local_builder: tohseno_protocol::identity::BuilderId,
) -> Result<tohseno_protocol::digest::ShotId, EngineError> {
    let shot_id =
        recorded_shot.ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
    let recorded =
        recorded_builder.ok_or_else(|| EngineError::LegacyRequiresAdoption(app_name.into()))?;
    if recorded != local_builder {
        return Err(EngineError::BuilderMismatch(app_name.into()));
    }
    Ok(shot_id)
}

#[derive(Debug)]
pub enum EngineError {
    Io(std::io::Error),
    Config(ConfigError),
    Ledger(LedgerError),
    Intent(IntentError),
    Genome(GenomeError),
    Conception(ConceptionError),
    CapabilityProfile(CapabilityProfileError),
    BirthPlan(BirthPlanError),
    Experience(ExperienceError),
    Build(build::BuildError),
    Device(device::DeviceError),
    Sign(sign::SignError),
    Install(install::InstallError),
    NoCompleteShot(String),
    ArtifactUnbuildable(String),
    FolderInProgress(String),
    NothingToSeal(String),
    NotAdoptable(String),
    WorkingTreeIncomplete(String),
    WorkingTreeUnbuildable { app: String, shot: u32 },
    SlotLimit,
    IdentityName,
    BuilderIdentity(BuilderIdentityError),
    ProtocolLifecycle(ProtocolLifecycleError),
    ShotLayout(ShotLayoutError),
    LegacyRequiresAdoption(String),
    BuilderMismatch(String),
    AlreadyProtocol(String),
    ProtocolBodyIncomplete(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Config(error) => write!(f, "{error}"),
            Self::Ledger(error) => write!(f, "{error}"),
            Self::Intent(error) => write!(f, "{error}"),
            Self::Genome(error) => write!(f, "{error}"),
            Self::Conception(error) => write!(f, "{error}"),
            Self::CapabilityProfile(error) => write!(f, "{error}"),
            Self::BirthPlan(error) => write!(f, "{error}"),
            Self::Experience(error) => write!(f, "{error}"),
            Self::Build(error) => write!(f, "{error}"),
            Self::Device(error) => write!(f, "{error}"),
            Self::Sign(error) => write!(f, "{error}"),
            Self::Install(error) => write!(f, "{error}"),
            Self::NoCompleteShot(app) => {
                write!(f, "{app} has no recorded evolution yet — build in the folder, then `tohseno evolve`")
            }
            Self::FolderInProgress(app) => {
                write!(
                    f,
                    "the {app} folder already holds work — `tohseno evolve {app}` records it"
                )
            }
            Self::NotAdoptable(app) => {
                write!(
                    f,
                    "adoption needs `{app}.xcodeproj` and `TohsenoFascia/` in the {app} folder first"
                )
            }
            Self::NothingToSeal(app) => {
                write!(
                    f,
                    "no canonical root Xcode project exists in the {app} folder; put exactly one real project at `./{app}.xcodeproj` before engine recording"
                )
            }
            Self::WorkingTreeIncomplete(detail) => {
                write!(f, "the folder is missing required anatomy: {detail}")
            }
            Self::WorkingTreeUnbuildable { app, shot } => {
                write!(
                    f,
                    "the folder does not build; see .tohseno/evolutions/{shot:04}/build.log, fix, then `tohseno evolve {app}`"
                )
            }
            Self::ArtifactUnbuildable(output) => {
                let tail: String = output
                    .lines()
                    .filter(|line| line.contains("error"))
                    .take(8)
                    .collect::<Vec<_>>()
                    .join("\n");
                write!(
                    f,
                    "the iOS device build passed but the Simulator artifact failed:\n{tail}"
                )
            }
            Self::SlotLimit => write!(
                f,
                "the connected iPhone's free-team TOHSENO app limit is full"
            ),
            Self::IdentityName => write!(f, "could not determine the local username"),
            Self::BuilderIdentity(error) => write!(f, "{error}"),
            Self::ProtocolLifecycle(error) => write!(f, "{error}"),
            Self::ShotLayout(error) => write!(f, "{error}"),
            Self::LegacyRequiresAdoption(app) => {
                write!(
                    f,
                    "{app} predates signed Evolutions — cd into its folder, then `tohseno adopt`"
                )
            }
            Self::BuilderMismatch(app) => write!(
                f,
                "{app} belongs to a different frozen v0.7 BuilderID than the stored local legacy identity"
            ),
            Self::AlreadyProtocol(app) => {
                write!(f, "{app} already has a signed TOHSENO identity")
            }
            Self::ProtocolBodyIncomplete(reason) => {
                write!(f, "the local Shot protocol body is incomplete: {reason}")
            }
        }
    }
}

impl std::error::Error for EngineError {}

impl From<std::io::Error> for EngineError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ConfigError> for EngineError {
    fn from(value: ConfigError) -> Self {
        Self::Config(value)
    }
}

impl From<LedgerError> for EngineError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl From<IntentError> for EngineError {
    fn from(value: IntentError) -> Self {
        Self::Intent(value)
    }
}

impl From<GenomeError> for EngineError {
    fn from(value: GenomeError) -> Self {
        Self::Genome(value)
    }
}

impl From<ConceptionError> for EngineError {
    fn from(value: ConceptionError) -> Self {
        Self::Conception(value)
    }
}

impl From<CapabilityProfileError> for EngineError {
    fn from(value: CapabilityProfileError) -> Self {
        Self::CapabilityProfile(value)
    }
}

impl From<BirthPlanError> for EngineError {
    fn from(value: BirthPlanError) -> Self {
        Self::BirthPlan(value)
    }
}

impl From<ExperienceError> for EngineError {
    fn from(value: ExperienceError) -> Self {
        Self::Experience(value)
    }
}

impl From<build::BuildError> for EngineError {
    fn from(value: build::BuildError) -> Self {
        Self::Build(value)
    }
}

impl From<device::DeviceError> for EngineError {
    fn from(value: device::DeviceError) -> Self {
        Self::Device(value)
    }
}

impl From<BuilderIdentityError> for EngineError {
    fn from(value: BuilderIdentityError) -> Self {
        Self::BuilderIdentity(value)
    }
}

impl From<ProtocolLifecycleError> for EngineError {
    fn from(value: ProtocolLifecycleError) -> Self {
        Self::ProtocolLifecycle(value)
    }
}

impl From<ShotLayoutError> for EngineError {
    fn from(value: ShotLayoutError) -> Self {
        Self::ShotLayout(value)
    }
}

impl From<EngineError> for std::io::Error {
    fn from(value: EngineError) -> Self {
        std::io::Error::other(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_bundle_identity_uses_the_candidate_device_namespace() {
        let bundle_id = candidate_bundle_id("Alice Example", "quiet-press");
        assert_eq!(bundle_id, "org.tohseno.genesis.alice-example.quiet-press");
        install::require_candidate_namespace(&bundle_id).unwrap();
    }

    #[test]
    fn birth_blockers_are_grouped_without_losing_typed_external_constraints() {
        let blockers = vec![
            "experience_scenario.first_launch: failed".into(),
            "experience_scenario.live_upload: failed".into(),
            "organ.upload/resumes: failed independently".into(),
            "organ.results/plays_outputs: failed independently".into(),
            "intent_fidelity.intelligent_review: failed".into(),
            "incompleteness.backend_dns [external_environment_constraint]: the required host did not resolve".into(),
            "incompleteness.contract [product_gap]: the live contract was not inspected".into(),
        ];

        let diagnostic = summarize_birth_candidate_blockers(&blockers);

        assert!(diagnostic.contains("failed scenarios (2)=first_launch, live_upload"));
        assert!(
            diagnostic.contains("failed organ criteria (2)=upload/resumes, results/plays_outputs")
        );
        assert!(diagnostic.contains("blocking incompleteness (2)="));
        assert!(diagnostic.contains("external_environment_constraint"));
        assert!(diagnostic.contains("product_gap"));
        assert_eq!(diagnostic.matches("failed independently").count(), 0);
    }

    #[test]
    fn missing_birth_project_diagnostic_names_the_canonical_root_without_contradicting_runner_ownership(
    ) {
        let diagnostic = EngineError::NothingToSeal("press".into()).to_string();
        assert!(diagnostic.contains("`./press.xcodeproj`"));
        assert!(!diagnostic.contains("tohseno evolve"));
    }

    #[test]
    fn birth_project_layout_rejects_wrong_or_nested_duplicates() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(
            directory
                .path()
                .join(".tohseno/evolutions/0001/src/archived.xcodeproj"),
        )
        .unwrap();

        let missing = validate_canonical_birth_project_layout(directory.path(), "press")
            .unwrap_err()
            .to_string();
        assert!(missing.contains("`./press.xcodeproj`"));
        assert!(missing.contains("observed none"));

        fs::create_dir_all(directory.path().join("press.xcodeproj")).unwrap();
        validate_canonical_birth_project_layout(directory.path(), "press").unwrap();

        fs::create_dir_all(directory.path().join("src/press.xcodeproj")).unwrap();
        let duplicate = validate_canonical_birth_project_layout(directory.path(), "press")
            .unwrap_err()
            .to_string();
        assert!(duplicate.contains("`./press.xcodeproj`"));
        assert!(duplicate.contains("`./src/press.xcodeproj`"));
        assert!(duplicate.contains("no nested duplicate"));
    }

    #[test]
    fn candidate_namespace_preserves_a_deterministic_fallback_identity() {
        assert_eq!(
            candidate_bundle_id("---", "press"),
            "org.tohseno.genesis.user.press"
        );
    }

    #[test]
    fn original_intention_keeps_text_inline_and_binary_sources_descriptor_only() {
        let binary = b"\x89PNG\r\nexact visual source";
        let reference = ArtifactAvailability {
            schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
            artifact: ArtifactDescriptor {
                digest: tohseno_protocol::digest::sha256(binary),
                media_type: "image/png".into(),
                byte_length: binary.len() as u64,
                name: Some("source.png".into()),
            },
            status: AvailabilityStatus::IntentionallyPrivate,
            locations: Vec::new(),
        };
        let materials =
            original_materials(b"Keep this exact human wording.", &[reference.clone()]).unwrap();
        assert_eq!(materials.len(), 2);
        assert_eq!(
            materials[0].inline_text.as_deref(),
            Some("Keep this exact human wording.")
        );
        assert_eq!(materials[1].artifact, reference);
        assert!(materials[1].inline_text.is_none());
        IntentionRecord::new(
            materials,
            CanonicalTimestamp::parse("2026-07-30T00:00:00Z").unwrap(),
        )
        .validate()
        .unwrap();
    }

    #[test]
    fn derived_capability_view_uses_protocol_reducer_order() {
        let expression_id = ExpressionId::random();
        let mut organs = crate::birth_plan::protocol_substrate_organs()
            .into_iter()
            .map(|organ| organ.to_protocol_organ(expression_id))
            .collect::<Vec<_>>();
        organs.reverse();

        let canonical = canonical_organ_view(&organs);

        assert_eq!(
            organs
                .iter()
                .map(|organ| organ.organ_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "substrate_signed_continuity",
                "substrate_installation_identity"
            ]
        );
        assert_eq!(
            canonical
                .iter()
                .map(|organ| organ.organ_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "substrate_installation_identity",
                "substrate_signed_continuity"
            ]
        );
    }

    #[test]
    fn expression_protocol_conversion_is_validated_before_genome_acceptance() {
        let plan = crate::anky_fixture::plan(tohseno_protocol::digest::sha256(b"atomic birth"));
        let mut expression = BirthExpressionPlan::from_birth_plan(&plan).unwrap();
        let app = AppRecord {
            name: plan.product_name.clone(),
            target_name: Some(plan.product_name.clone()),
            bundle_id: "org.tohseno.genesis.test.atomic-birth".into(),
            created_at_unix: 1,
            latest_evolution: None,
            shot_id: Some(ShotId::random()),
            builder_id: None,
            expression_id: Some(ExpressionId::random()),
            retired: false,
            parents: Default::default(),
        };

        prepare_initial_expression_parts(&app, &expression, &plan.genome).unwrap();

        // The planning-level expression validator intentionally does not own
        // every protocol token rule. The pre-acceptance conversion must catch
        // this before any GenomeAcceptance action is appended.
        expression.organs[0].owns_state = vec!["duplicate".into(), "duplicate".into()];
        let error = prepare_initial_expression_parts(&app, &expression, &plan.genome)
            .unwrap_err()
            .to_string();
        assert!(error.contains("must not contain duplicates"), "{error}");
    }

    #[test]
    fn engine_standing_orders_do_not_count_as_builder_work() {
        // A prepared Shot folder holds AGENTS.md and CLAUDE.md written by the
        // engine itself. Re-creating after a failed first handoff must see
        // that folder as pristine, or the Shot is wedged forever (F-003).
        let temporary = tempfile::tempdir().unwrap();
        let family = temporary.path().join("family");
        let machine = temporary.path().join("machine");
        let working = family.join("quiet-press");
        fs::create_dir_all(&working).unwrap();
        fs::write(
            working.join("AGENTS.md"),
            "# This folder is a TOHSENO Shot\n",
        )
        .unwrap();
        fs::write(working.join("CLAUDE.md"), "Read AGENTS.md.\n").unwrap();
        fs::write(working.join("README.md"), "# TOHSENO Shot\n").unwrap();

        let engine = Engine::at(
            Ledger::at_homes(&family, &machine),
            EventBus::default(),
            Config::default(),
        );
        assert!(engine.working_tree_has_content("quiet-press").unwrap());
        assert!(!engine.working_tree_has_user_content("quiet-press").unwrap());

        fs::write(working.join("Anything.swift"), "// builder work\n").unwrap();
        assert!(engine.working_tree_has_user_content("quiet-press").unwrap());
    }

    #[test]
    fn running_is_refused_without_a_recorded_local_builder() {
        // Running is a recording act: no anonymous executions, and no
        // executions of a folder recorded under someone else's Builder.
        let local = tohseno_protocol::identity::BuilderId::parse(
            "eip155:4663:0x1111111111111111111111111111111111111111",
        )
        .unwrap();
        let foreign = tohseno_protocol::identity::BuilderId::parse(
            "eip155:4663:0x2222222222222222222222222222222222222222",
        )
        .unwrap();
        let shot = tohseno_protocol::digest::ShotId::random();

        // An app that predates canonical bindings is not runnable.
        let error = verify_recorded_builder("quiet-press", None, Some(local), local).unwrap_err();
        assert!(matches!(error, EngineError::LegacyRequiresAdoption(_)));
        let error = verify_recorded_builder("quiet-press", Some(shot), None, local).unwrap_err();
        assert!(matches!(error, EngineError::LegacyRequiresAdoption(_)));

        // An app recorded under a different Builder is not runnable here.
        let error =
            verify_recorded_builder("quiet-press", Some(shot), Some(foreign), local).unwrap_err();
        assert!(matches!(error, EngineError::BuilderMismatch(_)));

        // The recorded local Builder is the one identity allowed to run.
        assert_eq!(
            verify_recorded_builder("quiet-press", Some(shot), Some(local), local).unwrap(),
            shot
        );
    }
}
