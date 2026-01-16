//! IPC request and response message types
//!
//! These types define the wire format for daemon communication.
//! Wire format is rkyv-serialized, but JSON is used for lock files.

use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

use crate::IPC_SCHEMA_VERSION;

/// IPC request envelope
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct IpcRequest {
    /// Schema version for compatibility checking
    pub ipc_schema_version: u32,
    /// Unique request ID for correlation
    pub request_id: String,
    /// Repository root path
    pub repo_root: String,
    /// Actor ID (hex-encoded 16 bytes)
    pub actor_id: String,
    /// Data directory path
    pub data_dir: String,
    /// The command to execute
    pub command: IpcCommand,
}

impl IpcRequest {
    /// Create a new request with the current schema version
    pub fn new(
        request_id: String,
        repo_root: String,
        actor_id: String,
        data_dir: String,
        command: IpcCommand,
    ) -> Self {
        Self {
            ipc_schema_version: IPC_SCHEMA_VERSION,
            request_id,
            repo_root,
            actor_id,
            data_dir,
            command,
        }
    }
}

/// IPC response envelope
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct IpcResponse {
    /// Schema version (must match request)
    pub ipc_schema_version: u32,
    /// Request ID for correlation
    pub request_id: String,
    /// Whether the request succeeded
    pub ok: bool,
    /// Response data (JSON-encoded for flexibility)
    pub data: Option<String>,
    /// Error details if ok=false
    pub error: Option<IpcErrorPayload>,
}

impl IpcResponse {
    /// Create a successful response
    pub fn success(request_id: String, data: Option<String>) -> Self {
        Self {
            ipc_schema_version: IPC_SCHEMA_VERSION,
            request_id,
            ok: true,
            data,
            error: None,
        }
    }

    /// Create an error response
    pub fn error(request_id: String, code: String, message: String) -> Self {
        Self {
            ipc_schema_version: IPC_SCHEMA_VERSION,
            request_id,
            ok: false,
            data: None,
            error: Some(IpcErrorPayload {
                code,
                message,
                details: None,
            }),
        }
    }
}

/// Error payload in responses
#[derive(Archive, Serialize, Deserialize, Debug, Clone, SerdeSerialize, SerdeDeserialize)]
#[rkyv(derive(Debug))]
pub struct IpcErrorPayload {
    /// Error code (matches docs/cli-json.md)
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Optional additional details (JSON-encoded)
    pub details: Option<String>,
}

/// Commands that can be sent to the daemon
///
/// These mirror the CLI commands. Payloads are equivalent to CLI flags.
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub enum IpcCommand {
    // Issue commands
    IssueCreate {
        title: String,
        body: String,
        labels: Vec<String>,
    },
    IssueList {
        state: Option<String>,
        label: Option<String>,
    },
    IssueShow {
        issue_id: String,
    },
    IssueUpdate {
        issue_id: String,
        title: Option<String>,
        body: Option<String>,
    },
    IssueComment {
        issue_id: String,
        body: String,
    },
    IssueLabel {
        issue_id: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
    IssueAssign {
        issue_id: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
    IssueClose {
        issue_id: String,
    },
    IssueReopen {
        issue_id: String,
    },
    IssueLink {
        issue_id: String,
        url: String,
        note: Option<String>,
    },
    IssueAttach {
        issue_id: String,
        file_path: String,
    },

    // Database commands
    DbStats,

    // Export command
    Export {
        format: String,
        since: Option<String>,
    },

    // Rebuild command
    Rebuild,

    // Sync command
    Sync {
        remote: String,
        pull: bool,
        push: bool,
    },

    // Snapshot commands
    SnapshotCreate,
    SnapshotList,
    SnapshotGc {
        keep: u32,
    },

    // Daemon commands
    DaemonStatus,
    DaemonStop,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_creation() {
        let req = IpcRequest::new(
            "test-123".to_string(),
            "/path/to/repo".to_string(),
            "abcd1234".to_string(),
            ".git/grit/actors/abcd1234".to_string(),
            IpcCommand::IssueList {
                state: Some("open".to_string()),
                label: None,
            },
        );

        assert_eq!(req.ipc_schema_version, IPC_SCHEMA_VERSION);
        assert_eq!(req.request_id, "test-123");
    }

    #[test]
    fn test_response_success() {
        let resp = IpcResponse::success(
            "test-123".to_string(),
            Some(r#"{"issues": []}"#.to_string()),
        );

        assert!(resp.ok);
        assert!(resp.error.is_none());
        assert!(resp.data.is_some());
    }

    #[test]
    fn test_response_error() {
        let resp = IpcResponse::error(
            "test-123".to_string(),
            "not_found".to_string(),
            "Issue not found".to_string(),
        );

        assert!(!resp.ok);
        assert!(resp.data.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, "not_found");
    }

    #[test]
    fn test_rkyv_roundtrip() {
        let req = IpcRequest::new(
            "test-456".to_string(),
            "/repo".to_string(),
            "actor123".to_string(),
            ".git/grit/actors/actor123".to_string(),
            IpcCommand::IssueCreate {
                title: "Test Issue".to_string(),
                body: "Description".to_string(),
                labels: vec!["bug".to_string()],
            },
        );

        // Serialize
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&req).unwrap();

        // Deserialize
        let archived = rkyv::access::<ArchivedIpcRequest, rkyv::rancor::Error>(&bytes).unwrap();
        assert_eq!(archived.request_id, "test-456");
        assert_eq!(archived.ipc_schema_version, IPC_SCHEMA_VERSION);
    }
}
