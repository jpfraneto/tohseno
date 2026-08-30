//! Privacy-safe Mac-to-phone workspace events.

use crate::canonical;
use crate::command::{CommandReceipt, ReceiptState};
use crate::crypto::sha256;
use crate::icon::IconBlob;
use crate::publication::PublicationApprovalRequest;
use crate::snapshot::{ExecutionStatus, ExecutionSummary, ShotSummary, WorkspaceSnapshot};
use crate::{parse_timestamp, require, validate_identifier, Result};
use serde::{Deserialize, Serialize};

pub const COMPANION_EVENT_SCHEMA: &str = "tohseno.companion-event/1";
pub const PRODUCT_ENTITLEMENT_SCHEMA: &str = "tohseno.private-product-entitlement/1";

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
}
