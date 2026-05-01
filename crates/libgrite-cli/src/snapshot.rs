use crate::context::GriteContext;
use crate::types::*;
use libgrite_core::GriteError;

/// Create a snapshot.
pub fn snapshot_create(ctx: &GriteContext, _opts: &SnapshotCreateOptions) -> Result<SnapshotCreateResult, GriteError> {
    let _ = ctx;
    todo!("snapshot_create not yet implemented in library")
}

/// List snapshots.
pub fn snapshot_list(ctx: &GriteContext, _opts: &SnapshotListOptions) -> Result<SnapshotListResult, GriteError> {
    let _ = ctx;
    todo!("snapshot_list not yet implemented in library")
}

/// Garbage-collect snapshots.
pub fn snapshot_gc(ctx: &GriteContext, _opts: &SnapshotGcOptions) -> Result<SnapshotGcResult, GriteError> {
    let _ = ctx;
    todo!("snapshot_gc not yet implemented in library")
}
