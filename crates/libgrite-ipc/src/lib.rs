//! IPC types and client for grite daemon communication
//!
//! This crate provides:
//! - Message types for daemon communication (IpcRequest, IpcResponse, IpcCommand)
//! - Notification types for pub/sub (EventApplied, WalSynced, etc.)
//! - Daemon lock management (DaemonLock)
//! - IPC client for connecting to the daemon

pub mod client;
pub mod error;
pub mod framing;
pub mod lock;
pub mod messages;
pub mod notifications;

pub use client::IpcClient;
pub use error::IpcError;
pub use lock::DaemonLock;
pub use messages::{IpcCommand, IpcErrorPayload, IpcRequest, IpcResponse};
pub use notifications::Notification;

/// Current IPC schema version
pub const IPC_SCHEMA_VERSION: u32 = 1;

/// Default request timeout in milliseconds
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;

/// Default lease duration for daemon locks in milliseconds
pub const DEFAULT_LEASE_MS: u64 = 30_000;

/// Issue action types returned in daemon responses
pub mod issue_action {
    pub const CREATED: &str = "created";
    pub const CLOSED: &str = "closed";
    pub const REOPENED: &str = "reopened";
}

/// Get the default Unix socket path for the daemon.
///
/// Uses user-specific path for security isolation:
/// - `XDG_RUNTIME_DIR` if available (Linux with systemd)
/// - `/tmp/grite-daemon-<uid>.sock` as fallback on Unix
/// - `/tmp/grite-daemon.sock` on non-Unix platforms
pub fn default_socket_path() -> String {
    // Prefer XDG_RUNTIME_DIR which is properly secured by systemd
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return format!("{}/grite-daemon.sock", runtime_dir);
    }

    // Fallback: user-specific path in /tmp
    #[cfg(unix)]
    {
        let uid = unsafe { libc::getuid() };
        format!("/tmp/grite-daemon-{}.sock", uid)
    }

    #[cfg(not(unix))]
    {
        "/tmp/grite-daemon.sock".to_string()
    }
}
