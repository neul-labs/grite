use crate::context::GriteContext;
use crate::types::*;
use libgrite_core::GriteError;

/// Export issues to JSON or Markdown.
pub fn export(ctx: &GriteContext, opts: &ExportOptions) -> Result<ExportResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("export not yet implemented in library")
}
