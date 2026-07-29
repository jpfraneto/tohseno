use thiserror::Error;

pub type Result<T> = std::result::Result<T, ProtocolError>;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid {field}: {reason}")]
    InvalidField { field: &'static str, reason: String },
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("canonical JSON failed: {0}")]
    CanonicalJson(String),
    #[error("invalid P-256 public key")]
    InvalidPublicKey,
    #[error("invalid P-256 signature")]
    InvalidSignature,
    #[error("P-256 signature is not low-s")]
    HighSignatureS,
    #[error("digest does not match the signed object")]
    DigestMismatch,
    #[error("lineage error at sequence {sequence}: {reason}")]
    Lineage { sequence: u32, reason: String },
    #[error("source-tree path is invalid: {0}")]
    InvalidTreePath(String),
    #[error("source-tree entry is a symbolic link: {0}")]
    TreeSymlink(String),
    #[error("source-tree entry is not a regular file or directory: {0}")]
    TreeEntryType(String),
    #[error("source-tree contains a forbidden private or generated path: {0}")]
    TreeForbidden(String),
    #[error("source-tree changed while it was being read: {0}")]
    TreeChanged(String),
    #[error("duplicate normalized source-tree path: {0}")]
    DuplicateTreePath(String),
    #[error("source-tree I/O failed at {path}: {source}")]
    TreeIo {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Fascia-tree path is invalid: {0}")]
    InvalidFasciaTreePath(String),
    #[error("Fascia-tree entry is a symbolic link: {0}")]
    FasciaTreeSymlink(String),
    #[error("Fascia-tree entry is not a regular file or directory: {0}")]
    FasciaTreeEntryType(String),
    #[error("Fascia-tree changed while it was being read: {0}")]
    FasciaTreeChanged(String),
    #[error("duplicate normalized Fascia-tree path: {0}")]
    DuplicateFasciaTreePath(String),
    #[error("Fascia-tree I/O failed at {path}: {source}")]
    FasciaTreeIo {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
