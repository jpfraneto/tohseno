use crate::command::{CommandKind, CommandOrigin, CommandState};
use crate::entitlement::{EntitlementStatus, EntitlementStore, SuccessfulDayEvidence};
use crate::execution_manager;
use crate::journal::{AdmissionMetadata, CommandJournal, JournalError};
use crate::snapshot::{
    build_workspace_snapshot, load_shot_icon, IconDescriptor, ShotKind, WorkspaceSnapshot,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use tohseno_engine::gates::intent::Intent;
use tohseno_engine::harness::HarnessSelection;
use tohseno_engine::machine::Evolved;
use tohseno_engine::{AcceptedVersionBase, Engine, EventBus, ShotRequest, StoredFeedback};
use tohseno_protocol::digest::{sha256, Bytes32, ExpressionId, ShotId, VersionId};
use tokio::sync::{Mutex, OwnedMutexGuard};

const MAXIMUM_INTENTION_BYTES: usize = 4 * 1024 * 1024;
const MAXIMUM_NOTE_BYTES: usize = 100_000;
const MAXIMUM_REFERENCE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub enum ApplicationError {
    Journal(JournalError),
    Engine(tohseno_engine::EngineError),
    Ledger(tohseno_engine::LedgerError),
    Layout(tohseno_engine::ShotLayoutError),
    Io(std::io::Error),
    Json(serde_json::Error),
    Invalid(String),
    Orchestration(String),
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Journal(error) => write!(formatter, "{error}"),
            Self::Engine(error) => write!(formatter, "{error}"),
            Self::Ledger(error) => write!(formatter, "{error}"),
            Self::Layout(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Invalid(message) | Self::Orchestration(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ApplicationError {}

impl From<JournalError> for ApplicationError {
    fn from(value: JournalError) -> Self {
        Self::Journal(value)
    }
}

impl From<tohseno_engine::EngineError> for ApplicationError {
    fn from(value: tohseno_engine::EngineError) -> Self {
        Self::Engine(value)
    }
}

impl From<tohseno_engine::LedgerError> for ApplicationError {
    fn from(value: tohseno_engine::LedgerError) -> Self {
        Self::Ledger(value)
    }
}

impl From<tohseno_engine::ShotLayoutError> for ApplicationError {
    fn from(value: tohseno_engine::ShotLayoutError) -> Self {
        Self::Layout(value)
    }
}

impl From<std::io::Error> for ApplicationError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ApplicationError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceInput {
    pub display_filename: String,
    pub media_type: String,
    pub origin: String,
    pub bytes: Vec<u8>,
}

impl ReferenceInput {
    pub fn from_path(path: &Path) -> Result<Self, ApplicationError> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MAXIMUM_REFERENCE_BYTES
        {
            return Err(ApplicationError::Invalid(format!(
                "reference {} must be a regular file no larger than {MAXIMUM_REFERENCE_BYTES} bytes",
                path.display()
            )));
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let mut file = options.open(path)?;
        let opened = file.metadata()?;
        if !opened.is_file() || opened.len() != metadata.len() {
            return Err(ApplicationError::Invalid(
                "reference changed while it was opened".into(),
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
        std::io::Read::by_ref(&mut file)
            .take(MAXIMUM_REFERENCE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAXIMUM_REFERENCE_BYTES {
            return Err(ApplicationError::Invalid("reference is too large".into()));
        }
        let display_filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| ApplicationError::Invalid("reference filename is not UTF-8".into()))?
            .to_owned();
        let media_type = media_type_for_filename(&display_filename)?.into();
        Ok(Self {
            display_filename,
            media_type,
            origin: path.display().to_string(),
            bytes,
        })
    }
}

#[derive(Clone, Debug)]
pub struct CreateShotCommand {
    pub command_id: String,
    pub origin: CommandOrigin,
    pub origin_device_id: Option<String>,
    pub name: String,
    pub name_was_supplied: bool,
    pub intention: String,
    pub references: Vec<ReferenceInput>,
    pub submitted_at: Option<String>,
    pub harness_selection: Option<HarnessSelection>,
}

#[derive(Clone, Debug)]
pub struct EvolveShotCommand {
    pub command_id: String,
    pub origin: CommandOrigin,
    pub origin_device_id: Option<String>,
    pub name: String,
    pub base_expression_id: ExpressionId,
    pub base_version_id: VersionId,
    pub base_version_ordinal: u64,
    pub intention: String,
    pub selected_feedback_actions: Vec<Bytes32>,
    pub references: Vec<ReferenceInput>,
    pub submitted_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SubmitFeedbackCommand {
    pub command_id: String,
    pub origin: CommandOrigin,
    pub origin_device_id: Option<String>,
    pub name: String,
    pub shot_id: ShotId,
    pub expression_id: ExpressionId,
    pub version_id: VersionId,
    pub version_ordinal: u64,
    pub body: String,
    pub companion_signature: Option<String>,
    pub submitted_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SubmitMarketingNoteCommand {
    pub command_id: String,
    pub origin: CommandOrigin,
    pub origin_device_id: Option<String>,
    pub name: String,
    pub shot_id: ShotId,
    pub body: String,
    pub companion_signature: Option<String>,
    pub submitted_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateShotReceipt {
    pub schema: String,
    pub command_id: String,
    pub shot_id: ShotId,
    pub execution_id: String,
    pub state: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvolveShotReceipt {
    pub schema: String,
    pub command_id: String,
    pub shot_id: ShotId,
    pub execution_id: String,
    pub base_expression_id: ExpressionId,
    pub base_version_id: VersionId,
    pub base_version_ordinal: u64,
    pub state: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackReceipt {
    pub schema: String,
    pub command_id: String,
    pub shot_id: ShotId,
    pub expression_id: ExpressionId,
    pub version_id: VersionId,
    pub version_ordinal: u64,
    pub feedback_id: Bytes32,
    pub action_commitment: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketingNoteReceipt {
    pub schema: String,
    pub command_id: String,
    pub note_id: String,
    pub shot_id: ShotId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryDefaults {
    pub schema: String,
    pub ready: bool,
    pub harness_id: Option<String>,
    pub harness_label: Option<String>,
    pub model_id: Option<String>,
    pub model_label: Option<String>,
    pub route_id: Option<String>,
    pub route_label: Option<String>,
    pub harnesses: Vec<FactoryHarnessOption>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryHarnessOption {
    pub id: String,
    pub label: String,
    pub models: Vec<FactoryModelOption>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryModelOption {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRecoverySummary {
    pub schema: String,
    pub examined: u64,
    pub resumed: u64,
    pub terminal: u64,
    pub still_running: u64,
    pub retryable_failures: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetireShotReceipt {
    pub schema: String,
    pub shot_id: String,
    pub display_name: String,
    pub phone_app_removed: bool,
    pub source_preserved: bool,
}

#[derive(Clone)]
pub struct ShotApplicationService {
    engine: Arc<Engine>,
    command_journal: CommandJournal,
    events: EventBus,
    workspace_id: String,
    selection: Option<HarnessSelection>,
    command_claims: Arc<Mutex<HashMap<String, Weak<Mutex<()>>>>>,
    entitlement: Option<EntitlementStore>,
}

impl ShotApplicationService {
    pub fn new(
        engine: Engine,
        command_journal: CommandJournal,
        events: EventBus,
        workspace_id: impl Into<String>,
    ) -> Self {
        Self {
            engine: Arc::new(engine),
            command_journal,
            events,
            workspace_id: workspace_id.into(),
            selection: None,
            command_claims: Arc::new(Mutex::new(HashMap::new())),
            entitlement: None,
        }
    }

    pub fn with_selection(mut self, selection: HarnessSelection) -> Self {
        self.selection = Some(selection);
        self
    }

    /// Installs the private product authority used by every real frontend.
    /// Unit-level callers that exercise lower orchestration primitives may
    /// omit it; the Local Workspace Service always supplies it.
    pub fn with_entitlement(mut self, entitlement: EntitlementStore) -> Self {
        self.entitlement = Some(entitlement);
        self
    }

    pub fn entitlement_status(&self) -> Result<Option<EntitlementStatus>, ApplicationError> {
        self.entitlement
            .as_ref()
            .map(EntitlementStore::status_now)
            .transpose()
            .map_err(|error| ApplicationError::Orchestration(error.to_string()))
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn journal(&self) -> &CommandJournal {
        &self.command_journal
    }

    async fn claim_command(&self, command_id: &str) -> OwnedMutexGuard<()> {
        let claim = {
            let mut claims = self.command_claims.lock().await;
            if claims.len() >= 1_024 {
                claims.retain(|_, claim| claim.strong_count() > 0);
            }
            if let Some(claim) = claims.get(command_id).and_then(Weak::upgrade) {
                claim
            } else {
                let claim = Arc::new(Mutex::new(()));
                claims.insert(command_id.to_owned(), Arc::downgrade(&claim));
                claim
            }
        };
        claim.lock_owned().await
    }

    /// Replays every nonterminal durable command through this same application
    /// boundary. This is safe on every service start: command payloads and
    /// private inputs are rechecked against their admission commitments, while
    /// semantic engine operations and execution identities remain idempotent.
    pub async fn recover_commands(&self) -> Result<CommandRecoverySummary, ApplicationError> {
        let records = self.command_journal.recoverable()?;
        let mut summary = CommandRecoverySummary {
            schema: "tohseno.command-recovery-summary/1".into(),
            ..CommandRecoverySummary::default()
        };
        for record in records {
            summary.examined += 1;
            let (_, before) = self.command_journal.load(&record.command_id)?;
            let recovered = if matches!(
                before.state,
                CommandState::Running | CommandState::WaitingForDevice
            ) {
                self.resume_running_execution(&record.command_id, &before)
            } else {
                self.replay_command(&record).await
            };
            let (_, after) = self.command_journal.load(&record.command_id)?;
            if recovered.is_ok() {
                summary.resumed += 1;
            } else {
                summary.retryable_failures += 1;
            }
            if after.state.terminal() {
                summary.terminal += 1;
            } else {
                summary.still_running += 1;
            }
        }
        Ok(summary)
    }

    pub async fn create_shot(
        &self,
        command: CreateShotCommand,
    ) -> Result<CreateShotReceipt, ApplicationError> {
        self.require_new_factory_mutation(&command.command_id)?;
        let (payload, files) = create_payload(&command)?;
        let admission = self.command_journal.admit_with_files(
            AdmissionMetadata {
                command_id: command.command_id.clone(),
                command_kind: CommandKind::ShotCreate,
                origin: command.origin,
                origin_device_id: command.origin_device_id.clone(),
                workspace_id: self.workspace_id.clone(),
                shot_id: None,
                base_expression_id: None,
                base_version_id: None,
                submitted_at: command.submitted_at.clone(),
            },
            &payload,
            &files,
        )?;
        let _claim = self.claim_command(&command.command_id).await;
        let (_, current_status) = self.command_journal.load(&command.command_id)?;
        if let Some(receipt) = receipt_from_status::<CreateShotReceipt>(&current_status)? {
            self.ensure_and_monitor_execution(
                &command.command_id,
                &command.name,
                &receipt.execution_id,
            )?;
            return Ok(receipt);
        }
        if let Err(error) = validate_intention(&command.intention) {
            return self.reject(&command.command_id, &error.to_string());
        }
        let resumed_state =
            resume_command(&self.command_journal, &command.command_id, &current_status)?;
        if resumed_state == CommandState::Accepted {
            if let Some(execution) = execution_manager::load_command_execution(
                &self.engine.ledger().working_tree(&command.name),
                &command.command_id,
            )
            .map_err(|error| ApplicationError::Orchestration(error.to_string()))?
            {
                if execution.app_name != command.name
                    || !matches!(
                        execution.mode,
                        tohseno_engine::ExecutionMode::Conception
                            | tohseno_engine::ExecutionMode::BirthMaterialization
                    )
                {
                    return Err(ApplicationError::Orchestration(
                        "reserved creation execution is bound to different work".into(),
                    ));
                }
                let receipt = CreateShotReceipt {
                    schema: "tohseno.create-shot-receipt/1".into(),
                    command_id: command.command_id.clone(),
                    shot_id: execution.shot_id,
                    execution_id: execution.execution_id,
                    state: "running".into(),
                };
                self.command_journal.transition(
                    &command.command_id,
                    CommandState::Running,
                    Some(serde_json::to_value(&receipt)?),
                    None,
                )?;
                self.ensure_and_monitor_execution(
                    &command.command_id,
                    &command.name,
                    &receipt.execution_id,
                )?;
                return Ok(receipt);
            }
        }
        let selection = match command
            .harness_selection
            .clone()
            .map(Ok)
            .unwrap_or_else(|| self.resolve_selection())
            .and_then(|selection| {
                execution_manager::validate_selection(&selection)
                    .map(|()| selection)
                    .map_err(|error| ApplicationError::Orchestration(error.to_string()))
            }) {
            Ok(selection) => selection,
            Err(error) => {
                return self.reject(
                    &command.command_id,
                    &format!("factory harness is unavailable: {error}"),
                );
            }
        };
        let image_paths = payload
            .references
            .iter()
            .map(|reference| admission.directory.join("inputs").join(&reference.filename))
            .collect();
        let request = ShotRequest {
            app_name: command.name.clone(),
            name_was_supplied: command.name_was_supplied,
            intent: Intent {
                prompt: command.intention,
                images: image_paths,
            },
            selected_feedback_actions: Vec::new(),
        };
        if resumed_state == CommandState::Validated {
            self.command_journal.transition(
                &command.command_id,
                CommandState::Accepted,
                None,
                None,
            )?;
        }
        let creation = match self.engine.create(&request) {
            Ok(creation) => creation,
            Err(error) => {
                let _ = self.command_journal.transition(
                    &command.command_id,
                    CommandState::Failed,
                    None,
                    Some(error.to_string()),
                );
                return Err(error.into());
            }
        };
        let app = self.engine.ledger().load_app(&command.name)?;
        let shot_id = app.shot_id.ok_or_else(|| {
            ApplicationError::Orchestration("factory creation did not reserve a ShotID".into())
        })?;
        let execution = execution_manager::prepare_for_command(
            &self.engine,
            &creation,
            &command.name,
            &selection,
            false,
            &self.events,
            &command.command_id,
        )
        .map_err(|error| ApplicationError::Orchestration(error.to_string()))?;
        let receipt = CreateShotReceipt {
            schema: "tohseno.create-shot-receipt/1".into(),
            command_id: command.command_id.clone(),
            shot_id,
            execution_id: execution.execution_id,
            state: "running".into(),
        };
        self.command_journal.transition(
            &command.command_id,
            CommandState::Running,
            Some(serde_json::to_value(&receipt)?),
            None,
        )?;
        self.ensure_and_monitor_execution(
            &command.command_id,
            &command.name,
            &receipt.execution_id,
        )?;
        Ok(receipt)
    }

    pub async fn evolve_shot(
        &self,
        command: EvolveShotCommand,
    ) -> Result<EvolveShotReceipt, ApplicationError> {
        self.require_new_factory_mutation(&command.command_id)?;
        let (payload, files) = evolve_payload(&command)?;
        let app = self.engine.ledger().load_app(&command.name)?;
        let shot_id = app.shot_id.ok_or_else(|| {
            ApplicationError::Invalid("evolution target is not a factory Shot".into())
        })?;
        let admission = self.command_journal.admit_with_files(
            AdmissionMetadata {
                command_id: command.command_id.clone(),
                command_kind: CommandKind::ShotEvolve,
                origin: command.origin,
                origin_device_id: command.origin_device_id.clone(),
                workspace_id: self.workspace_id.clone(),
                shot_id: Some(shot_id),
                base_expression_id: Some(command.base_expression_id),
                base_version_id: Some(command.base_version_id),
                submitted_at: command.submitted_at.clone(),
            },
            &payload,
            &files,
        )?;
        let _claim = self.claim_command(&command.command_id).await;
        let (_, current_status) = self.command_journal.load(&command.command_id)?;
        if let Some(receipt) = receipt_from_status::<EvolveShotReceipt>(&current_status)? {
            self.ensure_and_monitor_execution(
                &command.command_id,
                &command.name,
                &receipt.execution_id,
            )?;
            return Ok(receipt);
        }
        if command.intention.is_empty() && command.selected_feedback_actions.is_empty() {
            return self.reject(
                &command.command_id,
                "an evolution requires an exact intention or selected Feedback action",
            );
        }
        if !command.intention.is_empty() {
            if let Err(error) = validate_intention(&command.intention) {
                return self.reject(&command.command_id, &error.to_string());
            }
        }
        let resumed_state =
            resume_command(&self.command_journal, &command.command_id, &current_status)?;
        if resumed_state == CommandState::Accepted {
            if let Some(execution) = execution_manager::load_command_execution(
                &self.engine.ledger().working_tree(&command.name),
                &command.command_id,
            )
            .map_err(|error| ApplicationError::Orchestration(error.to_string()))?
            {
                if execution.app_name != command.name
                    || execution.mode != tohseno_engine::ExecutionMode::EvolutionMaterialization
                    || execution.version_ordinal != command.base_version_ordinal.saturating_add(1)
                {
                    return Err(ApplicationError::Orchestration(
                        "reserved evolution execution is bound to different work".into(),
                    ));
                }
                let receipt = EvolveShotReceipt {
                    schema: "tohseno.evolve-shot-receipt/1".into(),
                    command_id: command.command_id.clone(),
                    shot_id: execution.shot_id,
                    execution_id: execution.execution_id,
                    base_expression_id: command.base_expression_id,
                    base_version_id: command.base_version_id,
                    base_version_ordinal: command.base_version_ordinal,
                    state: "running".into(),
                };
                self.command_journal.transition(
                    &command.command_id,
                    CommandState::Running,
                    Some(serde_json::to_value(&receipt)?),
                    None,
                )?;
                self.ensure_and_monitor_execution(
                    &command.command_id,
                    &command.name,
                    &receipt.execution_id,
                )?;
                return Ok(receipt);
            }
        }
        let expected = AcceptedVersionBase {
            expression_id: command.base_expression_id,
            version_id: command.base_version_id,
            version_ordinal: command.base_version_ordinal,
        };
        if self.engine.current_accepted_base(&command.name)? != expected {
            return self.reject(
                &command.command_id,
                "stale evolution: the exact accepted base changed before admission",
            );
        }
        let selection = match self.resolve_selection().and_then(|selection| {
            execution_manager::validate_selection(&selection)
                .map(|()| selection)
                .map_err(|error| ApplicationError::Orchestration(error.to_string()))
        }) {
            Ok(selection) => selection,
            Err(error) => {
                return self.reject(
                    &command.command_id,
                    &format!("factory harness is unavailable: {error}"),
                );
            }
        };
        let image_paths = payload
            .references
            .iter()
            .map(|reference| admission.directory.join("inputs").join(&reference.filename))
            .collect();
        let request = ShotRequest {
            app_name: command.name.clone(),
            name_was_supplied: true,
            intent: Intent {
                prompt: command.intention,
                images: image_paths,
            },
            selected_feedback_actions: command.selected_feedback_actions,
        };
        if resumed_state == CommandState::Validated {
            self.command_journal.transition(
                &command.command_id,
                CommandState::Accepted,
                None,
                None,
            )?;
        }
        let creation = match self.engine.evolve_exact(&request, expected).await {
            Ok(Evolved::Conducted(creation)) => creation,
            Ok(_) => {
                return Err(ApplicationError::Orchestration(
                    "nonempty evolution did not enter materialization".into(),
                ))
            }
            Err(error) => {
                let _ = self.command_journal.transition(
                    &command.command_id,
                    CommandState::Failed,
                    None,
                    Some(error.to_string()),
                );
                return Err(error.into());
            }
        };
        let execution = execution_manager::prepare_for_command(
            &self.engine,
            &creation,
            &command.name,
            &selection,
            false,
            &self.events,
            &command.command_id,
        )
        .map_err(|error| ApplicationError::Orchestration(error.to_string()))?;
        let receipt = EvolveShotReceipt {
            schema: "tohseno.evolve-shot-receipt/1".into(),
            command_id: command.command_id.clone(),
            shot_id,
            execution_id: execution.execution_id,
            base_expression_id: command.base_expression_id,
            base_version_id: command.base_version_id,
            base_version_ordinal: command.base_version_ordinal,
            state: "running".into(),
        };
        self.command_journal.transition(
            &command.command_id,
            CommandState::Running,
            Some(serde_json::to_value(&receipt)?),
            None,
        )?;
        self.ensure_and_monitor_execution(
            &command.command_id,
            &command.name,
            &receipt.execution_id,
        )?;
        Ok(receipt)
    }

    pub async fn submit_feedback(
        &self,
        command: SubmitFeedbackCommand,
    ) -> Result<FeedbackReceipt, ApplicationError> {
        let payload = FeedbackPayload::from(&command);
        let admission = self.command_journal.admit(
            AdmissionMetadata {
                command_id: command.command_id.clone(),
                command_kind: CommandKind::FeedbackSubmit,
                origin: command.origin,
                origin_device_id: command.origin_device_id.clone(),
                workspace_id: self.workspace_id.clone(),
                shot_id: Some(command.shot_id),
                base_expression_id: Some(command.expression_id),
                base_version_id: Some(command.version_id),
                submitted_at: command.submitted_at.clone(),
            },
            &payload,
        )?;
        let _claim = self.claim_command(&command.command_id).await;
        let (_, current_status) = self.command_journal.load(&command.command_id)?;
        if let Some(receipt) = receipt_from_status::<FeedbackReceipt>(&current_status)? {
            return Ok(receipt);
        }
        if command.body.is_empty() || command.body.len() > MAXIMUM_NOTE_BYTES {
            return self.reject(
                &command.command_id,
                "feedback must contain 1..=100000 UTF-8 bytes",
            );
        }
        let resumed_state =
            resume_command(&self.command_journal, &command.command_id, &current_status)?;
        let app = self.engine.ledger().load_app(&command.name)?;
        if app.shot_id != Some(command.shot_id) || app.expression_id != Some(command.expression_id)
        {
            return self.reject(
                &command.command_id,
                "feedback target identity does not match the local Shot",
            );
        }
        let exact = self
            .engine
            .accepted_version_base(&command.name, command.version_ordinal);
        if !matches!(
            exact,
            Ok(base)
                if base.expression_id == command.expression_id
                    && base.version_id == command.version_id
                    && base.version_ordinal == command.version_ordinal
        ) {
            return self.reject(
                &command.command_id,
                "feedback target is not an exact accepted Version of this Expression",
            );
        }
        if resumed_state == CommandState::Validated {
            self.command_journal.transition(
                &command.command_id,
                CommandState::Accepted,
                None,
                None,
            )?;
        }
        let stored = self.engine.record_feedback_for_command(
            &command.name,
            command.version_ordinal,
            &command.body,
            &command.command_id,
            &admission.record.submitted_at,
        )?;
        let receipt = feedback_receipt(&command, stored);
        self.command_journal.transition(
            &command.command_id,
            CommandState::Completed,
            Some(serde_json::to_value(&receipt)?),
            None,
        )?;
        Ok(receipt)
    }

    pub async fn submit_marketing_note(
        &self,
        command: SubmitMarketingNoteCommand,
    ) -> Result<MarketingNoteReceipt, ApplicationError> {
        let payload = MarketingPayload::from(&command);
        let admission = self.command_journal.admit(
            AdmissionMetadata {
                command_id: command.command_id.clone(),
                command_kind: CommandKind::MarketingSubmit,
                origin: command.origin,
                origin_device_id: command.origin_device_id.clone(),
                workspace_id: self.workspace_id.clone(),
                shot_id: Some(command.shot_id),
                base_expression_id: None,
                base_version_id: None,
                submitted_at: command.submitted_at.clone(),
            },
            &payload,
        )?;
        let _claim = self.claim_command(&command.command_id).await;
        let (_, current_status) = self.command_journal.load(&command.command_id)?;
        if let Some(receipt) = receipt_from_status::<MarketingNoteReceipt>(&current_status)? {
            return Ok(receipt);
        }
        if command.body.is_empty() || command.body.len() > MAXIMUM_NOTE_BYTES {
            return self.reject(
                &command.command_id,
                "marketing note must contain 1..=100000 UTF-8 bytes",
            );
        }
        let resumed_state =
            resume_command(&self.command_journal, &command.command_id, &current_status)?;
        let app = self.engine.ledger().load_app(&command.name)?;
        if app.shot_id != Some(command.shot_id) {
            return self.reject(
                &command.command_id,
                "marketing note target does not match the local Shot",
            );
        }
        if resumed_state == CommandState::Validated {
            self.command_journal.transition(
                &command.command_id,
                CommandState::Accepted,
                None,
                None,
            )?;
        }
        let submitted_at = admission.record.submitted_at.clone();
        let note_id = format!(
            "note_{}",
            sha256(format!("TOHSENO-MARKETING-NOTE-ID-V1\0{}", command.command_id).as_bytes())
                .to_string()
                .trim_start_matches("0x")
        );
        let record = MarketingRecord {
            schema: "tohseno.marketing-note/1".into(),
            note_id: note_id.clone(),
            shot_id: command.shot_id,
            body: command.body,
            created_at: submitted_at,
            author_device_id: command.origin_device_id,
            companion_signature: command.companion_signature,
        };
        store_marketing_record(&self.engine.ledger().working_tree(&command.name), &record)?;
        let receipt = MarketingNoteReceipt {
            schema: "tohseno.marketing-note-receipt/1".into(),
            command_id: command.command_id.clone(),
            note_id,
            shot_id: command.shot_id,
        };
        self.command_journal.transition(
            &command.command_id,
            CommandState::Completed,
            Some(serde_json::to_value(&receipt)?),
            None,
        )?;
        Ok(receipt)
    }

    pub async fn workspace_snapshot(&self) -> Result<WorkspaceSnapshot, ApplicationError> {
        build_workspace_snapshot(&self.engine, &self.workspace_id, 1, 0, 0)
            .map_err(|error| ApplicationError::Orchestration(error.to_string()))
    }

    /// Removes an accepted app from the connected iPhone and retires its local
    /// projection. Source, immutable Shot history, and private receipts stay on
    /// this Mac so a UI action can never become an accidental history eraser.
    pub async fn retire_shot(&self, shot_id: &str) -> Result<RetireShotReceipt, ApplicationError> {
        let snapshot = self.workspace_snapshot().await?;
        let shot = snapshot
            .shots
            .iter()
            .find(|shot| shot.shot_id == shot_id)
            .ok_or_else(|| ApplicationError::Invalid("app does not exist".into()))?;
        let app = self.engine.ledger().load_app(&shot.display_name)?;
        if shot.execution.as_ref().is_some_and(|execution| {
            snapshot
                .active_executions
                .iter()
                .any(|active| active.execution_id == execution.execution_id)
        }) {
            return Err(ApplicationError::Invalid(
                "this app is still building; wait for it to finish before deleting it".into(),
            ));
        }
        if app.retired {
            return Ok(retire_receipt(shot_id, &shot.display_name, false));
        }

        let mut phone_app_removed = false;
        if shot.kind == ShotKind::FactoryShot && shot.latest_version_id.is_some() {
            let device = tohseno_engine::DevicePipeline::ready_device()?;
            let installed = tohseno_engine::gates::install::installed_candidate_apps(&device)
                .map_err(tohseno_engine::EngineError::Install)?;
            if installed
                .iter()
                .any(|installed| installed.bundle_id == app.bundle_id)
            {
                tohseno_engine::gates::install::retire(&device, &app.bundle_id)
                    .map_err(tohseno_engine::EngineError::Install)?;
                phone_app_removed = true;
            }
        }
        self.engine.ledger().set_retired(&app.name, true)?;
        let result = if phone_app_removed {
            format!(
                "{} was removed from your iPhone and retired locally.",
                app.name
            )
        } else {
            format!(
                "{} was retired locally; no installed iPhone copy remained.",
                app.name
            )
        };
        self.events.emit(tohseno_engine::Event::result(result));
        Ok(retire_receipt(
            shot_id,
            &shot.display_name,
            phone_app_removed,
        ))
    }

    pub fn shot_icon(&self, shot_id: &str) -> Result<Option<IconDescriptor>, ApplicationError> {
        load_shot_icon(&self.engine, &self.workspace_id, shot_id)
            .map_err(|error| ApplicationError::Orchestration(error.to_string()))
    }

    /// What the most recent execution of this Shot was asked, ran, cost, and
    /// did. The owner's Details disclosure; never part of the normal path.
    pub fn execution_receipt(
        &self,
        shot_id: &str,
    ) -> Result<Option<crate::receipt::ExecutionReceipt>, ApplicationError> {
        crate::receipt::load_execution_receipt(
            &self.engine,
            &self.workspace_id,
            self.command_journal.root(),
            shot_id,
        )
        .map_err(|error| ApplicationError::Orchestration(error.to_string()))
    }

    /// Privacy-safe, durable progress for the owner's open Details surface.
    pub fn execution_activity(
        &self,
        shot_id: &str,
    ) -> Result<Option<crate::receipt::ExecutionActivity>, ApplicationError> {
        crate::receipt::load_execution_activity(&self.engine, &self.workspace_id, shot_id)
            .map_err(|error| ApplicationError::Orchestration(error.to_string()))
    }

    pub fn shot_preview(&self, shot_id: &str) -> Result<Option<IconDescriptor>, ApplicationError> {
        crate::snapshot::load_shot_preview(&self.engine, &self.workspace_id, shot_id)
            .map_err(|error| ApplicationError::Orchestration(error.to_string()))
    }

    pub fn factory_defaults(&self) -> FactoryDefaults {
        let options = self.engine.harnesses();
        let selection = self.resolve_selection().ok();
        let option = selection
            .as_ref()
            .and_then(|selection| options.iter().find(|option| option.id == selection.harness))
            .or_else(|| options.iter().find(|option| option.selected))
            .or_else(|| options.iter().find(|option| option.installed));
        let model = option.and_then(|option| {
            selection
                .as_ref()
                .and_then(|selection| {
                    option
                        .models
                        .iter()
                        .find(|model| model.id == selection.model)
                })
                .or_else(|| option.models.iter().find(|model| model.is_default))
        });
        let route = option.and_then(|option| {
            selection
                .as_ref()
                .and_then(|selection| {
                    option
                        .routes
                        .iter()
                        .find(|route| route.id == selection.route)
                })
                .or_else(|| option.routes.iter().find(|route| route.available))
        });
        FactoryDefaults {
            schema: "tohseno.local-factory-defaults/1".into(),
            ready: selection.is_some(),
            harness_id: option.map(|option| option.id.clone()),
            harness_label: option.map(|option| option.label.clone()),
            model_id: model.map(|model| model.id.clone()),
            model_label: model.map(|model| model.label.clone()),
            route_id: route.map(|route| route.id.clone()),
            route_label: route.map(|route| route.label.clone()),
            harnesses: options
                .iter()
                .filter(|option| {
                    option.installed && option.routes.iter().any(|route| route.available)
                })
                .map(|option| FactoryHarnessOption {
                    id: option.id.clone(),
                    label: option.label.clone(),
                    models: option
                        .models
                        .iter()
                        .map(|model| FactoryModelOption {
                            id: model.id.clone(),
                            label: model.label.clone(),
                            is_default: model.is_default,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn harness_selection(
        &self,
        harness: &str,
        model: &str,
    ) -> Result<HarnessSelection, ApplicationError> {
        execution_manager::selection(&self.engine, Some(harness), Some(model), None)
            .map_err(|error| ApplicationError::Invalid(error.to_string()))
    }

    fn resolve_selection(&self) -> Result<HarnessSelection, ApplicationError> {
        if let Some(selection) = self.selection.clone() {
            return Ok(selection);
        }
        execution_manager::selection(&self.engine, None, None, None)
            .map_err(|error| ApplicationError::Orchestration(error.to_string()))
    }

    async fn replay_command(
        &self,
        record: &crate::command::CommandRecord,
    ) -> Result<(), ApplicationError> {
        match record.command_kind {
            CommandKind::ShotCreate => {
                let payload: CreatePayload = self.command_journal.payload(&record.command_id)?;
                let references = recover_references(
                    &self.command_journal,
                    &record.command_id,
                    &payload.references,
                )?;
                self.create_shot(CreateShotCommand {
                    command_id: record.command_id.clone(),
                    origin: record.origin,
                    origin_device_id: record.origin_device_id.clone(),
                    name: payload.name,
                    name_was_supplied: payload.name_was_supplied,
                    intention: payload.intention,
                    references,
                    submitted_at: Some(record.submitted_at.clone()),
                    harness_selection: payload.harness_selection,
                })
                .await?;
            }
            CommandKind::ShotEvolve => {
                let payload: EvolvePayload = self.command_journal.payload(&record.command_id)?;
                let references = recover_references(
                    &self.command_journal,
                    &record.command_id,
                    &payload.references,
                )?;
                self.evolve_shot(EvolveShotCommand {
                    command_id: record.command_id.clone(),
                    origin: record.origin,
                    origin_device_id: record.origin_device_id.clone(),
                    name: payload.name,
                    base_expression_id: payload.base_expression_id,
                    base_version_id: payload.base_version_id,
                    base_version_ordinal: payload.base_version_ordinal,
                    intention: payload.intention,
                    selected_feedback_actions: payload.selected_feedback_actions,
                    references,
                    submitted_at: Some(record.submitted_at.clone()),
                })
                .await?;
            }
            CommandKind::FeedbackSubmit => {
                let payload: FeedbackPayload = self.command_journal.payload(&record.command_id)?;
                self.submit_feedback(SubmitFeedbackCommand {
                    command_id: record.command_id.clone(),
                    origin: record.origin,
                    origin_device_id: record.origin_device_id.clone(),
                    name: payload.name,
                    shot_id: payload.shot_id,
                    expression_id: payload.expression_id,
                    version_id: payload.version_id,
                    version_ordinal: payload.version_ordinal,
                    body: payload.body,
                    companion_signature: payload.companion_signature,
                    submitted_at: Some(record.submitted_at.clone()),
                })
                .await?;
            }
            CommandKind::MarketingSubmit => {
                let payload: MarketingPayload = self.command_journal.payload(&record.command_id)?;
                self.submit_marketing_note(SubmitMarketingNoteCommand {
                    command_id: record.command_id.clone(),
                    origin: record.origin,
                    origin_device_id: record.origin_device_id.clone(),
                    name: payload.name,
                    shot_id: payload.shot_id,
                    body: payload.body,
                    companion_signature: payload.companion_signature,
                    submitted_at: Some(record.submitted_at.clone()),
                })
                .await?;
            }
        }
        Ok(())
    }

    fn resume_running_execution(
        &self,
        command_id: &str,
        status: &crate::command::CommandStatus,
    ) -> Result<(), ApplicationError> {
        let receipt = status.result.as_ref().ok_or_else(|| {
            ApplicationError::Orchestration(format!(
                "running command {command_id} has no stable execution receipt"
            ))
        })?;
        let (name, execution_id) =
            match serde_json::from_value::<CreateShotReceipt>(receipt.receipt.clone()) {
                Ok(receipt) => {
                    let payload: CreatePayload = self.command_journal.payload(command_id)?;
                    (payload.name, receipt.execution_id)
                }
                Err(_) => {
                    let receipt =
                        serde_json::from_value::<EvolveShotReceipt>(receipt.receipt.clone())?;
                    let payload: EvolvePayload = self.command_journal.payload(command_id)?;
                    (payload.name, receipt.execution_id)
                }
            };
        self.ensure_and_monitor_execution(command_id, &name, &execution_id)
    }

    fn ensure_and_monitor_execution(
        &self,
        command_id: &str,
        name: &str,
        execution_id: &str,
    ) -> Result<(), ApplicationError> {
        match self.reconcile_execution_status(command_id, name, execution_id) {
            Ok(true) => {}
            Ok(false) => {
                self.schedule_execution_monitor(command_id.into(), name.into(), execution_id.into())
            }
            Err(error) => {
                // A transient launch/read failure must not orphan a durable
                // Running command. The bounded monitor retries recovery and
                // publishes one privacy-safe failure if integrity cannot be
                // re-established.
                self.schedule_execution_monitor(
                    command_id.into(),
                    name.into(),
                    execution_id.into(),
                );
                return Err(error);
            }
        }
        Ok(())
    }

    fn reconcile_execution_status(
        &self,
        command_id: &str,
        name: &str,
        execution_id: &str,
    ) -> Result<bool, ApplicationError> {
        let repository = self.engine.ledger().working_tree(name);
        let execution =
            execution_manager::ensure_background_runner(&repository, execution_id, &self.events)
                .map_err(|error| ApplicationError::Orchestration(error.to_string()))?;
        let Some(completion) =
            tohseno_engine::shot_execution::load_completion(&repository, execution_id)
                .map_err(|error| ApplicationError::Orchestration(error.to_string()))?
        else {
            let (_, current) = self.command_journal.load(command_id)?;
            let projected = projected_command_state(execution.phase, current.state);
            if projected != current.state {
                self.command_journal
                    .transition(command_id, projected, None, None)?;
            }
            return Ok(false);
        };
        if completion.schema != tohseno_engine::shot_execution::COMPLETION_RECORD_SCHEMA
            || completion.execution_id != execution.execution_id
            || completion.shot_id != execution.shot_id
            || completion.version_ordinal != execution.version_ordinal
            || completion.harness != execution.harness
            || completion.model != execution.model
            || completion.route != execution.route
            || completion.intention_digest != execution.intention_digest
            || completion.reference_digests
                != execution
                    .references
                    .iter()
                    .map(|reference| reference.digest)
                    .collect::<Vec<_>>()
        {
            return Err(ApplicationError::Orchestration(
                "execution completion does not match its durable preparation".into(),
            ));
        }
        let (_, current) = self.command_journal.load(command_id)?;
        let receipt = current.result.as_ref().ok_or_else(|| {
            ApplicationError::Orchestration(
                "execution command has no stable receipt to reconcile".into(),
            )
        })?;
        let receipt_identity = serde_json::from_value::<CreateShotReceipt>(receipt.receipt.clone())
            .map(|receipt| (receipt.execution_id, receipt.shot_id))
            .or_else(|_| {
                serde_json::from_value::<EvolveShotReceipt>(receipt.receipt.clone())
                    .map(|receipt| (receipt.execution_id, receipt.shot_id))
            })?;
        if receipt_identity != (execution.execution_id.clone(), execution.shot_id) {
            return Err(ApplicationError::Orchestration(
                "execution completion does not match the stable command receipt".into(),
            ));
        }
        if completion.landed {
            let ordinal = u32::try_from(execution.version_ordinal).map_err(|_| {
                ApplicationError::Orchestration(
                    "accepted execution ordinal exceeds the local Version range".into(),
                )
            })?;
            let version = self.engine.ledger().shot(name, ordinal)?;
            tohseno_engine::protocol_lifecycle::verify_completed_evolution(&version)
                .map_err(|error| ApplicationError::Orchestration(error.to_string()))?;
            if let Some(entitlement) = &self.entitlement {
                let accepted = self.engine.current_accepted_base(name)?;
                entitlement
                    .record_successful_day_now(SuccessfulDayEvidence {
                        // The store replaces both clock-derived fields at the
                        // atomic recording boundary.
                        local_date: String::new(),
                        command_id: command_id.into(),
                        execution_id: execution_id.into(),
                        accepted_version_id: accepted.version_id.to_string(),
                        accepted_at: String::new(),
                    })
                    .map_err(|error| ApplicationError::Orchestration(error.to_string()))?;
            }
        }
        let (state, rejection) = if completion.landed {
            (CommandState::Completed, None)
        } else if completion.outcome == tohseno_engine::ExecutionOutcome::Cancelled {
            (
                CommandState::Cancelled,
                Some("execution was cancelled before an accepted Version landed".into()),
            )
        } else {
            (
                CommandState::Failed,
                Some("execution ended without an independently verified accepted Version".into()),
            )
        };
        if !current.state.terminal() {
            self.command_journal
                .transition(command_id, state, None, rejection)?;
        }
        Ok(true)
    }

    fn schedule_execution_monitor(&self, command_id: String, name: String, execution_id: String) {
        let service = self.clone();
        tokio::spawn(async move {
            let mut consecutive_errors = 0_u8;
            loop {
                match service.reconcile_execution_status(&command_id, &name, &execution_id) {
                    Ok(true) => break,
                    Ok(false) => consecutive_errors = 0,
                    Err(_) if consecutive_errors < 4 => consecutive_errors += 1,
                    Err(_) => {
                        if let Ok((_, status)) = service.command_journal.load(&command_id) {
                            if !status.state.terminal() {
                                let _ = service.command_journal.transition(
                                    &command_id,
                                    CommandState::Failed,
                                    None,
                                    Some(
                                        "local execution state could not be reconciled safely"
                                            .into(),
                                    ),
                                );
                            }
                        }
                        break;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    }

    fn reject<T>(&self, command_id: &str, reason: &str) -> Result<T, ApplicationError> {
        self.command_journal.transition(
            command_id,
            CommandState::Rejected,
            None,
            Some(reason.into()),
        )?;
        Err(ApplicationError::Invalid(reason.into()))
    }

    fn require_new_factory_mutation(&self, command_id: &str) -> Result<(), ApplicationError> {
        if self.command_journal.contains(command_id)? {
            return Ok(());
        }
        if let Some(entitlement) = &self.entitlement {
            entitlement
                .require_new_factory_mutation()
                .map_err(|error| ApplicationError::Invalid(error.to_string()))?;
        }
        Ok(())
    }
}

fn retire_receipt(shot_id: &str, display_name: &str, phone_app_removed: bool) -> RetireShotReceipt {
    RetireShotReceipt {
        schema: "tohseno.retire-shot-receipt/1".into(),
        shot_id: shot_id.into(),
        display_name: display_name.into(),
        phone_app_removed,
        source_preserved: true,
    }
}

fn projected_command_state(
    phase: tohseno_engine::ExecutionPhase,
    current: CommandState,
) -> CommandState {
    if phase == tohseno_engine::ExecutionPhase::WaitingForDevice {
        CommandState::WaitingForDevice
    } else if current == CommandState::WaitingForDevice {
        CommandState::Running
    } else {
        current
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceDescriptor {
    filename: String,
    media_type: String,
    origin: String,
    byte_length: u64,
    sha256: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatePayload {
    schema: String,
    name: String,
    #[serde(default = "historical_name_was_supplied")]
    name_was_supplied: bool,
    intention: String,
    references: Vec<ReferenceDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    harness_selection: Option<HarnessSelection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvolvePayload {
    schema: String,
    name: String,
    base_expression_id: ExpressionId,
    base_version_id: VersionId,
    base_version_ordinal: u64,
    intention: String,
    selected_feedback_actions: Vec<Bytes32>,
    references: Vec<ReferenceDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedbackPayload {
    schema: String,
    name: String,
    shot_id: ShotId,
    expression_id: ExpressionId,
    version_id: VersionId,
    version_ordinal: u64,
    body: String,
    companion_signature: Option<String>,
}

impl From<&SubmitFeedbackCommand> for FeedbackPayload {
    fn from(command: &SubmitFeedbackCommand) -> Self {
        Self {
            schema: "tohseno.feedback-command/1".into(),
            name: command.name.clone(),
            shot_id: command.shot_id,
            expression_id: command.expression_id,
            version_id: command.version_id,
            version_ordinal: command.version_ordinal,
            body: command.body.clone(),
            companion_signature: command.companion_signature.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketingPayload {
    schema: String,
    name: String,
    shot_id: ShotId,
    body: String,
    companion_signature: Option<String>,
}

impl From<&SubmitMarketingNoteCommand> for MarketingPayload {
    fn from(command: &SubmitMarketingNoteCommand) -> Self {
        Self {
            schema: "tohseno.marketing-command/1".into(),
            name: command.name.clone(),
            shot_id: command.shot_id,
            body: command.body.clone(),
            companion_signature: command.companion_signature.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketingRecord {
    schema: String,
    note_id: String,
    shot_id: ShotId,
    body: String,
    created_at: String,
    author_device_id: Option<String>,
    companion_signature: Option<String>,
}

type StagedCommandFiles = Vec<(String, Vec<u8>)>;
type PreparedCommandPayload<T> = (T, StagedCommandFiles);

fn create_payload(
    command: &CreateShotCommand,
) -> Result<PreparedCommandPayload<CreatePayload>, ApplicationError> {
    let (references, files) = reference_payload(&command.references)?;
    Ok((
        CreatePayload {
            schema: "tohseno.create-shot-command/1".into(),
            name: command.name.clone(),
            name_was_supplied: command.name_was_supplied,
            intention: command.intention.clone(),
            references,
            harness_selection: command.harness_selection.clone(),
        },
        files,
    ))
}

fn historical_name_was_supplied() -> bool {
    true
}

fn evolve_payload(
    command: &EvolveShotCommand,
) -> Result<PreparedCommandPayload<EvolvePayload>, ApplicationError> {
    let (references, files) = reference_payload(&command.references)?;
    let mut selected = command.selected_feedback_actions.clone();
    selected.sort_unstable();
    if selected.len() > 256
        || selected.contains(&Bytes32::ZERO)
        || selected.windows(2).any(|pair| pair[0] == pair[1])
    {
        return Err(ApplicationError::Invalid(
            "feedback action commitments must contain at most 256 nonzero unique values".into(),
        ));
    }
    Ok((
        EvolvePayload {
            schema: "tohseno.evolve-shot-command/1".into(),
            name: command.name.clone(),
            base_expression_id: command.base_expression_id,
            base_version_id: command.base_version_id,
            base_version_ordinal: command.base_version_ordinal,
            intention: command.intention.clone(),
            selected_feedback_actions: selected,
            references,
        },
        files,
    ))
}

fn reference_payload(
    inputs: &[ReferenceInput],
) -> Result<PreparedCommandPayload<Vec<ReferenceDescriptor>>, ApplicationError> {
    if inputs.len() > 8 {
        return Err(ApplicationError::Invalid(
            "a command accepts at most eight reference images".into(),
        ));
    }
    let mut names = BTreeSet::new();
    let mut descriptors = Vec::new();
    let mut files = Vec::new();
    for input in inputs {
        validate_display_filename(&input.display_filename)?;
        let expected_media_type = media_type_for_filename(&input.display_filename)?;
        if input.media_type != expected_media_type {
            return Err(ApplicationError::Invalid(format!(
                "reference image {} declares {}, expected {expected_media_type}",
                input.display_filename, input.media_type
            )));
        }
        if !names.insert(input.display_filename.clone()) {
            return Err(ApplicationError::Invalid(
                "reference image filenames must be unique".into(),
            ));
        }
        if input.bytes.len() as u64 > MAXIMUM_REFERENCE_BYTES {
            return Err(ApplicationError::Invalid(
                "reference image is too large".into(),
            ));
        }
        let digest = Bytes32::new(Sha256::digest(&input.bytes).into());
        descriptors.push(ReferenceDescriptor {
            filename: input.display_filename.clone(),
            media_type: input.media_type.clone(),
            origin: input.origin.clone(),
            byte_length: u64::try_from(input.bytes.len()).unwrap_or(u64::MAX),
            sha256: digest,
        });
        files.push((input.display_filename.clone(), input.bytes.clone()));
    }
    Ok((descriptors, files))
}

fn recover_references(
    journal: &CommandJournal,
    command_id: &str,
    descriptors: &[ReferenceDescriptor],
) -> Result<Vec<ReferenceInput>, ApplicationError> {
    if descriptors.len() > 8 {
        return Err(ApplicationError::Invalid(
            "durable command contains too many private references".into(),
        ));
    }
    descriptors
        .iter()
        .map(|descriptor| {
            let bytes = journal.input(command_id, &descriptor.filename)?;
            let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            if length != descriptor.byte_length
                || Bytes32::new(Sha256::digest(&bytes).into()) != descriptor.sha256
            {
                return Err(ApplicationError::Invalid(format!(
                    "durable private reference {} no longer matches its admitted descriptor",
                    descriptor.filename
                )));
            }
            Ok(ReferenceInput {
                display_filename: descriptor.filename.clone(),
                media_type: descriptor.media_type.clone(),
                origin: descriptor.origin.clone(),
                bytes,
            })
        })
        .collect()
}

fn receipt_from_status<T: for<'de> Deserialize<'de>>(
    status: &crate::command::CommandStatus,
) -> Result<Option<T>, ApplicationError> {
    // A failed, rejected, or cancelled command keeps its receipt as durable
    // evidence of how far execution reached. Replaying it as this command's
    // answer would report the state captured while the work was still in
    // flight, telling the caller the Shot is running when the command has
    // already terminated and nothing will resume it. Terminal failures fall
    // through to the resume path, which surfaces the recorded rejection.
    if matches!(
        status.state,
        CommandState::Rejected | CommandState::Failed | CommandState::Cancelled
    ) {
        return Ok(None);
    }
    status
        .result
        .as_ref()
        .map(|result| {
            serde_json::from_value(result.receipt.clone()).map_err(ApplicationError::from)
        })
        .transpose()
}

fn resume_command(
    journal: &CommandJournal,
    command_id: &str,
    status: &crate::command::CommandStatus,
) -> Result<CommandState, ApplicationError> {
    match status.state {
        CommandState::Received => {
            journal.transition(command_id, CommandState::Validated, None, None)?;
            Ok(CommandState::Validated)
        }
        CommandState::Validated | CommandState::Accepted => Ok(status.state),
        CommandState::Rejected => Err(ApplicationError::Invalid(
            status
                .rejection
                .clone()
                .unwrap_or_else(|| "command was rejected".into()),
        )),
        CommandState::Failed => Err(ApplicationError::Orchestration(
            status
                .rejection
                .clone()
                .unwrap_or_else(|| "command failed".into()),
        )),
        CommandState::Cancelled => Err(ApplicationError::Orchestration(
            "command was cancelled".into(),
        )),
        CommandState::Running | CommandState::WaitingForDevice | CommandState::Completed => {
            Err(ApplicationError::Orchestration(format!(
                "command {command_id} has {:?} state without its stable receipt",
                status.state
            )))
        }
    }
}

fn feedback_receipt(command: &SubmitFeedbackCommand, stored: StoredFeedback) -> FeedbackReceipt {
    FeedbackReceipt {
        schema: "tohseno.feedback-receipt/1".into(),
        command_id: command.command_id.clone(),
        shot_id: command.shot_id,
        expression_id: command.expression_id,
        version_id: command.version_id,
        version_ordinal: command.version_ordinal,
        feedback_id: stored.feedback_id,
        action_commitment: stored.action_commitment,
    }
}

fn validate_intention(intention: &str) -> Result<(), ApplicationError> {
    if intention.trim().is_empty() || intention.len() > MAXIMUM_INTENTION_BYTES {
        return Err(ApplicationError::Invalid(
            "intention must contain 1..=4194304 UTF-8 bytes".into(),
        ));
    }
    Ok(())
}

fn validate_display_filename(value: &str) -> Result<(), ApplicationError> {
    if value.is_empty()
        || value.len() > 255
        || matches!(value, "." | "..")
        || value
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
    {
        return Err(ApplicationError::Invalid(
            "reference filename must be one safe UTF-8 path component".into(),
        ));
    }
    media_type_for_filename(value)?;
    Ok(())
}

fn media_type_for_filename(value: &str) -> Result<&'static str, ApplicationError> {
    match Path::new(value)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok("image/png"),
        Some("jpg" | "jpeg") => Ok("image/jpeg"),
        Some("heic") => Ok("image/heic"),
        Some("webp") => Ok("image/webp"),
        _ => Err(ApplicationError::Invalid(
            "reference image must be PNG, JPEG, HEIC, or WebP".into(),
        )),
    }
}

fn store_marketing_record(root: &Path, record: &MarketingRecord) -> Result<(), ApplicationError> {
    let directory = root.join(".tohseno/private/marketing");
    ensure_private_directory(&directory)?;
    let path = directory.join(format!("{}.json", record.note_id));
    let bytes = tohseno_protocol::canonical::to_vec(record)
        .map_err(|error| ApplicationError::Invalid(error.to_string()))?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(ApplicationError::Invalid(
                "marketing record path is unsafe".into(),
            ));
        }
        Ok(_) => return require_exact_marketing_record(&path, &bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let staging = directory.join(format!(
        ".marketing-staging-{}-{}.tmp",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(&staging)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    match fs::hard_link(&staging, &path) {
        Ok(()) => {
            sync_directory(&directory)?;
            fs::remove_file(&staging)?;
            sync_directory(&directory)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::remove_file(&staging)?;
            require_exact_marketing_record(&path, &bytes)
        }
        Err(error) => {
            let _ = fs::remove_file(&staging);
            Err(error.into())
        }
    }
}

fn require_exact_marketing_record(path: &Path, expected: &[u8]) -> Result<(), ApplicationError> {
    let metadata = fs::symlink_metadata(path)?;
    let maximum = MAXIMUM_NOTE_BYTES as u64 + 16 * 1024;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err(ApplicationError::Invalid(
            "marketing record is not a bounded regular file".into(),
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err(ApplicationError::Invalid(
            "marketing record changed while it was opened".into(),
        ));
    }
    let mut existing = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(maximum + 1).read_to_end(&mut existing)?;
    if existing != expected {
        return Err(ApplicationError::Invalid(
            "marketing note identity already contains different bytes".into(),
        ));
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), ApplicationError> {
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}

fn ensure_private_directory(path: &Path) -> Result<(), ApplicationError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(ApplicationError::Invalid(format!(
                    "{} is not a real directory",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_with_receipt(state: CommandState) -> crate::command::CommandStatus {
        crate::command::CommandStatus {
            schema: "tohseno.local-command-status/1".into(),
            command_id: "command_fixture".into(),
            state,
            updated_at: "2026-01-01T00:00:00Z".into(),
            result: Some(crate::command::CommandResult {
                receipt: serde_json::json!({ "state": "running" }),
            }),
            rejection: None,
        }
    }

    #[test]
    fn a_terminally_failed_command_never_replays_its_in_flight_receipt() {
        // The receipt is written while work is still running and is kept as
        // durable evidence. Replaying it after the command terminated told the
        // caller a Shot was running when nothing would ever resume it.
        for state in [
            CommandState::Failed,
            CommandState::Rejected,
            CommandState::Cancelled,
        ] {
            let replayed =
                receipt_from_status::<serde_json::Value>(&status_with_receipt(state)).unwrap();
            assert_eq!(replayed, None, "{state:?} must not replay a receipt");
        }
    }

    #[test]
    fn an_unfinished_or_completed_command_still_replays_its_stable_receipt() {
        for state in [
            CommandState::Running,
            CommandState::WaitingForDevice,
            CommandState::Completed,
        ] {
            let replayed =
                receipt_from_status::<serde_json::Value>(&status_with_receipt(state)).unwrap();
            assert!(replayed.is_some(), "{state:?} must replay its receipt");
        }
    }

    #[test]
    fn reference_path_reader_rejects_symlinks_and_preserves_exact_bytes() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("reference.png");
        fs::write(&image, [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]).unwrap();
        let reference = ReferenceInput::from_path(&image).unwrap();
        assert_eq!(reference.display_filename, "reference.png");
        assert_eq!(reference.bytes, fs::read(&image).unwrap());
        #[cfg(unix)]
        {
            let link = root.path().join("link.png");
            std::os::unix::fs::symlink(&image, &link).unwrap();
            assert!(ReferenceInput::from_path(&link).is_err());
        }
    }

    #[test]
    fn reference_payload_binds_exact_bytes_and_origin() {
        let inputs = vec![ReferenceInput {
            display_filename: "one.png".into(),
            media_type: "image/png".into(),
            origin: "cli:/private/input/one.png".into(),
            bytes: vec![1, 2, 3],
        }];
        let (descriptors, files) = reference_payload(&inputs).unwrap();
        assert_eq!(descriptors[0].sha256, sha256(&[1, 2, 3]));
        assert_eq!(files[0].1, vec![1, 2, 3]);
    }

    #[test]
    fn reference_payload_rejects_a_media_type_that_disagrees_with_the_name() {
        let inputs = vec![ReferenceInput {
            display_filename: "one.png".into(),
            media_type: "text/plain".into(),
            origin: "companion:attachment_1".into(),
            bytes: vec![1, 2, 3],
        }];
        assert!(reference_payload(&inputs).is_err());
    }

    #[test]
    fn mac_and_companion_intentions_become_the_same_application_payload() {
        let cli = CreateShotCommand {
            command_id: "cli_create".into(),
            origin: CommandOrigin::Cli,
            origin_device_id: None,
            name: "counter".into(),
            name_was_supplied: true,
            intention: "Remember a tap count between launches.".into(),
            references: Vec::new(),
            submitted_at: None,
            harness_selection: None,
        };
        let companion = CreateShotCommand {
            command_id: "companion_create".into(),
            origin: CommandOrigin::Companion,
            origin_device_id: Some("device_fixture".into()),
            ..cli.clone()
        };
        assert_eq!(
            create_payload(&cli).unwrap().0,
            create_payload(&companion).unwrap().0
        );

        let historical: CreatePayload = serde_json::from_value(serde_json::json!({
            "schema": "tohseno.create-shot-command/1",
            "name": "counter",
            "intention": "Remember a tap count between launches.",
            "references": [],
        }))
        .unwrap();
        assert!(historical.name_was_supplied);
        assert!(historical.harness_selection.is_none());

        let selected = CreateShotCommand {
            command_id: "selected_create".into(),
            harness_selection: Some(HarnessSelection {
                harness: "claude-code".into(),
                model: "sonnet".into(),
                route: "claude-subscription".into(),
            }),
            ..cli.clone()
        };
        let selected_payload = create_payload(&selected).unwrap().0;
        assert_eq!(
            selected_payload.harness_selection,
            selected.harness_selection
        );
        assert_eq!(
            serde_json::from_value::<CreatePayload>(
                serde_json::to_value(selected_payload).unwrap()
            )
            .unwrap()
            .harness_selection,
            selected.harness_selection
        );

        let base_expression_id = ExpressionId::from_bytes([0x71; 32]);
        let base_version_id = VersionId::from_bytes([0x72; 32]);
        let mac_evolution = EvolveShotCommand {
            command_id: "studio_evolve".into(),
            origin: CommandOrigin::Studio,
            origin_device_id: None,
            name: "counter".into(),
            base_expression_id,
            base_version_id,
            base_version_ordinal: 1,
            intention: "Add a reset button without losing the count.".into(),
            selected_feedback_actions: Vec::new(),
            references: Vec::new(),
            submitted_at: None,
        };
        let phone_evolution = EvolveShotCommand {
            command_id: "companion_evolve".into(),
            origin: CommandOrigin::Companion,
            origin_device_id: Some("device_fixture".into()),
            ..mac_evolution.clone()
        };
        assert_eq!(
            evolve_payload(&mac_evolution).unwrap().0,
            evolve_payload(&phone_evolution).unwrap().0
        );
    }

    #[tokio::test]
    async fn invalid_private_command_is_durably_rejected_before_returning() {
        let temporary = tempfile::tempdir().unwrap();
        let temporary_root = temporary.path().canonicalize().unwrap();
        let ledger = tohseno_engine::Ledger::at_homes(
            temporary_root.join("family"),
            temporary_root.join("machine"),
        );
        ledger.initialize().unwrap();
        ledger
            .create_app("fixture", "local.tohseno.fixture")
            .unwrap();
        let shot_id = ShotId::from_bytes([0x30; 32]);
        let builder_id = tohseno_protocol::identity::BuilderId::parse(
            "eip155:4663:0x1111111111111111111111111111111111111111",
        )
        .unwrap();
        ledger
            .bind_protocol_identity("fixture", shot_id, builder_id)
            .unwrap();
        let journal = CommandJournal::open(temporary_root.join("service")).unwrap();
        let service = ShotApplicationService::new(
            Engine::at(
                ledger,
                EventBus::default(),
                tohseno_engine::Config::default(),
            ),
            journal.clone(),
            EventBus::default(),
            "workspace_fixture",
        );
        let command = SubmitMarketingNoteCommand {
            command_id: "marketing_invalid_fixture".into(),
            origin: CommandOrigin::Companion,
            origin_device_id: Some("device_fixture".into()),
            name: "fixture".into(),
            shot_id,
            body: String::new(),
            companion_signature: Some("fixture_signature".into()),
            submitted_at: Some("2026-08-16T00:00:00Z".into()),
        };

        assert!(service
            .submit_marketing_note(command.clone())
            .await
            .is_err());
        let (_, status) = journal.load(&command.command_id).unwrap();
        assert_eq!(status.state, CommandState::Rejected);
        assert!(status.rejection.is_some());
        assert!(service.submit_marketing_note(command).await.is_err());
        assert_eq!(
            journal.load("marketing_invalid_fixture").unwrap().1.state,
            CommandState::Rejected
        );
    }

    #[tokio::test]
    async fn concurrent_identical_marketing_commands_have_one_semantic_processor() {
        let temporary = tempfile::tempdir().unwrap();
        let temporary_root = temporary.path().canonicalize().unwrap();
        let ledger = tohseno_engine::Ledger::at_homes(
            temporary_root.join("family"),
            temporary_root.join("machine"),
        );
        ledger.initialize().unwrap();
        ledger
            .create_app("fixture", "local.tohseno.fixture")
            .unwrap();
        let shot_id = ShotId::from_bytes([0x39; 32]);
        let builder_id = tohseno_protocol::identity::BuilderId::parse(
            "eip155:4663:0x1111111111111111111111111111111111111111",
        )
        .unwrap();
        ledger
            .bind_protocol_identity("fixture", shot_id, builder_id)
            .unwrap();
        let journal = CommandJournal::open(temporary_root.join("service")).unwrap();
        let service = ShotApplicationService::new(
            Engine::at(
                ledger.clone(),
                EventBus::default(),
                tohseno_engine::Config::default(),
            ),
            journal.clone(),
            EventBus::default(),
            "workspace_fixture",
        );
        let command = SubmitMarketingNoteCommand {
            command_id: "marketing_concurrent_fixture".into(),
            origin: CommandOrigin::Companion,
            origin_device_id: Some("device_fixture".into()),
            name: "fixture".into(),
            shot_id,
            body: "One concurrently delivered private note.".into(),
            companion_signature: Some("fixture_signature".into()),
            submitted_at: Some("2026-08-16T00:00:00Z".into()),
        };

        // Hold the exact command claim while both frontends durably admit the
        // same bytes. Releasing it lets exactly one semantic processor run;
        // the waiter must observe and return the stable receipt it published.
        let held = service.claim_command(&command.command_id).await;
        let first_service = service.clone();
        let first_command = command.clone();
        let first =
            tokio::spawn(async move { first_service.submit_marketing_note(first_command).await });
        let second_service = service.clone();
        let second_command = command.clone();
        let second =
            tokio::spawn(async move { second_service.submit_marketing_note(second_command).await });
        tokio::task::yield_now().await;
        drop(held);

        let first = first.await.unwrap().unwrap();
        let second = second.await.unwrap().unwrap();
        assert_eq!(first, second);
        let marketing = ledger
            .working_tree("fixture")
            .join(".tohseno/private/marketing");
        assert_eq!(fs::read_dir(marketing).unwrap().count(), 1);
        let (_, status) = journal.load(&command.command_id).unwrap();
        assert_eq!(status.state, CommandState::Completed);
        assert_eq!(
            receipt_from_status::<MarketingNoteReceipt>(&status)
                .unwrap()
                .unwrap(),
            first
        );
    }

    #[tokio::test]
    async fn marketing_command_recovers_after_the_note_was_published_once() {
        let temporary = tempfile::tempdir().unwrap();
        let temporary_root = temporary.path().canonicalize().unwrap();
        let family = temporary_root.join("family");
        let machine = temporary_root.join("machine");
        let ledger = tohseno_engine::Ledger::at_homes(&family, &machine);
        ledger.initialize().unwrap();
        ledger
            .create_app("fixture", "local.tohseno.fixture")
            .unwrap();
        let shot_id = ShotId::from_bytes([0x31; 32]);
        let builder_id = tohseno_protocol::identity::BuilderId::parse(
            "eip155:4663:0x1111111111111111111111111111111111111111",
        )
        .unwrap();
        ledger
            .bind_protocol_identity("fixture", shot_id, builder_id)
            .unwrap();
        let journal = CommandJournal::open(temporary_root.join("service")).unwrap();
        let service = ShotApplicationService::new(
            Engine::at(
                ledger.clone(),
                EventBus::default(),
                tohseno_engine::Config::default(),
            ),
            journal.clone(),
            EventBus::default(),
            "workspace_fixture",
        );
        let command = SubmitMarketingNoteCommand {
            command_id: "marketing_crash_fixture".into(),
            origin: CommandOrigin::Companion,
            origin_device_id: Some("device_fixture".into()),
            name: "fixture".into(),
            shot_id,
            body: "One exact private marketing note.".into(),
            companion_signature: Some("fixture_signature".into()),
            submitted_at: Some("2026-08-16T00:00:00Z".into()),
        };
        let payload = MarketingPayload::from(&command);
        let admission = journal
            .admit(
                AdmissionMetadata {
                    command_id: command.command_id.clone(),
                    command_kind: CommandKind::MarketingSubmit,
                    origin: command.origin,
                    origin_device_id: command.origin_device_id.clone(),
                    workspace_id: "workspace_fixture".into(),
                    shot_id: Some(shot_id),
                    base_expression_id: None,
                    base_version_id: None,
                    submitted_at: command.submitted_at.clone(),
                },
                &payload,
            )
            .unwrap();
        journal
            .transition(&command.command_id, CommandState::Validated, None, None)
            .unwrap();
        journal
            .transition(&command.command_id, CommandState::Accepted, None, None)
            .unwrap();
        let note_id = format!(
            "note_{}",
            sha256(format!("TOHSENO-MARKETING-NOTE-ID-V1\0{}", command.command_id).as_bytes())
                .to_string()
                .trim_start_matches("0x")
        );
        let record = MarketingRecord {
            schema: "tohseno.marketing-note/1".into(),
            note_id: note_id.clone(),
            shot_id,
            body: command.body.clone(),
            created_at: admission.record.submitted_at,
            author_device_id: command.origin_device_id.clone(),
            companion_signature: command.companion_signature.clone(),
        };
        store_marketing_record(&ledger.working_tree("fixture"), &record).unwrap();

        let recovery = service.recover_commands().await.unwrap();
        assert_eq!(recovery.examined, 1);
        assert_eq!(recovery.resumed, 1);
        assert_eq!(recovery.terminal, 1);
        let recovered = service
            .submit_marketing_note(command.clone())
            .await
            .unwrap();
        let replayed = service.submit_marketing_note(command).await.unwrap();
        assert_eq!(recovered, replayed);
        assert_eq!(recovered.note_id, note_id);
        let marketing = ledger
            .working_tree("fixture")
            .join(".tohseno/private/marketing");
        let files = fs::read_dir(marketing)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(files, [std::ffi::OsString::from(format!("{note_id}.json"))]);
        let (_, status) = journal.load("marketing_crash_fixture").unwrap();
        assert_eq!(status.state, CommandState::Completed);
    }

    #[tokio::test]
    async fn startup_recovers_received_and_validated_marketing_commands() {
        let temporary = tempfile::tempdir().unwrap();
        let temporary_root = temporary.path().canonicalize().unwrap();
        let family = temporary_root.join("family");
        let machine = temporary_root.join("machine");
        let ledger = tohseno_engine::Ledger::at_homes(&family, &machine);
        ledger.initialize().unwrap();
        ledger
            .create_app("fixture", "local.tohseno.fixture")
            .unwrap();
        let shot_id = ShotId::from_bytes([0x32; 32]);
        let builder_id = tohseno_protocol::identity::BuilderId::parse(
            "eip155:4663:0x1111111111111111111111111111111111111111",
        )
        .unwrap();
        ledger
            .bind_protocol_identity("fixture", shot_id, builder_id)
            .unwrap();
        let journal = CommandJournal::open(temporary_root.join("service")).unwrap();
        let service = ShotApplicationService::new(
            Engine::at(
                ledger.clone(),
                EventBus::default(),
                tohseno_engine::Config::default(),
            ),
            journal.clone(),
            EventBus::default(),
            "workspace_fixture",
        );

        for (command_id, state) in [
            ("marketing_received", CommandState::Received),
            ("marketing_validated", CommandState::Validated),
        ] {
            let command = SubmitMarketingNoteCommand {
                command_id: command_id.into(),
                origin: CommandOrigin::Companion,
                origin_device_id: Some("device_fixture".into()),
                name: "fixture".into(),
                shot_id,
                body: format!("Private note for {command_id}."),
                companion_signature: Some("fixture_signature".into()),
                submitted_at: Some("2026-08-16T00:00:00Z".into()),
            };
            journal
                .admit(
                    AdmissionMetadata {
                        command_id: command.command_id.clone(),
                        command_kind: CommandKind::MarketingSubmit,
                        origin: command.origin,
                        origin_device_id: command.origin_device_id.clone(),
                        workspace_id: "workspace_fixture".into(),
                        shot_id: Some(shot_id),
                        base_expression_id: None,
                        base_version_id: None,
                        submitted_at: command.submitted_at.clone(),
                    },
                    &MarketingPayload::from(&command),
                )
                .unwrap();
            if state == CommandState::Validated {
                journal
                    .transition(command_id, CommandState::Validated, None, None)
                    .unwrap();
            }
        }

        let recovery = service.recover_commands().await.unwrap();
        assert_eq!(recovery.examined, 2);
        assert_eq!(recovery.resumed, 2);
        assert_eq!(recovery.terminal, 2);
        assert_eq!(recovery.still_running, 0);
        assert_eq!(recovery.retryable_failures, 0);
        for command_id in ["marketing_received", "marketing_validated"] {
            let (_, status) = journal.load(command_id).unwrap();
            assert_eq!(status.state, CommandState::Completed);
            assert!(status.result.is_some());
        }
        assert_eq!(service.recover_commands().await.unwrap().examined, 0);
        assert_eq!(
            fs::read_dir(
                ledger
                    .working_tree("fixture")
                    .join(".tohseno/private/marketing")
            )
            .unwrap()
            .count(),
            2
        );
    }

    #[tokio::test]
    async fn admitted_birth_is_visible_before_its_first_expression() {
        let temporary = tempfile::tempdir().unwrap();
        let temporary_root = temporary.path().canonicalize().unwrap();
        let family = temporary_root.join("family");
        let machine = temporary_root.join("machine");
        let ledger = tohseno_engine::Ledger::at_homes(&family, &machine);
        ledger.initialize().unwrap();
        ledger
            .create_app("fixture", "local.tohseno.fixture")
            .unwrap();
        let shot_id = ShotId::from_bytes([0x33; 32]);
        let builder_id = tohseno_protocol::identity::BuilderId::parse(
            "eip155:4663:0x1111111111111111111111111111111111111111",
        )
        .unwrap();
        ledger
            .bind_protocol_identity("fixture", shot_id, builder_id)
            .unwrap();
        let service = ShotApplicationService::new(
            Engine::at(
                ledger,
                EventBus::default(),
                tohseno_engine::Config::default(),
            ),
            CommandJournal::open(temporary_root.join("service")).unwrap(),
            EventBus::default(),
            "workspace_fixture",
        );

        let snapshot = service.workspace_snapshot().await.unwrap();
        assert_eq!(snapshot.shots.len(), 1);
        assert_eq!(snapshot.shots[0].shot_id, shot_id.to_string());
        assert_eq!(
            snapshot.shots[0].kind,
            crate::snapshot::ShotKind::FactoryShot
        );
        assert!(snapshot.shots[0].expression_id.is_some());
        assert_eq!(snapshot.shots[0].latest_version_id, None);
    }

    #[tokio::test]
    async fn retiring_an_uninstalled_app_preserves_its_source_and_history() {
        let temporary = tempfile::tempdir().unwrap();
        let temporary_root = temporary.path().canonicalize().unwrap();
        let ledger = tohseno_engine::Ledger::at_homes(
            temporary_root.join("family"),
            temporary_root.join("machine"),
        );
        ledger.initialize().unwrap();
        ledger
            .create_app("fixture", "org.tohseno.genesis.owner.fixture")
            .unwrap();
        let shot_id = ShotId::from_bytes([0x44; 32]);
        let builder_id = tohseno_protocol::identity::BuilderId::parse(
            "eip155:4663:0x1111111111111111111111111111111111111111",
        )
        .unwrap();
        ledger
            .bind_protocol_identity("fixture", shot_id, builder_id)
            .unwrap();
        let source = ledger.working_tree("fixture");
        let events = EventBus::default();
        let service = ShotApplicationService::new(
            Engine::at(ledger, events.clone(), tohseno_engine::Config::default()),
            CommandJournal::open(temporary_root.join("service")).unwrap(),
            events,
            "workspace_fixture",
        );

        let receipt = service.retire_shot(&shot_id.to_string()).await.unwrap();
        assert_eq!(receipt.schema, "tohseno.retire-shot-receipt/1");
        assert!(!receipt.phone_app_removed);
        assert!(receipt.source_preserved);
        assert!(source.is_dir());
        assert!(
            service
                .engine()
                .ledger()
                .load_app("fixture")
                .unwrap()
                .retired
        );
        assert!(service
            .workspace_snapshot()
            .await
            .unwrap()
            .shots
            .iter()
            .any(|shot| shot.shot_id == shot_id.to_string() && shot.retired));
    }

    #[test]
    fn execution_waiting_phase_projects_to_journal_and_resumes_running() {
        assert_eq!(
            projected_command_state(
                tohseno_engine::ExecutionPhase::WaitingForDevice,
                CommandState::Running,
            ),
            CommandState::WaitingForDevice
        );
        assert_eq!(
            projected_command_state(
                tohseno_engine::ExecutionPhase::ValidationStarted,
                CommandState::WaitingForDevice,
            ),
            CommandState::Running
        );
    }

    #[tokio::test]
    async fn restart_reconciles_zero_exit_without_landed_version_as_failed() {
        let temporary = tempfile::tempdir().unwrap();
        let temporary_root = temporary.path().canonicalize().unwrap();
        let ledger = tohseno_engine::Ledger::at(temporary_root.join("data"));
        ledger.initialize().unwrap();
        ledger
            .create_app("fixture", "local.tohseno.fixture")
            .unwrap();
        let journal = CommandJournal::open(temporary_root.join("service")).unwrap();
        let command_id = "unaccepted_zero_exit";
        let payload = CreatePayload {
            schema: "tohseno.create-shot-command/1".into(),
            name: "fixture".into(),
            name_was_supplied: true,
            intention: "Build the exact fixture.".into(),
            references: Vec::new(),
            harness_selection: None,
        };
        journal
            .admit(
                AdmissionMetadata {
                    command_id: command_id.into(),
                    command_kind: CommandKind::ShotCreate,
                    origin: CommandOrigin::Conformance,
                    origin_device_id: None,
                    workspace_id: "workspace_fixture".into(),
                    shot_id: None,
                    base_expression_id: None,
                    base_version_id: None,
                    submitted_at: Some("2026-08-16T00:00:00Z".into()),
                },
                &payload,
            )
            .unwrap();
        journal
            .transition(command_id, CommandState::Validated, None, None)
            .unwrap();
        journal
            .transition(command_id, CommandState::Accepted, None, None)
            .unwrap();
        let shot_id = ShotId::from_bytes([0x41; 32]);
        let execution_id = execution_manager::command_execution_id(command_id);
        let receipt = CreateShotReceipt {
            schema: "tohseno.create-shot-receipt/1".into(),
            command_id: command_id.into(),
            shot_id,
            execution_id: execution_id.clone(),
            state: "running".into(),
        };
        journal
            .transition(
                command_id,
                CommandState::Running,
                Some(serde_json::to_value(&receipt).unwrap()),
                None,
            )
            .unwrap();
        let execution_directory = ledger
            .working_tree("fixture")
            .join(".tohseno/executions")
            .join(&execution_id);
        fs::create_dir_all(&execution_directory).unwrap();
        let execution = tohseno_engine::PreparedExecution {
            schema: tohseno_engine::shot_execution::EXECUTION_RECORD_SCHEMA.into(),
            execution_id: execution_id.clone(),
            shot_id,
            version_ordinal: 1,
            app_name: "fixture".into(),
            repository: ledger.working_tree("fixture"),
            harness: "fixture".into(),
            harness_display_name: "Fixture".into(),
            model: "fixture".into(),
            route: "fixture".into(),
            route_billing: "none".into(),
            estimated_additional_cost_usd: Some(0.0),
            intention_digest: sha256(b"Build the exact fixture."),
            intent_path: ".tohseno/EVOLUTION_INTENT.md".into(),
            references: Vec::new(),
            mode: tohseno_engine::ExecutionMode::BirthMaterialization,
            auto_accept_genome: false,
            baseline: tohseno_engine::shot_execution::GitBoundary {
                tree: "0".repeat(40),
                head: None,
                pre_existing_status: Vec::new(),
            },
            prepared_at: "2026-08-16T00:00:00Z".into(),
            process_id: None,
            phase: tohseno_engine::ExecutionPhase::ExecutionFailed,
        };
        fs::write(
            execution_directory.join("execution.json"),
            serde_json::to_vec(&execution).unwrap(),
        )
        .unwrap();
        let completion = tohseno_engine::CompletionRecord {
            schema: tohseno_engine::shot_execution::COMPLETION_RECORD_SCHEMA.into(),
            execution_id: execution_id.clone(),
            shot_id,
            version_ordinal: 1,
            mode: Some(tohseno_engine::ExecutionMode::BirthMaterialization),
            outcome: tohseno_engine::ExecutionOutcome::Completed,
            landed: false,
            harness: "fixture".into(),
            model: "fixture".into(),
            route: "fixture".into(),
            intention_digest: sha256(b"Build the exact fixture."),
            reference_digests: Vec::new(),
            started_at: "2026-08-16T00:00:00Z".into(),
            ended_at: "2026-08-16T00:00:01Z".into(),
            duration_seconds: 1,
            exit_code: Some(0),
            files_changed: Vec::new(),
            git_diff_summary: String::new(),
            baseline_tree: "0".repeat(40),
            final_tree: "0".repeat(40),
            pre_existing_worktree_status: Vec::new(),
            validation_commands_executed: Vec::new(),
            validation_results: Vec::new(),
            harness_provided_final_summary: None,
            independently_computed_repository_state: "no accepted Version".into(),
            estimated_additional_cost_usd: Some(0.0),
            actual_additional_cost_usd: Some(0.0),
            token_usage: None,
            authoritative_next_action: "Repair and retry.".into(),
        };
        fs::write(
            execution_directory.join("completion.json"),
            serde_json::to_vec(&completion).unwrap(),
        )
        .unwrap();
        let service = ShotApplicationService::new(
            Engine::at(
                ledger,
                EventBus::default(),
                tohseno_engine::Config::default(),
            ),
            journal.clone(),
            EventBus::default(),
            "workspace_fixture",
        );

        let recovery = service.recover_commands().await.unwrap();
        assert_eq!(recovery.examined, 1);
        assert_eq!(recovery.resumed, 1);
        assert_eq!(recovery.terminal, 1);
        assert_eq!(recovery.still_running, 0);
        let (_, status) = journal.load(command_id).unwrap();
        assert_eq!(status.state, CommandState::Failed);
        assert_eq!(
            status.result.unwrap().receipt,
            serde_json::to_value(receipt).unwrap()
        );
    }
}
