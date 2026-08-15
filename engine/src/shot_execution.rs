//! Private local orchestration records for authentic coding-harness sessions.
//!
//! These records are deliberately outside canonical Shot lineage. They may
//! contain local repository paths, model choices, and private intent digests,
//! so they remain beneath the ignored `.tohseno/executions` boundary.

use crate::harness::{estimated_cost, resolve_selection, HarnessSelection};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_protocol::digest::{sha256, Bytes32, ShotId};

pub const EXECUTION_RECORD_SCHEMA: &str = "tohseno.local-shot-execution/1";
pub const EXECUTION_EVENT_SCHEMA: &str = "tohseno.local-shot-execution-event/1";
pub const COMPLETION_RECORD_SCHEMA: &str = "tohseno.local-shot-completion/1";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Conception,
    BirthMaterialization,
    #[default]
    EvolutionMaterialization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPhase {
    Prepared,
    RunnerStarted,
    // Kept so private records made by releases through 0.8.5 remain readable.
    TerminalOpened,
    ExecutionStarted,
    ContextLoaded,
    HarnessRunning,
    WorkspaceChanged,
    ValidationStarted,
    ValidationCompleted,
    ExecutionCompleted,
    ExecutionFailed,
    ExecutionCancelled,
}

impl ExecutionPhase {
    pub fn event_name(self) -> &'static str {
        match self {
            Self::Prepared => "shot.prepared",
            Self::RunnerStarted => "runner.started",
            Self::TerminalOpened => "terminal.opened",
            Self::ExecutionStarted => "execution.started",
            Self::ContextLoaded => "context.loaded",
            Self::HarnessRunning => "harness.running",
            Self::WorkspaceChanged => "workspace.changed",
            Self::ValidationStarted => "validation.started",
            Self::ValidationCompleted => "validation.completed",
            Self::ExecutionCompleted => "execution.completed",
            Self::ExecutionFailed => "execution.failed",
            Self::ExecutionCancelled => "execution.cancelled",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionReference {
    pub label: String,
    pub relative_path: String,
    pub digest: Bytes32,
    pub byte_length: u64,
    pub media_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionPreparation {
    pub app_name: String,
    pub shot_id: ShotId,
    pub version_ordinal: u64,
    pub selection: HarnessSelection,
    pub intent_path: String,
    pub intention_digest: Bytes32,
    pub references: Vec<ExecutionReference>,
    pub mode: ExecutionMode,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitBoundary {
    pub tree: String,
    pub head: Option<String>,
    pub pre_existing_status: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedExecution {
    pub schema: String,
    pub execution_id: String,
    pub shot_id: ShotId,
    pub version_ordinal: u64,
    pub app_name: String,
    pub repository: PathBuf,
    pub harness: String,
    pub harness_display_name: String,
    pub model: String,
    pub route: String,
    pub route_billing: String,
    pub estimated_additional_cost_usd: Option<f64>,
    pub intention_digest: Bytes32,
    pub intent_path: String,
    pub references: Vec<ExecutionReference>,
    #[serde(default)]
    pub mode: ExecutionMode,
    /// Read-only compatibility for execution records written through 0.8.5.
    /// New records omit this obsolete user-approval policy field.
    #[serde(default, skip_serializing)]
    pub auto_accept_genome: bool,
    pub baseline: GitBoundary,
    pub prepared_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_id: Option<u32>,
    pub phase: ExecutionPhase,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotExecutionEvent {
    pub schema: String,
    pub sequence: u64,
    pub event: String,
    pub execution_id: String,
    pub shot_id: ShotId,
    pub version_ordinal: u64,
    pub harness: String,
    pub model: String,
    pub timestamp: String,
    pub phase: ExecutionPhase,
    pub report: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOutcome {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangedFile {
    pub status: String,
    pub path: String,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationObservation {
    pub command: String,
    pub status: String,
    pub evidence: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionRecord {
    pub schema: String,
    pub execution_id: String,
    pub shot_id: ShotId,
    pub version_ordinal: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ExecutionMode>,
    pub outcome: ExecutionOutcome,
    pub landed: bool,
    pub harness: String,
    pub model: String,
    pub route: String,
    pub intention_digest: Bytes32,
    pub reference_digests: Vec<Bytes32>,
    pub started_at: String,
    pub ended_at: String,
    pub duration_seconds: u64,
    pub exit_code: Option<i32>,
    pub files_changed: Vec<ChangedFile>,
    pub git_diff_summary: String,
    pub baseline_tree: String,
    pub final_tree: String,
    pub pre_existing_worktree_status: Vec<String>,
    pub validation_commands_executed: Vec<String>,
    pub validation_results: Vec<ValidationObservation>,
    pub harness_provided_final_summary: Option<String>,
    pub independently_computed_repository_state: String,
    pub estimated_additional_cost_usd: Option<f64>,
    pub actual_additional_cost_usd: Option<f64>,
    pub authoritative_next_action: String,
}

#[derive(Debug)]
pub enum ShotExecutionError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Invalid(String),
    Git(String),
}

impl std::fmt::Display for ShotExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Invalid(reason) => write!(formatter, "{reason}"),
            Self::Git(reason) => write!(formatter, "Git boundary failed: {reason}"),
        }
    }
}

impl std::error::Error for ShotExecutionError {}

impl From<std::io::Error> for ShotExecutionError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ShotExecutionError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn prepare_execution(
    repository: &Path,
    preparation: ExecutionPreparation,
) -> Result<PreparedExecution, ShotExecutionError> {
    let (harness, _) =
        resolve_selection(&preparation.selection).map_err(ShotExecutionError::Invalid)?;
    let route = harness
        .routes
        .iter()
        .find(|route| route.id == preparation.selection.route)
        .ok_or_else(|| ShotExecutionError::Invalid("selected route disappeared".into()))?;
    ensure_shot_repository(repository)?;
    ensure_no_active_execution(repository)?;
    let execution_id = new_execution_id(preparation.shot_id, preparation.version_ordinal);
    let directory = execution_directory(repository, &execution_id);
    fs::create_dir_all(&directory)?;
    set_private_directory(&directory)?;
    let baseline = capture_git_boundary(repository, &directory)?;
    let prepared = PreparedExecution {
        schema: EXECUTION_RECORD_SCHEMA.into(),
        execution_id,
        shot_id: preparation.shot_id,
        version_ordinal: preparation.version_ordinal,
        app_name: preparation.app_name,
        repository: repository.to_path_buf(),
        harness: harness.id,
        harness_display_name: harness.label,
        model: preparation.selection.model.clone(),
        route: route.id.clone(),
        route_billing: route.billing.clone(),
        estimated_additional_cost_usd: estimated_cost(&preparation.selection)
            .map_err(ShotExecutionError::Invalid)?,
        intention_digest: preparation.intention_digest,
        intent_path: preparation.intent_path,
        references: preparation.references,
        mode: preparation.mode,
        auto_accept_genome: false,
        baseline,
        prepared_at: now()?,
        process_id: None,
        phase: ExecutionPhase::Prepared,
    };
    write_record(&prepared)?;
    append_event(
        &prepared,
        ExecutionPhase::Prepared,
        "The intention, references, and version boundary are prepared.",
    )?;
    Ok(prepared)
}

pub fn load_execution(
    repository: &Path,
    execution_id: &str,
) -> Result<PreparedExecution, ShotExecutionError> {
    validate_execution_id(execution_id)?;
    let path = execution_directory(repository, execution_id).join("execution.json");
    let bytes = fs::read(&path)?;
    let execution: PreparedExecution = serde_json::from_slice(&bytes)?;
    if execution.schema != EXECUTION_RECORD_SCHEMA
        || execution.execution_id != execution_id
        || execution.repository != repository
    {
        return Err(ShotExecutionError::Invalid(
            "execution record does not match this Shot repository".into(),
        ));
    }
    Ok(execution)
}

pub fn update_phase(
    execution: &mut PreparedExecution,
    phase: ExecutionPhase,
    report: impl Into<String>,
) -> Result<(), ShotExecutionError> {
    execution.phase = phase;
    write_record(execution)?;
    append_event(execution, phase, report)
}

/// Append a durable, privacy-preserving liveness event while a native harness
/// is still running. This deliberately does not replace the execution's
/// current phase (for example, `workspace_changed`) or proxy private harness
/// output into Studio.
pub fn append_harness_heartbeat(
    execution: &PreparedExecution,
    report: impl Into<String>,
) -> Result<(), ShotExecutionError> {
    append_event(execution, ExecutionPhase::HarnessRunning, report)
}

pub fn read_events(
    repository: &Path,
    execution_id: &str,
) -> Result<Vec<ShotExecutionEvent>, ShotExecutionError> {
    let execution = load_execution(repository, execution_id)?;
    let path = execution_directory(repository, execution_id).join("events.jsonl");
    let body = fs::read_to_string(path)?;
    body.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let event: ShotExecutionEvent = serde_json::from_str(line)?;
            if event.schema != EXECUTION_EVENT_SCHEMA
                || event.execution_id != execution.execution_id
                || event.shot_id != execution.shot_id
            {
                return Err(ShotExecutionError::Invalid(
                    "execution event identity mismatch".into(),
                ));
            }
            Ok(event)
        })
        .collect()
}

pub fn load_completion(
    repository: &Path,
    execution_id: &str,
) -> Result<Option<CompletionRecord>, ShotExecutionError> {
    validate_execution_id(execution_id)?;
    let path = execution_directory(repository, execution_id).join("completion.json");
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn complete_execution(
    execution: &mut PreparedExecution,
    started_at: String,
    duration_seconds: u64,
    exit_code: Option<i32>,
    cancelled: bool,
    accepted_version: bool,
    validation_passed: bool,
    validation_evidence: Option<String>,
) -> Result<CompletionRecord, ShotExecutionError> {
    execution.process_id = None;
    let directory = execution_directory(&execution.repository, &execution.execution_id);
    let final_tree = capture_tree(&execution.repository, &directory)?;
    let files_changed =
        changed_files(&execution.repository, &execution.baseline.tree, &final_tree)?;
    if !files_changed.is_empty() && execution.phase != ExecutionPhase::WorkspaceChanged {
        update_phase(
            execution,
            ExecutionPhase::WorkspaceChanged,
            format!(
                "{} workspace file(s) changed after the boundary.",
                files_changed.len()
            ),
        )?;
    }
    update_phase(
        execution,
        ExecutionPhase::ValidationStarted,
        "Checking the accepted-version and repository evidence.",
    )?;
    let validation_command = if execution.mode == ExecutionMode::BirthMaterialization {
        "engine birth acceptance: protocol_conformance + intent_fidelity + experience_verification"
    } else {
        "engine Version recording: declared verification gates"
    };
    let validation = if accepted_version {
        ValidationObservation {
            command: validation_command.into(),
            status: if validation_passed {
                "passed"
            } else {
                "failed"
            }
            .into(),
            evidence: validation_evidence,
        }
    } else {
        ValidationObservation {
            command: validation_command.into(),
            status: "not_accepted".into(),
            evidence: validation_evidence,
        }
    };
    update_phase(
        execution,
        ExecutionPhase::ValidationCompleted,
        if validation_passed && execution.mode == ExecutionMode::BirthMaterialization {
            "The accepted birth passed all three independent dimensions."
        } else if validation_passed {
            "The recorded Version passed every gate declared for that Version."
        } else if accepted_version {
            "An accepted Version exists, but independent verification failed."
        } else {
            "The candidate remains unsealed because engine acceptance did not pass."
        },
    )?;
    let outcome = if cancelled {
        ExecutionOutcome::Cancelled
    } else if exit_code == Some(0) {
        ExecutionOutcome::Completed
    } else {
        ExecutionOutcome::Failed
    };
    // Canonical acceptance outranks the wrapper's exit code. A harness can
    // fail after the engine durably accepted and verified the Version; that
    // failure remains the process outcome, but cannot un-land history.
    let landed = accepted_version && validation_passed;
    let git_diff_summary = git_output(
        &execution.repository,
        &["diff", "--shortstat", &execution.baseline.tree, &final_tree],
    )?
    .trim()
    .to_owned();
    let authoritative_next_action = if landed {
        "Experience the accepted app now running on the paired iPhone.".into()
    } else if cancelled {
        "The Shot was cancelled. Start a new execution only when you want to retry the preserved intention.".into()
    } else if !files_changed.is_empty() {
        "Review the engine diagnostic and unaccepted candidate evidence; repair the failed criteria without changing the accepted intention.".into()
    } else {
        "Inspect the execution events, then start a new execution for the preserved exact intention; final execution records are immutable.".into()
    };
    let ended_at = now()?;
    let duration_seconds = duration_seconds.max(elapsed_seconds(&started_at, &ended_at));
    let completion = CompletionRecord {
        schema: COMPLETION_RECORD_SCHEMA.into(),
        execution_id: execution.execution_id.clone(),
        shot_id: execution.shot_id,
        version_ordinal: execution.version_ordinal,
        mode: Some(execution.mode),
        outcome: outcome.clone(),
        landed,
        harness: execution.harness.clone(),
        model: execution.model.clone(),
        route: execution.route.clone(),
        intention_digest: execution.intention_digest,
        reference_digests: execution
            .references
            .iter()
            .map(|reference| reference.digest)
            .collect(),
        started_at,
        ended_at,
        duration_seconds,
        exit_code,
        files_changed,
        git_diff_summary,
        baseline_tree: execution.baseline.tree.clone(),
        final_tree: final_tree.clone(),
        pre_existing_worktree_status: execution.baseline.pre_existing_status.clone(),
        validation_commands_executed: if accepted_version {
            vec![if execution.mode == ExecutionMode::BirthMaterialization {
                "engine three-dimensional birth acceptance".into()
            } else {
                "engine declared Version gates".into()
            }]
        } else {
            Vec::new()
        },
        validation_results: vec![validation],
        harness_provided_final_summary: None,
        independently_computed_repository_state: format!(
            "Git tree {final_tree}; accepted version {:04}: {}; independent verification: {}.",
            execution.version_ordinal, accepted_version, validation_passed
        ),
        estimated_additional_cost_usd: execution.estimated_additional_cost_usd,
        actual_additional_cost_usd: if execution.route_billing == "subscription" {
            execution.estimated_additional_cost_usd
        } else {
            None
        },
        authoritative_next_action,
    };
    write_private_json(&directory.join("completion.json"), &completion)?;
    let final_phase = match outcome {
        ExecutionOutcome::Completed => ExecutionPhase::ExecutionCompleted,
        ExecutionOutcome::Failed => ExecutionPhase::ExecutionFailed,
        ExecutionOutcome::Cancelled => ExecutionPhase::ExecutionCancelled,
    };
    update_phase(
        execution,
        final_phase,
        if landed {
            if execution.mode == ExecutionMode::BirthMaterialization {
                "The complete birth passed all three dimensions and was accepted."
            } else {
                "The candidate became a verified accepted Version."
            }
        } else {
            "The harness ended with an unsealed candidate; no incomplete Version was accepted."
        },
    )?;
    Ok(completion)
}

pub fn capture_tree(
    repository: &Path,
    execution_directory: &Path,
) -> Result<String, ShotExecutionError> {
    let index = execution_directory.join(format!("git-index-{}", std::process::id()));
    if index.exists() {
        fs::remove_file(&index)?;
    }
    let mut read_tree = Command::new("git");
    read_tree
        .arg("-C")
        .arg(repository)
        .args(["read-tree", "--empty"])
        .env("GIT_INDEX_FILE", &index);
    command_ok(read_tree, "initialize temporary Git index")?;
    let mut add = Command::new("git");
    add.arg("-C")
        .arg(repository)
        .args(["add", "-A", "--", "."])
        .env("GIT_INDEX_FILE", &index);
    command_ok(add, "capture the Shot workspace")?;
    let mut write_tree = Command::new("git");
    write_tree
        .arg("-C")
        .arg(repository)
        .arg("write-tree")
        .env("GIT_INDEX_FILE", &index);
    let output = command_output(write_tree, "write the Shot boundary tree")?;
    let _ = fs::remove_file(index);
    let tree = output.trim().to_owned();
    if tree.len() != 40 || !tree.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ShotExecutionError::Git(
            "Git returned an invalid tree identity".into(),
        ));
    }
    Ok(tree)
}

pub fn has_workspace_changed(execution: &PreparedExecution) -> Result<bool, ShotExecutionError> {
    let directory = execution_directory(&execution.repository, &execution.execution_id);
    Ok(capture_tree(&execution.repository, &directory)? != execution.baseline.tree)
}

/// Summarize visible harness progress without exposing private prompt text,
/// source filenames, logs, or model output. Standardized artifact classes are
/// enough for Studio to distinguish planning, implementation, tests, and
/// experience evidence while the detached process remains the source of
/// truth for liveness.
pub fn privacy_safe_workspace_progress(
    execution: &PreparedExecution,
) -> Result<String, ShotExecutionError> {
    let directory = execution_directory(&execution.repository, &execution.execution_id);
    let current_tree = capture_tree(&execution.repository, &directory)?;
    let files = changed_files(
        &execution.repository,
        &execution.baseline.tree,
        &current_tree,
    )?;
    Ok(workspace_progress_from_changes(
        &execution.repository,
        &files,
    ))
}

fn workspace_progress_from_changes(repository: &Path, files: &[ChangedFile]) -> String {
    let mut signals = std::collections::BTreeSet::new();
    for file in files {
        let lower = file.path.to_ascii_lowercase();
        let components = lower.split('/').collect::<Vec<_>>();
        if components
            .iter()
            .any(|component| component.ends_with(".xcodeproj"))
            || lower == "project.yml"
        {
            signals.insert("Xcode project definition");
        }
        if lower.ends_with(".swift") {
            signals.insert("Swift source");
        }
        if lower.contains("test") {
            signals.insert("test source");
        }
        if lower.contains("assets.xcassets")
            || lower.ends_with(".strings")
            || lower.ends_with(".xcprivacy")
        {
            signals.insert("app resources");
        }
        if lower.starts_with("tohseno/") || lower.starts_with("tohsenofascia/") {
            signals.insert("protocol integration");
        }
    }

    for (relative, signal) in [
        (
            ".tohseno/private/planning/conception-output.json",
            "conception proposal",
        ),
        (
            ".tohseno/private/planning/accepted-conception-output.json",
            "accepted conception",
        ),
        (
            ".tohseno/private/planning/experience-trial.json",
            "Experience Trial",
        ),
    ] {
        if real_file(&repository.join(relative)) {
            signals.insert(signal);
        }
    }
    if real_directory(&repository.join(".tohseno/private/planning/evidence")) {
        signals.insert("experience evidence");
    }

    let scope = if files.is_empty() {
        "no source-tree file change is visible yet".to_owned()
    } else {
        format!("{} source-tree file(s) changed", files.len())
    };
    if signals.is_empty() {
        format!("{scope}; no standardized candidate artifact is visible yet")
    } else {
        format!(
            "{scope}; visible artifact classes: {}",
            signals.into_iter().collect::<Vec<_>>().join(", ")
        )
    }
}

fn real_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
}

fn real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

pub fn execution_directory(repository: &Path, execution_id: &str) -> PathBuf {
    repository
        .join(".tohseno")
        .join("executions")
        .join(execution_id)
}

fn capture_git_boundary(
    repository: &Path,
    directory: &Path,
) -> Result<GitBoundary, ShotExecutionError> {
    let head = match git_output(repository, &["rev-parse", "--verify", "HEAD"]) {
        Ok(output) => Some(output.trim().into()),
        Err(_) => None,
    };
    let status = git_output(
        repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let tree = capture_tree(repository, directory)?;
    Ok(GitBoundary {
        tree,
        head,
        pre_existing_status: status.lines().map(str::to_owned).collect(),
    })
}

fn ensure_shot_repository(repository: &Path) -> Result<(), ShotExecutionError> {
    let metadata = fs::symlink_metadata(repository)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ShotExecutionError::Invalid(
            "Shot repository is not a real directory".into(),
        ));
    }
    let top = git_output(repository, &["rev-parse", "--show-toplevel"]).ok();
    let is_own_repository = top
        .as_deref()
        .map(str::trim)
        .is_some_and(|top| Path::new(top) == repository);
    if !is_own_repository {
        let mut command = Command::new("git");
        command.arg("init").arg("--quiet").arg(repository);
        command_ok(command, "initialize the private Shot repository")?;
    }
    Ok(())
}

fn ensure_no_active_execution(repository: &Path) -> Result<(), ShotExecutionError> {
    let root = repository.join(".tohseno/executions");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path().join("execution.json");
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let existing: PreparedExecution = serde_json::from_slice(&bytes)?;
        if !matches!(
            existing.phase,
            ExecutionPhase::ExecutionCompleted
                | ExecutionPhase::ExecutionFailed
                | ExecutionPhase::ExecutionCancelled
        ) {
            return Err(ShotExecutionError::Invalid(format!(
                "execution {} is still {:?}; complete or cancel it before preparing another Shot version",
                existing.execution_id, existing.phase
            )));
        }
    }
    Ok(())
}

fn changed_files(
    repository: &Path,
    baseline: &str,
    final_tree: &str,
) -> Result<Vec<ChangedFile>, ShotExecutionError> {
    let names = git_output(repository, &["diff", "--name-status", baseline, final_tree])?;
    let numstat = git_output(repository, &["diff", "--numstat", baseline, final_tree])?;
    let mut counts = std::collections::BTreeMap::new();
    for line in numstat.lines() {
        let mut fields = line.split('\t');
        let additions = fields.next().and_then(|value| value.parse().ok());
        let deletions = fields.next().and_then(|value| value.parse().ok());
        if let Some(path) = fields.next_back() {
            counts.insert(path.to_owned(), (additions, deletions));
        }
    }
    let mut files = Vec::new();
    for line in names.lines() {
        let mut fields = line.split('\t');
        let status = fields.next().unwrap_or("?").to_owned();
        let path = fields.next_back().unwrap_or_default().to_owned();
        let (additions, deletions) = counts.get(&path).copied().unwrap_or((None, None));
        files.push(ChangedFile {
            status,
            path,
            additions,
            deletions,
        });
    }
    Ok(files)
}

fn append_event(
    execution: &PreparedExecution,
    phase: ExecutionPhase,
    report: impl Into<String>,
) -> Result<(), ShotExecutionError> {
    let directory = execution_directory(&execution.repository, &execution.execution_id);
    let path = directory.join("events.jsonl");
    let sequence = match fs::read_to_string(&path) {
        Ok(body) => body.lines().filter(|line| !line.trim().is_empty()).count() as u64 + 1,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 1,
        Err(error) => return Err(error.into()),
    };
    let event = ShotExecutionEvent {
        schema: EXECUTION_EVENT_SCHEMA.into(),
        sequence,
        event: phase.event_name().into(),
        execution_id: execution.execution_id.clone(),
        shot_id: execution.shot_id,
        version_ordinal: execution.version_ordinal,
        harness: execution.harness.clone(),
        model: execution.model.clone(),
        timestamp: now()?,
        phase,
        report: report.into(),
    };
    let mut encoded = serde_json::to_vec(&event)?;
    encoded.push(b'\n');
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    options.open(path)?.write_all(&encoded)?;
    Ok(())
}

fn write_record(execution: &PreparedExecution) -> Result<(), ShotExecutionError> {
    let directory = execution_directory(&execution.repository, &execution.execution_id);
    write_private_json(&directory.join("execution.json"), execution)
}

fn write_private_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ShotExecutionError> {
    let parent = path
        .parent()
        .ok_or_else(|| ShotExecutionError::Invalid("private record has no parent".into()))?;
    fs::create_dir_all(parent)?;
    let mut encoded = serde_json::to_vec_pretty(value)?;
    encoded.push(b'\n');
    let stage = parent.join(format!(".record-{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(&stage)?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    fs::rename(stage, path)?;
    Ok(())
}

fn set_private_directory(path: &Path) -> Result<(), ShotExecutionError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn validate_execution_id(value: &str) -> Result<(), ShotExecutionError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(ShotExecutionError::Invalid(
            "execution identity must be 32 lowercase hexadecimal characters".into(),
        ));
    }
    Ok(())
}

fn new_execution_id(shot_id: ShotId, version: u64) -> String {
    let random = ShotId::random();
    let material = format!("{shot_id}:{version}:{}:{random}", std::process::id());
    sha256(material.as_bytes())
        .to_string()
        .trim_start_matches("0x")
        .chars()
        .take(32)
        .collect()
}

fn now() -> Result<String, ShotExecutionError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| ShotExecutionError::Invalid(error.to_string()))
}

fn elapsed_seconds(started_at: &str, ended_at: &str) -> u64 {
    let Ok(started) = OffsetDateTime::parse(started_at, &Rfc3339) else {
        return 0;
    };
    let Ok(ended) = OffsetDateTime::parse(ended_at, &Rfc3339) else {
        return 0;
    };
    u64::try_from((ended - started).whole_seconds()).unwrap_or(0)
}

fn git_output(repository: &Path, arguments: &[&str]) -> Result<String, ShotExecutionError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repository).args(arguments);
    command_output(command, &format!("git {}", arguments.join(" ")))
}

fn command_ok(mut command: Command, operation: &str) -> Result<(), ShotExecutionError> {
    let output = command
        .output()
        .map_err(|error| ShotExecutionError::Git(format!("{operation}: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ShotExecutionError::Git(format!(
            "{operation}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn command_output(mut command: Command, operation: &str) -> Result<String, ShotExecutionError> {
    let output = command
        .output()
        .map_err(|error| ShotExecutionError::Git(format!("{operation}: {error}")))?;
    if !output.status.success() {
        return Err(ShotExecutionError::Git(format!(
            "{operation}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| ShotExecutionError::Git(format!("{operation}: output was not UTF-8")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::{build_evolution_command, HarnessSelection};
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn git_trees_distinguish_execution_changes_from_the_prepared_boundary() {
        let temporary = tempfile::tempdir().unwrap();
        let repository = temporary.path().join("shot");
        fs::create_dir(&repository).unwrap();
        fs::create_dir(repository.join(".tohseno")).unwrap();
        fs::write(repository.join(".gitignore"), ".tohseno/\n").unwrap();
        fs::write(repository.join("before.txt"), "pre-existing\n").unwrap();
        ensure_shot_repository(&repository).unwrap();
        let execution = repository.join(".tohseno/executions/test");
        fs::create_dir_all(&execution).unwrap();
        let baseline = capture_tree(&repository, &execution).unwrap();
        fs::write(repository.join("after.txt"), "execution\n").unwrap();
        let final_tree = capture_tree(&repository, &execution).unwrap();
        assert_ne!(baseline, final_tree);
        let files = changed_files(&repository, &baseline, &final_tree).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "after.txt");
    }

    #[test]
    fn execution_ids_are_private_path_components() {
        let id = new_execution_id(ShotId::random(), 1);
        assert_eq!(id.len(), 32);
        assert!(id.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(validate_execution_id(&id).is_ok());
        assert!(validate_execution_id("../escape").is_err());
    }

    #[test]
    fn elapsed_duration_is_recovered_from_durable_timestamps() {
        assert_eq!(
            elapsed_seconds("2026-08-06T03:43:04Z", "2026-08-06T04:18:26Z"),
            2_122
        );
        assert_eq!(elapsed_seconds("invalid", "2026-08-06T04:18:26Z"), 0);
    }

    #[test]
    fn workspace_progress_exposes_artifact_classes_without_private_filenames() {
        let temporary = tempfile::tempdir().unwrap();
        let repository = temporary.path();
        fs::create_dir_all(repository.join(".tohseno/private/planning/evidence")).unwrap();
        fs::write(
            repository.join(".tohseno/private/planning/experience-trial.json"),
            b"{}\n",
        )
        .unwrap();
        let files = vec![
            ChangedFile {
                status: "A".into(),
                path: "SecretProductName/SecretFeature.swift".into(),
                additions: Some(10),
                deletions: Some(0),
            },
            ChangedFile {
                status: "A".into(),
                path: "secret.xcodeproj/project.pbxproj".into(),
                additions: Some(10),
                deletions: Some(0),
            },
        ];

        let report = workspace_progress_from_changes(repository, &files);

        assert!(report.contains("2 source-tree file(s) changed"));
        assert!(report.contains("Xcode project definition"));
        assert!(report.contains("Swift source"));
        assert!(report.contains("Experience Trial"));
        assert!(report.contains("experience evidence"));
        assert!(!report.contains("SecretProductName"));
        assert!(!report.contains("SecretFeature"));
        assert!(!report.contains("secret.xcodeproj"));
    }

    #[test]
    fn fixture_execution_runs_through_adapter_events_diff_and_completion() {
        static ENVIRONMENT: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENVIRONMENT.get_or_init(|| Mutex::new(())).lock().unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let repository = temporary.path().join("fixture-shot");
        let bin = temporary.path().join("bin");
        fs::create_dir_all(repository.join(".tohseno/references")).unwrap();
        fs::create_dir(&bin).unwrap();
        fs::write(repository.join(".gitignore"), ".tohseno/\n").unwrap();
        fs::write(
            repository.join(".tohseno/EVOLUTION_INTENT.md"),
            "# Intent\n\nCreate the fixture output.\n",
        )
        .unwrap();
        fs::write(
            repository.join(".tohseno/references/image_1.png"),
            b"\x89PNG\r\n\x1a\nfixture",
        )
        .unwrap();
        let fake_codex = bin.join("codex");
        fs::write(
            &fake_codex,
            "#!/bin/sh\nprintf 'implemented by fixture\\n' > implemented.txt\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&fake_codex, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let old_path = std::env::var_os("PATH");
        let old_key = std::env::var_os("OPENAI_API_KEY");
        let path = std::env::join_paths(
            std::iter::once(bin.clone()).chain(
                old_path
                    .as_ref()
                    .map(std::env::split_paths)
                    .into_iter()
                    .flatten(),
            ),
        )
        .unwrap();
        std::env::set_var("PATH", path);
        std::env::set_var("OPENAI_API_KEY", "fixture-only");

        let selection = HarnessSelection {
            harness: "codex".into(),
            model: "default".into(),
            route: "openai-api".into(),
        };
        let image = repository.join(".tohseno/references/image_1.png");
        let image_bytes = fs::read(&image).unwrap();
        let mut execution = prepare_execution(
            &repository,
            ExecutionPreparation {
                app_name: "fixture-shot".into(),
                shot_id: ShotId::from_bytes([0x42; 32]),
                version_ordinal: 1,
                selection: selection.clone(),
                intent_path: ".tohseno/EVOLUTION_INTENT.md".into(),
                intention_digest: sha256(b"Create the fixture output."),
                references: vec![ExecutionReference {
                    label: "image_1".into(),
                    relative_path: ".tohseno/references/image_1.png".into(),
                    digest: sha256(&image_bytes),
                    byte_length: image_bytes.len() as u64,
                    media_type: "image/png".into(),
                }],
                mode: ExecutionMode::BirthMaterialization,
            },
        )
        .unwrap();
        update_phase(
            &mut execution,
            ExecutionPhase::ExecutionStarted,
            "Fixture confirmation.",
        )
        .unwrap();
        let started_at = read_events(&repository, &execution.execution_id)
            .unwrap()
            .last()
            .unwrap()
            .timestamp
            .clone();
        update_phase(
            &mut execution,
            ExecutionPhase::ContextLoaded,
            "Fixture context loaded.",
        )
        .unwrap();
        let command = build_evolution_command(
            &selection,
            Path::new(".tohseno/EVOLUTION_INTENT.md"),
            &[PathBuf::from(".tohseno/references/image_1.png")],
        )
        .unwrap();
        update_phase(
            &mut execution,
            ExecutionPhase::HarnessRunning,
            "Fixture adapter running.",
        )
        .unwrap();
        append_harness_heartbeat(&execution, "Fixture adapter is still running.").unwrap();
        assert_eq!(execution.phase, ExecutionPhase::HarnessRunning);
        let status = Command::new(command.program)
            .args(command.arguments)
            .current_dir(&repository)
            .status()
            .unwrap();
        let completion = complete_execution(
            &mut execution,
            started_at,
            0,
            status.code(),
            false,
            false,
            false,
            None,
        )
        .unwrap();
        assert_eq!(completion.outcome, ExecutionOutcome::Completed);
        assert!(!completion.landed);
        assert_eq!(completion.files_changed.len(), 1);
        assert_eq!(completion.files_changed[0].path, "implemented.txt");
        assert!(repository.join("implemented.txt").is_file());
        let names = read_events(&repository, &execution.execution_id)
            .unwrap()
            .into_iter()
            .map(|event| event.event)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "shot.prepared",
                "execution.started",
                "context.loaded",
                "harness.running",
                "harness.running",
                "workspace.changed",
                "validation.started",
                "validation.completed",
                "execution.completed",
            ]
        );

        match old_path {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
        match old_key {
            Some(value) => std::env::set_var("OPENAI_API_KEY", value),
            None => std::env::remove_var("OPENAI_API_KEY"),
        }
    }
}
