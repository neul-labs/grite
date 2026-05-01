use crate::types::*;
use libgrite_core::GriteError;

/// Start the daemon.
pub fn daemon_start(opts: &DaemonStartOptions) -> Result<DaemonStartResult, GriteError> {
    let _ = opts;
    todo!("daemon_start not yet implemented in library")
}

/// Show daemon status.
pub fn daemon_status() -> Result<DaemonStatusResult, GriteError> {
    todo!("daemon_status not yet implemented in library")
}

/// Stop the daemon.
pub fn daemon_stop() -> Result<(), GriteError> {
    todo!("daemon_stop not yet implemented in library")
}
