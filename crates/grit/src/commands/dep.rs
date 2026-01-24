use libgrit_core::{
    hash::compute_event_id,
    types::event::{DependencyType, Event, EventKind},
    types::ids::{id_to_hex, parse_issue_id},
    store::IssueFilter,
    GritError,
};
use crate::cli::{Cli, DepCommand};
use crate::context::GritContext;
use crate::output::output_success;
use crate::event_helper::insert_and_append;

pub fn run(cli: &Cli, cmd: DepCommand) -> Result<(), GritError> {
    match cmd {
        DepCommand::Add { id, target, r#type, lock: _ } => run_add(cli, id, target, r#type),
        DepCommand::Remove { id, target, r#type, lock: _ } => run_remove(cli, id, target, r#type),
        DepCommand::List { id, reverse } => run_list(cli, id, reverse),
        DepCommand::Topo { state, label } => run_topo(cli, state, label),
    }
}

fn parse_dep_type(s: &str) -> Result<DependencyType, GritError> {
    DependencyType::from_str(s).ok_or_else(|| {
        GritError::InvalidArgs(format!(
            "Invalid dependency type '{}'. Valid types: blocks, depends_on, related_to",
            s
        ))
    })
}

fn current_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn run_add(cli: &Cli, id: String, target: String, dep_type_str: String) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let store = ctx.open_store()?;
    let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;

    let issue_id = parse_issue_id(&id).map_err(|e| GritError::InvalidArgs(format!("Invalid issue ID: {}", e)))?;
    let target_id = parse_issue_id(&target).map_err(|e| GritError::InvalidArgs(format!("Invalid target ID: {}", e)))?;
    let dep_type = parse_dep_type(&dep_type_str)?;

    // Verify both issues exist
    if store.get_issue(&issue_id)?.is_none() {
        return Err(GritError::NotFound(format!("Issue {} not found", id)));
    }
    if store.get_issue(&target_id)?.is_none() {
        return Err(GritError::NotFound(format!("Target issue {} not found", target)));
    }

    // Check for cycles (only for acyclic types)
    if store.would_create_cycle(&issue_id, &target_id, &dep_type)? {
        return Err(GritError::InvalidArgs(format!(
            "Adding this dependency would create a cycle in the {} graph",
            dep_type.as_str()
        )));
    }

    let actor_id_bytes = libgrit_core::types::ids::hex_to_id::<16>(&ctx.actor_id)
        .map_err(|e| GritError::InvalidArgs(format!("Invalid actor ID: {}", e)))?;

    let ts = current_ts();
    let kind = EventKind::DependencyAdded {
        target: target_id,
        dep_type,
    };
    let event_id = compute_event_id(&issue_id, &actor_id_bytes, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor_id_bytes, ts, None, kind);

    insert_and_append(&store, &wal, &actor_id_bytes, &event)?;

    let output = serde_json::json!({
        "event_id": id_to_hex(&event_id),
        "issue_id": id,
        "target": target,
        "dep_type": dep_type.as_str(),
        "action": "added"
    });

    output_success(cli, &output);
    Ok(())
}

fn run_remove(cli: &Cli, id: String, target: String, dep_type_str: String) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let store = ctx.open_store()?;
    let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;

    let issue_id = parse_issue_id(&id).map_err(|e| GritError::InvalidArgs(format!("Invalid issue ID: {}", e)))?;
    let target_id = parse_issue_id(&target).map_err(|e| GritError::InvalidArgs(format!("Invalid target ID: {}", e)))?;
    let dep_type = parse_dep_type(&dep_type_str)?;

    let actor_id_bytes = libgrit_core::types::ids::hex_to_id::<16>(&ctx.actor_id)
        .map_err(|e| GritError::InvalidArgs(format!("Invalid actor ID: {}", e)))?;

    let ts = current_ts();
    let kind = EventKind::DependencyRemoved {
        target: target_id,
        dep_type,
    };
    let event_id = compute_event_id(&issue_id, &actor_id_bytes, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor_id_bytes, ts, None, kind);

    insert_and_append(&store, &wal, &actor_id_bytes, &event)?;

    let output = serde_json::json!({
        "event_id": id_to_hex(&event_id),
        "issue_id": id,
        "target": target,
        "dep_type": dep_type.as_str(),
        "action": "removed"
    });

    output_success(cli, &output);
    Ok(())
}

fn run_list(cli: &Cli, id: String, reverse: bool) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let store = ctx.open_store()?;

    let issue_id = parse_issue_id(&id).map_err(|e| GritError::InvalidArgs(format!("Invalid issue ID: {}", e)))?;

    let deps = if reverse {
        store.get_dependents(&issue_id)?
    } else {
        store.get_dependencies(&issue_id)?
    };

    let dep_list: Vec<serde_json::Value> = deps.iter().map(|(target, dep_type)| {
        let title = store.get_issue(target)
            .ok()
            .flatten()
            .map(|p| p.title.clone())
            .unwrap_or_else(|| "?".to_string());
        serde_json::json!({
            "issue_id": id_to_hex(target),
            "dep_type": dep_type.as_str(),
            "title": title,
        })
    }).collect();

    let output = serde_json::json!({
        "issue_id": id,
        "direction": if reverse { "dependents" } else { "dependencies" },
        "deps": dep_list,
    });

    output_success(cli, &output);
    Ok(())
}

fn run_topo(cli: &Cli, state: Option<String>, label: Option<String>) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let store = ctx.open_store()?;

    let filter = IssueFilter {
        state: state.as_deref().map(|s| match s {
            "open" => libgrit_core::types::event::IssueState::Open,
            "closed" => libgrit_core::types::event::IssueState::Closed,
            _ => libgrit_core::types::event::IssueState::Open,
        }),
        label,
    };

    let sorted = store.topological_order(&filter)?;

    let issues: Vec<serde_json::Value> = sorted.iter().map(|s| {
        serde_json::json!({
            "issue_id": id_to_hex(&s.issue_id),
            "title": s.title,
            "state": format!("{:?}", s.state).to_lowercase(),
            "labels": s.labels,
        })
    }).collect();

    let output = serde_json::json!({
        "issues": issues,
        "order": "topological",
    });

    output_success(cli, &output);
    Ok(())
}
