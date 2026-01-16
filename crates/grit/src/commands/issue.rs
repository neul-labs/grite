use libgrit_core::{
    hash::compute_event_id,
    lock::LockCheckResult,
    types::event::{Event, EventKind, IssueState},
    types::ids::{generate_issue_id, id_to_hex, hex_to_id, parse_issue_id},
    types::issue::IssueSummary,
    store::IssueFilter,
    GritError,
};
use libgrit_git;
use serde::Serialize;
use crate::cli::{Cli, IssueCommand, LabelCommand, AssigneeCommand, LinkCommand, AttachmentCommand};
use crate::context::GritContext;
use crate::output::output_success;
use crate::event_helper::insert_and_append;

/// Check lock for an issue operation
///
/// Returns Ok(()) if operation can proceed, with warnings printed to stderr if applicable.
/// Returns Err if blocked by lock policy.
fn check_issue_lock(cli: &Cli, ctx: &GritContext, issue_id_hex: &str) -> Result<(), GritError> {
    let resource = format!("issue:{}", issue_id_hex);
    match ctx.check_lock(&resource)? {
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
        LockCheckResult::Blocked(_) => {
            // This case is handled by ctx.check_lock returning Err
            unreachable!()
        }
    }
}

/// Check repo-level lock (for issue creation)
fn check_repo_lock(cli: &Cli, ctx: &GritContext) -> Result<(), GritError> {
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

/// RAII guard for auto-releasing locks
struct LockGuard<'a> {
    ctx: &'a GritContext,
    resource: String,
    acquired: bool,
}

impl<'a> LockGuard<'a> {
    /// Acquire a lock if requested
    fn acquire(ctx: &'a GritContext, issue_id_hex: &str, should_lock: bool) -> Result<Self, GritError> {
        let resource = format!("issue:{}", issue_id_hex);
        if should_lock {
            let lock_manager = ctx.open_lock_manager()
                .map_err(|e| GritError::Internal(e.to_string()))?;
            lock_manager.acquire(&resource, &ctx.actor_id, None)
                .map_err(|e| match e {
                    libgrit_git::GitError::LockConflict { resource, owner, expires_in_ms } => {
                        GritError::Conflict(format!(
                            "Cannot acquire lock on {} - held by {} (expires in {}s)",
                            resource, owner, expires_in_ms / 1000
                        ))
                    }
                    _ => GritError::Internal(e.to_string()),
                })?;
            Ok(Self { ctx, resource, acquired: true })
        } else {
            Ok(Self { ctx, resource, acquired: false })
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

#[derive(Serialize)]
struct IssueCreateOutput {
    issue_id: String,
    event_id: String,
    wal_head: Option<String>,
}

#[derive(Serialize)]
struct IssueListOutput {
    issues: Vec<IssueSummaryJson>,
    total: usize,
}

#[derive(Serialize)]
struct IssueSummaryJson {
    issue_id: String,
    title: String,
    state: String,
    labels: Vec<String>,
    assignees: Vec<String>,
    updated_ts: u64,
    comment_count: usize,
}

impl From<&IssueSummary> for IssueSummaryJson {
    fn from(s: &IssueSummary) -> Self {
        Self {
            issue_id: id_to_hex(&s.issue_id),
            title: s.title.clone(),
            state: format!("{:?}", s.state).to_lowercase(),
            labels: s.labels.clone(),
            assignees: s.assignees.clone(),
            updated_ts: s.updated_ts,
            comment_count: s.comment_count,
        }
    }
}

#[derive(Serialize)]
struct IssueShowOutput {
    issue: IssueSummaryJson,
    events: Vec<EventJson>,
}

#[derive(Serialize)]
struct EventJson {
    event_id: String,
    issue_id: String,
    actor: String,
    ts_unix_ms: u64,
    parent: Option<String>,
    kind: serde_json::Value,
}

#[derive(Serialize)]
struct IssueUpdateOutput {
    issue_id: String,
    event_id: String,
    wal_head: Option<String>,
}

#[derive(Serialize)]
struct IssueStateOutput {
    issue_id: String,
    event_id: String,
    state: String,
    wal_head: Option<String>,
}

pub fn run(cli: &Cli, cmd: IssueCommand) -> Result<(), GritError> {
    match cmd {
        IssueCommand::Create { title, body, label } => run_create(cli, title, body, label),
        IssueCommand::List { state, label } => run_list(cli, state, label),
        IssueCommand::Show { id } => run_show(cli, id),
        IssueCommand::Update { id, title, body, lock } => run_update(cli, id, title, body, lock),
        IssueCommand::Comment { id, body, lock } => run_comment(cli, id, body, lock),
        IssueCommand::Close { id, lock } => run_close(cli, id, lock),
        IssueCommand::Reopen { id, lock } => run_reopen(cli, id, lock),
        IssueCommand::Label { cmd } => run_label(cli, cmd),
        IssueCommand::Assignee { cmd } => run_assignee(cli, cmd),
        IssueCommand::Link { cmd } => run_link(cli, cmd),
        IssueCommand::Attachment { cmd } => run_attachment(cli, cmd),
    }
}

fn current_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn run_create(cli: &Cli, title: String, body: String, labels: Vec<String>) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;

    // Check for repo-level locks before creating
    check_repo_lock(cli, &ctx)?;

    let store = ctx.open_store()?;
    let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = generate_issue_id();
    let ts = current_ts();
    let kind = EventKind::IssueCreated { title, body, labels };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    let result = insert_and_append(&store, &wal, &actor, &event)?;

    output_success(cli, IssueCreateOutput {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
        wal_head: result.wal_head,
    });

    Ok(())
}

fn run_list(cli: &Cli, state: Option<String>, label: Option<String>) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let store = ctx.open_store()?;

    let state_filter = state.map(|s| {
        match s.to_lowercase().as_str() {
            "open" => IssueState::Open,
            "closed" => IssueState::Closed,
            _ => IssueState::Open,
        }
    });

    let filter = IssueFilter {
        state: state_filter,
        label,
    };

    let issues = store.list_issues(&filter)?;
    let total = issues.len();
    let issue_jsons: Vec<IssueSummaryJson> = issues.iter().map(IssueSummaryJson::from).collect();

    output_success(cli, IssueListOutput { issues: issue_jsons, total });

    Ok(())
}

fn run_show(cli: &Cli, id: String) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let store = ctx.open_store()?;

    let issue_id = parse_issue_id(&id)?;
    let proj = store.get_issue(&issue_id)?
        .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

    let events = store.get_issue_events(&issue_id)?;
    let event_jsons: Vec<EventJson> = events.iter().map(|e| {
        EventJson {
            event_id: id_to_hex(&e.event_id),
            issue_id: id_to_hex(&e.issue_id),
            actor: id_to_hex(&e.actor),
            ts_unix_ms: e.ts_unix_ms,
            parent: e.parent.as_ref().map(id_to_hex),
            kind: serde_json::to_value(&e.kind).unwrap_or(serde_json::Value::Null),
        }
    }).collect();

    let summary = IssueSummary::from(&proj);

    output_success(cli, IssueShowOutput {
        issue: IssueSummaryJson::from(&summary),
        events: event_jsons,
    });

    Ok(())
}

fn run_update(cli: &Cli, id: String, title: Option<String>, body: Option<String>, lock: bool) -> Result<(), GritError> {
    if title.is_none() && body.is_none() {
        return Err(GritError::InvalidArgs("At least one of --title or --body must be provided".to_string()));
    }

    let ctx = GritContext::resolve(cli)?;

    // Acquire lock if requested (or just check for conflicts)
    let _lock_guard = LockGuard::acquire(&ctx, &id, lock)?;
    if !lock {
        check_issue_lock(cli, &ctx, &id)?;
    }

    let store = ctx.open_store()?;
    let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = parse_issue_id(&id)?;

    // Verify issue exists
    store.get_issue(&issue_id)?
        .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

    let ts = current_ts();
    let kind = EventKind::IssueUpdated { title, body };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    let result = insert_and_append(&store, &wal, &actor, &event)?;

    output_success(cli, IssueUpdateOutput {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
        wal_head: result.wal_head,
    });

    Ok(())
}

fn run_comment(cli: &Cli, id: String, body: String, lock: bool) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;

    // Acquire lock if requested (or just check for conflicts)
    let _lock_guard = LockGuard::acquire(&ctx, &id, lock)?;
    if !lock {
        check_issue_lock(cli, &ctx, &id)?;
    }

    let store = ctx.open_store()?;
    let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = parse_issue_id(&id)?;

    // Verify issue exists
    store.get_issue(&issue_id)?
        .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

    let ts = current_ts();
    let kind = EventKind::CommentAdded { body };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    let result = insert_and_append(&store, &wal, &actor, &event)?;

    output_success(cli, IssueUpdateOutput {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
        wal_head: result.wal_head,
    });

    Ok(())
}

fn run_close(cli: &Cli, id: String, lock: bool) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;

    // Acquire lock if requested (or just check for conflicts)
    let _lock_guard = LockGuard::acquire(&ctx, &id, lock)?;
    if !lock {
        check_issue_lock(cli, &ctx, &id)?;
    }

    let store = ctx.open_store()?;
    let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = parse_issue_id(&id)?;

    // Verify issue exists
    store.get_issue(&issue_id)?
        .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

    let ts = current_ts();
    let kind = EventKind::StateChanged { state: IssueState::Closed };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    let result = insert_and_append(&store, &wal, &actor, &event)?;

    output_success(cli, IssueStateOutput {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
        state: "closed".to_string(),
        wal_head: result.wal_head,
    });

    Ok(())
}

fn run_reopen(cli: &Cli, id: String, lock: bool) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;

    // Acquire lock if requested (or just check for conflicts)
    let _lock_guard = LockGuard::acquire(&ctx, &id, lock)?;
    if !lock {
        check_issue_lock(cli, &ctx, &id)?;
    }

    let store = ctx.open_store()?;
    let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = parse_issue_id(&id)?;

    // Verify issue exists
    store.get_issue(&issue_id)?
        .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

    let ts = current_ts();
    let kind = EventKind::StateChanged { state: IssueState::Open };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);
    let event = ctx.sign_event(event);

    let result = insert_and_append(&store, &wal, &actor, &event)?;

    output_success(cli, IssueStateOutput {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
        state: "open".to_string(),
        wal_head: result.wal_head,
    });

    Ok(())
}

fn run_label(cli: &Cli, cmd: LabelCommand) -> Result<(), GritError> {
    match cmd {
        LabelCommand::Add { id, label, lock } => {
            let ctx = GritContext::resolve(cli)?;
            let _lock_guard = LockGuard::acquire(&ctx, &id, lock)?;
            if !lock {
                check_issue_lock(cli, &ctx, &id)?;
            }
            let store = ctx.open_store()?;
            let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
            let actor = ctx.actor_config.actor_id_bytes()?;

            let issue_id = parse_issue_id(&id)?;
            store.get_issue(&issue_id)?
                .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

            let ts = current_ts();
            let kind = EventKind::LabelAdded { label };
            let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
            let event = Event::new(event_id, issue_id, actor, ts, None, kind);
            let event = ctx.sign_event(event);

            let result = insert_and_append(&store, &wal, &actor, &event)?;

            output_success(cli, IssueUpdateOutput {
                issue_id: id_to_hex(&issue_id),
                event_id: id_to_hex(&event_id),
                wal_head: result.wal_head,
            });
        }
        LabelCommand::Remove { id, label, lock } => {
            let ctx = GritContext::resolve(cli)?;
            let _lock_guard = LockGuard::acquire(&ctx, &id, lock)?;
            if !lock {
                check_issue_lock(cli, &ctx, &id)?;
            }
            let store = ctx.open_store()?;
            let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
            let actor = ctx.actor_config.actor_id_bytes()?;

            let issue_id = parse_issue_id(&id)?;
            store.get_issue(&issue_id)?
                .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

            let ts = current_ts();
            let kind = EventKind::LabelRemoved { label };
            let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
            let event = Event::new(event_id, issue_id, actor, ts, None, kind);
            let event = ctx.sign_event(event);

            let result = insert_and_append(&store, &wal, &actor, &event)?;

            output_success(cli, IssueUpdateOutput {
                issue_id: id_to_hex(&issue_id),
                event_id: id_to_hex(&event_id),
                wal_head: result.wal_head,
            });
        }
    }
    Ok(())
}

fn run_assignee(cli: &Cli, cmd: AssigneeCommand) -> Result<(), GritError> {
    match cmd {
        AssigneeCommand::Add { id, user, lock } => {
            let ctx = GritContext::resolve(cli)?;
            let _lock_guard = LockGuard::acquire(&ctx, &id, lock)?;
            if !lock {
                check_issue_lock(cli, &ctx, &id)?;
            }
            let store = ctx.open_store()?;
            let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
            let actor = ctx.actor_config.actor_id_bytes()?;

            let issue_id = parse_issue_id(&id)?;
            store.get_issue(&issue_id)?
                .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

            let ts = current_ts();
            let kind = EventKind::AssigneeAdded { user };
            let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
            let event = Event::new(event_id, issue_id, actor, ts, None, kind);
            let event = ctx.sign_event(event);

            let result = insert_and_append(&store, &wal, &actor, &event)?;

            output_success(cli, IssueUpdateOutput {
                issue_id: id_to_hex(&issue_id),
                event_id: id_to_hex(&event_id),
                wal_head: result.wal_head,
            });
        }
        AssigneeCommand::Remove { id, user, lock } => {
            let ctx = GritContext::resolve(cli)?;
            let _lock_guard = LockGuard::acquire(&ctx, &id, lock)?;
            if !lock {
                check_issue_lock(cli, &ctx, &id)?;
            }
            let store = ctx.open_store()?;
            let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
            let actor = ctx.actor_config.actor_id_bytes()?;

            let issue_id = parse_issue_id(&id)?;
            store.get_issue(&issue_id)?
                .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

            let ts = current_ts();
            let kind = EventKind::AssigneeRemoved { user };
            let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
            let event = Event::new(event_id, issue_id, actor, ts, None, kind);
            let event = ctx.sign_event(event);

            let result = insert_and_append(&store, &wal, &actor, &event)?;

            output_success(cli, IssueUpdateOutput {
                issue_id: id_to_hex(&issue_id),
                event_id: id_to_hex(&event_id),
                wal_head: result.wal_head,
            });
        }
    }
    Ok(())
}

fn run_link(cli: &Cli, cmd: LinkCommand) -> Result<(), GritError> {
    match cmd {
        LinkCommand::Add { id, url, note, lock } => {
            let ctx = GritContext::resolve(cli)?;
            let _lock_guard = LockGuard::acquire(&ctx, &id, lock)?;
            if !lock {
                check_issue_lock(cli, &ctx, &id)?;
            }
            let store = ctx.open_store()?;
            let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
            let actor = ctx.actor_config.actor_id_bytes()?;

            let issue_id = parse_issue_id(&id)?;
            store.get_issue(&issue_id)?
                .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

            let ts = current_ts();
            let kind = EventKind::LinkAdded { url, note };
            let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
            let event = Event::new(event_id, issue_id, actor, ts, None, kind);
            let event = ctx.sign_event(event);

            let result = insert_and_append(&store, &wal, &actor, &event)?;

            output_success(cli, IssueUpdateOutput {
                issue_id: id_to_hex(&issue_id),
                event_id: id_to_hex(&event_id),
                wal_head: result.wal_head,
            });
        }
    }
    Ok(())
}

fn run_attachment(cli: &Cli, cmd: AttachmentCommand) -> Result<(), GritError> {
    match cmd {
        AttachmentCommand::Add { id, name, sha256, mime, lock } => {
            let ctx = GritContext::resolve(cli)?;
            let _lock_guard = LockGuard::acquire(&ctx, &id, lock)?;
            if !lock {
                check_issue_lock(cli, &ctx, &id)?;
            }
            let store = ctx.open_store()?;
            let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
            let actor = ctx.actor_config.actor_id_bytes()?;

            let issue_id = parse_issue_id(&id)?;
            store.get_issue(&issue_id)?
                .ok_or_else(|| GritError::NotFound(format!("Issue {} not found", id)))?;

            let sha256_bytes: [u8; 32] = hex_to_id(&sha256)?;

            let ts = current_ts();
            let kind = EventKind::AttachmentAdded { name, sha256: sha256_bytes, mime };
            let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
            let event = Event::new(event_id, issue_id, actor, ts, None, kind);
            let event = ctx.sign_event(event);

            let result = insert_and_append(&store, &wal, &actor, &event)?;

            output_success(cli, IssueUpdateOutput {
                issue_id: id_to_hex(&issue_id),
                event_id: id_to_hex(&event_id),
                wal_head: result.wal_head,
            });
        }
    }
    Ok(())
}
