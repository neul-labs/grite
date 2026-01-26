use thiserror::Error;

/// Errors that can occur during Git operations
#[derive(Debug, Error)]
pub enum GitError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("CBOR decode error: {0}")]
    CborDecode(String),

    #[error("Invalid chunk format: {0}")]
    InvalidChunk(String),

    #[error("WAL error: {0}")]
    Wal(String),

    #[error("Snapshot error: {0}")]
    Snapshot(String),

    #[error("Sync error: {0}")]
    Sync(String),

    #[error("Invalid event data: {0}")]
    InvalidEvent(String),

    #[error("Not a git repository")]
    NotARepo,

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Lock conflict: {resource} is locked by {owner} (expires in {expires_in_ms}ms)")]
    LockConflict {
        resource: String,
        owner: String,
        expires_in_ms: u64,
    },

    #[error("Lock not owned: {resource} is owned by {owner}")]
    LockNotOwned {
        resource: String,
        owner: String,
    },
}
