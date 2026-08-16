//! Privacy-safe, versioned workspace snapshots sent inside encrypted envelopes.

use crate::capability::CapabilityAction;
use crate::{parse_timestamp, require, validate_identifier, validate_text, Result};
use serde::{Deserialize, Serialize};

pub const WORKSPACE_SNAPSHOT_SCHEMA: &str = "tohseno.companion-workspace-snapshot/1";
pub const MAX_SHOTS_PER_SNAPSHOT: usize = 10_000;
pub const MAX_EXECUTIONS_PER_SNAPSHOT: usize = 1_000;
pub const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_ICON_DIMENSION: u32 = 2_048;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShotKind {
    FactoryShot,
    RecordingOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Queued,
    Planning,
    Conception,
    Materializing,
    Building,
    Testing,
    Verifying,
    Repairing,
    WaitingForDevice,
    Installing,
    Launching,
    Accepted,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IconDescriptor {
    /// Opaque lookup key for a separately encrypted blob. Never a local path.
    pub blob_id: String,
    pub revision: u64,
    pub media_type: String,
    pub byte_length: u64,
    pub width: u32,
    pub height: u32,
    pub placeholder: bool,
}

impl IconDescriptor {
    pub fn validate(&self) -> Result<()> {
        validate_identifier("icon blob ID", &self.blob_id)?;
        require(self.revision > 0, "icon revision must be positive")?;
        require(
            matches!(self.media_type.as_str(), "image/png" | "image/jpeg"),
            "icon media type must be image/png or image/jpeg",
        )?;
        require(
            (1..=MAX_ICON_BYTES).contains(&self.byte_length),
            "icon byte length is invalid",
        )?;
        require(
            (1..=MAX_ICON_DIMENSION).contains(&self.width)
                && (1..=MAX_ICON_DIMENSION).contains(&self.height),
            "icon dimensions are invalid",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSummary {
    pub execution_id: String,
    pub shot_id: String,
    pub state: ExecutionStatus,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<String>,
}

impl ExecutionSummary {
    pub fn validate(&self) -> Result<()> {
        validate_identifier("execution ID", &self.execution_id)?;
        validate_identifier("execution Shot ID", &self.shot_id)?;
        parse_timestamp(&self.updated_at)?;
        if let Some(code) = &self.failure_code {
            validate_identifier("privacy-safe failure code", code)?;
        }
        require(
            self.state == ExecutionStatus::Failed || self.failure_code.is_none(),
            "only failed executions may contain a failure code",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotSummary {
    pub shot_id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_identifier: Option<String>,
    pub kind: ShotKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconDescriptor>,
    pub icon_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version_ordinal: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version_created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<ExecutionSummary>,
    pub archived: bool,
    pub retired: bool,
    pub sort_index: i64,
    pub supported_companion_actions: Vec<CapabilityAction>,
}

impl ShotSummary {
    pub fn validate(&self) -> Result<()> {
        validate_identifier("Shot ID", &self.shot_id)?;
        validate_text("Shot display name", &self.display_name, 256)?;
        if let Some(bundle_identifier) = &self.bundle_identifier {
            validate_text("bundle identifier", bundle_identifier, 255)?;
            require(
                bundle_identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')),
                "bundle identifier contains unsupported characters",
            )?;
        }
        if let Some(icon) = &self.icon {
            icon.validate()?;
            require(
                icon.revision == self.icon_revision,
                "icon descriptor revision does not match Shot icon revision",
            )?;
        }
        let version_fields = [
            self.latest_version_id.is_some(),
            self.latest_version_ordinal.is_some(),
            self.latest_version_created_at.is_some(),
        ];
        require(
            version_fields.iter().all(|present| *present)
                || version_fields.iter().all(|present| !*present),
            "latest Version fields must be present or absent together",
        )?;
        if let Some(expression_id) = &self.expression_id {
            validate_identifier("Expression ID", expression_id)?;
        }
        if let Some(version_id) = &self.latest_version_id {
            validate_identifier("Version ID", version_id)?;
        }
        if let Some(ordinal) = self.latest_version_ordinal {
            require(ordinal > 0, "Version ordinal must be positive")?;
        }
        if let Some(created_at) = &self.latest_version_created_at {
            parse_timestamp(created_at)?;
        }
        if let Some(execution) = &self.execution {
            execution.validate()?;
            require(
                execution.shot_id == self.shot_id,
                "execution summary belongs to a different Shot",
            )?;
        }
        require(
            !(self.kind == ShotKind::RecordingOnly
                && (self.expression_id.is_some() || self.latest_version_id.is_some())),
            "recording-only folders cannot be silently represented as factory lineage",
        )?;
        require(
            !(self.archived && self.retired),
            "a Shot cannot be both archived and retired",
        )?;
        require(
            self.supported_companion_actions.len() <= 6,
            "too many supported companion actions",
        )?;
        let mut actions = self.supported_companion_actions.clone();
        actions.sort();
        actions.dedup();
        require(
            actions == self.supported_companion_actions,
            "supported companion actions must be unique and sorted",
        )?;
        require(
            !actions.contains(&CapabilityAction::ShotCreate),
            "Shot creation is a workspace action, not a per-Shot action",
        )?;
        if self.kind == ShotKind::RecordingOnly {
            require(
                !actions.contains(&CapabilityAction::FeedbackWrite)
                    && !actions.contains(&CapabilityAction::ShotEvolve),
                "recording-only folders cannot advertise factory lineage actions",
            )?;
        }
        if actions.contains(&CapabilityAction::FeedbackWrite)
            || actions.contains(&CapabilityAction::ShotEvolve)
        {
            require(
                self.expression_id.is_some() && self.latest_version_id.is_some(),
                "exact-Version actions require accepted Expression and Version identities",
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceCapabilityState {
    pub device_id: String,
    pub capability_id: String,
    pub revocation_epoch: u64,
    pub allowed_actions: Vec<CapabilityAction>,
    pub revoked: bool,
}

impl DeviceCapabilityState {
    pub fn validate(&self) -> Result<()> {
        validate_identifier("device ID", &self.device_id)?;
        validate_identifier("capability ID", &self.capability_id)?;
        require(
            self.allowed_actions.len() <= 6,
            "too many device capability actions",
        )?;
        let mut actions = self.allowed_actions.clone();
        actions.sort();
        actions.dedup();
        require(
            actions == self.allowed_actions,
            "device capability actions must be unique and sorted",
        )
    }
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
    pub device_capability_state: DeviceCapabilityState,
    pub next_cursor: u64,
}

impl WorkspaceSnapshot {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == WORKSPACE_SNAPSHOT_SCHEMA,
            "unsupported workspace snapshot schema",
        )?;
        validate_identifier("workspace ID", &self.workspace_id)?;
        require(
            self.snapshot_version > 0,
            "snapshot version must be positive",
        )?;
        parse_timestamp(&self.generated_at)?;
        validate_text("service version", &self.service_version, 64)?;
        require(
            self.shots.len() <= MAX_SHOTS_PER_SNAPSHOT,
            "workspace snapshot contains too many Shots",
        )?;
        require(
            self.active_executions.len() <= MAX_EXECUTIONS_PER_SNAPSHOT,
            "workspace snapshot contains too many executions",
        )?;
        for shot in &self.shots {
            shot.validate()?;
        }
        let mut shot_ids: Vec<_> = self.shots.iter().map(|shot| &shot.shot_id).collect();
        shot_ids.sort();
        shot_ids.dedup();
        require(
            shot_ids.len() == self.shots.len(),
            "workspace snapshot contains duplicate Shot IDs",
        )?;
        for execution in &self.active_executions {
            execution.validate()?;
            require(
                !matches!(
                    execution.state,
                    ExecutionStatus::Accepted | ExecutionStatus::Failed
                ),
                "active execution list contains a terminal execution",
            )?;
        }
        let mut execution_ids: Vec<_> = self
            .active_executions
            .iter()
            .map(|execution| &execution.execution_id)
            .collect();
        execution_ids.sort();
        execution_ids.dedup();
        require(
            execution_ids.len() == self.active_executions.len(),
            "workspace snapshot contains duplicate execution IDs",
        )?;
        self.device_capability_state.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(kind: ShotKind) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            schema: WORKSPACE_SNAPSHOT_SCHEMA.into(),
            workspace_id: "workspace_fixture".into(),
            snapshot_version: 4,
            generated_at: "2026-08-15T12:00:00Z".into(),
            service_version: "0.9.0".into(),
            shots: vec![ShotSummary {
                shot_id: "shot_fixture".into(),
                display_name: "Fixture".into(),
                bundle_identifier: None,
                kind,
                icon: None,
                icon_revision: 1,
                expression_id: None,
                latest_version_id: None,
                latest_version_ordinal: None,
                latest_version_created_at: None,
                execution: None,
                archived: false,
                retired: false,
                sort_index: 0,
                supported_companion_actions: vec![CapabilityAction::WorkspaceRead],
            }],
            active_executions: vec![],
            device_capability_state: DeviceCapabilityState {
                device_id: "device_fixture".into(),
                capability_id: "capability_fixture".into(),
                revocation_epoch: 0,
                allowed_actions: vec![CapabilityAction::WorkspaceRead],
                revoked: false,
            },
            next_cursor: 1,
        }
    }

    #[test]
    fn recording_only_snapshot_cannot_claim_lineage() {
        let mut value = snapshot(ShotKind::RecordingOnly);
        value.validate().unwrap();
        value.shots[0].expression_id = Some("expression_forbidden".into());
        assert!(value.validate().is_err());
        value.shots[0].expression_id = None;
        value.shots[0].supported_companion_actions = vec![CapabilityAction::FeedbackWrite];
        assert!(value.validate().is_err());
    }

    #[test]
    fn privacy_safe_snapshot_round_trips_strictly() {
        let value = snapshot(ShotKind::FactoryShot);
        value.validate().unwrap();
        let bytes = crate::canonical::to_vec(&value).unwrap();
        let decoded: WorkspaceSnapshot = crate::canonical::from_slice(&bytes).unwrap();
        assert_eq!(decoded, value);
        assert!(!String::from_utf8(bytes).unwrap().contains("source"));
    }
}
