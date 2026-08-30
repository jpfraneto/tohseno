use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("invalid network object: {0}")]
    Invalid(String),
    #[error("publication snapshot contains a secret-bearing path: {0}")]
    SecretPath(PathBuf),
    #[error("publication snapshot contains likely secret material: {0}")]
    SecretContent(PathBuf),
    #[error("publication snapshot contains an unsafe path: {0}")]
    UnsafePath(PathBuf),
    #[error("publication artifact exceeds the supported bound")]
    Oversized,
    #[error("publication artifact changed while it was being read: {0}")]
    ConcurrentMutation(PathBuf),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] tohseno_protocol::ProtocolError),
}

pub type Result<T> = std::result::Result<T, NetworkError>;
