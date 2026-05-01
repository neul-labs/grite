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

pub mod types;
pub mod hash;
pub mod projection;
pub mod store;
pub mod config;
pub mod export;
pub mod error;
pub mod lock;
pub mod signing;
pub mod integrity;
pub mod context;

pub use error::GriteError;
pub use types::{ActorId, EventId, IssueId};
pub use types::event::{Event, EventKind, IssueState, DependencyType, SymbolInfo};
pub use types::issue::{IssueProjection, IssueSummary, Version};
pub use types::actor::ActorConfig;
pub use types::ids::{generate_actor_id, generate_issue_id, id_to_hex, hex_to_id};
pub use types::context::{FileContext, ProjectContextEntry, ProjectContext};
pub use store::{GriteStore, LockedStore, IssueFilter, DbStats, RebuildStats};
pub use config::{RepoConfig, load_repo_config, save_repo_config, load_signing_key, repo_sled_path, actor_dir, list_actors};
pub use export::{export_json, export_markdown, ExportSince};
pub use integrity::{verify_event_hash, check_store_integrity, verify_store_signatures, IntegrityReport, CorruptEvent, CorruptionKind, SignatureError};
pub use lock::{Lock, LockPolicy, LockCheckResult, LockStatus, resource_hash, DEFAULT_LOCK_TTL_MS};
pub use signing::{SigningKeyPair, VerificationPolicy, SigningError, verify_signature};
