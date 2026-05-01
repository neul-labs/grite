use crate::context::GriteContext;
use crate::types::*;
use libgrite_core::GriteError;

/// Rebuild local database from events.
pub fn rebuild(ctx: &GriteContext, opts: &RebuildOptions) -> Result<RebuildResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("rebuild not yet implemented in library")
}
