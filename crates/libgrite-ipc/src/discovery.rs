//! Discovery protocol types
//!
//! Discovery uses NNG SURVEY sockets to find running daemons.

use rkyv::{Archive, Deserialize, Serialize};

use crate::{IPC_SCHEMA_VERSION, PROTOCOL_NAME};

/// Discovery request sent via SURVEY socket
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct DiscoverRequest {
    /// Protocol name (should be "grit-ipc")
    pub protocol: String,
    /// Minimum supported version
    pub min_version: u32,
}

impl DiscoverRequest {
    /// Create a new discovery request
    pub fn new() -> Self {
        Self {
            protocol: PROTOCOL_NAME.to_string(),
            min_version: 1,
        }
    }

    /// Create with a specific minimum version
    pub fn with_min_version(min_version: u32) -> Self {
        Self {
            protocol: PROTOCOL_NAME.to_string(),
            min_version,
        }
    }
}

impl Default for DiscoverRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Discovery response from a daemon
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct DiscoverResponse {
    /// Protocol name
    pub protocol: String,
    /// IPC schema version
    pub ipc_schema_version: u32,
    /// Unique daemon instance ID
    pub daemon_id: String,
    /// IPC endpoint to connect to
    pub endpoint: String,
    /// Workers managed by this daemon
    pub workers: Vec<WorkerInfo>,
}

impl DiscoverResponse {
    /// Create a new discovery response
    pub fn new(daemon_id: String, endpoint: String, workers: Vec<WorkerInfo>) -> Self {
        Self {
            protocol: PROTOCOL_NAME.to_string(),
            ipc_schema_version: IPC_SCHEMA_VERSION,
            daemon_id,
            endpoint,
            workers,
        }
    }

    /// Check if this daemon manages a specific repo/actor
    pub fn has_worker(&self, repo_root: &str, actor_id: &str) -> bool {
        self.workers
            .iter()
            .any(|w| w.repo_root == repo_root && w.actor_id == actor_id)
    }
}

/// Information about a worker managed by the daemon
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct WorkerInfo {
    /// Repository root path
    pub repo_root: String,
    /// Actor ID (hex-encoded)
    pub actor_id: String,
    /// Data directory path
    pub data_dir: String,
}

impl WorkerInfo {
    /// Create a new worker info
    pub fn new(repo_root: String, actor_id: String, data_dir: String) -> Self {
        Self {
            repo_root,
            actor_id,
            data_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_request() {
        let req = DiscoverRequest::new();
        assert_eq!(req.protocol, PROTOCOL_NAME);
        assert_eq!(req.min_version, 1);
    }

    #[test]
    fn test_discover_response() {
        let workers = vec![
            WorkerInfo::new(
                "/repo1".to_string(),
                "actor1".to_string(),
                "/repo1/.git/grite/actors/actor1".to_string(),
            ),
            WorkerInfo::new(
                "/repo2".to_string(),
                "actor2".to_string(),
                "/repo2/.git/grite/actors/actor2".to_string(),
            ),
        ];

        let resp = DiscoverResponse::new(
            "daemon-123".to_string(),
            "ipc:///tmp/grite-daemon.sock".to_string(),
            workers,
        );

        assert!(resp.has_worker("/repo1", "actor1"));
        assert!(resp.has_worker("/repo2", "actor2"));
        assert!(!resp.has_worker("/repo3", "actor3"));
    }

    #[test]
    fn test_rkyv_roundtrip() {
        let req = DiscoverRequest::new();
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&req).unwrap();
        let archived =
            rkyv::access::<ArchivedDiscoverRequest, rkyv::rancor::Error>(&bytes).unwrap();
        assert_eq!(archived.protocol.as_str(), PROTOCOL_NAME);
    }
}
