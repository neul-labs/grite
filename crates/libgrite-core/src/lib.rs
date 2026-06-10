//! Core library for Grite: event types, CRDT projections, hashing, and sled storage.
//!
//! This crate defines the data model and persistence layer used by all other Grite
//! crates. It is pure Rust with no async runtime dependency.
//!
//! ## Data model
//!
//! - [`Event`] — the atomic unit of change
//! - [`EventKind`] — what happened (issue created, label added, etc.)
//! - [`IssueProjection`] — materialized view of an issue
//!
//! ## Storage
//!
//! - [`GriteStore`] — sled-backed key-value store with CRUD operations
//! - [`LockedStore`] — process-safe wrapper using `flock`
//!
//! ## CRDT semantics
//!
//! Projections use deterministic merge rules:
//! - **LWW** for title, body, state
//! - **Commutative sets** for labels, assignees, dependencies
//! - **Append-only** for comments, links, attachments

pub mod config;
#[cfg(feature = "context")]
pub mod context;
pub mod error;
pub mod export;
pub mod hash;
pub mod integrity;
pub mod lock;
pub mod projection;
pub mod signing;
pub mod store;
pub mod types;

pub use config::{
    actor_dir, list_actors, load_repo_config, load_signing_key, repo_sled_path, save_repo_config,
    RepoConfig,
};
pub use error::GriteError;
pub use export::{export_json, export_markdown, ExportSince};
pub use integrity::{
    check_store_integrity, verify_event_hash, verify_store_signatures, CorruptEvent,
    CorruptionKind, IntegrityReport, SignatureError,
};
pub use lock::{resource_hash, Lock, LockCheckResult, LockPolicy, LockStatus, DEFAULT_LOCK_TTL_MS};
pub use signing::{verify_signature, SigningError, SigningKeyPair, VerificationPolicy};
pub use store::{DbStats, GriteStore, IssueFilter, LockedStore, RebuildStats};
pub use types::actor::ActorConfig;
#[cfg(feature = "context")]
pub use types::context::{FileContext, ProjectContext, ProjectContextEntry};
#[cfg(feature = "context")]
pub use types::event::SymbolInfo;
pub use types::event::{DependencyType, Event, EventKind, IssueState};
pub use types::ids::{generate_actor_id, generate_issue_id, hex_to_id, id_to_hex};
pub use types::issue::{IssueProjection, IssueSummary, Version};
pub use types::{ActorId, EventId, IssueId};
