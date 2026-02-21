//! Daemon management commands

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::thread;

use libgrite_core::GriteError;
use libgrite_ipc::{DaemonLock, IpcClient};

use crate::cli::{Cli, DaemonCommand};
use crate::context::GriteContext;

/// Get the default IPC endpoint for the daemon.
/// Uses user-specific path for security isolation:
/// - XDG_RUNTIME_DIR if available (Linux with systemd)
/// - /tmp/grite-daemon-<uid>.sock as fallback on Unix
/// - /tmp/grite-daemon.sock on non-Unix platforms
pub fn get_default_daemon_endpoint() -> String {
    // Prefer XDG_RUNTIME_DIR which is properly secured by systemd
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return format!("ipc://{}/grite-daemon.sock", runtime_dir);
    }

    // Fallback: user-specific path in /tmp
    #[cfg(unix)]
    {
        let uid = unsafe { libc::getuid() };
        return format!("ipc:///tmp/grite-daemon-{}.sock", uid);
    }

    #[cfg(not(unix))]
    {
        "ipc:///tmp/grite-daemon.sock".to_string()
    }
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
    let ctx = GriteContext::resolve(cli)?;

    // Check if daemon is already running
    if let Ok(Some(lock)) = DaemonLock::read(&ctx.data_dir) {
        if !lock.is_expired() {
            // Try to connect to verify it's actually running
            if IpcClient::connect(&lock.ipc_endpoint).is_ok() {
                if cli.json {
                    println!("{}", serde_json::json!({
                        "started": false,
                        "reason": "Daemon already running",
                        "pid": lock.pid,
                        "endpoint": lock.ipc_endpoint,
                    }));
                } else if !cli.quiet {
                    println!("Daemon already running (PID {})", lock.pid);
                }
                return Ok(());
            }
        }
        // Stale lock, clean it up
        let _ = DaemonLock::remove(&ctx.data_dir);
    }

    // Spawn grite-daemon in background
    let endpoint = get_default_daemon_endpoint();
    let result = spawn_daemon(&endpoint, idle_timeout)?;

    // Wait for daemon to be ready
    let ready = wait_for_daemon(&endpoint, Duration::from_secs(5))?;

    if ready {
        if cli.json {
            println!("{}", serde_json::json!({
                "started": true,
                "pid": result.pid,
                "endpoint": endpoint,
                "idle_timeout_secs": idle_timeout,
            }));
        } else if !cli.quiet {
            println!("Daemon started (PID {})", result.pid);
            println!("  Endpoint: {}", endpoint);
            println!("  Idle timeout: {}s", idle_timeout);
        }
    } else {
        return Err(GriteError::Internal("Daemon started but failed to become ready".to_string()));
    }

    Ok(())
}

/// Result of spawning daemon
struct SpawnResult {
    pid: u32,
}

/// Spawn the grite-daemon process in background
fn spawn_daemon(endpoint: &str, idle_timeout: u64) -> Result<SpawnResult, GriteError> {
    // Find grite-daemon binary - assume it's in the same directory as grite or in PATH
    let grite_daemon_path = find_grite_daemon_binary()?;

    let child = Command::new(&grite_daemon_path)
        .arg("--endpoint")
        .arg(endpoint)
        .arg("--idle-timeout")
        .arg(idle_timeout.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| GriteError::Internal(format!("Failed to spawn grite-daemon: {}", e)))?;

    Ok(SpawnResult { pid: child.id() })
}

/// Find the grite-daemon binary
fn find_grite_daemon_binary() -> Result<String, GriteError> {
    // First, try to find it relative to current executable
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

/// Wait for daemon to become ready
fn wait_for_daemon(endpoint: &str, timeout: Duration) -> Result<bool, GriteError> {
    let start = Instant::now();
    let mut delay = Duration::from_millis(50);

    while start.elapsed() < timeout {
        if IpcClient::connect(endpoint).is_ok() {
            return Ok(true);
        }
        thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_millis(500));
    }

    Ok(false)
}

/// Spawn daemon if not running (for auto-spawn from CLI commands)
pub fn ensure_daemon_running(cli: &Cli) -> Result<Option<String>, GriteError> {
    let ctx = GriteContext::resolve(cli)?;

    // Check if daemon is already running
    if let Ok(Some(lock)) = DaemonLock::read(&ctx.data_dir) {
        if !lock.is_expired() {
            if IpcClient::connect(&lock.ipc_endpoint).is_ok() {
                return Ok(Some(lock.ipc_endpoint));
            }
        }
        // Stale lock, clean it up
        let _ = DaemonLock::remove(&ctx.data_dir);
    }

    // Spawn daemon with default idle timeout (5 minutes)
    let endpoint = get_default_daemon_endpoint();
    let idle_timeout = 300; // 5 minutes default
    spawn_daemon(&endpoint, idle_timeout)?;

    // Wait for daemon to be ready
    if wait_for_daemon(&endpoint, Duration::from_secs(5))? {
        Ok(Some(endpoint))
    } else {
        Err(GriteError::Internal("Failed to start daemon".to_string()))
    }
}

/// Show daemon status
fn status(cli: &Cli) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;

    // Read daemon lock
    let lock = DaemonLock::read(&ctx.data_dir)
        .map_err(|e| GriteError::Internal(format!("Failed to read daemon lock: {}", e)))?;

    if cli.json {
        output_status_json(cli, &lock)?;
    } else {
        output_status_human(cli, &lock)?;
    }

    Ok(())
}

fn output_status_json(cli: &Cli, lock: &Option<DaemonLock>) -> Result<(), GriteError> {
    let output = match lock {
        Some(lock) => {
            let expired = lock.is_expired();
            serde_json::json!({
                "running": !expired,
                "pid": lock.pid,
                "host_id": lock.host_id,
                "ipc_endpoint": lock.ipc_endpoint,
                "started_ts": lock.started_ts,
                "expires_ts": lock.expires_ts,
                "expired": expired,
                "time_remaining_ms": lock.time_remaining_ms(),
            })
        }
        None => {
            serde_json::json!({
                "running": false,
            })
        }
    };

    if !cli.quiet {
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    Ok(())
}

fn output_status_human(cli: &Cli, lock: &Option<DaemonLock>) -> Result<(), GriteError> {
    if cli.quiet {
        return Ok(());
    }

    match lock {
        Some(lock) if !lock.is_expired() => {
            println!("Daemon is running");
            println!("  PID:            {}", lock.pid);
            println!("  Host ID:        {}", lock.host_id);
            println!("  IPC Endpoint:   {}", lock.ipc_endpoint);
            println!("  Started:        {}", format_timestamp(lock.started_ts));
            println!("  Expires in:     {}s", lock.time_remaining_ms() / 1000);
        }
        Some(_) => {
            println!("Daemon lock expired (stale lock file)");
        }
        None => {
            println!("Daemon is not running");
        }
    }

    Ok(())
}

/// Stop the daemon
fn stop(cli: &Cli) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;

    // Read daemon lock to get IPC endpoint
    let lock = DaemonLock::read(&ctx.data_dir)
        .map_err(|e| GriteError::Internal(format!("Failed to read daemon lock: {}", e)))?;

    match lock {
        Some(lock) if !lock.is_expired() => {
            // Try to connect and send stop command
            match libgrite_ipc::IpcClient::connect(&lock.ipc_endpoint) {
                Ok(client) => {
                    let request = libgrite_ipc::IpcRequest::new(
                        uuid::Uuid::new_v4().to_string(),
                        ctx.repo_root().to_string_lossy().to_string(),
                        ctx.actor_id.clone(),
                        ctx.data_dir.to_string_lossy().to_string(),
                        libgrite_ipc::IpcCommand::DaemonStop,
                    );

                    match client.send(&request) {
                        Ok(_) => {
                            if cli.json {
                                println!("{}", serde_json::json!({"stopped": true}));
                            } else if !cli.quiet {
                                println!("Daemon stopped");
                            }
                        }
                        Err(e) => {
                            // Daemon may have stopped before responding
                            if cli.json {
                                println!("{}", serde_json::json!({"stopped": true, "note": format!("Connection closed: {}", e)}));
                            } else if !cli.quiet {
                                println!("Daemon stopped (connection closed)");
                            }
                        }
                    }
                }
                Err(_) => {
                    // Can't connect - daemon may already be dead
                    // Clean up stale lock file
                    let _ = DaemonLock::remove(&ctx.data_dir);
                    if cli.json {
                        println!("{}", serde_json::json!({"stopped": false, "reason": "Daemon not reachable, cleaned up stale lock"}));
                    } else if !cli.quiet {
                        println!("Daemon not reachable (cleaned up stale lock)");
                    }
                }
            }
        }
        _ => {
            if cli.json {
                println!("{}", serde_json::json!({"stopped": false, "reason": "Daemon not running"}));
            } else if !cli.quiet {
                println!("Daemon is not running");
            }
        }
    }

    Ok(())
}

fn format_timestamp(ts_ms: u64) -> String {
    use chrono::{TimeZone, Utc};
    let dt = Utc.timestamp_millis_opt(ts_ms as i64);
    match dt {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        _ => format!("{}ms", ts_ms),
    }
}
