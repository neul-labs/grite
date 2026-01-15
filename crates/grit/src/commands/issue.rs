use libgrit_core::{
    hash::compute_event_id,
    types::event::{Event, EventKind, IssueState},
    types::ids::{generate_issue_id, id_to_hex, hex_to_id, parse_issue_id},
    types::issue::IssueSummary,
    store::IssueFilter,
    GritError,
};
use serde::Serialize;
use crate::cli::{Cli, IssueCommand, LabelCommand, AssigneeCommand, LinkCommand, AttachmentCommand};
use crate::context::GritContext;
use crate::output::output_success;
use crate::event_helper::insert_and_append;

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
        IssueCommand::Update { id, title, body } => run_update(cli, id, title, body),
        IssueCommand::Comment { id, body } => run_comment(cli, id, body),
        IssueCommand::Close { id } => run_close(cli, id),
        IssueCommand::Reopen { id } => run_reopen(cli, id),
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
    let store = ctx.open_store()?;
    let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
    let actor = ctx.actor_config.actor_id_bytes()?;

    let issue_id = generate_issue_id();
    let ts = current_ts();
    let kind = EventKind::IssueCreated { title, body, labels };
    let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor, ts, None, kind);

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

fn run_update(cli: &Cli, id: String, title: Option<String>, body: Option<String>) -> Result<(), GritError> {
    if title.is_none() && body.is_none() {
        return Err(GritError::InvalidArgs("At least one of --title or --body must be provided".to_string()));
    }

    let ctx = GritContext::resolve(cli)?;
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

    let result = insert_and_append(&store, &wal, &actor, &event)?;

    output_success(cli, IssueUpdateOutput {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
        wal_head: result.wal_head,
    });

    Ok(())
}

fn run_comment(cli: &Cli, id: String, body: String) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
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

    let result = insert_and_append(&store, &wal, &actor, &event)?;

    output_success(cli, IssueUpdateOutput {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
        wal_head: result.wal_head,
    });

    Ok(())
}

fn run_close(cli: &Cli, id: String) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
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

    let result = insert_and_append(&store, &wal, &actor, &event)?;

    output_success(cli, IssueStateOutput {
        issue_id: id_to_hex(&issue_id),
        event_id: id_to_hex(&event_id),
        state: "closed".to_string(),
        wal_head: result.wal_head,
    });

    Ok(())
}

fn run_reopen(cli: &Cli, id: String) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
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
        LabelCommand::Add { id, label } => {
            let ctx = GritContext::resolve(cli)?;
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

            let result = insert_and_append(&store, &wal, &actor, &event)?;

            output_success(cli, IssueUpdateOutput {
                issue_id: id_to_hex(&issue_id),
                event_id: id_to_hex(&event_id),
                wal_head: result.wal_head,
            });
        }
        LabelCommand::Remove { id, label } => {
            let ctx = GritContext::resolve(cli)?;
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
        AssigneeCommand::Add { id, user } => {
            let ctx = GritContext::resolve(cli)?;
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

            let result = insert_and_append(&store, &wal, &actor, &event)?;

            output_success(cli, IssueUpdateOutput {
                issue_id: id_to_hex(&issue_id),
                event_id: id_to_hex(&event_id),
                wal_head: result.wal_head,
            });
        }
        AssigneeCommand::Remove { id, user } => {
            let ctx = GritContext::resolve(cli)?;
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
        LinkCommand::Add { id, url, note } => {
            let ctx = GritContext::resolve(cli)?;
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
        AttachmentCommand::Add { id, name, sha256, mime } => {
            let ctx = GritContext::resolve(cli)?;
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
