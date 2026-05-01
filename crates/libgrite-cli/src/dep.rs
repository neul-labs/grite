use crate::context::GriteContext;
use crate::types::*;
use libgrite_core::GriteError;

/// Add a dependency.
pub fn dep_add(ctx: &GriteContext, opts: &DepAddOptions) -> Result<DepAddResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("dep_add not yet implemented in library")
}

/// Remove a dependency.
pub fn dep_remove(ctx: &GriteContext, opts: &DepRemoveOptions) -> Result<DepRemoveResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("dep_remove not yet implemented in library")
}

/// List dependencies.
pub fn dep_list(ctx: &GriteContext, opts: &DepListOptions) -> Result<DepListResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("dep_list not yet implemented in library")
}

/// Topological sort.
pub fn dep_topo(ctx: &GriteContext, opts: &DepTopoOptions) -> Result<DepTopoResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("dep_topo not yet implemented in library")
}
