//! IPC error types

use thiserror::Error;

/// Errors that can occur during IPC operations
#[derive(Error, Debug)]
pub enum IpcError {
    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Request timed out
    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Protocol version mismatch
    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u32, actual: u32 },

    /// Daemon not running
    #[error("Daemon not running")]
    DaemonNotRunning,

    /// Daemon returned an error
    #[error("Daemon error [{code}]: {message}")]
    DaemonError { code: String, message: String },

    /// Lock file error
    #[error("Lock file error: {0}")]
    LockFile(String),

    /// Lock is held by another process
    #[error("Lock held by process {pid} (expires in {expires_in_ms}ms)")]
    LockHeld { pid: u32, expires_in_ms: u64 },

    /// Lock has expired
    #[error("Lock expired")]
    LockExpired,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// NNG error
    #[error("NNG error: {0}")]
    Nng(String),
}

impl From<nng::Error> for IpcError {
    fn from(e: nng::Error) -> Self {
        IpcError::Nng(e.to_string())
    }
}

/// Error codes matching docs/cli-json.md
pub mod codes {
    pub const DB_BUSY: &str = "db_busy";
    pub const NOT_FOUND: &str = "not_found";
    pub const INVALID_INPUT: &str = "invalid_input";
    pub const INTERNAL: &str = "internal";
    pub const NOT_INITIALIZED: &str = "not_initialized";
    pub const IO_ERROR: &str = "io_error";
    pub const GIT_ERROR: &str = "git_error";
    pub const IPC_ERROR: &str = "ipc_error";
}
