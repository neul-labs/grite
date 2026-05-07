use crate::context::GriteContext;
use crate::types::*;
use libgrite_core::GriteError;

/// Acquire a lock.
pub fn lock_acquire(
    ctx: &GriteContext,
    opts: &LockAcquireOptions,
) -> Result<LockAcquireResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("lock_acquire not yet implemented in library")
}

/// Release a lock.
pub fn lock_release(ctx: &GriteContext, opts: &LockReleaseOptions) -> Result<(), GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("lock_release not yet implemented in library")
}

/// Renew a lock.
pub fn lock_renew(
    ctx: &GriteContext,
    opts: &LockRenewOptions,
) -> Result<LockRenewResult, GriteError> {
    let _ = ctx;
    let _ = opts;
    todo!("lock_renew not yet implemented in library")
}

/// Show lock status.
pub fn lock_status(ctx: &GriteContext) -> Result<LockStatusResult, GriteError> {
    let _ = ctx;
    todo!("lock_status not yet implemented in library")
}

/// Garbage-collect locks.
pub fn lock_gc(ctx: &GriteContext) -> Result<(), GriteError> {
    let _ = ctx;
    todo!("lock_gc not yet implemented in library")
}
