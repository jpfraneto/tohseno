use serde::{Deserialize, Serialize};
use serde_json::Value;
use tohseno_protocol::digest::{Bytes32, ExpressionId, ShotId, VersionId};

pub const COMMAND_SCHEMA: &str = "tohseno.local-command/1";
pub const COMMAND_STATUS_SCHEMA: &str = "tohseno.local-command-status/1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    ShotCreate,
    ShotEvolve,
    FeedbackSubmit,
    MarketingSubmit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOrigin {
    Cli,
    Studio,
    Companion,
    Conformance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandState {
    Received,
    Validated,
    Accepted,
    Running,
    WaitingForDevice,
    Completed,
    Rejected,
    Failed,
    Cancelled,
}

impl CommandState {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Rejected | Self::Failed | Self::Cancelled
        )
    }

    pub fn may_transition_to(self, next: Self) -> bool {
        use CommandState as State;
        self == next
            || matches!(
                (self, next),
                (
                    State::Received,
                    State::Validated | State::Rejected | State::Failed
                ) | (
                    State::Validated,
                    State::Accepted | State::Rejected | State::Failed
                ) | (
                    State::Accepted,
                    State::Running
                        | State::Completed
                        | State::Rejected
                        | State::Failed
                        | State::Cancelled
                ) | (
                    State::Running,
                    State::WaitingForDevice | State::Completed | State::Failed | State::Cancelled
                ) | (
                    State::WaitingForDevice,
                    State::Running | State::Completed | State::Failed | State::Cancelled
                )
            )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRecord {
    pub schema: String,
    pub command_id: String,
    pub command_kind: CommandKind,
    pub origin: CommandOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_device_id: Option<String>,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot_id: Option<ShotId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_expression_id: Option<ExpressionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_version_id: Option<VersionId>,
    pub submitted_at: String,
    pub payload_digest: Bytes32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResult {
    pub receipt: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandStatus {
    pub schema: String,
    pub command_id: String,
    pub state: CommandState,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<CommandResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection: Option<String>,
}
