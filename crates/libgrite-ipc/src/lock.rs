//! Daemon lock management
//!
//! The daemon lock prevents multiple processes from owning the same
//! actor data directory. It uses a lease-based approach with heartbeats.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::IpcError;
use crate::host::{host_id, process_alive};
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
    /// Unix socket path (e.g., "/tmp/grite-daemon.sock")
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

    /// Whether this lock was written by a process on the current host.
    ///
    /// PIDs are only comparable within a host, so every liveness check has to
    /// be gated on this.
    pub fn is_local_host(&self) -> bool {
        self.host_id == host_id()
    }

    /// Whether the process that wrote this lock is still running.
    ///
    /// Returns `true` for locks written on another host: we cannot verify
    /// them, so we must not assume they are dead.
    pub fn holder_alive(&self) -> bool {
        if !self.is_local_host() {
            return true;
        }
        process_alive(self.pid)
    }

    /// Whether this lock can be reclaimed.
    ///
    /// A lock is stale when its lease has run out *or* when the process that
    /// wrote it is gone. The second condition is what makes recovery from a
    /// crashed daemon immediate instead of waiting out the full lease, during
    /// which every command would otherwise fail with `db_busy`.
    pub fn is_stale(&self) -> bool {
        self.is_expired() || !self.holder_alive()
    }

    /// Whether this lock still represents a live holder.
    pub fn is_live(&self) -> bool {
        !self.is_stale()
    }

    /// Check if the lock is held by this process
    pub fn is_owned_by_current_process(&self) -> bool {
        self.pid == std::process::id()
    }

    /// Remaining time until expiration in milliseconds
    pub fn time_remaining_ms(&self) -> u64 {
        self.expires_ts.saturating_sub(current_time_ms())
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
        match fs::read_to_string(&path) {
            Ok(contents) => {
                let lock: DaemonLock = serde_json::from_str(&contents)?;
                Ok(Some(lock))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
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
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Try to acquire the lock
    ///
    /// Returns Ok(lock) if acquired, Err if held by another non-expired process.
    /// Uses atomic file creation to prevent two processes from acquiring
    /// the lock simultaneously when an expired lock is being replaced.
    pub fn acquire(
        data_dir: &Path,
        repo_root: String,
        actor_id: String,
        host_id: String,
        ipc_endpoint: String,
    ) -> Result<Self, IpcError> {
        let path = Self::lock_path(data_dir);

        // Check for existing lock
        if let Some(existing) = Self::read(data_dir)? {
            if existing.is_live() {
                return Err(IpcError::LockHeld {
                    pid: existing.pid,
                    expires_in_ms: existing.time_remaining_ms(),
                });
            }
            // Lock is stale (lease expired, or the holder process is gone)
            // — remove it so we can create exclusively.
            let _ = std::fs::remove_file(&path);
        }

        let lock = DaemonLock::new(
            std::process::id(),
            repo_root,
            actor_id,
            host_id,
            ipc_endpoint,
        );

        let contents = serde_json::to_string_pretty(&lock)?;

        // O_CREAT | O_EXCL: fails if another process created the file
        // between our remove and this create.
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(contents.as_bytes())?;
                Ok(lock)
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Another process beat us to it between our remove and create
                Err(IpcError::LockRace)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Remove the lock if it is stale, leaving live locks untouched.
    ///
    /// Returns the stale lock that was removed, if any. Used by the CLI to
    /// recover after a daemon crash without stepping on a running daemon.
    pub fn remove_if_stale(data_dir: &Path) -> Result<Option<Self>, IpcError> {
        match Self::read(data_dir)? {
            Some(lock) if lock.is_stale() => {
                Self::remove(data_dir)?;
                Ok(Some(lock))
            }
            _ => Ok(None),
        }
    }

    /// Remove the lock if nothing can be using it, and report what was removed.
    ///
    /// Stronger than [`remove_if_stale`](Self::remove_if_stale): it also
    /// clears a lock whose holder cannot be *verified* — one written on
    /// another host, or naming a PID that has since been recycled — when
    /// nothing is serving the endpoint that lock recorded. Such a lock cannot
    /// be doing its job, whoever wrote it.
    ///
    /// This is the operator escape hatch, reached only from an explicit
    /// `grite daemon stop`. Routing must stay conservative and use
    /// `remove_if_stale`, so that a daemon which is merely wedged is reported
    /// rather than having its lease pulled out from under it.
    pub fn remove_if_unusable(data_dir: &Path) -> Result<Option<Self>, IpcError> {
        match Self::read(data_dir)? {
            Some(lock) if lock.is_stale() || !crate::probe::is_listening(&lock.ipc_endpoint) => {
                Self::remove(data_dir)?;
                Ok(Some(lock))
            }
            _ => Ok(None),
        }
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
        .unwrap_or_default()
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
            "/tmp/test.sock".to_string(),
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
            "/tmp/test.sock".to_string(),
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
            "/tmp/test.sock".to_string(),
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
            "/tmp/test.sock".to_string(),
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
            "/tmp/old.sock".to_string(),
        );
        old_lock.expires_ts = 0; // Expired
        old_lock.write(data_dir).unwrap();

        // Should be able to acquire over expired lock
        let new_lock = DaemonLock::acquire(
            data_dir,
            "/repo".to_string(),
            "actor".to_string(),
            "host".to_string(),
            "/tmp/new.sock".to_string(),
        )
        .unwrap();

        assert!(new_lock.is_owned_by_current_process());
    }

    /// Spawn and reap a process so we have a PID that is certainly dead.
    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true")
            .spawn()
            .expect("spawn /usr/bin/true");
        let pid = child.id();
        child.wait().expect("wait for child");
        pid
    }

    #[test]
    fn test_lock_with_dead_holder_is_stale_before_lease_expiry() {
        let lock = DaemonLock::new(
            dead_pid(),
            "/repo".to_string(),
            "actor".to_string(),
            crate::host::host_id(),
            "/tmp/test.sock".to_string(),
        );

        // The lease is still valid on the clock...
        assert!(!lock.is_expired(), "lease should not have expired yet");
        // ...but the holder is gone, so the lock must not be trusted.
        assert!(!lock.holder_alive());
        assert!(lock.is_stale());
        assert!(!lock.is_live());
    }

    #[test]
    fn test_lock_with_live_holder_is_not_stale() {
        let lock = DaemonLock::new(
            std::process::id(),
            "/repo".to_string(),
            "actor".to_string(),
            crate::host::host_id(),
            "/tmp/test.sock".to_string(),
        );

        assert!(lock.is_live());
        assert!(!lock.is_stale());
    }

    #[test]
    fn test_lock_from_foreign_host_is_not_probed_for_liveness() {
        // A PID from another machine is meaningless here; the lock must be
        // honoured until its lease runs out rather than assumed dead.
        let lock = DaemonLock::new(
            dead_pid(),
            "/repo".to_string(),
            "actor".to_string(),
            "some-other-machine".to_string(),
            "/tmp/test.sock".to_string(),
        );

        assert!(!lock.is_local_host());
        assert!(lock.holder_alive());
        assert!(lock.is_live());
    }

    #[test]
    fn test_acquire_reclaims_lock_held_by_dead_process() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();

        // A crashed daemon leaves a lock with a live lease but a dead PID.
        DaemonLock::new(
            dead_pid(),
            "/repo".to_string(),
            "actor".to_string(),
            crate::host::host_id(),
            "/tmp/old.sock".to_string(),
        )
        .write(data_dir)
        .unwrap();

        let new_lock = DaemonLock::acquire(
            data_dir,
            "/repo".to_string(),
            "actor".to_string(),
            crate::host::host_id(),
            "/tmp/new.sock".to_string(),
        )
        .expect("should reclaim a lock whose holder is dead");

        assert!(new_lock.is_owned_by_current_process());
        assert_eq!(new_lock.ipc_endpoint, "/tmp/new.sock");
    }

    #[test]
    fn test_acquire_refuses_lock_held_by_live_process() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();

        DaemonLock::new(
            std::process::id(),
            "/repo".to_string(),
            "actor".to_string(),
            crate::host::host_id(),
            "/tmp/live.sock".to_string(),
        )
        .write(data_dir)
        .unwrap();

        let result = DaemonLock::acquire(
            data_dir,
            "/repo".to_string(),
            "actor".to_string(),
            crate::host::host_id(),
            "/tmp/new.sock".to_string(),
        );

        assert!(matches!(result, Err(IpcError::LockHeld { .. })));
    }

    #[test]
    fn test_remove_if_stale_keeps_live_lock_and_drops_dead_one() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();

        // Live holder: must survive.
        DaemonLock::new(
            std::process::id(),
            "/repo".to_string(),
            "actor".to_string(),
            crate::host::host_id(),
            "/tmp/live.sock".to_string(),
        )
        .write(data_dir)
        .unwrap();
        assert!(DaemonLock::remove_if_stale(data_dir).unwrap().is_none());
        assert!(DaemonLock::read(data_dir).unwrap().is_some());

        // Dead holder: must be cleaned up and reported.
        DaemonLock::new(
            dead_pid(),
            "/repo".to_string(),
            "actor".to_string(),
            crate::host::host_id(),
            "/tmp/dead.sock".to_string(),
        )
        .write(data_dir)
        .unwrap();
        let removed = DaemonLock::remove_if_stale(data_dir).unwrap();
        assert_eq!(
            removed.map(|l| l.ipc_endpoint).as_deref(),
            Some("/tmp/dead.sock")
        );
        assert!(DaemonLock::read(data_dir).unwrap().is_none());
    }

    #[test]
    fn test_remove_if_unusable_clears_a_foreign_host_lock_with_no_listener() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();

        // A lock we cannot verify: another host, live lease, live-looking PID.
        // Nothing is serving its endpoint, so it cannot be doing its job.
        DaemonLock::new(
            std::process::id(),
            "/repo".to_string(),
            "actor".to_string(),
            "some-other-machine".to_string(),
            "/tmp/grite-nothing-here-8c1d.sock".to_string(),
        )
        .write(data_dir)
        .unwrap();

        // The conservative path leaves it alone...
        assert!(DaemonLock::remove_if_stale(data_dir).unwrap().is_none());
        assert!(DaemonLock::read(data_dir).unwrap().is_some());

        // ...and the operator escape hatch clears it.
        let removed = DaemonLock::remove_if_unusable(data_dir).unwrap();
        assert!(removed.is_some(), "an unreachable lock must be clearable");
        assert!(DaemonLock::read(data_dir).unwrap().is_none());
    }

    #[test]
    fn test_remove_if_unusable_keeps_a_lock_whose_endpoint_answers() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();
        let socket = temp.path().join("live.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();

        DaemonLock::new(
            std::process::id(),
            "/repo".to_string(),
            "actor".to_string(),
            crate::host::host_id(),
            socket.to_str().unwrap().to_string(),
        )
        .write(data_dir)
        .unwrap();

        assert!(DaemonLock::remove_if_unusable(data_dir).unwrap().is_none());
        assert!(DaemonLock::read(data_dir).unwrap().is_some());
    }

    #[test]
    fn test_custom_lease() {
        let lock = DaemonLock::new(
            1234,
            "/repo".to_string(),
            "actor".to_string(),
            "host".to_string(),
            "/tmp/test.sock".to_string(),
        )
        .with_lease(60_000); // 60 seconds

        assert_eq!(lock.lease_ms, 60_000);
    }
}
