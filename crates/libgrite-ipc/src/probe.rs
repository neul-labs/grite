//! The daemon liveness oracle.
//!
//! Every caller that needs to know whether a daemon is usable must ask this
//! module, so that `daemon status`, `daemon stop` and command routing cannot
//! disagree. Two weaker oracles were tried and both are wrong:
//!
//! - A lock file says nothing: it is written lazily by the first worker, and
//!   it outlives a daemon that crashed.
//! - A bare `connect()` says almost nothing: the kernel completes the
//!   handshake from the listen backlog, so a daemon that is stopped, swapped
//!   out, or blocked on its worker mutex still accepts connections while
//!   answering nothing.
//!
//! Only a completed request/response round-trip proves the daemon is making
//! progress, so that is what [`probe_daemon`] does.

use std::time::Duration;

use crate::client::IpcClient;
use crate::messages::{IpcCommand, IpcRequest};

/// Budget for a liveness probe.
///
/// Generous enough that a busy-but-healthy daemon on a loaded machine is not
/// declared dead, and short enough to stay off the critical path: a healthy
/// daemon answers in well under a millisecond.
pub const PROBE_TIMEOUT_MS: u64 = 5_000;

/// A daemon's self-reported identity and state.
#[derive(Debug, Clone)]
pub struct DaemonInfo {
    pub pid: u32,
    pub daemon_id: String,
    pub host_id: String,
    pub endpoint: String,
    pub started_ts: u64,
    pub worker_count: u64,
    pub state: String,
}

/// Whether *something* holds the listening end of this endpoint.
///
/// This is deliberately weaker than [`probe_daemon`] and answers a different
/// question: not "is the daemon healthy" but "would binding here collide, and
/// is this socket file safe to unlink". Never use it to decide routing.
pub fn is_listening(endpoint: &str) -> bool {
    std::os::unix::net::UnixStream::connect(endpoint).is_ok()
}

/// Ask the daemon at `endpoint` to identify itself, within [`PROBE_TIMEOUT_MS`].
pub fn probe_daemon(endpoint: &str) -> Option<DaemonInfo> {
    probe_daemon_with_timeout(endpoint, Duration::from_millis(PROBE_TIMEOUT_MS))
}

/// Ask the daemon at `endpoint` to identify itself, within `timeout`.
///
/// Returns `None` for every flavour of unusable: nothing listening, a stale
/// socket file, a peer that accepts but never answers, a protocol mismatch.
/// `DaemonStatus` is answered by the supervisor itself and never touches a
/// worker or the sled store, so a healthy daemon always answers it promptly
/// even while workers are busy.
pub fn probe_daemon_with_timeout(endpoint: &str, timeout: Duration) -> Option<DaemonInfo> {
    let timeout_ms = timeout.as_millis().min(u128::from(u64::MAX)) as u64;
    let mut client = IpcClient::connect_with_timeout(endpoint, timeout_ms).ok()?;

    let request = IpcRequest::new(
        uuid::Uuid::new_v4().to_string(),
        String::new(),
        String::new(),
        String::new(),
        IpcCommand::DaemonStatus,
    );

    let response = client.send(&request).ok()?;
    let value: serde_json::Value = serde_json::from_str(&response.data?).ok()?;

    Some(DaemonInfo {
        pid: value.get("pid")?.as_u64()? as u32,
        daemon_id: json_str(&value, "daemon_id"),
        host_id: json_str(&value, "host_id"),
        endpoint: value
            .get("ipc_endpoint")
            .and_then(|v| v.as_str())
            .unwrap_or(endpoint)
            .to_string(),
        started_ts: value
            .get("started_ts")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        worker_count: value
            .get("worker_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        state: json_str(&value, "state"),
    })
}

fn json_str(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_of_a_missing_endpoint_is_none() {
        assert!(probe_daemon("/tmp/grite-definitely-not-here-9f2c.sock").is_none());
        assert!(!is_listening("/tmp/grite-definitely-not-here-9f2c.sock"));
    }

    #[test]
    fn probe_of_a_plain_file_is_none() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("not-a-socket");
        std::fs::write(&path, b"").unwrap();
        let path = path.to_str().unwrap();

        assert!(!is_listening(path));
        assert!(probe_daemon(path).is_none());
    }

    /// A listener that accepts connections and then says nothing stands in for
    /// a daemon that is stopped, swapped out, or blocked on a lock. It must
    /// read as *not usable*, even though `connect()` succeeds.
    #[test]
    fn listening_but_silent_peer_is_not_a_live_daemon() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("silent.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let path = path.to_str().unwrap().to_string();

        // Hold accepted connections open without ever replying.
        let accepted = std::thread::spawn(move || {
            let mut held = Vec::new();
            for stream in listener.incoming().take(1) {
                held.push(stream);
            }
            held
        });

        assert!(is_listening(&path), "connect() must succeed here");
        assert!(
            probe_daemon_with_timeout(&path, Duration::from_millis(300)).is_none(),
            "a peer that never answers must not read as a live daemon"
        );

        drop(accepted);
    }
}
