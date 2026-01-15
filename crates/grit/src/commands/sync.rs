//! Sync command implementation

use libgrit_core::GritError;
use serde::Serialize;
use crate::cli::Cli;
use crate::context::GritContext;
use crate::output::output_success;

#[derive(Serialize)]
struct SyncOutput {
    pulled: bool,
    pushed: bool,
    pull_events: usize,
    pull_wal_head: Option<String>,
    push_success: bool,
    push_rebased: bool,
    message: String,
}

#[derive(Serialize)]
struct PullOutput {
    success: bool,
    events: usize,
    wal_head: Option<String>,
    message: String,
}

#[derive(Serialize)]
struct PushOutput {
    success: bool,
    rebased: bool,
    message: String,
}

pub fn run(cli: &Cli, remote: String, pull_only: bool, push_only: bool) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let sync_mgr = ctx.open_sync().map_err(|e| GritError::Internal(e.to_string()))?;

    // If neither flag is set, do both pull and push
    let do_pull = !push_only;
    let do_push = !pull_only;

    if do_pull && !do_push {
        // Pull only
        let result = sync_mgr.pull(&remote).map_err(|e| GritError::Internal(e.to_string()))?;
        output_success(cli, PullOutput {
            success: result.success,
            events: result.events_pulled,
            wal_head: result.new_wal_head.map(|oid| oid.to_string()),
            message: result.message,
        });
    } else if do_push && !do_pull {
        // Push only
        let result = sync_mgr.push(&remote).map_err(|e| GritError::Internal(e.to_string()))?;
        output_success(cli, PushOutput {
            success: result.success,
            rebased: result.rebased,
            message: result.message,
        });
    } else {
        // Full sync: pull then push
        let (pull_result, push_result) = sync_mgr.sync(&remote).map_err(|e| GritError::Internal(e.to_string()))?;
        output_success(cli, SyncOutput {
            pulled: true,
            pushed: true,
            pull_events: pull_result.events_pulled,
            pull_wal_head: pull_result.new_wal_head.map(|oid| oid.to_string()),
            push_success: push_result.success,
            push_rebased: push_result.rebased,
            message: format!("{} / {}", pull_result.message, push_result.message),
        });
    }

    Ok(())
}
