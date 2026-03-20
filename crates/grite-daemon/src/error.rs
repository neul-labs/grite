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

    /// Core grit error
    #[error("Grit error: {0}")]
    Grit(#[from] libgrite_core::GriteError),

    /// Git error
    #[error("Git error: {0}")]
    Git(#[from] libgrite_git::GitError),

    /// IPC error
    #[error("IPC error: {0}")]
    Ipc(#[from] libgrite_ipc::IpcError),

    /// NNG error
    #[error("NNG error: {0}")]
    Nng(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

}

impl From<nng::Error> for DaemonError {
    fn from(e: nng::Error) -> Self {
        DaemonError::Nng(e.to_string())
    }
}
