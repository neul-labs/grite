use serde::{Deserialize, Serialize};
use super::ids::{ActorId, EventId, IssueId};

/// Issue state enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    Open,
    Closed,
}

impl IssueState {
    /// Convert to lowercase string for CBOR encoding
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
        }
    }
}

/// Dependency relationship type between issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    /// This issue blocks the target from proceeding
    Blocks,
    /// This issue depends on the target being completed
    DependsOn,
    /// Symmetric relationship, no ordering constraint
    RelatedTo,
}

impl DependencyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DependencyType::Blocks => "blocks",
            DependencyType::DependsOn => "depends_on",
            DependencyType::RelatedTo => "related_to",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "blocks" => Some(DependencyType::Blocks),
            "depends_on" => Some(DependencyType::DependsOn),
            "related_to" => Some(DependencyType::RelatedTo),
            _ => None,
        }
    }

    /// Whether this relationship type has directed acyclic constraints
    pub fn is_acyclic(&self) -> bool {
        matches!(self, DependencyType::Blocks | DependencyType::DependsOn)
    }
}

/// Symbol information extracted from source code
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    pub line_start: u32,
    pub line_end: u32,
}

/// Event kind enum representing all possible issue events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    IssueCreated {
        title: String,
        body: String,
        labels: Vec<String>,
    },
    IssueUpdated {
        title: Option<String>,
        body: Option<String>,
    },
    CommentAdded {
        body: String,
    },
    LabelAdded {
        label: String,
    },
    LabelRemoved {
        label: String,
    },
    StateChanged {
        state: IssueState,
    },
    LinkAdded {
        url: String,
        note: Option<String>,
    },
    AssigneeAdded {
        user: String,
    },
    AssigneeRemoved {
        user: String,
    },
    AttachmentAdded {
        name: String,
        sha256: [u8; 32],
        mime: String,
    },
    DependencyAdded {
        target: IssueId,
        dep_type: DependencyType,
    },
    DependencyRemoved {
        target: IssueId,
        dep_type: DependencyType,
    },
    ContextUpdated {
        path: String,
        language: String,
        symbols: Vec<SymbolInfo>,
        summary: String,
        content_hash: [u8; 32],
    },
    ProjectContextUpdated {
        key: String,
        value: String,
    },
}

impl EventKind {
    /// Get the kind tag for CBOR encoding (from data-model.md)
    pub fn kind_tag(&self) -> u32 {
        match self {
            EventKind::IssueCreated { .. } => 1,
            EventKind::IssueUpdated { .. } => 2,
            EventKind::CommentAdded { .. } => 3,
            EventKind::LabelAdded { .. } => 4,
            EventKind::LabelRemoved { .. } => 5,
            EventKind::StateChanged { .. } => 6,
            EventKind::LinkAdded { .. } => 7,
            EventKind::AssigneeAdded { .. } => 8,
            EventKind::AssigneeRemoved { .. } => 9,
            EventKind::AttachmentAdded { .. } => 10,
            EventKind::DependencyAdded { .. } => 11,
            EventKind::DependencyRemoved { .. } => 12,
            EventKind::ContextUpdated { .. } => 13,
            EventKind::ProjectContextUpdated { .. } => 14,
        }
    }
}

/// An event in the issue tracking system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Content-addressed event ID (BLAKE2b-256)
    pub event_id: EventId,
    /// Issue this event belongs to
    pub issue_id: IssueId,
    /// Actor who created this event
    pub actor: ActorId,
    /// Unix timestamp in milliseconds
    pub ts_unix_ms: u64,
    /// Parent event ID (for causal ordering)
    pub parent: Option<EventId>,
    /// The event payload
    pub kind: EventKind,
    /// Optional signature over event_id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig: Option<Vec<u8>>,
}

impl Event {
    /// Create a new event (event_id must be computed separately via hash::compute_event_id)
    pub fn new(
        event_id: EventId,
        issue_id: IssueId,
        actor: ActorId,
        ts_unix_ms: u64,
        parent: Option<EventId>,
        kind: EventKind,
    ) -> Self {
        Self {
            event_id,
            issue_id,
            actor,
            ts_unix_ms,
            parent,
            kind,
            sig: None,
        }
    }

    /// Get the version tuple for LWW comparison: (ts, actor, event_id)
    pub fn version(&self) -> (u64, &ActorId, &EventId) {
        (self.ts_unix_ms, &self.actor, &self.event_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_state_as_str() {
        assert_eq!(IssueState::Open.as_str(), "open");
        assert_eq!(IssueState::Closed.as_str(), "closed");
    }

    #[test]
    fn test_event_kind_tags() {
        assert_eq!(
            EventKind::IssueCreated {
                title: String::new(),
                body: String::new(),
                labels: vec![]
            }
            .kind_tag(),
            1
        );
        assert_eq!(
            EventKind::IssueUpdated {
                title: None,
                body: None
            }
            .kind_tag(),
            2
        );
        assert_eq!(
            EventKind::CommentAdded {
                body: String::new()
            }
            .kind_tag(),
            3
        );
        assert_eq!(
            EventKind::LabelAdded {
                label: String::new()
            }
            .kind_tag(),
            4
        );
        assert_eq!(
            EventKind::LabelRemoved {
                label: String::new()
            }
            .kind_tag(),
            5
        );
        assert_eq!(
            EventKind::StateChanged {
                state: IssueState::Open
            }
            .kind_tag(),
            6
        );
        assert_eq!(
            EventKind::LinkAdded {
                url: String::new(),
                note: None
            }
            .kind_tag(),
            7
        );
        assert_eq!(
            EventKind::AssigneeAdded {
                user: String::new()
            }
            .kind_tag(),
            8
        );
        assert_eq!(
            EventKind::AssigneeRemoved {
                user: String::new()
            }
            .kind_tag(),
            9
        );
        assert_eq!(
            EventKind::AttachmentAdded {
                name: String::new(),
                sha256: [0; 32],
                mime: String::new()
            }
            .kind_tag(),
            10
        );
        assert_eq!(
            EventKind::DependencyAdded {
                target: [0; 16],
                dep_type: DependencyType::Blocks
            }
            .kind_tag(),
            11
        );
        assert_eq!(
            EventKind::DependencyRemoved {
                target: [0; 16],
                dep_type: DependencyType::DependsOn
            }
            .kind_tag(),
            12
        );
        assert_eq!(
            EventKind::ContextUpdated {
                path: String::new(),
                language: String::new(),
                symbols: vec![],
                summary: String::new(),
                content_hash: [0; 32]
            }
            .kind_tag(),
            13
        );
        assert_eq!(
            EventKind::ProjectContextUpdated {
                key: String::new(),
                value: String::new()
            }
            .kind_tag(),
            14
        );
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::new(
            [0u8; 32],
            [1u8; 16],
            [2u8; 16],
            1700000000000,
            None,
            EventKind::IssueCreated {
                title: "Test".to_string(),
                body: "Body".to_string(),
                labels: vec!["bug".to_string()],
            },
        );

        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }
}
