//! Sync command implementation

use libgrit_core::{GritError, lock::LockCheckResult};
use libgrit_core::types::ids::ActorId;
use serde::Serialize;
use crate::cli::Cli;
use crate::context::GritContext;
use crate::output::{output_success, print_human};

/// Check repo lock for push operations
fn check_push_lock(cli: &Cli, ctx: &GritContext) -> Result<(), GritError> {
    match ctx.check_lock("repo:global")? {
        LockCheckResult::Clear => Ok(()),
        LockCheckResult::Warning(conflicts) => {
            if !cli.quiet {
                for lock in &conflicts {
                    eprintln!(
                        "Warning: {} is locked by {} (expires in {}s)",
                        lock.resource,
                        lock.owner,
                        lock.time_remaining_ms() / 1000
                    );
                }
            }
            Ok(())
        }
        LockCheckResult::Blocked(_) => unreachable!(),
    }
}

#[derive(Serialize)]
struct SyncOutput {
    pulled: bool,
    pushed: bool,
    pull_events: usize,
    pull_wal_head: Option<String>,
    push_success: bool,
    push_rebased: bool,
    push_events_rebased: usize,
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
    events_rebased: usize,
    message: String,
}

pub fn run(cli: &Cli, remote: String, pull_only: bool, push_only: bool) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let sync_mgr = ctx.open_sync().map_err(|e| GritError::Internal(e.to_string()))?;

    // Parse actor_id for push operations that may need rebase
    let actor_id: ActorId = hex::decode(&ctx.actor_id)
        .map_err(|e| GritError::Internal(format!("Invalid actor ID: {}", e)))?
        .try_into()
        .map_err(|_| GritError::Internal("Actor ID must be 16 bytes".to_string()))?;

    // If neither flag is set, do both pull and push
    let do_pull = !push_only;
    let do_push = !pull_only;

    // Check locks for push operations
    if do_push {
        check_push_lock(cli, &ctx)?;
    }

    if do_pull && !do_push {
        // Pull only
        let result = sync_mgr.pull(&remote).map_err(|e| GritError::Internal(e.to_string()))?;

        // Human-readable output
        if result.events_pulled > 0 {
            print_human(cli, &format!("Pulled {} events from {}", result.events_pulled, remote));
        } else {
            print_human(cli, &format!("Already up to date with {}", remote));
        }

        output_success(cli, PullOutput {
            success: result.success,
            events: result.events_pulled,
            wal_head: result.new_wal_head.map(|oid| oid.to_string()),
            message: result.message,
        });
    } else if do_push && !do_pull {
        // Push only with auto-rebase on conflict
        let result = sync_mgr.push_with_rebase(&remote, &actor_id)
            .map_err(|e| GritError::Internal(e.to_string()))?;

        // Human-readable output with conflict reporting
        if result.rebased {
            print_human(cli, &format!(
                "Conflict resolved: rebased {} local events on top of remote",
                result.events_rebased
            ));
        }
        if result.success {
            print_human(cli, &format!("Pushed to {}", remote));
        } else {
            print_human(cli, &format!("Push failed: {}", result.message));
        }

        output_success(cli, PushOutput {
            success: result.success,
            rebased: result.rebased,
            events_rebased: result.events_rebased,
            message: result.message,
        });
    } else {
        // Full sync: pull then push with auto-rebase
        let (pull_result, push_result) = sync_mgr.sync_with_rebase(&remote, &actor_id)
            .map_err(|e| GritError::Internal(e.to_string()))?;

        // Human-readable output with conflict reporting
        if pull_result.events_pulled > 0 {
            print_human(cli, &format!("Pulled {} events from {}", pull_result.events_pulled, remote));
        }

        if push_result.rebased {
            print_human(cli, &format!(
                "Conflict resolved: rebased {} local events on top of remote",
                push_result.events_rebased
            ));
        }

        if push_result.success {
            print_human(cli, &format!("Pushed to {}", remote));
        } else {
            print_human(cli, &format!("Push failed: {}", push_result.message));
        }

        output_success(cli, SyncOutput {
            pulled: true,
            pushed: true,
            pull_events: pull_result.events_pulled,
            pull_wal_head: pull_result.new_wal_head.map(|oid| oid.to_string()),
            push_success: push_result.success,
            push_rebased: push_result.rebased,
            push_events_rebased: push_result.events_rebased,
            message: format!("{} / {}", pull_result.message, push_result.message),
        });
    }

    Ok(())
}
