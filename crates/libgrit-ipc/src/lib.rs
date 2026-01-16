//! IPC types and client for grit daemon communication
//!
//! This crate provides:
//! - Message types for daemon communication (IpcRequest, IpcResponse, IpcCommand)
//! - Notification types for pub/sub (EventApplied, WalSynced, etc.)
//! - Daemon lock management (DaemonLock)
//! - Discovery protocol types
//! - IPC client for connecting to the daemon

pub mod client;
pub mod discovery;
pub mod error;
pub mod lock;
pub mod messages;
pub mod notifications;

pub use client::IpcClient;
pub use discovery::{DiscoverRequest, DiscoverResponse, WorkerInfo};
pub use error::IpcError;
pub use lock::DaemonLock;
pub use messages::{IpcCommand, IpcRequest, IpcResponse, IpcErrorPayload};
pub use notifications::Notification;

/// Current IPC schema version
pub const IPC_SCHEMA_VERSION: u32 = 1;

/// Protocol identifier for discovery
pub const PROTOCOL_NAME: &str = "grit-ipc";

/// Default request timeout in milliseconds
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;

/// Default lease duration for daemon locks in milliseconds
pub const DEFAULT_LEASE_MS: u64 = 30_000;
