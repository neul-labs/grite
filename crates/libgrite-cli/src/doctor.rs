use crate::context::GriteContext;
use crate::types::*;
use libgrite_core::GriteError;

/// Run health checks.
pub fn doctor(ctx: &GriteContext, opts: &DoctorOptions) -> Result<DoctorResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("doctor not yet implemented in library")
}
