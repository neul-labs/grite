//! Git-backed WAL and sync operations for Grite
//!
//! This crate provides Git integration for Grite's event system:
//! - CBOR chunk encoding/decoding for portable event storage
//! - WAL (Write-Ahead Log) operations via `refs/grite/wal`
//! - Snapshot management via `refs/grite/snapshots/<ts>`
//! - Push/pull sync operations with conflict handling

mod chunk;
mod error;
mod lock_manager;
mod snapshot;
mod sync;
mod wal;

pub use chunk::{chunk_hash, decode_chunk, encode_chunk, CHUNK_CODEC, CHUNK_MAGIC, CHUNK_VERSION};
pub use error::GitError;
pub use lock_manager::{LockGcStats, LockManager};
pub use snapshot::{SnapshotManager, SnapshotMeta, SnapshotRef};
pub use sync::{PullResult, PushResult, SyncManager};
pub use wal::{WalCommit, WalManager};
