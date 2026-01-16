//! Lock management commands

use libgrit_core::GritError;
use libgrit_git::LockManager;
use serde::Serialize;

use crate::cli::{Cli, LockCommand};
use crate::context::GritContext;
use crate::output::output_success;

#[derive(Serialize)]
struct LockAcquireOutput {
    resource: String,
    owner: String,
    nonce: String,
    expires_unix_ms: u64,
    ttl_seconds: u64,
}

#[derive(Serialize)]
struct LockReleaseOutput {
    resource: String,
    released: bool,
}

#[derive(Serialize)]
struct LockRenewOutput {
    resource: String,
    owner: String,
    expires_unix_ms: u64,
    ttl_seconds: u64,
}

#[derive(Serialize)]
struct LockStatusOutput {
    locks: Vec<LockInfo>,
    total: usize,
}

#[derive(Serialize)]
struct LockInfo {
    resource: String,
    owner: String,
    expires_unix_ms: u64,
    time_remaining_seconds: u64,
    expired: bool,
}

#[derive(Serialize)]
struct LockGcOutput {
    removed: usize,
    kept: usize,
}

pub fn run(cli: &Cli, cmd: LockCommand) -> Result<(), GritError> {
    match cmd {
        LockCommand::Acquire { resource, ttl } => run_acquire(cli, resource, ttl),
        LockCommand::Release { resource } => run_release(cli, resource),
        LockCommand::Renew { resource, ttl } => run_renew(cli, resource, ttl),
        LockCommand::Status => run_status(cli),
        LockCommand::Gc => run_gc(cli),
    }
}

fn run_acquire(cli: &Cli, resource: String, ttl_seconds: u64) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let git_dir = ctx.repo_root().join(".git");
    let manager = LockManager::open(&git_dir)
        .map_err(|e| GritError::Internal(e.to_string()))?;

    let ttl_ms = ttl_seconds * 1000;
    let lock = manager.acquire(&resource, &ctx.actor_id, Some(ttl_ms))
        .map_err(|e| match e {
            libgrit_git::GitError::LockConflict { resource, owner, expires_in_ms } => {
                GritError::Conflict(format!(
                    "Lock on {} is held by {} (expires in {}s)",
                    resource, owner, expires_in_ms / 1000
                ))
            }
            _ => GritError::Internal(e.to_string()),
        })?;

    output_success(cli, LockAcquireOutput {
        resource: lock.resource,
        owner: lock.owner,
        nonce: lock.nonce,
        expires_unix_ms: lock.expires_unix_ms,
        ttl_seconds,
    });

    Ok(())
}

fn run_release(cli: &Cli, resource: String) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let git_dir = ctx.repo_root().join(".git");
    let manager = LockManager::open(&git_dir)
        .map_err(|e| GritError::Internal(e.to_string()))?;

    manager.release(&resource, &ctx.actor_id)
        .map_err(|e| match e {
            libgrit_git::GitError::LockNotOwned { resource, owner } => {
                GritError::Conflict(format!(
                    "Cannot release lock on {} - owned by {}",
                    resource, owner
                ))
            }
            _ => GritError::Internal(e.to_string()),
        })?;

    output_success(cli, LockReleaseOutput {
        resource,
        released: true,
    });

    Ok(())
}

fn run_renew(cli: &Cli, resource: String, ttl_seconds: u64) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let git_dir = ctx.repo_root().join(".git");
    let manager = LockManager::open(&git_dir)
        .map_err(|e| GritError::Internal(e.to_string()))?;

    let ttl_ms = ttl_seconds * 1000;
    let lock = manager.renew(&resource, &ctx.actor_id, Some(ttl_ms))
        .map_err(|e| match e {
            libgrit_git::GitError::LockNotOwned { resource, owner } => {
                GritError::Conflict(format!(
                    "Cannot renew lock on {} - owned by {}",
                    resource, owner
                ))
            }
            _ => GritError::Internal(e.to_string()),
        })?;

    output_success(cli, LockRenewOutput {
        resource: lock.resource,
        owner: lock.owner,
        expires_unix_ms: lock.expires_unix_ms,
        ttl_seconds,
    });

    Ok(())
}

fn run_status(cli: &Cli) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let git_dir = ctx.repo_root().join(".git");
    let manager = LockManager::open(&git_dir)
        .map_err(|e| GritError::Internal(e.to_string()))?;

    let locks = manager.list_locks()
        .map_err(|e| GritError::Internal(e.to_string()))?;

    let lock_infos: Vec<LockInfo> = locks.iter().map(|lock| {
        LockInfo {
            resource: lock.resource.clone(),
            owner: lock.owner.clone(),
            expires_unix_ms: lock.expires_unix_ms,
            time_remaining_seconds: lock.time_remaining_ms() / 1000,
            expired: lock.is_expired(),
        }
    }).collect();

    let total = lock_infos.len();

    output_success(cli, LockStatusOutput {
        locks: lock_infos,
        total,
    });

    Ok(())
}

fn run_gc(cli: &Cli) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let git_dir = ctx.repo_root().join(".git");
    let manager = LockManager::open(&git_dir)
        .map_err(|e| GritError::Internal(e.to_string()))?;

    let stats = manager.gc()
        .map_err(|e| GritError::Internal(e.to_string()))?;

    output_success(cli, LockGcOutput {
        removed: stats.removed,
        kept: stats.kept,
    });

    Ok(())
}
