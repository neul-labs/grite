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

    /// Core grite error
    #[error("grite error: {0}")]
    Core(#[from] libgrite_core::GriteError),

    /// Git error
    #[error("Git error: {0}")]
    Git(#[from] libgrite_git::GitError),

    /// IPC error
    #[error("IPC error: {0}")]
    Ipc(#[from] libgrite_ipc::IpcError),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
