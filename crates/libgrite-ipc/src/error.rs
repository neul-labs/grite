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

    /// Lost lock acquisition race (another process acquired it first)
    #[error("Lock acquisition race lost: another process acquired the lock simultaneously")]
    LockRace,

    /// Lock has expired
    #[error("Lock expired")]
    LockExpired,

    /// Client is poisoned (stream has stale data from a previous failed exchange)
    #[error("Client connection is poisoned after a previous error; reconnect to continue")]
    ClientPoisoned,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Bridge IpcError into GriteError, preserving semantic variants.
impl From<IpcError> for libgrite_core::GriteError {
    fn from(e: IpcError) -> Self {
        match e {
            IpcError::DaemonNotRunning => {
                libgrite_core::GriteError::Ipc("Daemon not running".to_string())
            }
            IpcError::Timeout(_) => libgrite_core::GriteError::Ipc("IPC timeout".to_string()),
            IpcError::LockHeld { pid, expires_in_ms } => {
                libgrite_core::GriteError::DbBusy(format!(
                    "Daemon lock held by process {} (expires in {}s)",
                    pid,
                    expires_in_ms / 1000
                ))
            }
            IpcError::LockRace => libgrite_core::GriteError::Conflict(
                "Another process acquired the daemon lock simultaneously".to_string(),
            ),
            IpcError::LockExpired => {
                libgrite_core::GriteError::Ipc("Daemon lock expired".to_string())
            }
            other => libgrite_core::GriteError::Ipc(other.to_string()),
        }
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
