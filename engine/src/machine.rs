use crate::builder_identity::{BuilderIdentity, BuilderIdentityError, BuilderIdentityManager};
use crate::config::{Config, ConfigError};
use crate::events::{Event, EventBus};
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
use std::collections::BTreeSet;
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
    GENOME_ACCEPTANCE_SCHEMA, GENOME_PROPOSAL_SCHEMA, GENOME_SCHEMA, ORGAN_SCHEMA,
    VERIFICATION_RESULT_SCHEMA, VERSION_SCHEMA,
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

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialExpressionPlan {
    pub schema: String,
    pub kind: String,
    pub name: String,
    pub platforms: Vec<String>,
    pub genome_revision: u64,
    pub genome_digest: tohseno_protocol::digest::Bytes32,
    pub organs: Vec<InitialOrganPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialOrganPlan {
    pub organ_id: String,
    pub provides: Vec<String>,
    pub owns_state: Vec<String>,
    pub permissions: Vec<String>,
    pub dependencies: Vec<String>,
    pub emits: Vec<String>,
    pub consumes: Vec<String>,
    pub satisfies_genome_constraints: Vec<String>,
    pub acceptance_tests: Vec<String>,
    pub platforms: Vec<String>,
}

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

    /// Produce the conservative, deterministic revision-1 Genome presented
    /// for review. The exact human source remains separately preserved and
    /// signed; this proposal never replaces it and is not accepted here.
    pub fn propose_initial_genome(
        request: &ShotRequest,
    ) -> Result<tohseno_protocol::Genome, EngineError> {
        crate::ledger::validate_app_name(&request.app_name)?;
        let excerpt = intention_excerpt(&request.intent.prompt, 3000);
        if excerpt.is_empty() {
            return Err(EngineError::ProtocolBodyIncomplete(
                "a Genome proposal requires a nonempty coherent intention".into(),
            ));
        }
        let genome = tohseno_protocol::Genome {
            schema: GENOME_SCHEMA.into(),
            revision: 1,
            purpose: format!(
                "Bring the owner's preserved coherent intention into a useful native Apple expression: {excerpt}"
            ),
            intended_for: vec![
                "The owner and the people identified by the preserved intention.".into(),
            ],
            essential_experience: vec![
                "The expression makes the preserved intention tangible without setup ceremony."
                    .into(),
                "The core experience remains useful on the device when offline.".into(),
            ],
            behavioral_invariants: vec![
                "Preserve Shot identity and signed continuity across every accepted version."
                    .into(),
                "Keep owner-created state available locally and fail without inventing data."
                    .into(),
            ],
            interaction_laws: vec![
                "Prefer direct native interaction over explanatory or administrative screens."
                    .into(),
            ],
            aesthetic_principles: vec![
                "Use legible, calm, platform-native presentation unless the owner accepts a different principle."
                    .into(),
            ],
            privacy_principles: vec![
                "Keep intention, feedback, and owner data private by default.".into(),
                "Do not add telemetry, tracking, or silent identity linkage.".into(),
            ],
            ownership_principles: vec![
                "Only the recognized Shot controller accepts continuity-changing actions.".into(),
            ],
            platform_commitments: vec![
                "The first expression is a native iPhone application.".into(),
            ],
            boundaries: vec![
                "The software expression does not redefine the Shot, its owner, or its original intention."
                    .into(),
            ],
            non_goals: vec![
                "A token, repository, deployment, or bundle identifier is never treated as the Shot identity."
                    .into(),
            ],
            required_capabilities: vec![
                "embedded_shot_identity".into(),
                "local_persistence".into(),
                "native_navigation".into(),
                "exact_version_feedback".into(),
            ],
            forbidden_transformations: vec![
                "Do not discard or silently rewrite the original intention.".into(),
                "Do not publish private intention, feedback, or working memory by default.".into(),
                "Do not silently mutate an accepted Genome during implementation evolution."
                    .into(),
            ],
            acceptance_principles: vec![
                "A deterministic Release build and the declared privacy and identity gates pass."
                    .into(),
                "Embedded identity binds the exact Shot, expression, Genome revision, and Version."
                    .into(),
            ],
            freely_changeable: vec![
                "Implementation details, typography, layout, and internal structure may evolve when invariants remain true."
                    .into(),
            ],
        };
        genome.validate().map_err(ShotLayoutError::from)?;
        Ok(genome)
    }

    /// Produce the reviewable first Apple-expression/capability plan for an
    /// accepted or proposed Genome. No lineage action is written here.
    pub fn propose_initial_expression_plan(
        request: &ShotRequest,
        genome: &tohseno_protocol::Genome,
    ) -> Result<InitialExpressionPlan, EngineError> {
        crate::ledger::validate_app_name(&request.app_name)?;
        genome.validate().map_err(ShotLayoutError::from)?;
        let plan = InitialExpressionPlan {
            schema: "tohseno.initial-expression-plan/1".into(),
            kind: "native_apple_application".into(),
            name: request.app_name.clone(),
            platforms: vec!["iphone".into()],
            genome_revision: genome.revision,
            genome_digest: genome.digest().map_err(ShotLayoutError::from)?,
            organs: default_initial_organs(),
        };
        validate_initial_expression_plan(&plan, genome)?;
        Ok(plan)
    }

    /// Takes the Shot: creates the visible folder, writes the briefing and
    /// standing orders, and hands the builder's own agent the work. The
    /// agent records evolution 1 itself with `tohseno evolve`.
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
        self.check_slot_limit()?;
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
        let (_, source_references) =
            layout.prepare_intent_package(request.intent.prompt.as_bytes(), &reference_sources)?;
        let source_materials = source_references
            .iter()
            .map(|reference| reference.availability.clone())
            .collect::<Vec<_>>();
        self.ensure_origin_lineage(
            &layout,
            &app,
            &builder,
            &identity_manager,
            request.intent.prompt.as_bytes(),
            &source_materials,
        )?;
        self.genome.compose_briefing(
            &self.ledger,
            &request.app_name,
            &app.bundle_id,
            &request.intent,
            &source_references,
        )?;
        self.genome.write_standing_orders(
            &self.ledger.working_tree(&request.app_name),
            &request.app_name,
        )?;
        Ok(ConductedCreation {
            folder: self.ledger.working_tree(&request.app_name),
            agent_command: None,
            instruction: "Review and explicitly accept the proposed Genome and first expression plan before materialization.".into(),
        })
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
            instruction: "Read INTENTION.md, GENOME.md, AGENTS.md, and .tohseno/TASK.md, then materialize the accepted first expression. When it builds and is whole, record it with: tohseno evolve".into(),
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
        let expression_id = app.expression_id.ok_or_else(|| {
            EngineError::ProtocolBodyIncomplete("the Shot has no stable ExpressionID".into())
        })?;
        if plan.schema != "tohseno.initial-expression-plan/1"
            || plan.kind != "native_apple_application"
            || plan.name != app.target_name()
            || plan.platforms.is_empty()
            || plan.organs.is_empty()
        {
            return Err(EngineError::ProtocolBodyIncomplete(
                "initial expression plan is not the reviewed Apple plan for this Shot".into(),
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
        validate_initial_expression_plan(plan, &accepted.genome)?;

        let plan_bytes = tohseno_protocol::canonical::to_vec(plan)
            .map_err(|error| ShotLayoutError::Encoding(error.to_string()))?;
        layout.preserve_private_planning_file("expression-plan.json", &plan_bytes)?;
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
                        EngineError::ProtocolBodyIncomplete(
                            "expression plan length overflowed".into(),
                        )
                    })?,
                    name: Some("expression-plan.json".into()),
                },
                status: AvailabilityStatus::LocallyAvailable,
                locations: Vec::new(),
            },
        };
        expression.validate().map_err(ShotLayoutError::from)?;
        let organs = plan
            .organs
            .iter()
            .map(|organ| Organ {
                schema: ORGAN_SCHEMA.into(),
                expression_id,
                organ_id: organ.organ_id.clone(),
                provides: organ.provides.clone(),
                owns_state: organ.owns_state.clone(),
                permissions: organ.permissions.clone(),
                dependencies: organ.dependencies.clone(),
                emits: organ.emits.clone(),
                consumes: organ.consumes.clone(),
                satisfies_genome_constraints: organ.satisfies_genome_constraints.clone(),
                acceptance_tests: organ.acceptance_tests.clone(),
                platforms: organ.platforms.clone(),
            })
            .collect::<Vec<_>>();
        for organ in &organs {
            organ.validate().map_err(ShotLayoutError::from)?;
        }
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
            layout.write_metadata_json("capabilities.lock", &organs, false)?;
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
        layout.write_metadata_json("capabilities.lock", &organs, false)?;
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
                        self.record_locked(&request.app_name, &app, &builder, None, None)
                            .await?,
                    )
                }
            }
            None if self.working_tree_has_content(&request.app_name)? => Some(
                self.record_locked(&request.app_name, &app, &builder, None, None)
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
            "Read AGENTS.md and MEMORY.md. The builder asks: {}\nEvolve the app in this folder accordingly. When it builds and is whole, record it yourself by running: tohseno evolve",
            request.intent.prompt.trim()
        );
        Ok(Evolved::Conducted(ConductedCreation {
            folder: self.ledger.working_tree(&request.app_name),
            agent_command: self.preferred_agent_command(),
            instruction,
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

    /// The shared birth of every Evolution: protocol record, Simulator
    /// artifact, signature, conformance, finalization, working-tree
    /// checkout, then a non-blocking phone offer. The Mac is enough; the
    /// phone is a destination resumed through `tohseno refresh`.
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
    ) -> Result<Evolution, EngineError> {
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
        let mut known_incompleteness = Vec::new();
        if let Err(reason) = preview::capture(&artifact, bundle_id, &shot.path.join("preview.png"))
        {
            self.events
                .emit(Event::status(format!("no preview: {reason}")));
            known_incompleteness.push(format!(
                "Preview capture was unavailable: {}",
                intention_excerpt(&reason, 1800)
            ));
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

        let manager = BuilderIdentityManager::for_ledger(&self.ledger);
        let accepted_at = canonical_now_at_least(&lineage_input.last_timestamp)?;
        let mut gates = completed
            .conformance
            .checks
            .iter()
            .map(|check| VerificationGate {
                name: check.id.clone(),
                passed: check.status == CheckStatus::Pass,
                deterministic: true,
                evidence: None,
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
                gates.push(VerificationGate {
                    name: organ_acceptance_gate_name(organ, index)
                        .map_err(ShotLayoutError::from)?,
                    passed: completed.conformance.conformant,
                    deterministic: true,
                    evidence: Some(capability_graph_evidence.clone()),
                });
            }
        }
        let verification = VerificationResult {
            schema: VERIFICATION_RESULT_SCHEMA.into(),
            expression_id: lineage_input.expression_id,
            candidate_version_id,
            genome_revision: lineage_input.genome_revision,
            genome_digest: lineage_input.genome_digest,
            source_digest: completed.record.source_tree_sha256,
            capability_graph_digest: lineage_input.capability_graph_digest,
            gates,
            passed: completed.conformance.conformant,
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
        self.events.emit(Event::result(format!(
            "evolution {} of {} is complete and verified on this Mac.",
            shot.number, app_name
        )));
        self.events.emit(Event::status(format!(
            "folder: {}",
            self.ledger.working_tree(app_name).display()
        )));

        match device::check() {
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
                    "evolution {} of {} is on your phone.",
                    shot.number, app_name
                )));
            }
            _ => {
                self.events.emit(Event::handoff(format!(
                    "Plug in your iPhone anytime, then run `tohseno refresh {app_name}`.",
                )));
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
        self.record_locked(app_name, &app, &builder, note, None)
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
        } else {
            self.check_slot_limit()?;
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
            if app.retired {
                self.check_slot_limit()?;
            }
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
            if app.retired {
                self.check_slot_limit()?;
            }
            let shot = self.ledger.latest_evolution(&app.name)?.unwrap();
            protocol_lifecycle::verify_completed_evolution(&shot)?;
            let recorded_artifact = shot
                .artifact_path()
                .join(format!("{}.app", app.target_name()));
            if sign::days_until_expiry(&recorded_artifact).is_some_and(|days| days <= 0) {
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

    pub async fn retire(&self, app_name: &str) -> Result<(), EngineError> {
        crate::ledger::validate_app_name(app_name)?;
        let _app_lock = self.ledger.lock_app(app_name)?;
        let app = self.ledger.load_app(app_name)?;
        install::require_candidate_namespace(&app.bundle_id).map_err(EngineError::Install)?;
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
        match toolchain::check() {
            ToolchainState::Ready => {
                self.events.emit(Event::status("Xcode is ready."));
                Ok(true)
            }
            ToolchainState::Missing => {
                let _ = toolchain::trigger_install();
                self.events.emit(Event::handoff(
                    "Install Xcode from the App Store, then open it once.",
                ));
                Ok(false)
            }
        }
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

    fn check_slot_limit(&self) -> Result<(), EngineError> {
        let active = self
            .ledger
            .list_apps()?
            .into_iter()
            .filter(|app| !app.retired && app.latest_evolution.is_some())
            .collect::<Vec<_>>();
        if active.len() >= 3 {
            let candidate = &active[0].name;
            self.emit_upsell_once(
                "slots",
                "A paid Apple Developer membership raises this limit: developer.apple.com.",
            )?;
            self.events.emit(Event::handoff(format!(
                "Run `tohseno retire {candidate}` to free one iPhone slot."
            )));
            return Err(EngineError::SlotLimit);
        }
        Ok(())
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
        self.events
            .emit(Event::status(format!("signing evolution {shot_number}…")));
        let app = sign::build_signed(sign::SignRequest {
            source,
            artifact_directory,
            app_name,
            bundle_id,
            shot_number,
            device: &device,
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
                    (Some("Plug in your iPhone with a cable."), None)
                }
                DeviceState::TrustRequired => (Some("Tap Trust on your iPhone."), None),
                DeviceState::DeveloperModeRequired => (
                    Some("Enable Developer Mode: Settings → Privacy & Security → Developer Mode, then let your phone restart."),
                    None,
                ),
            };
            if let Some(device) = ready {
                self.events
                    .emit(Event::status(format!("found {} over USB.", device.name)));
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

fn default_initial_organs() -> Vec<InitialOrganPlan> {
    vec![
        InitialOrganPlan {
            organ_id: "installation_identity".into(),
            provides: vec![
                "embedded_shot_identity".into(),
                "app_scoped_installation_identity".into(),
            ],
            owns_state: vec!["installation_key_reference".into()],
            permissions: Vec::new(),
            dependencies: Vec::new(),
            emits: vec!["identity_ready".into()],
            consumes: Vec::new(),
            satisfies_genome_constraints: vec![
                "Preserve Shot identity and signed continuity across every accepted version."
                    .into(),
            ],
            acceptance_tests: vec![
                "Embedded metadata matches the accepted Shot, Expression, Version, and Genome facts."
                    .into(),
                "Fascia declares app-scoped installation identity without substituting it for ownership."
                    .into(),
            ],
            platforms: vec!["iphone".into()],
        },
        InitialOrganPlan {
            organ_id: "local_memory".into(),
            provides: vec!["local_persistence".into()],
            owns_state: vec!["owner_created_application_state".into()],
            permissions: Vec::new(),
            dependencies: vec!["installation_identity".into()],
            emits: vec!["state_changed".into()],
            consumes: vec!["identity_ready".into()],
            satisfies_genome_constraints: vec![
                "Keep owner-created state available locally and fail without inventing data."
                    .into(),
            ],
            acceptance_tests: vec![
                "Fascia declares local-first storage with no cloud default.".into(),
            ],
            platforms: vec!["iphone".into()],
        },
        InitialOrganPlan {
            organ_id: "native_navigation".into(),
            provides: vec!["native_navigation".into()],
            owns_state: vec!["navigation_path".into()],
            permissions: Vec::new(),
            dependencies: Vec::new(),
            emits: vec!["destination_changed".into()],
            consumes: Vec::new(),
            satisfies_genome_constraints: vec![
                "The expression makes the preserved intention tangible without setup ceremony."
                    .into(),
            ],
            acceptance_tests: vec![
                "The native Apple source anatomy builds into the retained application artifact."
                    .into(),
            ],
            platforms: vec!["iphone".into()],
        },
        InitialOrganPlan {
            organ_id: "version_feedback".into(),
            provides: vec!["exact_version_feedback".into()],
            owns_state: vec!["private_feedback_records".into()],
            permissions: Vec::new(),
            dependencies: vec!["installation_identity".into()],
            emits: vec!["feedback_recorded".into()],
            consumes: vec!["identity_ready".into()],
            satisfies_genome_constraints: vec![
                "Keep intention, feedback, and owner data private by default.".into(),
            ],
            acceptance_tests: vec![
                "Embedded metadata supplies the exact ExpressionID, VersionID, and build identity needed for feedback binding."
                    .into(),
            ],
            platforms: vec!["iphone".into()],
        },
    ]
}

fn validate_initial_expression_plan(
    plan: &InitialExpressionPlan,
    genome: &tohseno_protocol::Genome,
) -> Result<(), EngineError> {
    if plan.schema != "tohseno.initial-expression-plan/1"
        || plan.kind != "native_apple_application"
        || plan.platforms.as_slice() != ["iphone"]
        || plan.organs.is_empty()
    {
        return Err(EngineError::ProtocolBodyIncomplete(
            "the first factory resolves exactly one native iPhone expression".into(),
        ));
    }
    if plan.genome_revision != genome.revision
        || plan.genome_digest != genome.digest().map_err(ShotLayoutError::from)?
    {
        return Err(EngineError::ProtocolBodyIncomplete(
            "the initial Expression plan does not bind the reviewed Genome".into(),
        ));
    }

    let incompatible_platforms = genome
        .platform_commitments
        .iter()
        .filter(|commitment| !native_iphone_satisfies_platform_commitment(commitment))
        .cloned()
        .collect::<Vec<_>>();
    if !incompatible_platforms.is_empty() {
        return Err(EngineError::ProtocolBodyIncomplete(format!(
            "the native iPhone factory cannot satisfy Genome platform commitment(s): {}",
            incompatible_platforms.join("; ")
        )));
    }

    let mut declared_organs = BTreeSet::new();
    let mut provided_capabilities = BTreeSet::new();
    for organ in &plan.organs {
        if organ.organ_id.trim().is_empty() || declared_organs.contains(organ.organ_id.as_str()) {
            return Err(EngineError::ProtocolBodyIncomplete(
                "the initial Organ graph contains an empty or duplicate organ ID".into(),
            ));
        }
        if organ
            .dependencies
            .iter()
            .any(|dependency| !declared_organs.contains(dependency.as_str()))
        {
            return Err(EngineError::ProtocolBodyIncomplete(format!(
                "Organ {} depends on an undeclared or later Organ",
                organ.organ_id
            )));
        }
        declared_organs.insert(organ.organ_id.as_str());
        if !plan
            .platforms
            .iter()
            .all(|platform| organ.platforms.contains(platform))
        {
            return Err(EngineError::ProtocolBodyIncomplete(format!(
                "Organ {} does not support every Expression platform",
                organ.organ_id
            )));
        }
        provided_capabilities.extend(organ.provides.iter().map(String::as_str));
    }

    let missing = genome
        .required_capabilities
        .iter()
        .filter(|capability| !provided_capabilities.contains(capability.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(EngineError::ProtocolBodyIncomplete(format!(
            "the resolved Organ graph does not provide required Genome capability(s): {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

fn native_iphone_satisfies_platform_commitment(commitment: &str) -> bool {
    let words = commitment
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<BTreeSet<_>>();
    let incompatible_words = [
        "android", "browser", "ipad", "ipados", "linux", "mac", "macos", "server", "tvos",
        "visionos", "watchos", "web", "website", "windows",
    ];
    !(incompatible_words.iter().any(|word| words.contains(*word))
        || words.contains("apple") && words.contains("watch")
        || words.contains("apple") && words.contains("vision")
        || words.contains("apple") && words.contains("tv"))
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

#[derive(Debug)]
pub enum EngineError {
    Io(std::io::Error),
    Config(ConfigError),
    Ledger(LedgerError),
    Intent(IntentError),
    Genome(GenomeError),
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
                    "no Xcode project in the {app} folder yet — build one, then `tohseno evolve {app}`"
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
            Self::SlotLimit => write!(f, "the free Apple ID app limit is full"),
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

impl From<build::BuildError> for EngineError {
    fn from(value: build::BuildError) -> Self {
        Self::Build(value)
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

    fn quiet_press_request() -> ShotRequest {
        ShotRequest {
            app_name: "quiet-press".into(),
            intent: Intent {
                prompt: "A quiet place\n\nfor one honest sentence.".into(),
                images: Vec::new(),
            },
            selected_feedback_actions: Vec::new(),
        }
    }

    fn resolved_organs(plan: &InitialExpressionPlan, expression_id: ExpressionId) -> Vec<Organ> {
        plan.organs
            .iter()
            .map(|organ| Organ {
                schema: ORGAN_SCHEMA.into(),
                expression_id,
                organ_id: organ.organ_id.clone(),
                provides: organ.provides.clone(),
                owns_state: organ.owns_state.clone(),
                permissions: organ.permissions.clone(),
                dependencies: organ.dependencies.clone(),
                emits: organ.emits.clone(),
                consumes: organ.consumes.clone(),
                satisfies_genome_constraints: organ.satisfies_genome_constraints.clone(),
                acceptance_tests: organ.acceptance_tests.clone(),
                platforms: organ.platforms.clone(),
            })
            .collect()
    }

    #[test]
    fn generated_bundle_identity_uses_the_candidate_device_namespace() {
        let bundle_id = candidate_bundle_id("Alice Example", "quiet-press");
        assert_eq!(bundle_id, "org.tohseno.genesis.alice-example.quiet-press");
        install::require_candidate_namespace(&bundle_id).unwrap();
    }

    #[test]
    fn candidate_namespace_preserves_a_deterministic_fallback_identity() {
        assert_eq!(
            candidate_bundle_id("---", "press"),
            "org.tohseno.genesis.user.press"
        );
    }

    #[test]
    fn initial_genome_and_expression_plan_are_deterministic_unaccepted_views() {
        let request = quiet_press_request();
        let first = Engine::propose_initial_genome(&request).unwrap();
        let second = Engine::propose_initial_genome(&request).unwrap();
        assert_eq!(first, second);
        first.validate().unwrap();
        assert!(first
            .purpose
            .contains("A quiet place for one honest sentence."));
        let plan = Engine::propose_initial_expression_plan(&request, &first).unwrap();
        assert_eq!(plan.genome_digest, first.digest().unwrap());
        assert_eq!(plan.kind, "native_apple_application");
        assert_eq!(plan.organs.len(), 4);
        assert!(plan
            .organs
            .iter()
            .any(|organ| organ.organ_id == "version_feedback"));

        let expression_id = ExpressionId::from_bytes([0x45; 32]);
        let organs = resolved_organs(&plan, expression_id);
        let graph_digest = capability_graph_digest(&organs).unwrap();
        let mut reversed = organs;
        reversed.reverse();
        assert_eq!(
            graph_digest,
            capability_graph_digest(&reversed).unwrap(),
            "the capability lock must not depend on input iteration order"
        );
        let provided = plan
            .organs
            .iter()
            .flat_map(|organ| organ.provides.iter())
            .collect::<BTreeSet<_>>();
        assert!(first
            .required_capabilities
            .iter()
            .all(|capability| provided.contains(capability)));
    }

    #[test]
    fn initial_expression_plan_rejects_unsatisfied_genome_capabilities() {
        let request = quiet_press_request();
        let mut genome = Engine::propose_initial_genome(&request).unwrap();
        genome
            .required_capabilities
            .push("cloud_synchronization".into());

        let error = Engine::propose_initial_expression_plan(&request, &genome).unwrap_err();
        assert!(error.to_string().contains("cloud_synchronization"));
    }

    #[test]
    fn initial_expression_plan_rejects_non_iphone_genome_commitments() {
        let request = quiet_press_request();
        let mut genome = Engine::propose_initial_genome(&request).unwrap();
        genome.platform_commitments = vec!["The first expression is a native macOS app.".into()];

        let error = Engine::propose_initial_expression_plan(&request, &genome).unwrap_err();
        assert!(error
            .to_string()
            .contains("cannot satisfy Genome platform commitment"));
    }

    #[test]
    fn initial_expression_plan_rejects_unsupported_or_invalid_organ_graphs() {
        let request = quiet_press_request();
        let genome = Engine::propose_initial_genome(&request).unwrap();
        let mut plan = Engine::propose_initial_expression_plan(&request, &genome).unwrap();
        plan.organs[0].platforms = vec!["macos".into()];
        assert!(validate_initial_expression_plan(&plan, &genome)
            .unwrap_err()
            .to_string()
            .contains("does not support every Expression platform"));

        let mut self_dependent =
            Engine::propose_initial_expression_plan(&request, &genome).unwrap();
        self_dependent.organs[0].dependencies = vec!["installation_identity".into()];
        assert!(validate_initial_expression_plan(&self_dependent, &genome)
            .unwrap_err()
            .to_string()
            .contains("undeclared or later Organ"));
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
    fn engine_standing_orders_do_not_count_as_builder_work() {
        // A prepared Shot folder holds AGENTS.md and CLAUDE.md written by the
        // engine itself. Re-creating after a failed first handoff must see
        // that folder as pristine, or the Shot is wedged forever (F-003).
        let temporary = tempfile::tempdir().unwrap();
        let family = temporary.path().join("family");
        let machine = temporary.path().join("machine");
        let working = family.join("quiet-press");
        fs::create_dir_all(&working).unwrap();
        fs::write(working.join("AGENTS.md"), "# This folder is a TOHSENO Shot\n").unwrap();
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
}
