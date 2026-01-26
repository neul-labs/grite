//! Daemon-specific error types

use thiserror::Error;

/// Errors specific to daemon operations
#[derive(Error, Debug)]
pub enum DaemonError {
    /// Failed to bind to IPC socket
    #[error("Failed to bind to socket: {0}")]
    BindFailed(String),

    /// Failed to acquire daemon lock
    #[error("Failed to acquire lock: {0}")]
    LockFailed(String),

    /// Worker not found for request
    #[error("No worker for repo={repo_root}, actor={actor_id}")]
    WorkerNotFound { repo_root: String, actor_id: String },

    /// Worker already exists
    #[error("Worker already exists for repo={repo_root}, actor={actor_id}")]
    WorkerExists { repo_root: String, actor_id: String },

    /// Core grit error
    #[error("Grit error: {0}")]
    Grit(#[from] libgrit_core::GritError),

    /// Git error
    #[error("Git error: {0}")]
    Git(#[from] libgrit_git::GitError),

    /// IPC error
    #[error("IPC error: {0}")]
    Ipc(#[from] libgrit_ipc::IpcError),

    /// NNG error
    #[error("NNG error: {0}")]
    Nng(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Channel send error
    #[error("Channel error: {0}")]
    Channel(String),

    /// Shutdown requested
    #[error("Shutdown requested")]
    Shutdown,
}

impl From<nng::Error> for DaemonError {
    fn from(e: nng::Error) -> Self {
        DaemonError::Nng(e.to_string())
    }
}
