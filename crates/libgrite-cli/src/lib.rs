//! Programmatic CLI library for grite
//!
//! This crate exposes the command logic from the `grite` CLI as a reusable
//! Rust library. It can be used by other Rust programs (e.g., multi-agent
//! harnesses) to drive grite natively without shelling out to subprocesses.
//!
//! Each command module provides both synchronous and (with the `async` feature)
//! asynchronous variants for heavy I/O operations.

pub mod actor;
pub mod context;
#[cfg(feature = "context")]
pub mod context_cmd;
pub mod daemon;
pub mod db;
pub mod dep;
pub mod doctor;
pub mod event_helper;
pub mod export;
pub mod init;
pub mod issue;
pub mod lock;
pub mod rebuild;
pub mod snapshot;
pub mod sync;

#[cfg(feature = "async")]
pub mod async_wrappers;

/// Shared option and result types for all commands.
pub mod types;

/// AGENTS.md template content.
pub mod agents_md;

pub use context::GriteContext;
pub use types::*;

/// Re-export core error type for convenience.
pub use libgrite_core::GriteError;
