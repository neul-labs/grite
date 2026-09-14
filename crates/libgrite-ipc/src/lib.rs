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
pub mod host;
pub mod lock;
pub mod messages;
pub mod notifications;
pub mod probe;

pub use client::IpcClient;
pub use error::IpcError;
pub use host::{host_id, process_alive};
pub use lock::DaemonLock;
pub use messages::{IpcCommand, IpcErrorPayload, IpcRequest, IpcResponse};
pub use notifications::Notification;
pub use probe::{
    is_listening, probe, probe_daemon, probe_daemon_with_timeout, probe_with_timeout, DaemonInfo,
    ProbeOutcome,
};

/// Current IPC schema version.
///
/// Bumped to 2 when `IpcRequest` gained `git_dir`. The wire format is rkyv,
/// so a mismatched peer cannot be parsed safely — the supervisor rejects
/// foreign versions before dispatch, and the CLI surfaces that as a daemon
/// that needs restarting rather than as an opaque failure.
pub const IPC_SCHEMA_VERSION: u32 = 2;

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

/// Environment variable that overrides the daemon socket path.
///
/// Set this to run an isolated daemon (tests, sandboxes, per-checkout
/// daemons). Every grite process that shares a repository must agree on
/// the value, otherwise they will not find each other's daemon.
pub const SOCKET_ENV: &str = "GRITE_DAEMON_SOCKET";

/// Environment variable that, when set to a truthy value, turns a failure to
/// reach or start the daemon into a hard error instead of a silent fallback
/// to single-process execution.
pub const REQUIRE_DAEMON_ENV: &str = "GRITE_REQUIRE_DAEMON";

/// Whether the caller has demanded that commands go through the daemon.
///
/// Concurrent agents set this so that a daemon failure surfaces as an error
/// rather than degrading into direct sled access, which serialises badly and
/// reports `db_busy`.
pub fn require_daemon() -> bool {
    matches!(
        std::env::var(REQUIRE_DAEMON_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// Get the default Unix socket path for the daemon.
///
/// Uses user-specific path for security isolation:
/// - `GRITE_DAEMON_SOCKET` if set (explicit override)
/// - `XDG_RUNTIME_DIR` if available (Linux with systemd)
/// - `/tmp/grite-daemon-<uid>.sock` as fallback on Unix
/// - `/tmp/grite-daemon.sock` on non-Unix platforms
pub fn default_socket_path() -> String {
    // Explicit override wins so tests and sandboxes can isolate a daemon
    if let Ok(path) = std::env::var(SOCKET_ENV) {
        if !path.is_empty() {
            return path;
        }
    }

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
