use crate::presentation::Presentation;
use serde::{Deserialize, Serialize};
use std::fs;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_engine::safe_file::read_bounded_regular_file;
use tohseno_engine::shot_execution::{
    elapsed_seconds_between, load_execution, load_state_transition_receipt, read_events,
    ExecutionPhase, ShotExecutionEvent, StateTransitionReceipt,
};
use tohseno_engine::{AppKind, Engine, ShotLayout};
use tohseno_protocol::digest::{sha256, ExpressionId, VersionId};

pub const WORKSPACE_SNAPSHOT_SCHEMA: &str = "tohseno.companion-workspace-snapshot/1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShotKind {
    FactoryShot,
    RecordingOnly,
    /// Private owner-local source project. It has no public Shot lineage.
    AdoptedProject,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportedCompanionAction {
    Read,
    Feedback,
    Marketing,
    Evolve,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IconDescriptor {
    pub revision: String,
    pub blob_id: String,
    pub media_type: String,
    pub byte_length: u64,
    pub placeholder: bool,
    /// Exact private bytes for the local companion projector. They are never
    /// serialized into Studio/API snapshots; the projector places them only
    /// inside recipient-encrypted companion envelopes.
    #[serde(skip)]
    pub private_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSummary {
    pub execution_id: String,
    pub shot_id: String,
    pub state: String,
    pub version_ordinal: u64,
    pub started_at: String,
    pub elapsed_seconds: u64,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_transition: Option<StateTransitionReceipt>,
}

/// Bounded private continuity projected to the owner UI. The authoritative
/// adopted-project record remains in the versioned living-project store.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvolutionHistorySummary {
    pub evolution_id: String,
    pub requested_at: String,
    pub request_summary: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_summary: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotSummary {
    pub shot_id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_identifier: Option<String>,
    pub kind: ShotKind,
    /// Private source-state token for an adopted project. Never a protocol
    /// Expression or Version identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_state: Option<String>,
    pub icon: IconDescriptor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression_id: Option<ExpressionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version_id: Option<VersionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version_ordinal: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version_created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<ExecutionSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_evolutions: Vec<EvolutionHistorySummary>,
    /// The one human state a person sees. Studio renders this instead of
    /// interpreting execution phases for itself.
    pub presentation: Presentation,
    pub archived: bool,
    pub retired: bool,
    pub sort_index: u64,
    pub supported_companion_actions: Vec<SupportedCompanionAction>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSnapshot {
    pub schema: String,
    pub workspace_id: String,
    pub snapshot_version: u64,
    pub generated_at: String,
    pub service_version: String,
    pub shots: Vec<ShotSummary>,
    pub active_executions: Vec<ExecutionSummary>,
    pub device_capability_epoch: u64,
    pub next_cursor: u64,
}

type AcceptedHead = (
    Option<ExpressionId>,
    Option<VersionId>,
    Option<u64>,
    Option<String>,
);

pub fn build_workspace_snapshot(
    engine: &Engine,
    workspace_id: &str,
    snapshot_version: u64,
    device_capability_epoch: u64,
    next_cursor: u64,
) -> Result<WorkspaceSnapshot, Box<dyn std::error::Error + Send + Sync>> {
    let generated_at = now();
    let mut shots = Vec::new();
    let mut active_executions = Vec::new();
    for (index, app) in engine.ledger().list_apps()?.into_iter().enumerate() {
        let kind = match engine.app_kind(&app.name)? {
            AppKind::FactoryShot => ShotKind::FactoryShot,
            AppKind::RecordingOnly => ShotKind::RecordingOnly,
            AppKind::LegacyProtocol => continue,
        };
        let stable_id = match (kind, app.shot_id) {
            (ShotKind::FactoryShot, Some(shot_id)) => shot_id.to_string(),
            (ShotKind::FactoryShot, None) => continue,
            (ShotKind::RecordingOnly, _) => recording_id(workspace_id, &app.name),
            (ShotKind::AdoptedProject, _) => continue,
        };
        let latest_execution = latest_execution(engine, &app.name, &stable_id)?;
        let execution = latest_execution
            .as_ref()
            .map(|(summary, _)| summary.clone());
        if let Some((execution, true)) = latest_execution {
            active_executions.push(execution);
        }
        let (expression_id, latest_version_id, latest_version_ordinal, accepted_at) =
            if kind == ShotKind::FactoryShot {
                accepted_head(engine, &app.name, app.expression_id)?
            } else {
                (None, None, app.latest_evolution.map(u64::from), None)
            };
        let icon = discover_icon(engine, &app.name)?;
        let supported_companion_actions = if kind == ShotKind::FactoryShot {
            let mut actions = vec![SupportedCompanionAction::Read];
            if latest_version_id.is_some() {
                actions.push(SupportedCompanionAction::Feedback);
            }
            actions.push(SupportedCompanionAction::Marketing);
            if latest_version_id.is_some() {
                actions.push(SupportedCompanionAction::Evolve);
            }
            actions
        } else {
            vec![SupportedCompanionAction::Read]
        };
        let presentation = Presentation::for_app(
            &app.name,
            execution.as_ref().map(|summary| summary.state.as_str()),
            latest_version_id.is_some(),
        );
        shots.push(ShotSummary {
            shot_id: stable_id,
            display_name: app.name,
            bundle_identifier: (kind == ShotKind::FactoryShot).then_some(app.bundle_id),
            kind,
            source_state: None,
            icon,
            expression_id,
            latest_version_id,
            latest_version_ordinal,
            latest_version_created_at: accepted_at,
            execution,
            recent_evolutions: Vec::new(),
            presentation,
            archived: false,
            retired: app.retired,
            sort_index: u64::try_from(index).unwrap_or(u64::MAX),
            supported_companion_actions,
        });
    }
    active_executions.sort_by(|left, right| left.execution_id.cmp(&right.execution_id));
    Ok(WorkspaceSnapshot {
        schema: WORKSPACE_SNAPSHOT_SCHEMA.into(),
        workspace_id: workspace_id.into(),
        snapshot_version,
        generated_at,
        service_version: env!("CARGO_PKG_VERSION").into(),
        shots,
        active_executions,
        device_capability_epoch,
        next_cursor,
    })
}

pub fn load_shot_icon(
    engine: &Engine,
    workspace_id: &str,
    shot_id: &str,
) -> Result<Option<IconDescriptor>, Box<dyn std::error::Error + Send + Sync>> {
    for app in engine.ledger().list_apps()? {
        let kind = match engine.app_kind(&app.name)? {
            AppKind::FactoryShot => ShotKind::FactoryShot,
            AppKind::RecordingOnly => ShotKind::RecordingOnly,
            AppKind::LegacyProtocol => continue,
        };
        let stable_id = match (kind, app.shot_id) {
            (ShotKind::FactoryShot, Some(value)) => value.to_string(),
            (ShotKind::FactoryShot, None) => continue,
            (ShotKind::RecordingOnly, _) => recording_id(workspace_id, &app.name),
            (ShotKind::AdoptedProject, _) => continue,
        };
        if stable_id == shot_id {
            return discover_icon(engine, &app.name).map(Some);
        }
    }
    Ok(None)
}

pub fn load_shot_preview(
    engine: &Engine,
    workspace_id: &str,
    shot_id: &str,
) -> Result<Option<IconDescriptor>, Box<dyn std::error::Error + Send + Sync>> {
    for app in engine.ledger().list_apps()? {
        let kind = match engine.app_kind(&app.name)? {
            AppKind::FactoryShot => ShotKind::FactoryShot,
            AppKind::RecordingOnly => ShotKind::RecordingOnly,
            AppKind::LegacyProtocol => continue,
        };
        let stable_id = match (kind, app.shot_id) {
            (ShotKind::FactoryShot, Some(value)) => value.to_string(),
            (ShotKind::FactoryShot, None) => continue,
            (ShotKind::RecordingOnly, _) => recording_id(workspace_id, &app.name),
            (ShotKind::AdoptedProject, _) => continue,
        };
        if stable_id != shot_id {
            continue;
        }
        let path = engine.ledger().working_tree(&app.name).join("preview.png");
        let bytes = match read_bounded_regular_file(&path, 32 * 1024 * 1024) {
            Ok(bytes) if !bytes.is_empty() => bytes,
            Ok(_) => return Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let digest = sha256(&bytes);
        return Ok(Some(IconDescriptor {
            revision: digest.to_string(),
            blob_id: digest.to_string(),
            media_type: "image/png".into(),
            byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            placeholder: false,
            private_bytes: bytes,
        }));
    }
    Ok(None)
}

fn accepted_head(
    engine: &Engine,
    app_name: &str,
    expression_id: Option<ExpressionId>,
) -> Result<AcceptedHead, Box<dyn std::error::Error + Send + Sync>> {
    let Some(expression_id) = expression_id else {
        return Ok((None, None, None, None));
    };
    let layout = ShotLayout::at(engine.ledger().working_tree(app_name));
    let lineage = layout.read_lineage()?;
    if lineage.is_empty() {
        let app = engine.ledger().load_app(app_name)?;
        if app.latest_evolution.is_none() {
            // Identity publication precedes initial lineage publication. A
            // crash in that bounded window is recoverable by the durable
            // create command, and Studio must still be able to show the
            // reserved Shot rather than losing the entire workspace view.
            return Ok((Some(expression_id), None, None, None));
        }
        return Err("an app with accepted history has no canonical lineage".into());
    }
    let state = tohseno_protocol::reduce_lineage(&lineage)?;
    let Some(expression) = state.expression(expression_id) else {
        return Ok((Some(expression_id), None, None, None));
    };
    let Some(version_id) = expression.current_version else {
        return Ok((Some(expression_id), None, None, None));
    };
    let version = expression
        .versions
        .iter()
        .find(|version| version.version_id == version_id)
        .ok_or("current Version is absent from the reduced lineage")?;
    Ok((
        Some(expression_id),
        Some(version_id),
        Some(version.ordinal),
        Some(version.accepted_at.as_str().to_owned()),
    ))
}

fn latest_execution(
    engine: &Engine,
    app_name: &str,
    shot_id: &str,
) -> Result<Option<(ExecutionSummary, bool)>, Box<dyn std::error::Error + Send + Sync>> {
    let root = engine
        .ledger()
        .working_tree(app_name)
        .join(".tohseno/executions");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut summaries = Vec::new();
    for entry in entries.take(10_000) {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let execution = match load_execution(&engine.ledger().working_tree(app_name), &id) {
            Ok(execution) => execution,
            Err(_) => continue,
        };
        let active = !matches!(
            execution.phase,
            ExecutionPhase::ExecutionCompleted
                | ExecutionPhase::ExecutionFailed
                | ExecutionPhase::ExecutionCancelled
        );
        let events = read_events(
            &engine.ledger().working_tree(app_name),
            &execution.execution_id,
        )
        .unwrap_or_default();
        let observed_at = now();
        let (started_at, updated_at, elapsed_seconds) =
            execution_timing(&execution.prepared_at, &events, &observed_at, active);
        let state_transition = if active {
            None
        } else {
            load_state_transition_receipt(
                &engine.ledger().working_tree(app_name),
                &execution.execution_id,
            )
            .ok()
            .flatten()
        };
        summaries.push((
            ExecutionSummary {
                execution_id: execution.execution_id,
                shot_id: shot_id.into(),
                state: privacy_safe_phase(execution.phase).into(),
                version_ordinal: execution.version_ordinal,
                elapsed_seconds,
                started_at,
                updated_at,
                state_transition,
            },
            active,
        ));
    }
    summaries.sort_by(|(left, _), (right, _)| {
        right
            .version_ordinal
            .cmp(&left.version_ordinal)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| right.execution_id.cmp(&left.execution_id))
    });
    Ok(summaries.into_iter().next())
}

fn execution_timing(
    prepared_at: &str,
    events: &[ShotExecutionEvent],
    observed_at: &str,
    active: bool,
) -> (String, String, u64) {
    let started_at = events
        .first()
        .map(|event| event.timestamp.clone())
        .unwrap_or_else(|| prepared_at.to_owned());
    let updated_at = events
        .last()
        .map(|event| event.timestamp.clone())
        .unwrap_or_else(|| prepared_at.to_owned());
    // A completed execution is a historical fact. Its elapsed time ends at
    // its last durable event and must not grow every time Studio refreshes.
    let elapsed_until = if active { observed_at } else { &updated_at };
    let elapsed = elapsed_seconds_between(&started_at, elapsed_until);
    (started_at, updated_at, elapsed)
}

pub(crate) fn privacy_safe_phase(phase: ExecutionPhase) -> &'static str {
    match phase {
        ExecutionPhase::Prepared
        | ExecutionPhase::RunnerStarted
        | ExecutionPhase::TerminalOpened => "queued",
        ExecutionPhase::ExecutionStarted | ExecutionPhase::ContextLoaded => "planning",
        ExecutionPhase::Conception => "conception",
        ExecutionPhase::Materializing
        | ExecutionPhase::HarnessRunning
        | ExecutionPhase::WorkspaceChanged => "materializing",
        ExecutionPhase::Building => "building",
        ExecutionPhase::Testing => "testing",
        ExecutionPhase::Verifying | ExecutionPhase::ValidationStarted => "verifying",
        ExecutionPhase::Repairing => "repairing",
        ExecutionPhase::Installing => "installing",
        ExecutionPhase::Launching => "launching",
        ExecutionPhase::WaitingForDevice => "waiting_for_device",
        ExecutionPhase::ValidationCompleted => "verifying",
        ExecutionPhase::ExecutionCompleted => "accepted",
        ExecutionPhase::ExecutionFailed => "failed",
        ExecutionPhase::ExecutionCancelled => "cancelled",
    }
}

pub(crate) fn recording_id(workspace_id: &str, name: &str) -> String {
    let digest =
        sha256(format!("TOHSENO-RECORDING-SUMMARY-ID-V1\0{workspace_id}\0{name}").as_bytes());
    format!(
        "recording_{}",
        digest
            .to_string()
            .trim_start_matches("0x")
            .chars()
            .take(24)
            .collect::<String>()
    )
}

fn discover_icon(
    engine: &Engine,
    app_name: &str,
) -> Result<IconDescriptor, Box<dyn std::error::Error + Send + Sync>> {
    let root = engine.ledger().working_tree(app_name);
    let mut candidates = Vec::new();
    collect_icon_candidates(&root, &root, 0, &mut candidates)?;
    candidates.sort();
    candidates.reverse();
    for (_, path) in candidates {
        let bytes = match read_icon_file(&path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let digest = sha256(&bytes);
        let media_type = match path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "jpg" | "jpeg" => "image/jpeg",
            _ => "image/png",
        };
        return Ok(IconDescriptor {
            revision: digest.to_string(),
            blob_id: digest.to_string(),
            media_type: media_type.into(),
            byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            placeholder: false,
            private_bytes: bytes,
        });
    }
    let bytes = include_bytes!("../../brand/logos/tohseno-app-icon-1024.png").to_vec();
    let digest = sha256(&bytes);
    Ok(IconDescriptor {
        revision: digest.to_string(),
        blob_id: digest.to_string(),
        media_type: "image/png".into(),
        byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        placeholder: true,
        private_bytes: bytes,
    })
}

fn collect_icon_candidates(
    root: &std::path::Path,
    directory: &std::path::Path,
    depth: usize,
    output: &mut Vec<(u64, std::path::PathBuf)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if depth > 8 || output.len() >= 128 {
        return Ok(());
    }
    for entry in fs::read_dir(directory)?.take(2048) {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let relative = path.strip_prefix(root)?;
        if relative.components().any(|component| {
            matches!(
                component.as_os_str().to_str(),
                Some(".git" | ".tohseno" | ".build" | "build")
            )
        }) {
            continue;
        }
        if metadata.is_dir() {
            collect_icon_candidates(root, &path, depth + 1, output)?;
        } else if metadata.is_file()
            && metadata.len() <= 2 * 1024 * 1024
            && path.to_string_lossy().contains("AppIcon")
            && matches!(
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("png" | "jpg" | "jpeg")
            )
        {
            output.push((metadata.len(), path));
        }
    }
    Ok(())
}

fn read_icon_file(
    path: &std::path::Path,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Read;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 2 * 1024 * 1024
    {
        return Err("icon is not a bounded regular file".into());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err("icon changed while it was opened".into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("icon exceeds the private blob limit".into());
    }
    Ok(bytes)
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .expect("zero nanoseconds are valid")
        .format(&Rfc3339)
        .expect("UTC timestamps format")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_ids_are_private_opaque_and_stable() {
        let first = recording_id("workspace_1", "notes");
        assert_eq!(first, recording_id("workspace_1", "notes"));
        assert_ne!(first, recording_id("workspace_2", "notes"));
        assert!(first.starts_with("recording_"));
        assert!(!first.contains("notes"));
    }

    #[test]
    fn execution_phase_projection_never_exposes_harness_details() {
        assert_eq!(
            privacy_safe_phase(ExecutionPhase::HarnessRunning),
            "materializing"
        );
        assert_eq!(
            privacy_safe_phase(ExecutionPhase::ValidationStarted),
            "verifying"
        );
        assert_eq!(
            privacy_safe_phase(ExecutionPhase::ValidationCompleted),
            "verifying"
        );
    }

    #[test]
    fn active_total_elapsed_uses_the_observation_across_repair_attempts() {
        let event = |sequence, phase: ExecutionPhase, timestamp: &str| ShotExecutionEvent {
            schema: "tohseno.local-shot-execution-event/1".into(),
            sequence,
            event: phase.event_name().into(),
            execution_id: "execution_fixture".into(),
            shot_id: tohseno_protocol::digest::ShotId::from_bytes([0x61; 32]),
            version_ordinal: 1,
            harness: "fixture".into(),
            model: "fixture".into(),
            timestamp: timestamp.into(),
            phase,
            report: "fixture".into(),
        };
        let events = vec![
            event(1, ExecutionPhase::Prepared, "2026-08-21T04:38:00Z"),
            event(2, ExecutionPhase::Materializing, "2026-08-21T04:39:00Z"),
            event(3, ExecutionPhase::Repairing, "2026-08-21T10:39:00Z"),
        ];
        let (started_at, updated_at, elapsed) = execution_timing(
            "2026-08-21T04:38:00Z",
            &events,
            "2026-08-21T10:50:00Z",
            true,
        );
        assert_eq!(started_at, "2026-08-21T04:38:00Z");
        assert_eq!(updated_at, "2026-08-21T10:39:00Z");
        assert_eq!(elapsed, 6 * 3_600 + 12 * 60);
    }

    #[test]
    fn terminal_total_elapsed_freezes_at_the_last_durable_event() {
        let event = |sequence, phase: ExecutionPhase, timestamp: &str| ShotExecutionEvent {
            schema: "tohseno.local-shot-execution-event/1".into(),
            sequence,
            event: phase.event_name().into(),
            execution_id: "execution_fixture".into(),
            shot_id: tohseno_protocol::digest::ShotId::from_bytes([0x61; 32]),
            version_ordinal: 1,
            harness: "fixture".into(),
            model: "fixture".into(),
            timestamp: timestamp.into(),
            phase,
            report: "fixture".into(),
        };
        let events = vec![
            event(1, ExecutionPhase::Prepared, "2026-08-21T04:38:00Z"),
            event(
                2,
                ExecutionPhase::ExecutionCompleted,
                "2026-08-21T04:53:24Z",
            ),
        ];
        let (_, updated_at, elapsed) = execution_timing(
            "2026-08-21T04:38:00Z",
            &events,
            "2026-08-27T10:50:00Z",
            false,
        );
        assert_eq!(updated_at, "2026-08-21T04:53:24Z");
        assert_eq!(elapsed, 15 * 60 + 24);
    }

    #[test]
    fn empty_lineage_is_tolerated_only_before_any_accepted_history() {
        let temporary = tempfile::tempdir().unwrap();
        let temporary_root = temporary.path().canonicalize().unwrap();
        let ledger = tohseno_engine::Ledger::at_homes(
            temporary_root.join("family"),
            temporary_root.join("machine"),
        );
        let engine = Engine::at(
            ledger.clone(),
            tohseno_engine::EventBus::default(),
            tohseno_engine::Config::default(),
        );
        let working = engine.initialize_app("fixture").unwrap();
        fs::write(working.join("source.txt"), b"accepted history claim\n").unwrap();
        engine.record_version("fixture", None).unwrap();
        let builder_id = tohseno_protocol::identity::BuilderId::parse(
            "eip155:4663:0x1111111111111111111111111111111111111111",
        )
        .unwrap();
        let app = ledger
            .bind_protocol_identity(
                "fixture",
                tohseno_protocol::digest::ShotId::from_bytes([0x51; 32]),
                builder_id,
            )
            .unwrap();

        let error = accepted_head(&engine, "fixture", app.expression_id)
            .unwrap_err()
            .to_string();
        assert!(error.contains("accepted history has no canonical lineage"));
    }
}
