//! Daemon management commands
//!
//! Liveness is always established by talking to the daemon over IPC, never by
//! reading a lock file. The lock file at `<git-dir>/grite/daemon.lock` is a
//! worker's advisory lease over the sled cache: it is written lazily, when the
//! first repo-scoped command creates a worker, so its absence says nothing
//! about whether a daemon is running, and its presence says nothing about
//! whether that daemon is still alive.
//!
//! This module requires Unix (uses Unix domain sockets and signals).

#[cfg(not(unix))]
compile_error!("daemon commands require a Unix platform");

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use libgrite_core::GriteError;
use libgrite_ipc::{is_listening, DaemonInfo, DaemonLock, IpcClient, IpcCommand, IpcRequest};

use crate::cli::{Cli, DaemonCommand};
use crate::context::GriteContext;

/// How long `daemon start` waits for a freshly spawned daemon to answer.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a stop waits for the daemon process to actually disappear.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// Bytes of the daemon log to quote when startup fails.
const LOG_TAIL_BYTES: u64 = 4096;

/// Environment override for locating the `grite-daemon` executable.
const DAEMON_BIN_ENV: &str = "GRITE_DAEMON_BIN";

/// Get the default IPC endpoint for the daemon.
pub fn get_default_daemon_endpoint() -> String {
    libgrite_ipc::default_socket_path()
}

/// Ask the daemon at `endpoint` to identify itself.
///
/// Delegates to the shared oracle in `libgrite-ipc` so that `status`, `stop`
/// and command routing cannot drift apart — an earlier revision had `status`
/// doing a round-trip while routing did a bare `connect()`, so a wedged
/// daemon was reported dead by one and used by the other.
pub fn query_daemon(endpoint: &str) -> Option<DaemonInfo> {
    libgrite_ipc::probe_daemon(endpoint)
}

/// Check if a daemon is reachable on the default endpoint.
pub fn is_daemon_running(_cli: &Cli) -> bool {
    query_daemon(&get_default_daemon_endpoint()).is_some()
}

pub fn run(cli: &Cli, cmd: DaemonCommand) -> Result<(), GriteError> {
    match cmd {
        DaemonCommand::Start { idle_timeout } => start(cli, idle_timeout),
        DaemonCommand::Status => status(cli),
        DaemonCommand::Stop => stop(cli),
    }
}

/// Start the daemon in background
fn start(cli: &Cli, idle_timeout: u64) -> Result<(), GriteError> {
    start_internal(cli, idle_timeout)
}

/// Start the daemon (public for use by other commands like doctor).
pub fn start_daemon(cli: &Cli, idle_timeout: u64) -> Result<(), GriteError> {
    start_internal(cli, idle_timeout)
}

fn start_internal(cli: &Cli, idle_timeout: u64) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let grite_dir = ctx.git_dir.join("grite");
    let endpoint = get_default_daemon_endpoint();

    // Is someone already serving this endpoint? Ask them, do not infer it
    // from a lock file that may not exist yet or may outlive its writer.
    if let Some(info) = query_daemon(&endpoint) {
        if cli.json {
            println!(
                "{}",
                serde_json::json!({
                    "started": false,
                    "ready": true,
                    "reason": "Daemon already running",
                    "pid": info.pid,
                    "endpoint": info.endpoint,
                })
            );
        } else if !cli.quiet {
            println!("Daemon already running (PID {})", info.pid);
        }
        return Ok(());
    }

    // Nothing answered, but something may still hold the listening end. A
    // daemon that is merely slow to respond is not a reason to spawn a
    // competitor: the new process would lose the bind and exit, turning a
    // transient stall into a hard failure. Report the occupant instead.
    if is_listening(&endpoint) {
        let occupant = DaemonLock::read(&grite_dir).ok().flatten();
        if cli.json {
            println!(
                "{}",
                serde_json::json!({
                    "started": false,
                    "ready": false,
                    "reason": "Endpoint is occupied by a daemon that did not answer a status request",
                    "endpoint": endpoint,
                    "pid": occupant.as_ref().map(|l| l.pid),
                })
            );
        } else if !cli.quiet {
            println!(
                "Daemon on {} is not answering (it may be busy or stopped).",
                endpoint
            );
            println!("  Run `grite daemon stop` to clear it, then start again.");
        }
        return Ok(());
    }

    // No daemon answered. Any lock left behind belongs to a dead or
    // unreachable holder; drop it so a fresh worker can take the lease.
    let _ = DaemonLock::remove_if_stale(&grite_dir);

    let log_path = daemon_log_path(&grite_dir);
    let mut child = spawn_daemon(&endpoint, idle_timeout, log_path.as_deref())?;

    let info = wait_for_daemon(&endpoint, &mut child, READY_TIMEOUT, log_path.as_deref())?;

    if cli.json {
        println!(
            "{}",
            serde_json::json!({
                "started": true,
                "ready": true,
                "pid": info.pid,
                "endpoint": info.endpoint,
                "idle_timeout_secs": idle_timeout,
            })
        );
    } else if !cli.quiet {
        println!("Daemon started (PID {})", info.pid);
        println!("  Endpoint: {}", info.endpoint);
        println!("  Idle timeout: {}s", idle_timeout);
    }

    Ok(())
}

/// Where to send the daemon's stderr so startup failures can be attributed.
fn daemon_log_path(grite_dir: &Path) -> Option<PathBuf> {
    std::fs::create_dir_all(grite_dir).ok()?;
    Some(grite_dir.join("daemon.log"))
}

/// Spawn the grite-daemon process in the background.
///
/// The child is placed in its own session (`setsid`) so it outlives the
/// terminal or process group of whatever invoked `grite daemon start`; an
/// agent harness that cleans up its process group must not take the shared
/// daemon down with it.
fn spawn_daemon(
    endpoint: &str,
    idle_timeout: u64,
    log_path: Option<&Path>,
) -> Result<Child, GriteError> {
    let grite_daemon_path = find_grite_daemon_binary()?;

    let stderr = match log_path {
        Some(path) => match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            Ok(file) => Stdio::from(file),
            Err(_) => Stdio::null(),
        },
        None => Stdio::null(),
    };

    let mut command = Command::new(&grite_daemon_path);
    command
        .arg("--endpoint")
        .arg(endpoint)
        .arg("--idle-timeout")
        .arg(idle_timeout.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid() is async-signal-safe and touches no allocator or
        // lock state, which is the requirement for pre_exec after fork.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    // Already a session leader is fine; anything else is not
                    // worth failing the spawn over.
                    let err = std::io::Error::last_os_error();
                    if err.raw_os_error() != Some(libc::EPERM) {
                        return Err(err);
                    }
                }
                Ok(())
            });
        }
    }

    command.spawn().map_err(|e| {
        GriteError::Internal(format!(
            "Failed to spawn grite-daemon ({}): {}",
            grite_daemon_path, e
        ))
    })
}

/// Find the grite-daemon binary
fn find_grite_daemon_binary() -> Result<String, GriteError> {
    // Explicit override wins (tests, unusual installs)
    if let Ok(path) = std::env::var(DAEMON_BIN_ENV) {
        if !path.is_empty() {
            return Ok(path);
        }
    }

    // Then look next to the current executable
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let grite_daemon_path = dir.join("grite-daemon");
            if grite_daemon_path.exists() {
                return Ok(grite_daemon_path.to_string_lossy().to_string());
            }
        }
    }

    // Fall back to PATH
    Ok("grite-daemon".to_string())
}

/// Wait for a freshly spawned daemon to answer a status round-trip.
///
/// Polls the child process alongside the endpoint so an early exit is
/// reported immediately, with its exit status and the tail of its log,
/// instead of being hidden behind a generic readiness timeout.
fn wait_for_daemon(
    endpoint: &str,
    child: &mut Child,
    timeout: Duration,
    log_path: Option<&Path>,
) -> Result<DaemonInfo, GriteError> {
    let start = Instant::now();
    let mut delay = Duration::from_millis(20);

    loop {
        if let Some(info) = query_daemon(endpoint) {
            return Ok(info);
        }

        // Did the child die on us?
        match child.try_wait() {
            Ok(Some(exit_status)) => {
                // It may have exited *because* another daemon won the race
                // and is now serving the endpoint — that is a success.
                if let Some(info) = query_daemon(endpoint) {
                    return Ok(info);
                }
                return Err(GriteError::Internal(format!(
                    "grite-daemon exited during startup ({}){}",
                    describe_exit(&exit_status),
                    log_excerpt(log_path)
                )));
            }
            Ok(None) => {}
            Err(e) => {
                return Err(GriteError::Internal(format!(
                    "Failed to check grite-daemon process: {}",
                    e
                )));
            }
        }

        if start.elapsed() >= timeout {
            // Still alive but unresponsive: leave it running rather than
            // killing a process that may be serving another repository.
            return Err(GriteError::Internal(format!(
                "grite-daemon (PID {}) did not become ready on {} within {}s{}",
                child.id(),
                endpoint,
                timeout.as_secs(),
                log_excerpt(log_path)
            )));
        }

        thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_millis(200));
    }
}

fn describe_exit(status: &std::process::ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit code {}", code),
        None => format!("terminated by signal: {}", status),
    }
}

/// Read the tail of the daemon log so the caller sees the real cause.
fn log_excerpt(log_path: Option<&Path>) -> String {
    let Some(path) = log_path else {
        return String::new();
    };
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if len > LOG_TAIL_BYTES {
        use std::io::Seek;
        let _ = file.seek(std::io::SeekFrom::Start(len - LOG_TAIL_BYTES));
    }
    let mut contents = String::new();
    if file.read_to_string(&mut contents).is_err() {
        return String::new();
    }
    let tail: Vec<&str> = contents
        .lines()
        .filter(|l| !l.trim().is_empty())
        .rev()
        .take(5)
        .collect();
    if tail.is_empty() {
        return format!("; see {}", path.display());
    }
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    format!(
        "; last log lines: {} (full log: {})",
        tail.join(" | "),
        path.display()
    )
}

/// Ensure a daemon is reachable, spawning one if needed.
///
/// Returns the endpoint of a daemon that has answered a status round-trip, or
/// a precise error explaining why one could not be reached. Callers must not
/// translate the error into a silent single-process fallback without telling
/// the user: direct sled access serialises poorly and surfaces as `db_busy`.
pub fn ensure_daemon_running(cli: &Cli) -> Result<String, GriteError> {
    let endpoint = get_default_daemon_endpoint();

    if query_daemon(&endpoint).is_some() {
        return Ok(endpoint);
    }

    let log_path = GriteContext::resolve(cli)
        .ok()
        .and_then(|ctx| daemon_log_path(&ctx.git_dir.join("grite")));

    // Default idle timeout (5 minutes), matching `daemon start`.
    let mut child = spawn_daemon(&endpoint, 300, log_path.as_deref())?;
    let info = wait_for_daemon(&endpoint, &mut child, READY_TIMEOUT, log_path.as_deref())?;
    Ok(info.endpoint)
}

/// Show daemon status
fn status(cli: &Cli) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let grite_dir = ctx.git_dir.join("grite");
    let endpoint = get_default_daemon_endpoint();

    let info = query_daemon(&endpoint);
    let lock = DaemonLock::read(&grite_dir).ok().flatten();

    if cli.json {
        output_status_json(cli, &endpoint, &info, &lock)?;
    } else {
        output_status_human(cli, &endpoint, &info, &lock)?;
    }

    Ok(())
}

/// Describe the lock file without letting it decide liveness.
fn lock_json(lock: &Option<DaemonLock>) -> serde_json::Value {
    match lock {
        Some(lock) => serde_json::json!({
            "present": true,
            "pid": lock.pid,
            "host_id": lock.host_id,
            "ipc_endpoint": lock.ipc_endpoint,
            "expires_ts": lock.expires_ts,
            "expired": lock.is_expired(),
            "holder_alive": lock.holder_alive(),
            "stale": lock.is_stale(),
            "time_remaining_ms": lock.time_remaining_ms(),
        }),
        None => serde_json::json!({ "present": false }),
    }
}

fn output_status_json(
    cli: &Cli,
    endpoint: &str,
    info: &Option<DaemonInfo>,
    lock: &Option<DaemonLock>,
) -> Result<(), GriteError> {
    let output = match info {
        Some(info) => serde_json::json!({
            "running": true,
            "pid": info.pid,
            "daemon_id": info.daemon_id,
            "host_id": info.host_id,
            "ipc_endpoint": info.endpoint,
            "started_ts": info.started_ts,
            "worker_count": info.worker_count,
            "state": info.state,
            "lock": lock_json(lock),
        }),
        None => serde_json::json!({
            "running": false,
            "ipc_endpoint": endpoint,
            "reason": "No daemon answered on the IPC endpoint",
            "lock": lock_json(lock),
        }),
    };

    if !cli.quiet {
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    Ok(())
}

fn output_status_human(
    cli: &Cli,
    endpoint: &str,
    info: &Option<DaemonInfo>,
    lock: &Option<DaemonLock>,
) -> Result<(), GriteError> {
    if cli.quiet {
        return Ok(());
    }

    match info {
        Some(info) => {
            println!("Daemon is running");
            println!("  PID:            {}", info.pid);
            println!("  Host ID:        {}", info.host_id);
            println!("  IPC Endpoint:   {}", info.endpoint);
            println!("  Started:        {}", format_timestamp(info.started_ts));
            println!("  Workers:        {}", info.worker_count);
            println!("  State:          {}", info.state);
        }
        None => {
            println!("Daemon is not running");
            println!("  IPC Endpoint:   {} (no response)", endpoint);
        }
    }

    if let Some(lock) = lock {
        if info.is_none() && lock.is_stale() {
            println!(
                "  Stale lock:     PID {} (holder gone) — `grite daemon start` will clear it",
                lock.pid
            );
        } else if info.is_none() {
            println!(
                "  Cache lease:    held by PID {} on {}, expires in {}s",
                lock.pid,
                lock.host_id,
                lock.time_remaining_ms() / 1000
            );
        }
    }

    Ok(())
}

/// Stop the daemon
fn stop(cli: &Cli) -> Result<(), GriteError> {
    stop_internal(cli)
}

/// Stop the daemon (public for use by other commands like doctor).
pub fn stop_daemon(cli: &Cli) -> Result<(), GriteError> {
    stop_internal(cli)
}

fn stop_internal(cli: &Cli) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let grite_dir = ctx.git_dir.join("grite");
    let endpoint = get_default_daemon_endpoint();

    // Resolve what to stop. A daemon that does not answer a status probe may
    // still be there, holding the listening end — busy, paged out, or blocked
    // — and `stop` must be able to shut it down anyway. Falling through to
    // "not running" would leave it alive with no way to remove it.
    let probed = query_daemon(&endpoint);
    let occupied = probed.is_some() || is_listening(&endpoint);

    if !occupied {
        // Really nothing there. Clear whatever a crashed daemon left behind
        // so the next command can start cleanly. This is the operator escape
        // hatch, so it also clears a lock whose holder cannot be verified
        // (foreign host, recycled PID) when nothing serves its endpoint.
        let stale_lock = DaemonLock::remove_if_unusable(&grite_dir)
            .ok()
            .flatten()
            .is_some();
        let stale_socket = remove_stale_socket(&endpoint);

        if cli.json {
            println!(
                "{}",
                serde_json::json!({
                    "stopped": false,
                    "reason": "Daemon not running",
                    "cleaned_stale_lock": stale_lock,
                    "cleaned_stale_socket": stale_socket,
                })
            );
        } else if !cli.quiet {
            if stale_lock || stale_socket {
                println!("Daemon is not running (cleaned up stale state)");
            } else {
                println!("Daemon is not running");
            }
        }
        return Ok(());
    }

    // PID for the exit wait: the daemon's own answer when we have it, else
    // the lease it wrote. Without either we can still send the stop and
    // confirm by watching the endpoint go quiet.
    let pid = probed
        .as_ref()
        .map(|info| info.pid)
        .or_else(|| DaemonLock::read(&grite_dir).ok().flatten().map(|l| l.pid));

    // Ask the daemon to shut down. The connection may close before the
    // response arrives, which is not an error.
    if let Ok(mut client) = IpcClient::connect(&endpoint) {
        let request = IpcRequest::new(
            uuid::Uuid::new_v4().to_string(),
            ctx.repo_root().to_string_lossy().to_string(),
            ctx.actor_id.clone(),
            ctx.data_dir.to_string_lossy().to_string(),
            IpcCommand::DaemonStop,
        );
        let _ = client.send(&request);
    }

    let exited = wait_for_daemon_exit(pid, &endpoint, STOP_TIMEOUT);

    // The daemon releases its own lock and socket on a clean shutdown; clear
    // anything left over from an unclean one.
    if exited {
        let _ = DaemonLock::remove_if_unusable(&grite_dir);
        remove_stale_socket(&endpoint);
    }

    let failure = (!exited).then(|| match pid {
        Some(pid) => format!(
            "Daemon (PID {}) did not exit within {}s",
            pid,
            STOP_TIMEOUT.as_secs()
        ),
        None => format!(
            "Daemon on {} did not stop within {}s",
            endpoint,
            STOP_TIMEOUT.as_secs()
        ),
    });

    if cli.json {
        println!(
            "{}",
            serde_json::json!({
                "stopped": exited,
                "pid": pid,
                "reason": failure,
            })
        );
    } else if !cli.quiet {
        match &failure {
            None => println!("Daemon stopped"),
            Some(message) => println!("{}", message),
        }
    }

    match failure {
        None => Ok(()),
        Some(message) => Err(GriteError::Internal(message)),
    }
}

/// Remove a socket file that no process is listening on.
///
/// Returns true if a stale file was removed. A socket someone is still
/// serving is left alone.
fn remove_stale_socket(endpoint: &str) -> bool {
    let path = Path::new(endpoint);
    if !path.exists() {
        return false;
    }
    if std::os::unix::net::UnixStream::connect(path).is_ok() {
        return false;
    }
    std::fs::remove_file(path).is_ok()
}

/// Wait for the daemon to disappear: both the process and the endpoint.
///
/// The endpoint going quiet is the part that always applies; the PID check is
/// an extra confirmation for the common case where we know which process to
/// watch. `is_listening` is the right test here rather than a status probe:
/// we are waiting for the listening end to be released, and a daemon that
/// stops answering while still holding the socket has not stopped.
fn wait_for_daemon_exit(pid: Option<u32>, endpoint: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    let mut delay = Duration::from_millis(20);

    loop {
        let process_gone = !pid.is_some_and(libgrite_ipc::process_alive);
        if process_gone && !is_listening(endpoint) {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_millis(200));
    }
}

fn format_timestamp(ts_ms: u64) -> String {
    use chrono::{TimeZone, Utc};
    let dt = Utc.timestamp_millis_opt(ts_ms as i64);
    match dt {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        _ => format!("{}ms", ts_ms),
    }
}
