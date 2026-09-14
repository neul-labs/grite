//! Regression tests for daemon lifecycle: readiness, liveness reporting,
//! stale-state recovery, and concurrent access from linked worktrees.
//!
//! These tests drive the real `grite` and `grite-daemon` binaries. Each test
//! gets its own Unix socket via `GRITE_DAEMON_SOCKET`, so they never touch
//! the developer's daemon or each other's.
//!
//! `grite-daemon` lives in a different package, so `CARGO_BIN_EXE_*` is not
//! available for it. It is located next to the `grite` binary instead, which
//! holds under `cargo test --all` (what CI runs) and `cargo test` at the
//! workspace root. Running `cargo test -p grite` alone will not have built
//! it; the tests say so rather than silently passing.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use tempfile::TempDir;

/// Unique suffix per socket. Unix socket paths are capped near 104 bytes on
/// macOS, so these stay short and live directly in /tmp.
static SOCKET_SEQ: AtomicU32 = AtomicU32::new(0);

fn grite_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_grite"))
}

fn daemon_bin() -> PathBuf {
    let path = grite_bin()
        .parent()
        .expect("grite binary has a parent directory")
        .join("grite-daemon");
    assert!(
        path.exists(),
        "grite-daemon binary not found at {}. Build the whole workspace \
         (`cargo test --all`, as CI does) so the daemon binary exists.",
        path.display()
    );
    path
}

/// An isolated repository plus its own daemon endpoint.
struct Fixture {
    _temp: TempDir,
    main: PathBuf,
    socket: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().expect("create temp dir");
        let main = temp.path().join("main");
        std::fs::create_dir_all(&main).expect("create repo dir");

        git(&["init", "-q"], &main);
        git(&["config", "user.email", "test@example.com"], &main);
        git(&["config", "user.name", "Test"], &main);
        git(&["commit", "-q", "--allow-empty", "-m", "init"], &main);

        let seq = SOCKET_SEQ.fetch_add(1, Ordering::SeqCst);
        let socket = PathBuf::from(format!("/tmp/grite-t{}-{}.sock", std::process::id(), seq));
        let _ = std::fs::remove_file(&socket);

        let fixture = Self {
            _temp: temp,
            main,
            socket,
        };
        fixture.grite(&["init"], None).assert_success("grite init");
        fixture
    }

    /// The shared git directory, where grite keeps its state.
    fn grite_dir(&self) -> PathBuf {
        self.main.join(".git").join("grite")
    }

    fn lock_path(&self) -> PathBuf {
        self.grite_dir().join("daemon.lock")
    }

    /// Add a linked worktree with its own actor, returning (path, actor_id).
    fn add_worktree(&self, name: &str) -> (PathBuf, String) {
        let path = self.main.parent().unwrap().join(name);
        git(
            &["worktree", "add", "-q", path.to_str().unwrap(), "-b", name],
            &self.main,
        );

        let out = self.grite_in(&path, &["actor", "init", "--label", name], None);
        out.assert_success("actor init");
        let actor_id = json(&out)["data"]["actor_id"]
            .as_str()
            .expect("actor_id in output")
            .to_string();
        (path, actor_id)
    }

    fn grite(&self, args: &[&str], socket_override: Option<&Path>) -> Output {
        self.grite_in(&self.main.clone(), args, socket_override)
    }

    /// Run `grite --json <args>` in `cwd` against this fixture's endpoint.
    fn grite_in(&self, cwd: &Path, args: &[&str], socket_override: Option<&Path>) -> Output {
        let socket = socket_override.unwrap_or(&self.socket);
        Command::new(grite_bin())
            .arg("--json")
            .args(args)
            .current_dir(cwd)
            .env("GRITE_DAEMON_SOCKET", socket)
            .env("GRITE_DAEMON_BIN", daemon_bin())
            .env_remove("XDG_RUNTIME_DIR")
            .output()
            .expect("run grite")
    }

    fn status(&self) -> serde_json::Value {
        json(&self.grite(&["daemon", "status"], None))
    }

    fn stop(&self) {
        let _ = self.grite(&["daemon", "stop"], None);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Never leave a daemon behind holding a temp directory open.
        let _ = Command::new(grite_bin())
            .args(["--json", "--quiet", "daemon", "stop"])
            .current_dir(&self.main)
            .env("GRITE_DAEMON_SOCKET", &self.socket)
            .env("GRITE_DAEMON_BIN", daemon_bin())
            .output();
        let _ = std::fs::remove_file(&self.socket);
    }
}

fn git(args: &[&str], dir: &Path) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn json(out: &Output) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "expected JSON on stdout, got {:?} (stderr: {}) — {}",
            stdout,
            String::from_utf8_lossy(&out.stderr),
            e
        )
    })
}

trait OutputExt {
    fn assert_success(&self, what: &str);
    fn stderr_text(&self) -> String;
}

impl OutputExt for Output {
    fn assert_success(&self, what: &str) {
        assert!(
            self.status.success(),
            "{} failed ({}): stdout={} stderr={}",
            what,
            self.status,
            String::from_utf8_lossy(&self.stdout),
            String::from_utf8_lossy(&self.stderr)
        );
    }

    fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).to_string()
    }
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    cond()
}

// ---------------------------------------------------------------------------
// 1. Start readiness handshake
// ---------------------------------------------------------------------------

/// `daemon start` must not report success until the daemon answers IPC.
///
/// Regression: readiness was a bare socket connect, and success was reported
/// with the *spawned child's* PID rather than the PID of whatever ended up
/// serving the endpoint.
#[test]
fn start_reports_ready_only_when_daemon_answers_ipc() {
    let fx = Fixture::new();

    let out = fx.grite(&["daemon", "start"], None);
    out.assert_success("daemon start");
    let started = json(&out);

    assert_eq!(started["started"], true);
    assert_eq!(started["ready"], true);
    assert_eq!(started["endpoint"], fx.socket.to_str().unwrap());

    let pid = started["pid"].as_u64().expect("pid in start output") as u32;
    assert!(pid_alive(pid), "reported PID {pid} should be running");

    // The daemon must be answering right now, with no further commands run.
    let status = fx.status();
    assert_eq!(status["running"], true);
    assert_eq!(status["pid"].as_u64().unwrap() as u32, pid);

    fx.stop();
}

/// Starting twice reports the already-running daemon rather than a second one.
#[test]
fn start_is_idempotent_and_reports_the_live_daemon() {
    let fx = Fixture::new();

    let first = json(&fx.grite(&["daemon", "start"], None));
    let pid = first["pid"].as_u64().unwrap();

    let second = json(&fx.grite(&["daemon", "start"], None));
    assert_eq!(second["started"], false);
    assert_eq!(second["ready"], true);
    assert_eq!(second["reason"], "Daemon already running");
    assert_eq!(
        second["pid"].as_u64().unwrap(),
        pid,
        "second start must report the original daemon"
    );

    fx.stop();
}

// ---------------------------------------------------------------------------
// 2. Status immediately after start
// ---------------------------------------------------------------------------

/// The original bug: `start` returned `{"started":true}` and the very next
/// `status` returned `{"running":false}`, because status was derived from a
/// `daemon.lock` file that only gets written when the first repo-scoped
/// command creates a worker.
#[test]
fn status_is_true_immediately_after_start_with_no_lock_file() {
    let fx = Fixture::new();

    let started = json(&fx.grite(&["daemon", "start"], None));
    assert_eq!(started["started"], true);

    let status = fx.status();
    assert_eq!(
        status["running"], true,
        "status must reflect the live daemon, not the absence of a lock file"
    );

    // No worker has run yet, so there is deliberately no lease on disk. That
    // must not change the verdict.
    assert!(
        !fx.lock_path().exists(),
        "no worker has run, so no lease should exist yet"
    );
    assert_eq!(status["lock"]["present"], false);
    assert_eq!(status["worker_count"].as_u64().unwrap(), 0);

    fx.stop();
}

/// A sibling linked worktree sees the same daemon at the same moment — there
/// is no window where one worktree sees it and another does not.
#[test]
fn sibling_worktree_sees_the_daemon_immediately() {
    let fx = Fixture::new();
    let (wt, actor) = fx.add_worktree("wt-sibling");

    let started = json(&fx.grite(&["daemon", "start"], None));
    let pid = started["pid"].as_u64().unwrap();

    let sibling = json(&fx.grite_in(&wt, &["--actor", &actor, "daemon", "status"], None));
    assert_eq!(sibling["running"], true);
    assert_eq!(sibling["pid"].as_u64().unwrap(), pid);

    fx.stop();
}

// ---------------------------------------------------------------------------
// 3. Daemon exits during startup
// ---------------------------------------------------------------------------

/// When the daemon dies while coming up, `start` must fail with the cause,
/// not report success and not time out with a generic message.
#[test]
fn start_fails_with_cause_when_daemon_exits_during_startup() {
    let fx = Fixture::new();

    // An endpoint under a directory that does not exist cannot be bound.
    let unbindable = PathBuf::from("/tmp/grite-no-such-dir-xyz/daemon.sock");
    let out = fx.grite(&["daemon", "start"], Some(&unbindable));

    assert!(
        !out.status.success(),
        "start must fail when the daemon cannot bind: stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );

    let stderr = out.stderr_text();
    assert!(
        stderr.contains("exited during startup"),
        "error should say the child exited, got: {stderr}"
    );
    assert!(
        stderr.contains("Failed to bind") || stderr.contains("last log lines"),
        "error should carry the underlying bind failure, got: {stderr}"
    );
}

/// A daemon that fails to bind must exit non-zero so its supervisor can tell
/// "died" from "still starting".
#[test]
fn daemon_exits_non_zero_when_it_cannot_bind() {
    let out = Command::new(daemon_bin())
        .args(["--endpoint", "/tmp/grite-no-such-dir-xyz/daemon.sock"])
        .output()
        .expect("run grite-daemon");

    assert_eq!(out.status.code(), Some(1), "bind failure must exit 1");
    assert!(
        out.stderr_text().contains("Failed to bind"),
        "stderr should name the bind failure, got: {}",
        out.stderr_text()
    );
}

// ---------------------------------------------------------------------------
// 4. Stale lock / socket recovery
// ---------------------------------------------------------------------------

/// After the daemon is killed, its lease survives on disk with a valid
/// expiry. Status must not report that dead PID as running.
#[test]
fn status_does_not_report_a_dead_daemon_as_running() {
    let fx = Fixture::new();

    let started = json(&fx.grite(&["daemon", "start"], None));
    let pid = started["pid"].as_u64().unwrap() as u32;

    // Create a worker so the lease actually gets written.
    fx.grite(&["issue", "list"], None)
        .assert_success("issue list");
    assert!(
        fx.lock_path().exists(),
        "worker should have written a lease"
    );

    kill9(pid);
    assert!(wait_until(Duration::from_secs(5), || !pid_alive(pid)));

    let status = fx.status();
    assert_eq!(
        status["running"], false,
        "a killed daemon must not be reported as running"
    );
    // The lease is still there and not yet expired — which is exactly the
    // state that used to produce a false "running: true".
    assert_eq!(status["lock"]["present"], true);
    assert_eq!(status["lock"]["expired"], false);
    assert_eq!(status["lock"]["holder_alive"], false);
    assert_eq!(status["lock"]["stale"], true);
}

/// A crashed daemon must not block the next command with `db_busy` for the
/// remainder of its lease.
#[test]
fn commands_recover_immediately_after_a_daemon_crash() {
    let fx = Fixture::new();

    json(&fx.grite(&["daemon", "start"], None));
    fx.grite(&["issue", "create", "--title", "before crash"], None)
        .assert_success("issue create");

    let pid = fx.status()["pid"].as_u64().unwrap() as u32;
    kill9(pid);
    assert!(wait_until(Duration::from_secs(5), || !pid_alive(pid)));

    // Stale lease still on disk, well inside its 30s lease window.
    assert!(fx.lock_path().exists());

    let out = fx.grite(&["issue", "create", "--title", "after crash"], None);
    out.assert_success("issue create after crash");
    let stderr = out.stderr_text();
    assert!(
        !stderr.contains("db_busy") && !stderr.contains("--no-daemon"),
        "recovery must not degrade into a --no-daemon suggestion, got: {stderr}"
    );

    // And a fresh daemon is serving again.
    assert_eq!(fx.status()["running"], true);

    fx.stop();
}

/// A leftover socket file with nothing behind it must not stop a new daemon.
#[test]
fn start_recovers_from_a_stale_socket_file() {
    let fx = Fixture::new();

    // A plain file where the socket belongs: nothing is listening on it.
    std::fs::write(&fx.socket, b"").expect("create stale socket file");
    assert!(fx.socket.exists());

    let started = json(&fx.grite(&["daemon", "start"], None));
    assert_eq!(started["started"], true);
    assert_eq!(fx.status()["running"], true);

    fx.stop();
}

/// `daemon stop` on a dead daemon cleans up rather than leaving the mess.
#[test]
fn stop_cleans_up_stale_state_when_the_daemon_is_gone() {
    let fx = Fixture::new();

    json(&fx.grite(&["daemon", "start"], None));
    fx.grite(&["issue", "list"], None)
        .assert_success("issue list");
    let pid = fx.status()["pid"].as_u64().unwrap() as u32;

    kill9(pid);
    assert!(wait_until(Duration::from_secs(5), || !pid_alive(pid)));
    assert!(fx.lock_path().exists());

    let stopped = json(&fx.grite(&["daemon", "stop"], None));
    assert_eq!(stopped["stopped"], false);
    assert_eq!(stopped["reason"], "Daemon not running");
    assert_eq!(stopped["cleaned_stale_lock"], true);
    assert!(
        !fx.lock_path().exists(),
        "stale lease should have been removed"
    );
}

// ---------------------------------------------------------------------------
// 5. Concurrent writes from two linked worktrees with distinct actors
// ---------------------------------------------------------------------------

/// Sixteen simultaneous writes, split across two linked worktrees with
/// distinct actors, must all land through the one daemon.
///
/// Run directly against sled (`--no-daemon`) these contend on the same store
/// and most fail with `db_busy`; that is what the daemon exists to prevent,
/// so the daemon path must not silently degrade into it.
#[test]
fn concurrent_writes_from_two_worktrees_all_succeed() {
    let fx = Fixture::new();
    let (wt_a, actor_a) = fx.add_worktree("wt-a");
    let (wt_b, actor_b) = fx.add_worktree("wt-b");

    json(&fx.grite(&["daemon", "start"], None));

    const PER_WORKTREE: usize = 8;
    let mut children = Vec::new();

    for i in 0..PER_WORKTREE {
        for (cwd, actor) in [(&wt_a, &actor_a), (&wt_b, &actor_b)] {
            let title = format!("{}-{}", actor, i);
            children.push(
                Command::new(grite_bin())
                    .args([
                        "--json", "--quiet", "--actor", actor, "issue", "create", "--title",
                    ])
                    .arg(&title)
                    .current_dir(cwd)
                    .env("GRITE_DAEMON_SOCKET", &fx.socket)
                    .env("GRITE_DAEMON_BIN", daemon_bin())
                    // Fail loudly instead of degrading to in-process access.
                    .env("GRITE_REQUIRE_DAEMON", "1")
                    .env_remove("XDG_RUNTIME_DIR")
                    .spawn()
                    .expect("spawn grite issue create"),
            );
        }
    }

    let mut failures = Vec::new();
    for child in children {
        let out = child.wait_with_output().expect("wait for grite");
        if !out.status.success() {
            failures.push(format!(
                "status={} stdout={} stderr={}",
                out.status,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} concurrent writes failed:\n{}",
        failures.len(),
        PER_WORKTREE * 2,
        failures.join("\n")
    );

    // Every write is visible, and only one daemon served them all.
    let listed = json(&fx.grite(&["issue", "list"], None));
    let issues = listed["issues"]
        .as_array()
        .expect("daemon returns an issues array");
    assert_eq!(
        issues.len(),
        PER_WORKTREE * 2,
        "every concurrent write should be readable back"
    );

    assert_eq!(
        fx.status()["worker_count"].as_u64().unwrap(),
        1,
        "linked worktrees of one repository share a single worker"
    );

    fx.stop();
}

/// Two worktrees, distinct actors, both get a working daemon route without
/// either having to start it explicitly.
#[test]
fn either_worktree_can_auto_start_the_shared_daemon() {
    let fx = Fixture::new();
    let (wt_a, actor_a) = fx.add_worktree("wt-auto-a");
    let (wt_b, actor_b) = fx.add_worktree("wt-auto-b");

    // No explicit `daemon start` anywhere.
    fx.grite_in(
        &wt_a,
        &["--actor", &actor_a, "issue", "create", "--title", "a"],
        None,
    )
    .assert_success("create from worktree a");

    let from_b = fx.grite_in(&wt_b, &["--actor", &actor_b, "daemon", "status"], None);
    assert_eq!(json(&from_b)["running"], true);

    fx.stop();
}

// ---------------------------------------------------------------------------
// 6. Graceful stop and restart
// ---------------------------------------------------------------------------

/// Stop must actually reap the process, clear the socket, and leave the
/// repository in a state a later start can use.
#[test]
fn stop_then_start_gives_a_new_live_daemon() {
    let fx = Fixture::new();

    let first_pid = json(&fx.grite(&["daemon", "start"], None))["pid"]
        .as_u64()
        .unwrap() as u32;
    fx.grite(&["issue", "create", "--title", "before restart"], None)
        .assert_success("issue create");

    let stopped = json(&fx.grite(&["daemon", "stop"], None));
    assert_eq!(stopped["stopped"], true);
    assert_eq!(stopped["pid"].as_u64().unwrap() as u32, first_pid);
    assert!(
        wait_until(Duration::from_secs(5), || !pid_alive(first_pid)),
        "daemon PID {first_pid} should be gone after stop"
    );
    assert_eq!(fx.status()["running"], false);
    assert!(
        !fx.lock_path().exists(),
        "a clean stop should leave no lease behind"
    );
    assert!(
        !fx.socket.exists(),
        "a clean stop should remove its socket file"
    );

    let second_pid = json(&fx.grite(&["daemon", "start"], None))["pid"]
        .as_u64()
        .unwrap() as u32;
    assert_ne!(second_pid, first_pid, "restart should be a new process");
    assert_eq!(fx.status()["running"], true);

    // State written before the restart survived — the WAL, not the cache,
    // is authoritative.
    let listed = json(&fx.grite(&["issue", "list"], None));
    assert_eq!(listed["issues"].as_array().unwrap().len(), 1);

    fx.stop();
}

/// Stopping when nothing is running is not an error and says so plainly.
#[test]
fn stop_when_not_running_is_a_clean_no_op() {
    let fx = Fixture::new();

    let out = fx.grite(&["daemon", "stop"], None);
    out.assert_success("daemon stop with no daemon");
    let stopped = json(&out);
    assert_eq!(stopped["stopped"], false);
    assert_eq!(stopped["reason"], "Daemon not running");
    assert_eq!(stopped["cleaned_stale_lock"], false);
}

// ---------------------------------------------------------------------------
// 7. A daemon that is listening but not answering
// ---------------------------------------------------------------------------
//
// The kernel completes a connect() from the listen backlog, so a daemon that
// is stopped, paged out, or blocked on its worker mutex still accepts
// connections while answering nothing. An earlier revision decided routing on
// a bare connect() while `status` did a full round-trip, so the two disagreed
// about the same daemon: `status` called it dead and the next command routed
// to it and hung.

/// A bare listener with nobody serving it: the cheap, deterministic stand-in
/// for a wedged daemon.
fn occupy(path: &Path) -> std::os::unix::net::UnixListener {
    std::os::unix::net::UnixListener::bind(path).expect("bind stand-in listener")
}

#[test]
fn status_reports_a_listening_but_silent_endpoint_as_not_running() {
    let fx = Fixture::new();
    let _listener = occupy(&fx.socket);

    let status = fx.status();
    assert_eq!(
        status["running"], false,
        "a peer that never answers is not a usable daemon"
    );
}

/// `start` must not spawn a competitor for an occupied endpoint: the new
/// process would lose the bind and exit, turning a transient stall into a
/// hard failure. Regression — this used to exit 1 with "Another supervisor is
/// already listening".
#[test]
fn start_against_an_occupied_endpoint_reports_rather_than_failing() {
    let fx = Fixture::new();
    let _listener = occupy(&fx.socket);

    let out = fx.grite(&["daemon", "start"], None);
    out.assert_success("daemon start against an occupied endpoint");

    let started = json(&out);
    assert_eq!(started["started"], false);
    assert_eq!(started["ready"], false);
    assert!(
        started["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("did not answer"),
        "reason should name the unanswering occupant, got {:?}",
        started["reason"]
    );
}

/// Routing must reach the same verdict as `status` — not treat the wedged
/// daemon as usable and block on it.
#[test]
fn routing_agrees_with_status_about_a_silent_endpoint() {
    let fx = Fixture::new();
    let _listener = occupy(&fx.socket);

    assert_eq!(fx.status()["running"], false);

    let started = Instant::now();
    let out = fx.grite(&["issue", "list"], None);
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(30),
        "routing should not block on a silent daemon; took {elapsed:?}"
    );
    // Either it reports the endpoint as unusable, or it runs in-process — but
    // it must not hang, and must not claim the daemon served the request.
    assert!(
        !out.stderr_text().contains("timed out after"),
        "routing should not have waited out an IPC timeout: {}",
        out.stderr_text()
    );
}

/// A real wedged daemon: SIGSTOP, then confirm `stop` still delivers the
/// shutdown (queued in the socket) rather than declaring it "not running" and
/// walking away, which would leave it alive with no way to remove it.
/// Resumes a frozen process on drop.
///
/// Without this, a panic between the SIGSTOP and the SIGCONT strands the
/// daemon: `Fixture::drop` runs `daemon stop`, which a frozen process cannot
/// answer, and a frozen process never reaches its idle timeout either — so it
/// sits on the machine indefinitely, immune to SIGTERM (the signal stays
/// pending until it resumes). Observed in practice after a deliberately
/// failing run.
struct Resume(u32);

impl Drop for Resume {
    fn drop(&mut self) {
        signal_quietly(self.0, "CONT");
    }
}

#[test]
fn stop_reaches_a_stopped_daemon_once_it_resumes() {
    let fx = Fixture::new();
    let pid = json(&fx.grite(&["daemon", "start"], None))["pid"]
        .as_u64()
        .unwrap() as u32;

    let _resume = Resume(pid);
    signal(pid, "STOP");

    // `stop` cannot confirm an exit while the process is frozen, so it fails
    // loudly rather than silently reporting success.
    let out = fx.grite(&["daemon", "stop"], None);
    assert!(
        !out.status.success(),
        "stop must not claim success it could not confirm"
    );
    assert_eq!(json(&out)["stopped"], false);

    // But the DaemonStop was delivered: on resume, the daemon acts on it.
    signal(pid, "CONT");
    assert!(
        wait_until(Duration::from_secs(20), || !pid_alive(pid)),
        "a resumed daemon should act on the queued stop; PID {pid} still alive"
    );
}

// ---------------------------------------------------------------------------
// 8. Endpoint precedence across two sockets
// ---------------------------------------------------------------------------

/// The lease says which daemon owns the sled store. Routing must follow it,
/// even when this process is configured to use a different endpoint —
/// otherwise a second daemon steals the routing and cannot open the store,
/// and every command fails `db_busy`.
///
/// `GRITE_DAEMON_SOCKET` makes this configuration easy to reach: two agents
/// that disagree about it share one repository.
#[test]
fn a_live_lease_wins_over_a_differently_configured_endpoint() {
    let fx = Fixture::new();
    let other_socket = PathBuf::from(format!(
        "/tmp/grite-t{}-{}b.sock",
        std::process::id(),
        SOCKET_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_file(&other_socket);

    // Daemon A owns the store: start it and make it create a worker, so the
    // lease on disk records A's endpoint.
    json(&fx.grite(&["daemon", "start"], None));
    fx.grite(&["issue", "create", "--title", "owned by A"], None)
        .assert_success("issue create via A");
    assert!(fx.lock_path().exists(), "A should hold the lease");

    // Daemon B is live on a different socket.
    json(&fx.grite(&["daemon", "start"], Some(&other_socket)));

    // A command configured for B must still reach A, which owns the store.
    let out = fx.grite_in(
        &fx.main.clone(),
        &[
            "issue",
            "create",
            "--title",
            "routed while configured for B",
        ],
        Some(&other_socket),
    );
    out.assert_success("write while configured for the non-owning endpoint");
    assert!(
        !out.stderr_text().contains("db_busy"),
        "must not contend for the store: {}",
        out.stderr_text()
    );

    // A did the work; B never opened the store.
    let a_status = fx.status();
    assert_eq!(a_status["worker_count"].as_u64().unwrap(), 1);
    let b_status = json(&fx.grite(&["daemon", "status"], Some(&other_socket)));
    assert_eq!(
        b_status["worker_count"].as_u64().unwrap(),
        0,
        "the non-owning daemon should never have opened the store"
    );

    let _ = fx.grite(&["daemon", "stop"], Some(&other_socket));
    let _ = std::fs::remove_file(&other_socket);
    fx.stop();
}

// ---------------------------------------------------------------------------
// 9. Host identity must not depend on the environment
// ---------------------------------------------------------------------------

/// Stale-lock recovery compares the lock's `host_id` against our own, and
/// skips the PID check when they differ. So if `HOSTNAME` could shift that
/// identity, a daemon and a later CLI launched from different environments
/// would disagree, liveness checking would switch off, and a crashed daemon
/// would wedge the repository for its whole lease.
///
/// Containers set `HOSTNAME` to the container id, and agent harnesses may set
/// it per worktree, so this is not hypothetical.
#[test]
fn a_conflicting_hostname_env_var_does_not_disable_stale_lock_recovery() {
    let fx = Fixture::new();

    // Start the daemon from an environment that claims a different hostname.
    let out = Command::new(grite_bin())
        .args(["--json", "daemon", "start"])
        .current_dir(&fx.main)
        .env("GRITE_DAEMON_SOCKET", &fx.socket)
        .env("GRITE_DAEMON_BIN", daemon_bin())
        .env("HOSTNAME", "some-container-id-4f2a")
        .env_remove("XDG_RUNTIME_DIR")
        .output()
        .expect("run grite");
    out.assert_success("daemon start with a conflicting HOSTNAME");
    let pid = json(&out)["pid"].as_u64().unwrap() as u32;

    // Make it write the lease.
    fx.grite(&["issue", "list"], None)
        .assert_success("issue list");
    let lock: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fx.lock_path()).expect("read lease"))
            .expect("parse lease");
    assert_ne!(
        lock["host_id"], "some-container-id-4f2a",
        "host identity must come from the kernel, not the environment"
    );

    // Now crash it and recover from an environment with no HOSTNAME at all.
    kill9(pid);
    assert!(wait_until(Duration::from_secs(5), || !pid_alive(pid)));

    let status = fx.status();
    assert_eq!(status["running"], false);
    assert_eq!(
        status["lock"]["stale"], true,
        "the lease must be recognised as stale despite the HOSTNAME mismatch"
    );

    fx.grite(&["issue", "create", "--title", "after crash"], None)
        .assert_success("write after crash");

    fx.stop();
}

/// Whatever the host bookkeeping says, an explicit `daemon stop` must be able
/// to clear a lease that nothing is serving. This is the operator escape
/// hatch the `db_busy` message points at, so it has to actually work.
#[test]
fn stop_clears_a_lease_whose_endpoint_nobody_serves() {
    let fx = Fixture::new();

    // A lease from another machine: we cannot check its PID, and its endpoint
    // does not exist.
    std::fs::create_dir_all(fx.grite_dir()).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    std::fs::write(
        fx.lock_path(),
        serde_json::to_string_pretty(&serde_json::json!({
            "pid": std::process::id(),
            "started_ts": now,
            "repo_root": fx.main.to_str().unwrap(),
            "actor_id": "0".repeat(32),
            "host_id": "some-other-machine",
            "ipc_endpoint": "/tmp/grite-nobody-serves-this-7b3e.sock",
            "lease_ms": 30_000u64,
            "last_heartbeat_ts": now,
            "expires_ts": now + 30_000,
        }))
        .unwrap(),
    )
    .unwrap();

    let stopped = json(&fx.grite(&["daemon", "stop"], None));
    assert_eq!(stopped["cleaned_stale_lock"], true);
    assert!(
        !fx.lock_path().exists(),
        "an unreachable lease must be clearable by an explicit stop"
    );
}

// ---------------------------------------------------------------------------
// 10. Repository layouts the daemon must not have to guess
// ---------------------------------------------------------------------------
//
// The daemon used to rebuild the git directory as `<repo_root>/.git`, where
// `repo_root` was itself derived as `commondir.parent()`. That round-trip
// loses information for every layout except the common one. The client now
// resolves `commondir` with git2 and sends it, so these work by construction.

/// `git init --separate-git-dir` puts the git directory outside the working
/// tree and leaves a gitlink file behind. Reconstructing `<root>/.git` gave
/// "not a git repository" and every command failed.
#[test]
fn works_in_a_repository_with_a_separate_git_dir() {
    let temp = TempDir::new().expect("temp dir");
    let work = temp.path().join("work");
    let gitdir = temp.path().join("elsewhere.git");
    std::fs::create_dir_all(&work).unwrap();

    let out = Command::new("git")
        .args(["init", "-q", "--separate-git-dir"])
        .arg(&gitdir)
        .arg(&work)
        .output()
        .expect("git init --separate-git-dir");
    assert!(
        out.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    git(&["config", "user.email", "test@example.com"], &work);
    git(&["config", "user.name", "Test"], &work);
    git(&["commit", "-q", "--allow-empty", "-m", "init"], &work);
    assert!(
        work.join(".git").is_file(),
        ".git should be a gitlink file in this layout"
    );

    let seq = SOCKET_SEQ.fetch_add(1, Ordering::SeqCst);
    let socket = PathBuf::from(format!("/tmp/grite-sgd{}-{}.sock", std::process::id(), seq));
    let _ = std::fs::remove_file(&socket);

    let run = |args: &[&str]| {
        Command::new(grite_bin())
            .arg("--json")
            .args(args)
            .current_dir(&work)
            .env("GRITE_DAEMON_SOCKET", &socket)
            .env("GRITE_DAEMON_BIN", daemon_bin())
            .env("GRITE_REQUIRE_DAEMON", "1")
            .env_remove("XDG_RUNTIME_DIR")
            .output()
            .expect("run grite")
    };

    run(&["init"]).assert_success("grite init with a separate git dir");
    run(&["daemon", "start"]).assert_success("daemon start");

    // The real test: a write must route through the daemon and land.
    let created = run(&["issue", "create", "--title", "separate git dir"]);
    created.assert_success("issue create through the daemon");

    let listed = run(&["issue", "list"]);
    listed.assert_success("issue list");
    assert_eq!(
        json(&listed)["issues"].as_array().unwrap().len(),
        1,
        "the write should be readable back"
    );

    // Grite state belongs in the real git directory, not next to the gitlink.
    assert!(
        gitdir.join("grite").is_dir(),
        "grite state should live in the separate git dir"
    );
    assert!(
        !work.join(".git").is_dir(),
        ".git must remain a gitlink file"
    );

    let _ = run(&["daemon", "stop"]);
    let _ = std::fs::remove_file(&socket);
}

/// Two spellings of one path (`/tmp` and `/private/tmp` are the same directory
/// on macOS) must not produce two workers fighting over one sled store.
#[test]
fn differently_spelled_paths_share_one_worker() {
    let fx = Fixture::new();
    json(&fx.grite(&["daemon", "start"], None));

    // Reach the same repository by a path with a `..` segment in it.
    let indirect = fx
        .main
        .join("..")
        .join(fx.main.file_name().expect("repo directory has a name"));

    fx.grite_in(
        &fx.main.clone(),
        &["issue", "create", "--title", "direct"],
        None,
    )
    .assert_success("write via the direct path");
    fx.grite_in(&indirect, &["issue", "create", "--title", "indirect"], None)
        .assert_success("write via the indirect path");

    assert_eq!(
        fx.status()["worker_count"].as_u64().unwrap(),
        1,
        "one repository must map to one worker however its path is spelled"
    );
    let listed = json(&fx.grite(&["issue", "list"], None));
    assert_eq!(listed["issues"].as_array().unwrap().len(), 2);

    fx.stop();
}

fn kill9(pid: u32) {
    signal(pid, "KILL");
}

fn signal(pid: u32, name: &str) {
    let out = Command::new("kill")
        .args([&format!("-{name}"), &pid.to_string()])
        .output()
        .expect("send signal");
    assert!(
        out.status.success(),
        "kill -{name} {pid} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Send a signal without asserting — for cleanup paths, which may run while
/// unwinding and must not panic again.
fn signal_quietly(pid: u32, name: &str) {
    let _ = Command::new("kill")
        .args([&format!("-{name}"), &pid.to_string()])
        .output();
}
