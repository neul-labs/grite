//! Notification types for pub/sub
//!
//! The daemon emits these notifications asynchronously.
//! Clients should treat unknown variants as ignorable.

use rkyv::{Archive, Deserialize, Serialize};

/// Notifications emitted by the daemon
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub enum Notification {
    /// An event was applied to an issue
    EventApplied {
        /// Issue ID (hex-encoded)
        issue_id: String,
        /// Event ID (hex-encoded)
        event_id: String,
        /// Timestamp in milliseconds since Unix epoch
        ts_unix_ms: u64,
    },

    /// WAL was synced with a remote
    WalSynced {
        /// New WAL head commit hash
        wal_head: String,
        /// Remote name (e.g., "origin")
        remote: String,
    },

    /// A lock changed state
    LockChanged {
        /// Resource being locked (e.g., "path:docs/")
        resource: String,
        /// Lock owner identifier
        owner: String,
        /// When the lock expires (0 = released)
        expires_unix_ms: u64,
    },

    /// A snapshot was created
    SnapshotCreated {
        /// Snapshot ref (e.g., "refs/grite/snapshots/1700000000000")
        snapshot_ref: String,
    },

    /// Worker started for a repository
    WorkerStarted {
        /// Repository root path
        repo_root: String,
        /// Actor ID (hex-encoded)
        actor_id: String,
    },

    /// Worker stopped for a repository
    WorkerStopped {
        /// Repository root path
        repo_root: String,
        /// Actor ID (hex-encoded)
        actor_id: String,
        /// Reason for stopping
        reason: String,
    },
}

impl Notification {
    /// Get the notification type as a string (for filtering)
    pub fn notification_type(&self) -> &'static str {
        match self {
            Notification::EventApplied { .. } => "EventApplied",
            Notification::WalSynced { .. } => "WalSynced",
            Notification::LockChanged { .. } => "LockChanged",
            Notification::SnapshotCreated { .. } => "SnapshotCreated",
            Notification::WorkerStarted { .. } => "WorkerStarted",
            Notification::WorkerStopped { .. } => "WorkerStopped",
        }
    }

    /// Create an EventApplied notification
    pub fn event_applied(issue_id: String, event_id: String, ts_unix_ms: u64) -> Self {
        Notification::EventApplied {
            issue_id,
            event_id,
            ts_unix_ms,
        }
    }

    /// Create a WalSynced notification
    pub fn wal_synced(wal_head: String, remote: String) -> Self {
        Notification::WalSynced { wal_head, remote }
    }

    /// Create a LockChanged notification
    pub fn lock_changed(resource: String, owner: String, expires_unix_ms: u64) -> Self {
        Notification::LockChanged {
            resource,
            owner,
            expires_unix_ms,
        }
    }

    /// Create a SnapshotCreated notification
    pub fn snapshot_created(snapshot_ref: String) -> Self {
        Notification::SnapshotCreated { snapshot_ref }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_type() {
        let n = Notification::event_applied("issue1".to_string(), "event1".to_string(), 1000);
        assert_eq!(n.notification_type(), "EventApplied");

        let n = Notification::wal_synced("abc123".to_string(), "origin".to_string());
        assert_eq!(n.notification_type(), "WalSynced");
    }

    #[test]
    fn test_rkyv_roundtrip() {
        let notification = Notification::EventApplied {
            issue_id: "issue123".to_string(),
            event_id: "event456".to_string(),
            ts_unix_ms: 1700000000000,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&notification).unwrap();
        let archived =
            rkyv::access::<ArchivedNotification, rkyv::rancor::Error>(&bytes).unwrap();

        match archived {
            ArchivedNotification::EventApplied {
                issue_id,
                event_id,
                ts_unix_ms,
            } => {
                assert_eq!(issue_id.as_str(), "issue123");
                assert_eq!(event_id.as_str(), "event456");
                assert_eq!(*ts_unix_ms, 1700000000000);
            }
            _ => panic!("Wrong variant"),
        }
    }
}
