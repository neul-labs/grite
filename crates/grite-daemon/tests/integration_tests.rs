//! Integration tests for grit-daemon daemon

use std::time::Duration;

use libgrite_ipc::DaemonLock;
use tempfile::tempdir;

/// Test that daemon lock can be acquired and released
#[test]
fn test_daemon_lock_lifecycle() {
    let temp = tempdir().unwrap();
    let data_dir = temp.path().to_path_buf();

    // Acquire lock
    let lock = DaemonLock::acquire(
        &data_dir,
        "/tmp/test-repo".to_string(),
        "abc123".to_string(),
        "test-host".to_string(),
        "ipc:///tmp/test.sock".to_string(),
    ).unwrap();

    // Verify lock file was created
    let lock_path = data_dir.join("daemon.lock");
    assert!(lock_path.exists());

    // Verify lock can be read
    let read_lock = DaemonLock::read(&data_dir).unwrap().unwrap();
    assert_eq!(read_lock.repo_root, "/tmp/test-repo");
    assert_eq!(read_lock.actor_id, "abc123");
    assert!(!read_lock.is_expired());

    // Release lock
    DaemonLock::release(&data_dir).unwrap();

    // Verify lock file was removed
    assert!(!lock_path.exists());
}

/// Test that lock cannot be acquired twice
#[test]
fn test_daemon_lock_double_acquire_fails() {
    let temp = tempdir().unwrap();
    let data_dir = temp.path().to_path_buf();

    // Acquire first lock
    let _lock1 = DaemonLock::acquire(
        &data_dir,
        "/tmp/test-repo".to_string(),
        "abc123".to_string(),
        "test-host".to_string(),
        "ipc:///tmp/test1.sock".to_string(),
    ).unwrap();

    // Try to acquire second lock - should fail
    let result = DaemonLock::acquire(
        &data_dir,
        "/tmp/test-repo".to_string(),
        "abc123".to_string(),
        "other-host".to_string(),
        "ipc:///tmp/test2.sock".to_string(),
    );

    assert!(result.is_err());
}

/// Test that expired lock can be taken over
#[test]
fn test_daemon_lock_expired_takeover() {
    let temp = tempdir().unwrap();
    let data_dir = temp.path().to_path_buf();

    // Create an already-expired lock
    let mut lock = DaemonLock::new(
        99999, // fake PID
        "/tmp/test-repo".to_string(),
        "abc123".to_string(),
        "old-host".to_string(),
        "ipc:///tmp/old.sock".to_string(),
    );
    // Set expires_ts to past
    lock.expires_ts = lock.started_ts - 1000;
    lock.write(&data_dir).unwrap();

    // Verify it's expired
    let read_lock = DaemonLock::read(&data_dir).unwrap().unwrap();
    assert!(read_lock.is_expired());

    // Acquire new lock - should succeed because old one expired
    let new_lock = DaemonLock::acquire(
        &data_dir,
        "/tmp/test-repo".to_string(),
        "abc123".to_string(),
        "new-host".to_string(),
        "ipc:///tmp/new.sock".to_string(),
    ).unwrap();

    // Verify new lock is in place
    let read_lock = DaemonLock::read(&data_dir).unwrap().unwrap();
    assert_eq!(read_lock.host_id, "new-host");
    assert!(!read_lock.is_expired());
}

/// Test lock heartbeat refresh
#[test]
fn test_daemon_lock_heartbeat_refresh() {
    let temp = tempdir().unwrap();
    let data_dir = temp.path().to_path_buf();

    // Acquire lock
    let mut lock = DaemonLock::acquire(
        &data_dir,
        "/tmp/test-repo".to_string(),
        "abc123".to_string(),
        "test-host".to_string(),
        "ipc:///tmp/test.sock".to_string(),
    ).unwrap();

    let original_expires = lock.expires_ts;
    let original_heartbeat = lock.last_heartbeat_ts;

    // Wait a tiny bit
    std::thread::sleep(Duration::from_millis(10));

    // Refresh
    lock.refresh();
    lock.write(&data_dir).unwrap();

    // Verify timestamps updated
    assert!(lock.last_heartbeat_ts > original_heartbeat);
    assert!(lock.expires_ts > original_expires);

    // Read back and verify
    let read_lock = DaemonLock::read(&data_dir).unwrap().unwrap();
    assert!(read_lock.last_heartbeat_ts > original_heartbeat);
}

/// Test IPC message serialization roundtrip
#[test]
fn test_ipc_message_roundtrip() {
    use libgrite_ipc::messages::{IpcRequest, IpcResponse};
    use libgrite_ipc::IpcCommand;

    // Test IssueList command
    let request = IpcRequest::new(
        "req-123".to_string(),
        "/tmp/repo".to_string(),
        "actor123".to_string(),
        "/tmp/data".to_string(),
        IpcCommand::IssueList {
            state: Some("open".to_string()),
            label: None,
        },
    );

    // Serialize
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&request).unwrap();

    // Deserialize
    let archived = rkyv::access::<rkyv::Archived<IpcRequest>, rkyv::rancor::Error>(&bytes).unwrap();
    let restored: IpcRequest = rkyv::deserialize::<IpcRequest, rkyv::rancor::Error>(archived).unwrap();

    assert_eq!(restored.request_id, "req-123");
    assert_eq!(restored.repo_root, "/tmp/repo");

    // Test response
    let response = IpcResponse::success(
        "req-123".to_string(),
        Some(r#"{"issues":[]}"#.to_string()),
    );

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&response).unwrap();
    let archived = rkyv::access::<rkyv::Archived<IpcResponse>, rkyv::rancor::Error>(&bytes).unwrap();
    let restored: IpcResponse = rkyv::deserialize::<IpcResponse, rkyv::rancor::Error>(archived).unwrap();

    assert!(restored.ok);
    assert_eq!(restored.data, Some(r#"{"issues":[]}"#.to_string()));
}

/// Test notification serialization
#[test]
fn test_notification_roundtrip() {
    use libgrite_ipc::Notification;

    let notification = Notification::EventApplied {
        issue_id: "issue123".to_string(),
        event_id: "event456".to_string(),
        ts_unix_ms: 1700000000000,
    };

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&notification).unwrap();
    let archived = rkyv::access::<rkyv::Archived<Notification>, rkyv::rancor::Error>(&bytes).unwrap();
    let restored: Notification = rkyv::deserialize::<Notification, rkyv::rancor::Error>(archived).unwrap();

    match restored {
        Notification::EventApplied { issue_id, event_id, ts_unix_ms } => {
            assert_eq!(issue_id, "issue123");
            assert_eq!(event_id, "event456");
            assert_eq!(ts_unix_ms, 1700000000000);
        }
        _ => panic!("Wrong notification type"),
    }
}
