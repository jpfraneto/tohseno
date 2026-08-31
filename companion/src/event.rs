//! Privacy-safe Mac-to-phone workspace events.

use crate::canonical;
use crate::capability::CapabilityGrant;
use crate::command::{CommandReceipt, ReceiptState};
use crate::crypto::{base64url, decode_array, sha256};
use crate::icon::IconBlob;
use crate::publication::PublicationApprovalRequest;
use crate::snapshot::{ExecutionStatus, ExecutionSummary, ShotSummary, WorkspaceSnapshot};
use crate::{parse_timestamp, require, validate_identifier, validate_text, Result};
use serde::{Deserialize, Serialize};

pub const COMPANION_EVENT_SCHEMA: &str = "tohseno.companion-event/1";
pub const PRODUCT_ENTITLEMENT_SCHEMA: &str = "tohseno.private-product-entitlement/1";
pub const BUILDER_FOLLOWS_SCHEMA: &str = "tohseno.private-builder-follows/1";
pub const PRIVATE_UPDATE_ITEM_SCHEMA: &str = "tohseno.private-update/1";
pub const PRIVATE_UPDATES_SCHEMA: &str = "tohseno.private-updates/1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivateUpdateKind {
    Claimed,
    ClaimedAppUpdated,
    PreparationReady,
    ForkShipped,
    EditionClosed,
    AliasApproved,
    PublicationApproval,
    EvolutionFinished,
}

impl PrivateUpdateKind {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Claimed => "claimed",
            Self::ClaimedAppUpdated => "claimed_app_updated",
            Self::PreparationReady => "preparation_ready",
            Self::ForkShipped => "fork_shipped",
            Self::EditionClosed => "edition_closed",
            Self::AliasApproved => "alias_approved",
            Self::PublicationApproval => "publication_approval",
            Self::EvolutionFinished => "evolution_finished",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateUpdateItem {
    pub schema: String,
    pub update_id: String,
    pub kind: PrivateUpdateKind,
    pub subject_id: String,
    pub evidence_id: String,
    pub title: String,
    pub detail: String,
    pub occurred_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<String>,
}

impl PrivateUpdateItem {
    pub fn stable_id(kind: PrivateUpdateKind, subject_id: &str, evidence_id: &str) -> String {
        let mut material = b"TOHSENO-PRIVATE-UPDATE-V1\0".to_vec();
        material.extend_from_slice(kind.wire_name().as_bytes());
        material.push(0);
        material.extend_from_slice(subject_id.as_bytes());
        material.push(0);
        material.extend_from_slice(evidence_id.as_bytes());
        format!("update_{}", base64url(&sha256(&material)))
    }

    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == PRIVATE_UPDATE_ITEM_SCHEMA,
            "unsupported private Update schema",
        )?;
        validate_identifier("private Update ID", &self.update_id)?;
        validate_text("private Update subject", &self.subject_id, 256)?;
        validate_text("private Update evidence", &self.evidence_id, 256)?;
        validate_text("private Update title", &self.title, 160)?;
        validate_text("private Update detail", &self.detail, 512)?;
        parse_timestamp(&self.occurred_at)?;
        if let Some(read_at) = &self.read_at {
            parse_timestamp(read_at)?;
        }
        require(
            self.update_id == Self::stable_id(self.kind, &self.subject_id, &self.evidence_id),
            "private Update ID does not match its stable evidence",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateUpdateProjection {
    pub schema: String,
    pub items: Vec<PrivateUpdateItem>,
    pub updated_at: String,
}

impl PrivateUpdateProjection {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == PRIVATE_UPDATES_SCHEMA,
            "unsupported private Updates schema",
        )?;
        require(
            self.items.len() <= 1_000,
            "private Updates exceed their bound",
        )?;
        for item in &self.items {
            item.validate()?;
        }
        require(
            self.items.windows(2).all(|pair| {
                pair[0].occurred_at > pair[1].occurred_at
                    || (pair[0].occurred_at == pair[1].occurred_at
                        && pair[0].update_id < pair[1].update_id)
            }),
            "private Updates must be unique and ordered",
        )?;
        parse_timestamp(&self.updated_at).map(|_| ())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderFollowProjection {
    pub schema: String,
    pub builder_ids: Vec<String>,
    pub updated_at: String,
}

impl BuilderFollowProjection {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == BUILDER_FOLLOWS_SCHEMA,
            "unsupported private Builder follow schema",
        )?;
        require(
            self.builder_ids.len() <= 10_000
                && self.builder_ids.windows(2).all(|pair| pair[0] < pair[1]),
            "private Builder follows must be bounded, unique, and sorted",
        )?;
        for builder_id in &self.builder_ids {
            require(
                builder_id.len() == 54
                    && builder_id.starts_with("eip155:4663:0x")
                    && builder_id[14..]
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                    && builder_id[14..].bytes().any(|byte| byte != b'0'),
                "private Builder follow contains an invalid BuilderID",
            )?;
        }
        parse_timestamp(&self.updated_at).map(|_| ())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductEntitlementProjection {
    pub schema: String,
    pub phase: String,
    pub successful_days: u8,
    pub required_successful_days: u8,
    pub factory_mutations_allowed: bool,
    pub purchase_allowed: bool,
}

impl ProductEntitlementProjection {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == PRODUCT_ENTITLEMENT_SCHEMA,
            "unsupported private product entitlement schema",
        )?;
        require(
            matches!(
                self.phase.as_str(),
                "genesis_incomplete"
                    | "trial_active"
                    | "trial_qualified"
                    | "trial_expired"
                    | "pro_monthly"
                    | "pro_yearly"
                    | "pro_lapsed"
            ),
            "private product entitlement phase is invalid",
        )?;
        require(
            self.successful_days <= self.required_successful_days
                && self.required_successful_days == 5,
            "private product successful-day count is invalid",
        )?;
        require(
            self.factory_mutations_allowed
                == matches!(
                    self.phase.as_str(),
                    "trial_active" | "pro_monthly" | "pro_yearly"
                ),
            "private product mutation projection differs from phase",
        )?;
        require(
            self.purchase_allowed
                == matches!(self.phase.as_str(), "trial_qualified" | "pro_lapsed"),
            "private product purchase projection differs from phase",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event_kind")]
pub enum WorkspaceEventPayload {
    #[serde(rename = "workspace.snapshot")]
    WorkspaceSnapshot { snapshot: Box<WorkspaceSnapshot> },
    #[serde(rename = "product.entitlement")]
    ProductEntitlement {
        entitlement: ProductEntitlementProjection,
    },
    #[serde(rename = "builder.follows")]
    BuilderFollows { follows: BuilderFollowProjection },
    #[serde(rename = "capability.updated")]
    CapabilityUpdated { capability: CapabilityGrant },
    #[serde(rename = "private.updates")]
    PrivateUpdates { updates: PrivateUpdateProjection },
    #[serde(rename = "shot.upsert")]
    ShotUpsert { shot: Box<ShotSummary> },
    #[serde(rename = "shot.archive")]
    ShotArchive { shot_id: String },
    #[serde(rename = "shot.remove")]
    ShotRemove { shot_id: String },
    #[serde(rename = "icon.blob")]
    IconBlob { blob: Box<IconBlob> },
    #[serde(rename = "version.accepted")]
    VersionAccepted {
        shot_id: String,
        expression_id: String,
        version_id: String,
        version_ordinal: u64,
        accepted_at: String,
    },
    #[serde(rename = "execution.queued")]
    ExecutionQueued { execution: ExecutionSummary },
    #[serde(rename = "execution.started")]
    ExecutionStarted { execution: ExecutionSummary },
    #[serde(rename = "execution.updated")]
    ExecutionUpdated { execution: ExecutionSummary },
    #[serde(rename = "execution.waiting_for_device")]
    ExecutionWaitingForDevice { execution: ExecutionSummary },
    #[serde(rename = "execution.completed")]
    ExecutionCompleted { execution: ExecutionSummary },
    #[serde(rename = "execution.failed")]
    ExecutionFailed { execution: ExecutionSummary },
    #[serde(rename = "command.acknowledged")]
    CommandAcknowledged { receipt: CommandReceipt },
    #[serde(rename = "command.rejected")]
    CommandRejected { receipt: CommandReceipt },
    #[serde(rename = "device.revoked")]
    DeviceRevoked {
        device_id: String,
        revocation_epoch: u64,
    },
    #[serde(rename = "publication.approval.requested")]
    PublicationApprovalRequested {
        request: Box<PublicationApprovalRequest>,
    },
}

impl WorkspaceEventPayload {
    fn validate(&self) -> Result<()> {
        match self {
            Self::WorkspaceSnapshot { snapshot } => snapshot.validate(),
            Self::ProductEntitlement { entitlement } => entitlement.validate(),
            Self::BuilderFollows { follows } => follows.validate(),
            Self::CapabilityUpdated { capability } => {
                let key = decode_array::<32>(
                    "updated capability Studio signing key",
                    &capability.body.studio_signing_public_key,
                )?;
                capability.verify(&key, time::OffsetDateTime::now_utc())
            }
            Self::PrivateUpdates { updates } => updates.validate(),
            Self::ShotUpsert { shot } => shot.validate(),
            Self::ShotArchive { shot_id } | Self::ShotRemove { shot_id } => {
                validate_identifier("event Shot ID", shot_id)
            }
            Self::IconBlob { blob } => blob.validate(),
            Self::VersionAccepted {
                shot_id,
                expression_id,
                version_id,
                version_ordinal,
                accepted_at,
            } => {
                validate_identifier("accepted Version Shot ID", shot_id)?;
                validate_identifier("accepted Expression ID", expression_id)?;
                validate_identifier("accepted Version ID", version_id)?;
                require(
                    *version_ordinal > 0,
                    "accepted Version ordinal must be positive",
                )?;
                parse_timestamp(accepted_at).map(|_| ())
            }
            Self::ExecutionQueued { execution } => {
                validate_execution_state(execution, &[ExecutionStatus::Queued])
            }
            Self::ExecutionStarted { execution } => validate_execution_state(
                execution,
                &[
                    ExecutionStatus::Planning,
                    ExecutionStatus::Conception,
                    ExecutionStatus::Materializing,
                ],
            ),
            Self::ExecutionUpdated { execution } => execution.validate(),
            Self::ExecutionWaitingForDevice { execution } => {
                validate_execution_state(execution, &[ExecutionStatus::WaitingForDevice])
            }
            Self::ExecutionCompleted { execution } => {
                validate_execution_state(execution, &[ExecutionStatus::Accepted])
            }
            Self::ExecutionFailed { execution } => {
                validate_execution_state(execution, &[ExecutionStatus::Failed])
            }
            Self::CommandAcknowledged { receipt } => {
                receipt.validate()?;
                require(
                    receipt.state != ReceiptState::Rejected,
                    "acknowledged command event contains a rejected receipt",
                )
            }
            Self::CommandRejected { receipt } => {
                receipt.validate()?;
                require(
                    receipt.state == ReceiptState::Rejected,
                    "rejected command event requires a rejected receipt",
                )
            }
            Self::DeviceRevoked {
                device_id,
                revocation_epoch,
            } => {
                validate_identifier("revoked device ID", device_id)?;
                require(*revocation_epoch > 0, "revocation epoch must be positive")
            }
            Self::PublicationApprovalRequested { request } => request.validate(),
        }
    }
}

fn validate_execution_state(
    execution: &ExecutionSummary,
    expected: &[ExecutionStatus],
) -> Result<()> {
    execution.validate()?;
    require(
        expected.contains(&execution.state),
        "execution event does not match its privacy-safe state",
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceEvent {
    pub schema: String,
    pub event_id: String,
    pub workspace_id: String,
    pub cursor: u64,
    pub emitted_at: String,
    pub payload: WorkspaceEventPayload,
}

impl WorkspaceEvent {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == COMPANION_EVENT_SCHEMA,
            "unsupported companion event schema",
        )?;
        validate_identifier("event ID", &self.event_id)?;
        validate_identifier("event workspace ID", &self.workspace_id)?;
        require(self.cursor > 0, "event cursor must be positive")?;
        parse_timestamp(&self.emitted_at)?;
        self.payload.validate()
    }

    pub fn digest(&self) -> Result<[u8; 32]> {
        self.validate()?;
        Ok(sha256(&canonical::to_vec(self)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn execution(state: ExecutionStatus) -> ExecutionSummary {
        ExecutionSummary {
            execution_id: "execution_fixture".into(),
            shot_id: "shot_fixture".into(),
            state,
            updated_at: "2026-08-15T12:00:00Z".into(),
            failure_code: None,
        }
    }

    #[test]
    fn event_name_and_state_must_agree() {
        let event = WorkspaceEvent {
            schema: COMPANION_EVENT_SCHEMA.into(),
            event_id: "event_fixture".into(),
            workspace_id: "workspace_fixture".into(),
            cursor: 1,
            emitted_at: "2026-08-15T12:00:00Z".into(),
            payload: WorkspaceEventPayload::ExecutionWaitingForDevice {
                execution: execution(ExecutionStatus::WaitingForDevice),
            },
        };
        event.validate().unwrap();
        let mut invalid = event;
        invalid.payload = WorkspaceEventPayload::ExecutionCompleted {
            execution: execution(ExecutionStatus::Building),
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn failed_execution_may_only_expose_a_bounded_code() {
        let mut failed = execution(ExecutionStatus::Failed);
        failed.failure_code = Some("verification_failed".into());
        failed.validate().unwrap();
        failed.failure_code = Some("raw harness output contains spaces".into());
        assert!(failed.validate().is_err());
    }

    #[test]
    fn private_update_identity_is_language_stable_and_projection_is_strictly_ordered() {
        let id =
            PrivateUpdateItem::stable_id(PrivateUpdateKind::ClaimedAppUpdated, "0x1111", "0x2222");
        assert_eq!(id, "update_eQPmHhbsHqXFJ-LydeluMnMIloHQDl7wOfOKVeGH3Nw");
        let item = PrivateUpdateItem {
            schema: PRIVATE_UPDATE_ITEM_SCHEMA.into(),
            update_id: id,
            kind: PrivateUpdateKind::ClaimedAppUpdated,
            subject_id: "0x1111".into(),
            evidence_id: "0x2222".into(),
            title: "Claimed app updated".into(),
            detail: "One canonical public release moved.".into(),
            occurred_at: "2026-08-31T12:00:00Z".into(),
            read_at: None,
        };
        item.validate().unwrap();
        let projection = PrivateUpdateProjection {
            schema: PRIVATE_UPDATES_SCHEMA.into(),
            items: vec![item.clone(), item],
            updated_at: "2026-08-31T12:00:00Z".into(),
        };
        assert!(
            projection.validate().is_err(),
            "duplicate evidence must fail closed"
        );
    }
}
