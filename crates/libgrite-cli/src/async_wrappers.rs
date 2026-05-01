//! Async wrappers for libgrite-cli operations.
//!
//! These functions run the synchronous operations on tokio's blocking thread pool
//! using `spawn_blocking`. Enable the `async` feature to use this module.
//!
//! ```rust,no_run
//! use libgrite_cli::{GriteContext, types::*};
//! use libgrite_cli::async_wrappers::*;
//!
//! async fn example(ctx: &GriteContext) {
//!     let issues = issue_list_async(ctx, IssueListOptions::default()).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?;
//! }
//! ```

use crate::context::GriteContext;
use crate::types::*;
use libgrite_core::GriteError;

macro_rules! async_wrapper {
    ($name:ident, $sync_fn:path, ($ctx:ident, $opts:ident)) => {
        pub async fn $name(
            $ctx: &GriteContext,
            $opts: $opts,
        ) -> Result<$ret, GriteError>
        where
            $opts: Send + 'static,
            $ret: Send + 'static,
        {
            let ctx = $ctx.clone();
            tokio::task::spawn_blocking(move || $sync_fn(&ctx, $opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
        }
    };
}

/// Async: create a new issue.
pub async fn issue_create_async(
    ctx: &GriteContext,
    opts: IssueCreateOptions,
) -> Result<IssueCreateResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::issue::issue_create(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: list issues.
pub async fn issue_list_async(
    ctx: &GriteContext,
    opts: IssueListOptions,
) -> Result<IssueListResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::issue::issue_list(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: show issue details.
pub async fn issue_show_async(
    ctx: &GriteContext,
    opts: IssueShowOptions,
) -> Result<IssueShowResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::issue::issue_show(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: update an issue.
pub async fn issue_update_async(
    ctx: &GriteContext,
    opts: IssueUpdateOptions,
) -> Result<IssueUpdateResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::issue::issue_update(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: add a comment.
pub async fn issue_comment_async(
    ctx: &GriteContext,
    opts: IssueCommentOptions,
) -> Result<IssueCommentResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::issue::issue_comment(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: close an issue.
pub async fn issue_close_async(
    ctx: &GriteContext,
    opts: IssueStateOptions,
) -> Result<IssueStateResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::issue::issue_close(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: reopen an issue.
pub async fn issue_reopen_async(
    ctx: &GriteContext,
    opts: IssueStateOptions,
) -> Result<IssueStateResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::issue::issue_reopen(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: sync with remote.
pub async fn sync_async(
    ctx: &GriteContext,
    opts: SyncOptions,
) -> Result<SyncResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::sync::sync(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: export issues.
pub async fn export_async(
    ctx: &GriteContext,
    opts: ExportOptions,
) -> Result<ExportResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::export::export(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: rebuild database.
pub async fn rebuild_async(
    ctx: &GriteContext,
    opts: RebuildOptions,
) -> Result<RebuildResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::rebuild::rebuild(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: index context.
pub async fn context_index_async(
    ctx: &GriteContext,
    opts: ContextIndexOptions,
) -> Result<ContextIndexResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::context_cmd::context_index(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: check database.
pub async fn db_check_async(
    ctx: &GriteContext,
    opts: DbCheckOptions,
) -> Result<DbCheckResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::db::db_check(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}

/// Async: run doctor.
pub async fn doctor_async(
    ctx: &GriteContext,
    opts: DoctorOptions,
) -> Result<DoctorResult, GriteError> {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || crate::doctor::doctor(&ctx, &opts)).await.map_err(|e| GriteError::Internal(format!("task join error: {}", e)))?
}
