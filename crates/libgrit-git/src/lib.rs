//! Git-backed WAL and sync operations for Grit
//!
//! This crate provides Git integration for Grit's event system:
//! - CBOR chunk encoding/decoding for portable event storage
//! - WAL (Write-Ahead Log) operations via `refs/grit/wal`
//! - Snapshot management via `refs/grit/snapshots/<ts>`
//! - Push/pull sync operations with conflict handling

mod error;
mod chunk;
mod wal;
mod snapshot;
mod sync;

pub use error::GitError;
pub use chunk::{encode_chunk, decode_chunk, chunk_hash, CHUNK_MAGIC, CHUNK_VERSION, CHUNK_CODEC};
pub use wal::{WalManager, WalCommit};
pub use snapshot::{SnapshotManager, SnapshotRef, SnapshotMeta};
pub use sync::{SyncManager, PullResult, PushResult};
