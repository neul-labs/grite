use crate::context::GriteContext;
use crate::types::*;
use libgrite_core::GriteError;

/// Sync with remote repository.
pub fn sync(ctx: &GriteContext, opts: &SyncOptions) -> Result<SyncResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("sync not yet implemented in library")
}
