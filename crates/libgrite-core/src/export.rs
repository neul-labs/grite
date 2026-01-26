use serde::Serialize;
use crate::error::GriteError;
use crate::store::{GritStore, IssueFilter};
use crate::types::event::{Event, EventKind};
use crate::types::ids::{id_to_hex, EventId};
use crate::types::issue::IssueSummary;

/// Export metadata
#[derive(Debug, Serialize)]
pub struct ExportMeta {
    pub schema_version: u32,
    pub generated_ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wal_head: Option<String>,
    pub event_count: usize,
}

/// JSON export format (from export-format.md)
#[derive(Debug, Serialize)]
pub struct JsonExport {
    pub meta: ExportMeta,
    pub issues: Vec<IssueSummaryJson>,
    pub events: Vec<EventJson>,
}

/// Issue summary for JSON export
#[derive(Debug, Serialize)]
pub struct IssueSummaryJson {
    pub issue_id: String,
    pub title: String,
    pub state: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub updated_ts: u64,
    pub comment_count: usize,
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

/// Event for JSON export
#[derive(Debug, Serialize)]
pub struct EventJson {
    pub event_id: String,
    pub issue_id: String,
    pub actor: String,
    pub ts_unix_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub kind: serde_json::Value,
}

impl From<&Event> for EventJson {
    fn from(e: &Event) -> Self {
        Self {
            event_id: id_to_hex(&e.event_id),
            issue_id: id_to_hex(&e.issue_id),
            actor: id_to_hex(&e.actor),
            ts_unix_ms: e.ts_unix_ms,
            parent: e.parent.as_ref().map(id_to_hex),
            kind: event_kind_to_json(&e.kind),
        }
    }
}

fn event_kind_to_json(kind: &EventKind) -> serde_json::Value {
    match kind {
        EventKind::IssueCreated { title, body, labels } => {
            serde_json::json!({
                "IssueCreated": {
                    "title": title,
                    "body": body,
                    "labels": labels
                }
            })
        }
        EventKind::IssueUpdated { title, body } => {
            serde_json::json!({
                "IssueUpdated": {
                    "title": title,
                    "body": body
                }
            })
        }
        EventKind::CommentAdded { body } => {
            serde_json::json!({
                "CommentAdded": {
                    "body": body
                }
            })
        }
        EventKind::LabelAdded { label } => {
            serde_json::json!({
                "LabelAdded": {
                    "label": label
                }
            })
        }
        EventKind::LabelRemoved { label } => {
            serde_json::json!({
                "LabelRemoved": {
                    "label": label
                }
            })
        }
        EventKind::StateChanged { state } => {
            serde_json::json!({
                "StateChanged": {
                    "state": state.as_str()
                }
            })
        }
        EventKind::LinkAdded { url, note } => {
            serde_json::json!({
                "LinkAdded": {
                    "url": url,
                    "note": note
                }
            })
        }
        EventKind::AssigneeAdded { user } => {
            serde_json::json!({
                "AssigneeAdded": {
                    "user": user
                }
            })
        }
        EventKind::AssigneeRemoved { user } => {
            serde_json::json!({
                "AssigneeRemoved": {
                    "user": user
                }
            })
        }
        EventKind::AttachmentAdded { name, sha256, mime } => {
            serde_json::json!({
                "AttachmentAdded": {
                    "name": name,
                    "sha256": id_to_hex(sha256),
                    "mime": mime
                }
            })
        }
        EventKind::DependencyAdded { target, dep_type } => {
            serde_json::json!({
                "DependencyAdded": {
                    "target": id_to_hex(target),
                    "dep_type": dep_type.as_str()
                }
            })
        }
        EventKind::DependencyRemoved { target, dep_type } => {
            serde_json::json!({
                "DependencyRemoved": {
                    "target": id_to_hex(target),
                    "dep_type": dep_type.as_str()
                }
            })
        }
        EventKind::ContextUpdated { path, language, symbols, summary, content_hash } => {
            serde_json::json!({
                "ContextUpdated": {
                    "path": path,
                    "language": language,
                    "symbol_count": symbols.len(),
                    "summary": summary,
                    "content_hash": id_to_hex(content_hash)
                }
            })
        }
        EventKind::ProjectContextUpdated { key, value } => {
            serde_json::json!({
                "ProjectContextUpdated": {
                    "key": key,
                    "value": value
                }
            })
        }
    }
}

/// Filter for incremental exports
pub enum ExportSince {
    Timestamp(u64),
    EventId(EventId),
}

/// Export to JSON format
pub fn export_json(store: &GritStore, since: Option<ExportSince>) -> Result<JsonExport, GriteError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    // Get all issues
    let issues: Vec<IssueSummaryJson> = store
        .list_issues(&IssueFilter::default())?
        .iter()
        .map(IssueSummaryJson::from)
        .collect();

    // Get all events
    let mut events = store.get_all_events()?;

    // Apply since filter
    if let Some(since_filter) = since {
        events = events
            .into_iter()
            .filter(|e| match &since_filter {
                ExportSince::Timestamp(ts) => e.ts_unix_ms > *ts,
                ExportSince::EventId(event_id) => {
                    // Include events after the given event_id in sort order
                    (&e.issue_id, e.ts_unix_ms, &e.actor, &e.event_id)
                        > (&e.issue_id, e.ts_unix_ms, &e.actor, event_id)
                }
            })
            .collect();
    }

    let event_jsons: Vec<EventJson> = events.iter().map(EventJson::from).collect();

    Ok(JsonExport {
        meta: ExportMeta {
            schema_version: 1,
            generated_ts: now,
            wal_head: None, // M1 has no WAL
            event_count: event_jsons.len(),
        },
        issues,
        events: event_jsons,
    })
}

/// Export to Markdown format
pub fn export_markdown(store: &GritStore, _since: Option<ExportSince>) -> Result<String, GriteError> {
    let mut md = String::new();

    md.push_str("# Grit Export\n\n");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    md.push_str(&format!("Generated: {}\n\n", now));

    // List issues
    let issues = store.list_issues(&IssueFilter::default())?;

    if issues.is_empty() {
        md.push_str("No issues found.\n");
        return Ok(md);
    }

    md.push_str("## Issues\n\n");

    for summary in &issues {
        let issue_id_hex = id_to_hex(&summary.issue_id);
        let state_str = format!("{:?}", summary.state).to_lowercase();

        md.push_str(&format!("### {} [{}]\n\n", summary.title, state_str));
        md.push_str(&format!("**ID:** `{}`\n\n", issue_id_hex));

        if !summary.labels.is_empty() {
            md.push_str(&format!("**Labels:** {}\n\n", summary.labels.join(", ")));
        }

        if !summary.assignees.is_empty() {
            md.push_str(&format!(
                "**Assignees:** {}\n\n",
                summary.assignees.join(", ")
            ));
        }

        if summary.comment_count > 0 {
            md.push_str(&format!("**Comments:** {}\n\n", summary.comment_count));
        }

        // Get full issue for body and comments
        if let Some(proj) = store.get_issue(&summary.issue_id)? {
            if !proj.body.is_empty() {
                md.push_str(&format!("{}\n\n", proj.body));
            }

            if !proj.comments.is_empty() {
                md.push_str("#### Comments\n\n");
                for comment in &proj.comments {
                    let actor_hex = id_to_hex(&comment.actor);
                    md.push_str(&format!(
                        "> **{}** at {}:\n> {}\n\n",
                        &actor_hex[..8],
                        comment.ts_unix_ms,
                        comment.body
                    ));
                }
            }
        }

        md.push_str("---\n\n");
    }

    Ok(md)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::compute_event_id;
    use crate::types::ids::generate_issue_id;
    use tempfile::tempdir;

    #[test]
    fn test_export_json() {
        let dir = tempdir().unwrap();
        let store = GritStore::open(dir.path()).unwrap();

        let issue_id = generate_issue_id();
        let actor = [1u8; 16];
        let kind = EventKind::IssueCreated {
            title: "Test".to_string(),
            body: "Body".to_string(),
            labels: vec!["bug".to_string()],
        };
        let event_id = compute_event_id(&issue_id, &actor, 1000, None, &kind);
        let event = Event::new(event_id, issue_id, actor, 1000, None, kind);
        store.insert_event(&event).unwrap();

        let export = export_json(&store, None).unwrap();
        assert_eq!(export.meta.schema_version, 1);
        assert_eq!(export.issues.len(), 1);
        assert_eq!(export.events.len(), 1);
        assert_eq!(export.issues[0].title, "Test");
    }

    #[test]
    fn test_export_markdown() {
        let dir = tempdir().unwrap();
        let store = GritStore::open(dir.path()).unwrap();

        let issue_id = generate_issue_id();
        let actor = [1u8; 16];
        let kind = EventKind::IssueCreated {
            title: "Test Issue".to_string(),
            body: "This is the body".to_string(),
            labels: vec!["bug".to_string()],
        };
        let event_id = compute_event_id(&issue_id, &actor, 1000, None, &kind);
        let event = Event::new(event_id, issue_id, actor, 1000, None, kind);
        store.insert_event(&event).unwrap();

        let md = export_markdown(&store, None).unwrap();
        assert!(md.contains("# Grit Export"));
        assert!(md.contains("Test Issue"));
        assert!(md.contains("bug"));
    }
}
