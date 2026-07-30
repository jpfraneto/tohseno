use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, NodeError>;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("unsafe storage path: {0}")]
    UnsafeStorage(PathBuf),
    #[error("action exceeds the {limit}-byte limit")]
    ActionTooLarge { limit: usize },
    #[error("node action limit of {limit} reached")]
    ActionLimit { limit: usize },
    #[error("private or local-only actions are not accepted for replication")]
    NotPublic,
    #[error("lineage action already exists with different bytes")]
    ContentCollision,
    #[error("lineage action {0} is not available")]
    ActionMissing(String),
    #[error("Shot {0} is not available")]
    ShotMissing(String),
    #[error("invalid causal lineage: {0}")]
    Causal(String),
    #[error("invalid peer URL: {0}")]
    InvalidPeer(String),
    #[error("peer response exceeds the {limit}-byte limit")]
    PeerResponseTooLarge { limit: usize },
    #[error("peer returned an invalid response: {0}")]
    PeerResponse(String),
    #[error("peer request failed: {0}")]
    PeerRequest(String),
    #[error("configured peer limit of {limit} exceeded")]
    PeerLimit { limit: usize },
    #[error("a sync is already in progress")]
    SyncInProgress,
    #[error("internal state lock is poisoned")]
    LockPoisoned,
}

impl From<tohseno_protocol::ProtocolError> for NodeError {
    fn from(value: tohseno_protocol::ProtocolError) -> Self {
        Self::Protocol(value.to_string())
    }
}
