use libgrite_core::{
    hash::compute_event_id,
    lock::LockCheckResult,
    store::IssueFilter,
    types::event::{Event, EventKind, IssueState},
    types::ids::{generate_issue_id, id_to_hex},
    GriteError,
};

use crate::context::GriteContext;
use crate::event_helper::insert_and_append;
use crate::types::*;

fn current_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// RAII guard for auto-releasing locks
struct LockGuard<'a> {
    ctx: &'a GriteContext,
    resource: String,
    acquired: bool,
}

impl<'a> LockGuard<'a> {
    fn acquire(
        ctx: &'a GriteContext,
        issue_id_hex: &str,
        should_lock: bool,
    ) -> Result<Self, GriteError> {
        let resource = format!("issue:{}", issue_id_hex);
        if should_lock {
            let lock_manager = ctx.open_lock_manager()?;
            lock_manager
                .acquire(&resource, &ctx.actor_id, None)
                .map_err(|e| match e {
                    libgrite_git::GitError::LockConflict {
                        resource,
                        owner,
                        expires_in_ms,
                    } => GriteError::Conflict(format!(
                        "Cannot acquire lock on {} - held by {} (expires in {}s)",
                        resource,
                        owner,
                        expires_in_ms / 1000
                    )),
                    _ => GriteError::Internal(e.to_string()),
                })?;
            Ok(Self {
                ctx,
                resource,
                acquired: true,
            })
        } else {
            Ok(Self {
                ctx,
                resource,
                acquired: false,
            })
        }
    }
}

impl<'a> Drop for LockGuard<'a> {
    fn drop(&mut self) {
        if self.acquired {
            if let Ok(lock_manager) = self.ctx.open_lock_manager() {
                let _ = lock_manager.release(&self.resource, &self.ctx.actor_id);
            }
        }
    }
}

/// Create a new issue.
pub fn issue_create(
    ctx: &GriteContext,
    opts: &IssueCreateOptions,
) -> Result<IssueCreateResult, GriteError> {
    // Check for repo-level locks before creating
    match ctx.check_lock("repo:global")? {
        LockCheckResult::Clear => {}
        LockCheckResult::Warning(_) => {}
        LockCheckResult::Blocked(_) => {
            return Err(GriteError::Conflict(
                "Repository is locked by another process".to_string(),
            ));
        }
    }

    let store = ctx.open_store()?;
    let wal = ctx.open_wal()?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = generate_issue_id();
    let ts = current_ts();
    let kind = EventKind::IssueCreated {
        title: opts.title.clone(),
        body: opts.body.clone(),
        labels: opts.labels.clone(),
    };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    insert_and_append(&store, &wal, &actor, &event)?;

    Ok(IssueCreateResult {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
    })
}

/// List issues.
pub fn issue_list(
    ctx: &GriteContext,
    opts: &IssueListOptions,
) -> Result<IssueListResult, GriteError> {
    let store = ctx.open_store()?;

    let state_filter = opts
        .state
        .as_ref()
        .map(|s| match s.to_lowercase().as_str() {
            "open" => IssueState::Open,
            "closed" => IssueState::Closed,
            _ => IssueState::Open,
        });

    let filter = IssueFilter {
        state: state_filter,
        label: opts.label.clone(),
    };

    let issues = store.list_issues(&filter)?;

    Ok(IssueListResult { issues })
}

/// Show issue details.
pub fn issue_show(
    ctx: &GriteContext,
    opts: &IssueShowOptions,
) -> Result<IssueShowResult, GriteError> {
    let store = ctx.open_store()?;

    let issue_id = store.resolve_issue_id(&opts.issue_id)?;
    let proj = store
        .get_issue(&issue_id)?
        .ok_or_else(|| GriteError::NotFound(format!("Issue {} not found", opts.issue_id)))?;

    let events = store.get_issue_events(&issue_id)?;

    Ok(IssueShowResult {
        issue: proj,
        events,
    })
}

/// Update an issue.
pub fn issue_update(
    ctx: &GriteContext,
    opts: &IssueUpdateOptions,
) -> Result<IssueUpdateResult, GriteError> {
    if opts.title.is_none() && opts.body.is_none() {
        return Err(GriteError::InvalidArgs(
            "Either --title or --body must be provided".to_string(),
        ));
    }

    let _guard = LockGuard::acquire(ctx, &opts.issue_id, opts.acquire_lock)?;

    let store = ctx.open_store()?;
    let wal = ctx.open_wal()?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = store.resolve_issue_id(&opts.issue_id)?;
    let _existing = store
        .get_issue(&issue_id)?
        .ok_or_else(|| GriteError::NotFound(format!("Issue {} not found", opts.issue_id)))?;

    let title = opts.title.clone();
    let body = opts.body.clone();

    let ts = current_ts();
    let kind = EventKind::IssueUpdated { title, body };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    insert_and_append(&store, &wal, &actor, &event)?;

    Ok(IssueUpdateResult {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
    })
}

/// Add a comment to an issue.
pub fn issue_comment(
    ctx: &GriteContext,
    opts: &IssueCommentOptions,
) -> Result<IssueCommentResult, GriteError> {
    let _guard = LockGuard::acquire(ctx, &opts.issue_id, opts.acquire_lock)?;

    let store = ctx.open_store()?;
    let wal = ctx.open_wal()?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = store.resolve_issue_id(&opts.issue_id)?;
    let _existing = store
        .get_issue(&issue_id)?
        .ok_or_else(|| GriteError::NotFound(format!("Issue {} not found", opts.issue_id)))?;

    let ts = current_ts();
    let kind = EventKind::CommentAdded {
        body: opts.body.clone(),
    };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    insert_and_append(&store, &wal, &actor, &event)?;

    Ok(IssueCommentResult {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
    })
}

/// Close an issue.
pub fn issue_close(
    ctx: &GriteContext,
    opts: &IssueStateOptions,
) -> Result<IssueStateResult, GriteError> {
    let _guard = LockGuard::acquire(ctx, &opts.issue_id, opts.acquire_lock)?;

    let store = ctx.open_store()?;
    let wal = ctx.open_wal()?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = store.resolve_issue_id(&opts.issue_id)?;
    let _existing = store
        .get_issue(&issue_id)?
        .ok_or_else(|| GriteError::NotFound(format!("Issue {} not found", opts.issue_id)))?;

    let ts = current_ts();
    let kind = EventKind::StateChanged {
        state: IssueState::Closed,
    };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    insert_and_append(&store, &wal, &actor, &event)?;

    Ok(IssueStateResult {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
        action: "closed".to_string(),
    })
}

/// Reopen an issue.
pub fn issue_reopen(
    ctx: &GriteContext,
    opts: &IssueStateOptions,
) -> Result<IssueStateResult, GriteError> {
    let _guard = LockGuard::acquire(ctx, &opts.issue_id, opts.acquire_lock)?;

    let store = ctx.open_store()?;
    let wal = ctx.open_wal()?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = store.resolve_issue_id(&opts.issue_id)?;
    let _existing = store
        .get_issue(&issue_id)?
        .ok_or_else(|| GriteError::NotFound(format!("Issue {} not found", opts.issue_id)))?;

    let ts = current_ts();
    let kind = EventKind::StateChanged {
        state: IssueState::Open,
    };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    insert_and_append(&store, &wal, &actor, &event)?;

    Ok(IssueStateResult {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
        action: "reopened".to_string(),
    })
}

/// Add or remove labels.
pub fn issue_label(
    ctx: &GriteContext,
    opts: &IssueLabelOptions,
) -> Result<IssueLabelResult, GriteError> {
    let _guard = LockGuard::acquire(ctx, &opts.issue_id, opts.acquire_lock)?;

    let store = ctx.open_store()?;
    let wal = ctx.open_wal()?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = store.resolve_issue_id(&opts.issue_id)?;
    let _existing = store
        .get_issue(&issue_id)?
        .ok_or_else(|| GriteError::NotFound(format!("Issue {} not found", opts.issue_id)))?;

    let ts = current_ts();
    let kind = if !opts.add.is_empty() {
        EventKind::LabelAdded {
            label: opts.add[0].clone(),
        }
    } else if !opts.remove.is_empty() {
        EventKind::LabelRemoved {
            label: opts.remove[0].clone(),
        }
    } else {
        return Err(GriteError::InvalidArgs(
            "No labels to add or remove".to_string(),
        ));
    };

    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    insert_and_append(&store, &wal, &actor, &event)?;

    Ok(IssueLabelResult {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
    })
}

/// Add or remove assignees.
pub fn issue_assign(
    ctx: &GriteContext,
    opts: &IssueAssignOptions,
) -> Result<IssueAssignResult, GriteError> {
    let _guard = LockGuard::acquire(ctx, &opts.issue_id, opts.acquire_lock)?;

    let store = ctx.open_store()?;
    let wal = ctx.open_wal()?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = store.resolve_issue_id(&opts.issue_id)?;
    let _existing = store
        .get_issue(&issue_id)?
        .ok_or_else(|| GriteError::NotFound(format!("Issue {} not found", opts.issue_id)))?;

    let ts = current_ts();
    let kind = if !opts.add.is_empty() {
        EventKind::AssigneeAdded {
            user: opts.add[0].clone(),
        }
    } else if !opts.remove.is_empty() {
        EventKind::AssigneeRemoved {
            user: opts.remove[0].clone(),
        }
    } else {
        return Err(GriteError::InvalidArgs(
            "No assignees to add or remove".to_string(),
        ));
    };

    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    insert_and_append(&store, &wal, &actor, &event)?;

    Ok(IssueAssignResult {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
    })
}

/// Add a link.
pub fn issue_link(
    ctx: &GriteContext,
    opts: &IssueLinkOptions,
) -> Result<IssueLinkResult, GriteError> {
    let _guard = LockGuard::acquire(ctx, &opts.issue_id, opts.acquire_lock)?;

    let store = ctx.open_store()?;
    let wal = ctx.open_wal()?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = store.resolve_issue_id(&opts.issue_id)?;
    let _existing = store
        .get_issue(&issue_id)?
        .ok_or_else(|| GriteError::NotFound(format!("Issue {} not found", opts.issue_id)))?;

    let ts = current_ts();
    let kind = EventKind::LinkAdded {
        url: opts.url.clone(),
        note: opts.note.clone(),
    };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    insert_and_append(&store, &wal, &actor, &event)?;

    Ok(IssueLinkResult {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
    })
}

/// Add an attachment.
pub fn issue_attach(
    ctx: &GriteContext,
    opts: &IssueAttachOptions,
) -> Result<IssueAttachResult, GriteError> {
    let _guard = LockGuard::acquire(ctx, &opts.issue_id, opts.acquire_lock)?;

    let store = ctx.open_store()?;
    let wal = ctx.open_wal()?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = store.resolve_issue_id(&opts.issue_id)?;
    let _existing = store
        .get_issue(&issue_id)?
        .ok_or_else(|| GriteError::NotFound(format!("Issue {} not found", opts.issue_id)))?;

    let ts = current_ts();
    let sha256_bytes = hex::decode(&opts.sha256)
        .map_err(|e| GriteError::InvalidArgs(format!("Invalid sha256 hex: {}", e)))?
        .try_into()
        .map_err(|_| GriteError::InvalidArgs("sha256 must be 32 bytes".to_string()))?;
    let kind = EventKind::AttachmentAdded {
        name: opts.name.clone(),
        sha256: sha256_bytes,
        mime: opts.mime.clone(),
    };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    insert_and_append(&store, &wal, &actor, &event)?;

    Ok(IssueAttachResult {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
    })
}
