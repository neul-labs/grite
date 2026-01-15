use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use super::ids::{ActorId, EventId, IssueId};
use super::event::IssueState;

/// A comment on an issue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub event_id: EventId,
    pub actor: ActorId,
    pub ts_unix_ms: u64,
    pub body: String,
}

/// A link attached to an issue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub event_id: EventId,
    pub url: String,
    pub note: Option<String>,
}

/// An attachment on an issue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub event_id: EventId,
    pub name: String,
    pub sha256: [u8; 32],
    pub mime: String,
}

/// Version tuple for LWW comparison: (timestamp, actor, event_id)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub ts_unix_ms: u64,
    pub actor: ActorId,
    pub event_id: EventId,
}

impl Version {
    pub fn new(ts_unix_ms: u64, actor: ActorId, event_id: EventId) -> Self {
        Self { ts_unix_ms, actor, event_id }
    }

    /// Compare versions for LWW: (ts, actor, event_id)
    pub fn is_newer_than(&self, other: &Version) -> bool {
        (self.ts_unix_ms, &self.actor, &self.event_id) > (other.ts_unix_ms, &other.actor, &other.event_id)
    }
}

/// Full projection of an issue from its event stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueProjection {
    pub issue_id: IssueId,
    pub title: String,
    pub body: String,
    pub state: IssueState,
    /// Labels sorted lexicographically (BTreeSet for determinism)
    pub labels: BTreeSet<String>,
    /// Assignees sorted lexicographically (BTreeSet for determinism)
    pub assignees: BTreeSet<String>,
    /// Comments in event order (append-only)
    pub comments: Vec<Comment>,
    /// Links in event order (append-only)
    pub links: Vec<Link>,
    /// Attachments in event order (append-only)
    pub attachments: Vec<Attachment>,
    /// Timestamp when issue was created
    pub created_ts: u64,
    /// Timestamp of last update
    pub updated_ts: u64,
    /// Version tracking for LWW on title
    pub title_version: Version,
    /// Version tracking for LWW on body
    pub body_version: Version,
    /// Version tracking for LWW on state
    pub state_version: Version,
}

impl IssueProjection {
    /// Create a new projection from initial IssueCreated event data
    pub fn new(
        issue_id: IssueId,
        title: String,
        body: String,
        labels: Vec<String>,
        ts_unix_ms: u64,
        actor: ActorId,
        event_id: EventId,
    ) -> Self {
        let version = Version::new(ts_unix_ms, actor, event_id);
        Self {
            issue_id,
            title,
            body,
            state: IssueState::Open,
            labels: labels.into_iter().collect(),
            assignees: BTreeSet::new(),
            comments: Vec::new(),
            links: Vec::new(),
            attachments: Vec::new(),
            created_ts: ts_unix_ms,
            updated_ts: ts_unix_ms,
            title_version: version.clone(),
            body_version: version.clone(),
            state_version: version,
        }
    }
}

/// Summary of an issue for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    pub issue_id: IssueId,
    pub title: String,
    pub state: IssueState,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub updated_ts: u64,
    pub comment_count: usize,
}

impl From<&IssueProjection> for IssueSummary {
    fn from(proj: &IssueProjection) -> Self {
        Self {
            issue_id: proj.issue_id,
            title: proj.title.clone(),
            state: proj.state,
            labels: proj.labels.iter().cloned().collect(),
            assignees: proj.assignees.iter().cloned().collect(),
            updated_ts: proj.updated_ts,
            comment_count: proj.comments.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        let v1 = Version::new(1000, [0u8; 16], [0u8; 32]);
        let v2 = Version::new(2000, [0u8; 16], [0u8; 32]);
        assert!(v2.is_newer_than(&v1));
        assert!(!v1.is_newer_than(&v2));
    }

    #[test]
    fn test_version_tiebreak_by_actor() {
        let v1 = Version::new(1000, [0u8; 16], [0u8; 32]);
        let v2 = Version::new(1000, [1u8; 16], [0u8; 32]);
        assert!(v2.is_newer_than(&v1));
    }

    #[test]
    fn test_version_tiebreak_by_event_id() {
        let v1 = Version::new(1000, [0u8; 16], [0u8; 32]);
        let v2 = Version::new(1000, [0u8; 16], [1u8; 32]);
        assert!(v2.is_newer_than(&v1));
    }

    #[test]
    fn test_issue_projection_new() {
        let proj = IssueProjection::new(
            [0u8; 16],
            "Test".to_string(),
            "Body".to_string(),
            vec!["bug".to_string(), "p0".to_string()],
            1700000000000,
            [1u8; 16],
            [2u8; 32],
        );
        assert_eq!(proj.title, "Test");
        assert_eq!(proj.state, IssueState::Open);
        assert!(proj.labels.contains("bug"));
        assert!(proj.labels.contains("p0"));
    }

    #[test]
    fn test_issue_summary_from_projection() {
        let proj = IssueProjection::new(
            [0u8; 16],
            "Test".to_string(),
            "Body".to_string(),
            vec!["bug".to_string()],
            1700000000000,
            [1u8; 16],
            [2u8; 32],
        );
        let summary = IssueSummary::from(&proj);
        assert_eq!(summary.title, "Test");
        assert_eq!(summary.comment_count, 0);
    }
}
