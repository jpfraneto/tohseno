//! One honest answer to "what did that actually do?"
//!
//! The factory already records everything an owner needs to trust it: the
//! exact words that were sent, the harness that ran them, what it burned, and
//! the deterministic gate that refused the result. Until this projection
//! existed those facts were reachable only by reading private JSON on disk,
//! so a failure looked like a shrug.
//!
//! This is a Details-only disclosure. It adds no vocabulary to the normal
//! Create/Evolve path, and it never invents a fact: anything the factory did
//! not record is absent rather than defaulted.

use crate::execution_manager::command_execution_id;
use crate::snapshot::{recording_id, ShotKind};
use serde::Serialize;
use std::fs;
use std::path::Path;
use tohseno_engine::shot_execution::{
    execution_directory, load_completion, load_state_transition_receipt, preserved_intent,
    read_events, ExecutionPhase, StateTransitionReceipt,
};
use tohseno_engine::{AppKind, Engine};

pub const EXECUTION_RECEIPT_SCHEMA: &str = "tohseno.execution-receipt/1";
pub const EXECUTION_ACTIVITY_SCHEMA: &str = "tohseno.execution-activity/1";

/// The most intention text one receipt will carry.
const MAXIMUM_INTENTION_CHARACTERS: usize = 20_000;
/// The most gate evidence one refusal will carry.
const MAXIMUM_EVIDENCE_CHARACTERS: usize = 8_000;
const MAXIMUM_JOURNAL_ENTRIES: usize = 100_000;
const MAXIMUM_JOURNAL_PAYLOAD_BYTES: u64 = 4 * 1024 * 1024;
const MAXIMUM_ACTIVITY_ENTRIES: usize = 200;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// What was asked, what ran, what it cost, and what happened.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExecutionReceipt {
    pub schema: &'static str,
    pub execution_id: String,
    pub app_name: String,
    pub version_ordinal: u64,
    pub phase: String,

    /// The exact words this execution was prepared from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intention: Option<String>,
    /// Where those words were read from, so a missing intention is legible.
    pub intention_source: IntentionSource,
    pub intention_digest: String,
    pub reference_count: usize,

    /// The harness that actually ran, never the currently configured one.
    pub harness: String,
    pub harness_id: String,
    pub model: String,
    pub route: String,
    pub route_billing: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,

    /// Tokens the harness reported burning, summed across its attempts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harness_attempts: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_cost_usd: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_changed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_transition: Option<StateTransitionReceipt>,
    /// Every deterministic gate that declined to seal the candidate.
    pub refusals: Vec<Refusal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
}

/// A bounded, privacy-safe live projection of one execution's durable journal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutionActivity {
    pub schema: &'static str,
    pub execution_id: String,
    pub complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    pub entries: Vec<ExecutionActivityEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutionActivityEntry {
    pub sequence: u64,
    pub timestamp: String,
    pub phase: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentionSource {
    /// This execution's own preserved copy.
    Execution,
    /// The durable command journal, for executions prepared before TOHSENO
    /// kept a per-execution copy.
    CommandJournal,
    /// Neither survived; the digest is still shown rather than a guess.
    Unavailable,
}

/// One gate that refused, said the way the engine said it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Refusal {
    pub check: String,
    pub status: String,
    /// The failing criterion, when the evidence names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// Assemble the receipt for a Shot's most recent execution.
pub fn load_execution_receipt(
    engine: &Engine,
    workspace_id: &str,
    journal_root: &Path,
    shot_id: &str,
) -> Result<Option<ExecutionReceipt>, BoxError> {
    let Some(app_name) = resolve_app_name(engine, workspace_id, shot_id)? else {
        return Ok(None);
    };
    let repository = engine.ledger().working_tree(&app_name);
    let Some(execution_id) = latest_execution_id(&repository)? else {
        return Ok(None);
    };
    let execution = tohseno_engine::shot_execution::load_execution(&repository, &execution_id)?;
    let completion = load_completion(&repository, &execution_id).ok().flatten();

    let (intention, intention_source) = resolve_intention(&repository, &execution_id, journal_root);

    // Executions that finished before TOHSENO metered anything still have
    // their private harness log. Reading it here means the owner sees the
    // real number for work already done, not a blank for everything old.
    let usage = completion
        .as_ref()
        .and_then(|record| record.token_usage.clone())
        .or_else(|| {
            tohseno_engine::read_harness_usage(
                &execution.harness,
                &execution_directory(&repository, &execution_id).join("harness.log"),
            )
        });
    let refusals = completion
        .as_ref()
        .map(|record| {
            record
                .validation_results
                .iter()
                // The engine records "passed", "failed", or "not_accepted".
                // Only the two that withheld a Version are refusals.
                .filter(|observation| observation.status != "passed")
                .map(|observation| Refusal {
                    check: observation.command.clone(),
                    status: observation.status.clone(),
                    gate: observation
                        .evidence
                        .as_deref()
                        .and_then(named_field("gate=")),
                    evidence: observation
                        .evidence
                        .as_deref()
                        .map(|value| bounded(value, MAXIMUM_EVIDENCE_CHARACTERS)),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(Some(ExecutionReceipt {
        schema: EXECUTION_RECEIPT_SCHEMA,
        execution_id,
        app_name,
        version_ordinal: execution.version_ordinal,
        phase: phase_label(execution.phase).into(),
        intention,
        intention_source,
        intention_digest: execution.intention_digest.to_string(),
        reference_count: execution.references.len(),
        harness: execution.harness_display_name.clone(),
        harness_id: execution.harness.clone(),
        model: execution.model.clone(),
        route: execution.route.clone(),
        route_billing: execution.route_billing.clone(),
        started_at: completion
            .as_ref()
            .map(|record| record.started_at.clone())
            .or_else(|| Some(execution.prepared_at.clone())),
        ended_at: completion.as_ref().map(|record| record.ended_at.clone()),
        duration_seconds: completion.as_ref().map(|record| record.duration_seconds),
        exit_code: completion.as_ref().and_then(|record| record.exit_code),
        total_tokens: usage.as_ref().map(|usage| usage.total_tokens),
        harness_attempts: usage.as_ref().map(|usage| usage.reported_attempts),
        additional_cost_usd: completion
            .as_ref()
            .and_then(|record| record.actual_additional_cost_usd),
        outcome: completion
            .as_ref()
            .map(|record| outcome_label(&record.outcome).into()),
        landed: completion.as_ref().map(|record| record.landed),
        files_changed: completion.as_ref().map(|record| record.files_changed.len()),
        diff_summary: completion
            .as_ref()
            .map(|record| record.git_diff_summary.clone())
            .filter(|summary| !summary.trim().is_empty()),
        state_transition: load_state_transition_receipt(&repository, &execution.execution_id)
            .ok()
            .flatten(),
        refusals,
        next_action: completion
            .as_ref()
            .map(|record| record.authoritative_next_action.clone()),
    }))
}

/// Load the latest execution's safe activity without disclosing harness text.
pub fn load_execution_activity(
    engine: &Engine,
    workspace_id: &str,
    shot_id: &str,
) -> Result<Option<ExecutionActivity>, BoxError> {
    let Some(app_name) = resolve_app_name(engine, workspace_id, shot_id)? else {
        return Ok(None);
    };
    let repository = engine.ledger().working_tree(&app_name);
    let Some(execution_id) = latest_execution_id(&repository)? else {
        return Ok(None);
    };
    let execution = tohseno_engine::shot_execution::load_execution(&repository, &execution_id)?;
    let completion = load_completion(&repository, &execution_id)?;
    let usage = completion
        .as_ref()
        .and_then(|record| record.token_usage.clone())
        .or_else(|| {
            tohseno_engine::read_harness_usage(
                &execution.harness,
                &execution_directory(&repository, &execution_id).join("harness.log"),
            )
        });
    let events = read_events(&repository, &execution_id)?;
    let first = events.len().saturating_sub(MAXIMUM_ACTIVITY_ENTRIES);
    let entries = events[first..]
        .iter()
        .map(|event| ExecutionActivityEntry {
            sequence: event.sequence,
            timestamp: event.timestamp.clone(),
            phase: phase_label(event.phase).into(),
            message: event.report.clone(),
        })
        .collect();
    Ok(Some(ExecutionActivity {
        schema: EXECUTION_ACTIVITY_SCHEMA,
        execution_id,
        complete: completion.is_some(),
        total_tokens: usage.map(|value| value.total_tokens),
        entries,
    }))
}

fn resolve_app_name(
    engine: &Engine,
    workspace_id: &str,
    shot_id: &str,
) -> Result<Option<String>, BoxError> {
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
        };
        if stable_id == shot_id {
            return Ok(Some(app.name));
        }
    }
    Ok(None)
}

fn latest_execution_id(repository: &Path) -> Result<Option<String>, BoxError> {
    let root = repository.join(".tohseno/executions");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut best: Option<(u64, String, String)> = None;
    for entry in entries.take(10_000) {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(execution) = tohseno_engine::shot_execution::load_execution(repository, &id) else {
            continue;
        };
        let candidate = (
            execution.version_ordinal,
            execution.prepared_at.clone(),
            id.clone(),
        );
        if best.as_ref().is_none_or(|current| *current < candidate) {
            best = Some(candidate);
        }
    }
    Ok(best.map(|(_, _, id)| id))
}

/// Read the exact words, preferring this execution's own copy.
fn resolve_intention(
    repository: &Path,
    execution_id: &str,
    journal_root: &Path,
) -> (Option<String>, IntentionSource) {
    if let Ok(Some(document)) = preserved_intent(repository, execution_id) {
        return (
            Some(bounded(
                extract_intention(&document),
                MAXIMUM_INTENTION_CHARACTERS,
            )),
            IntentionSource::Execution,
        );
    }
    if let Some(intention) = journal_intention(journal_root, execution_id) {
        return (
            Some(bounded(&intention, MAXIMUM_INTENTION_CHARACTERS)),
            IntentionSource::CommandJournal,
        );
    }
    // The app-level document belongs to whichever request wrote it last, so it
    // is only trustworthy while this execution is the newest one. That case is
    // already covered by the preserved copy, and guessing here would show one
    // request's words under another request's receipt.
    (None, IntentionSource::Unavailable)
}

/// Find the durable command that reserved this execution identity.
///
/// The execution ID is a one-way derivation of the command ID, so the journal
/// is searched by recomputing it rather than by trusting a stored back-link.
fn journal_intention(journal_root: &Path, execution_id: &str) -> Option<String> {
    let entries = fs::read_dir(journal_root).ok()?;
    for entry in entries.take(MAXIMUM_JOURNAL_ENTRIES) {
        let entry = entry.ok()?;
        let Some(command_id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if command_execution_id(&command_id) != execution_id {
            continue;
        }
        let path = entry.path().join("payload.json");
        let metadata = fs::symlink_metadata(&path).ok()?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MAXIMUM_JOURNAL_PAYLOAD_BYTES
        {
            return None;
        }
        let bytes = fs::read(&path).ok()?;
        let payload: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        return payload
            .get("intention")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
    }
    None
}

/// Pull the human's words out of the deterministic prepared intent document.
///
/// The document is generated by `ShotLayout::prepare_intent_package`, so these
/// markers are exact. A document that does not match is shown whole rather
/// than silently truncated to nothing.
fn extract_intention(document: &str) -> &str {
    const OPENING: &str = "## Intention\n\n";
    const CLOSING: &str = "\n\n## Reference images";
    let Some(start) = document.find(OPENING) else {
        return document.trim();
    };
    let body = &document[start + OPENING.len()..];
    match body.find(CLOSING) {
        Some(end) => body[..end].trim(),
        None => body.trim(),
    }
}

/// Read one `key=value` field out of an engine evidence string.
fn named_field(key: &'static str) -> impl Fn(&str) -> Option<String> {
    move |evidence| {
        let start = evidence.find(key)? + key.len();
        let value: String = evidence[start..]
            .chars()
            .take_while(|character| !character.is_whitespace())
            .collect();
        (!value.is_empty()).then_some(value)
    }
}

fn bounded(value: &str, maximum: usize) -> String {
    if value.chars().count() <= maximum {
        return value.into();
    }
    let kept: String = value.chars().take(maximum).collect();
    format!("{kept}…")
}

fn phase_label(phase: ExecutionPhase) -> &'static str {
    crate::snapshot::privacy_safe_phase(phase)
}

fn outcome_label(outcome: &tohseno_engine::shot_execution::ExecutionOutcome) -> &'static str {
    use tohseno_engine::shot_execution::ExecutionOutcome;
    match outcome {
        ExecutionOutcome::Completed => "completed",
        ExecutionOutcome::Failed => "failed",
        ExecutionOutcome::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prepared_document_yields_only_the_human_words() {
        let document = "# TOHSENO Evolution Intent\n\n## Intention\n\nMake the timer breathe.\n\n## Reference images\n\nNo reference images were supplied.\n";
        assert_eq!(extract_intention(document), "Make the timer breathe.");
    }

    #[test]
    fn an_unrecognized_document_is_shown_rather_than_lost() {
        assert_eq!(extract_intention("  just words  "), "just words");
    }

    #[test]
    fn the_failing_gate_is_named_from_engine_evidence() {
        let evidence = "gate=fascia.capability_reconciliation category=protocol_integrity file=breathwork/BreathDetector.swift";
        assert_eq!(
            named_field("gate=")(evidence).as_deref(),
            Some("fascia.capability_reconciliation")
        );
        assert_eq!(named_field("gate=")("no fields here"), None);
    }

    #[test]
    fn long_material_is_bounded_with_an_honest_ellipsis() {
        assert_eq!(bounded("abcdef", 3), "abc…");
        assert_eq!(bounded("abc", 3), "abc");
    }
}
