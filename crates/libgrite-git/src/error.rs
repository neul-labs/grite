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

/// Bridge GitError into GriteError, preserving semantic variants.
impl From<GitError> for libgrite_core::GriteError {
    fn from(e: GitError) -> Self {
        match e {
            GitError::LockConflict { resource, owner, expires_in_ms } => {
                libgrite_core::GriteError::Conflict(format!(
                    "Resource '{}' is locked by {} (expires in {}s)",
                    resource, owner, expires_in_ms / 1000
                ))
            }
            GitError::LockNotOwned { resource, owner } => {
                libgrite_core::GriteError::Conflict(format!(
                    "Cannot release lock on '{}': owned by {}", resource, owner
                ))
            }
            GitError::NotARepo => {
                libgrite_core::GriteError::NotFound("Not a git repository".to_string())
            }
            GitError::Git(g) if g.code() == git2::ErrorCode::NotFound => {
                libgrite_core::GriteError::NotFound(g.message().to_string())
            }
            other => libgrite_core::GriteError::Internal(other.to_string()),
        }
    }
}
