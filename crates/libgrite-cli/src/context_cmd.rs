use crate::context::GriteContext;
use crate::types::*;
use libgrite_core::GriteError;

/// Index files in the repository.
pub fn context_index(ctx: &GriteContext, opts: &ContextIndexOptions) -> Result<ContextIndexResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("context_index not yet implemented in library")
}

/// Query symbols.
pub fn context_query(ctx: &GriteContext, opts: &ContextQueryOptions) -> Result<ContextQueryResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("context_query not yet implemented in library")
}

/// Show context for a file.
pub fn context_show(ctx: &GriteContext, opts: &ContextShowOptions) -> Result<ContextShowResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("context_show not yet implemented in library")
}

/// Show project context.
pub fn context_project(ctx: &GriteContext, opts: &ContextProjectOptions) -> Result<ContextProjectResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("context_project not yet implemented in library")
}

/// Set project context.
pub fn context_set(ctx: &GriteContext, opts: &ContextSetOptions) -> Result<ContextSetResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("context_set not yet implemented in library")
}
