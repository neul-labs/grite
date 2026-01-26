//! Daemon lock management
//!
//! The daemon lock prevents multiple processes from owning the same
//! actor data directory. It uses a lease-based approach with heartbeats.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::IpcError;
use crate::DEFAULT_LEASE_MS;

/// Daemon lock stored at `.git/grite/actors/<actor_id>/daemon.lock`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLock {
    /// Process ID of the lock holder
    pub pid: u32,
    /// When the daemon started (Unix timestamp in ms)
    pub started_ts: u64,
    /// Repository root path
    pub repo_root: String,
    /// Actor ID (hex-encoded)
    pub actor_id: String,
    /// Stable host identifier
    pub host_id: String,
    /// IPC endpoint (e.g., "ipc:///tmp/grite-daemon.sock")
    pub ipc_endpoint: String,
    /// Lease duration in milliseconds
    pub lease_ms: u64,
    /// Last heartbeat timestamp (Unix timestamp in ms)
    pub last_heartbeat_ts: u64,
    /// When the lock expires (Unix timestamp in ms)
    pub expires_ts: u64,
}

impl DaemonLock {
    /// Create a new daemon lock
    pub fn new(
        pid: u32,
        repo_root: String,
        actor_id: String,
        host_id: String,
        ipc_endpoint: String,
    ) -> Self {
        let now = current_time_ms();
        Self {
            pid,
            started_ts: now,
            repo_root,
            actor_id,
            host_id,
            ipc_endpoint,
            lease_ms: DEFAULT_LEASE_MS,
            last_heartbeat_ts: now,
            expires_ts: now + DEFAULT_LEASE_MS,
        }
    }

    /// Create a lock with a custom lease duration
    pub fn with_lease(mut self, lease_ms: u64) -> Self {
        let now = current_time_ms();
        self.lease_ms = lease_ms;
        self.expires_ts = now + lease_ms;
        self
    }

    /// Check if the lock has expired
    pub fn is_expired(&self) -> bool {
        current_time_ms() > self.expires_ts
    }

    /// Check if the lock is held by this process
    pub fn is_owned_by_current_process(&self) -> bool {
        self.pid == std::process::id()
    }

    /// Remaining time until expiration in milliseconds
    pub fn time_remaining_ms(&self) -> u64 {
        let now = current_time_ms();
        if now >= self.expires_ts {
            0
        } else {
            self.expires_ts - now
        }
    }

    /// Refresh the heartbeat and extend the lease
    pub fn refresh(&mut self) {
        let now = current_time_ms();
        self.last_heartbeat_ts = now;
        self.expires_ts = now + self.lease_ms;
    }

    /// Get the lock file path for an actor data directory
    pub fn lock_path(data_dir: &Path) -> PathBuf {
        data_dir.join("daemon.lock")
    }

    /// Read a lock from the filesystem
    pub fn read(data_dir: &Path) -> Result<Option<Self>, IpcError> {
        let path = Self::lock_path(data_dir);
        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path)?;
        let lock: DaemonLock = serde_json::from_str(&contents)?;
        Ok(Some(lock))
    }

    /// Write the lock to the filesystem
    pub fn write(&self, data_dir: &Path) -> Result<(), IpcError> {
        let path = Self::lock_path(data_dir);
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }

    /// Remove the lock file
    pub fn remove(data_dir: &Path) -> Result<(), IpcError> {
        let path = Self::lock_path(data_dir);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Try to acquire the lock
    ///
    /// Returns Ok(lock) if acquired, Err if held by another non-expired process
    pub fn acquire(
        data_dir: &Path,
        repo_root: String,
        actor_id: String,
        host_id: String,
        ipc_endpoint: String,
    ) -> Result<Self, IpcError> {
        // Check for existing lock
        if let Some(existing) = Self::read(data_dir)? {
            if !existing.is_expired() {
                return Err(IpcError::LockHeld {
                    pid: existing.pid,
                    expires_in_ms: existing.time_remaining_ms(),
                });
            }
            // Lock is expired, we can take it
        }

        let lock = DaemonLock::new(
            std::process::id(),
            repo_root,
            actor_id,
            host_id,
            ipc_endpoint,
        );
        lock.write(data_dir)?;
        Ok(lock)
    }

    /// Release the lock (only if owned by current process)
    pub fn release(data_dir: &Path) -> Result<(), IpcError> {
        if let Some(lock) = Self::read(data_dir)? {
            if lock.is_owned_by_current_process() {
                Self::remove(data_dir)?;
            }
        }
        Ok(())
    }
}

/// Get current time in milliseconds since Unix epoch
fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_lock_creation() {
        let lock = DaemonLock::new(
            1234,
            "/repo".to_string(),
            "actor123".to_string(),
            "host456".to_string(),
            "ipc:///tmp/test.sock".to_string(),
        );

        assert_eq!(lock.pid, 1234);
        assert_eq!(lock.repo_root, "/repo");
        assert_eq!(lock.actor_id, "actor123");
        assert!(!lock.is_expired());
    }

    #[test]
    fn test_lock_expiration() {
        let mut lock = DaemonLock::new(
            1234,
            "/repo".to_string(),
            "actor".to_string(),
            "host".to_string(),
            "ipc:///tmp/test.sock".to_string(),
        );

        // Set expiration to the past
        lock.expires_ts = 0;
        assert!(lock.is_expired());

        // Refresh should extend the lease
        lock.refresh();
        assert!(!lock.is_expired());
    }

    #[test]
    fn test_lock_read_write() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();

        let lock = DaemonLock::new(
            std::process::id(),
            "/repo".to_string(),
            "actor123".to_string(),
            "host456".to_string(),
            "ipc:///tmp/test.sock".to_string(),
        );

        // Write
        lock.write(data_dir).unwrap();

        // Read back
        let read_lock = DaemonLock::read(data_dir).unwrap().unwrap();
        assert_eq!(read_lock.pid, lock.pid);
        assert_eq!(read_lock.actor_id, lock.actor_id);
    }

    #[test]
    fn test_lock_acquire_release() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();

        // Acquire lock
        let lock = DaemonLock::acquire(
            data_dir,
            "/repo".to_string(),
            "actor".to_string(),
            "host".to_string(),
            "ipc:///tmp/test.sock".to_string(),
        )
        .unwrap();

        assert!(lock.is_owned_by_current_process());

        // Release lock
        DaemonLock::release(data_dir).unwrap();

        // Lock should be gone
        assert!(DaemonLock::read(data_dir).unwrap().is_none());
    }

    #[test]
    fn test_lock_acquire_expired() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();

        // Create an expired lock
        let mut old_lock = DaemonLock::new(
            9999, // Different PID
            "/repo".to_string(),
            "actor".to_string(),
            "host".to_string(),
            "ipc:///tmp/old.sock".to_string(),
        );
        old_lock.expires_ts = 0; // Expired
        old_lock.write(data_dir).unwrap();

        // Should be able to acquire over expired lock
        let new_lock = DaemonLock::acquire(
            data_dir,
            "/repo".to_string(),
            "actor".to_string(),
            "host".to_string(),
            "ipc:///tmp/new.sock".to_string(),
        )
        .unwrap();

        assert!(new_lock.is_owned_by_current_process());
    }

    #[test]
    fn test_custom_lease() {
        let lock = DaemonLock::new(
            1234,
            "/repo".to_string(),
            "actor".to_string(),
            "host".to_string(),
            "ipc:///tmp/test.sock".to_string(),
        )
        .with_lease(60_000); // 60 seconds

        assert_eq!(lock.lease_ms, 60_000);
    }
}
