//! Daemon management commands

use libgrit_core::GritError;
use libgrit_ipc::DaemonLock;

use crate::cli::{Cli, DaemonCommand};
use crate::context::GritContext;

pub fn run(cli: &Cli, cmd: DaemonCommand) -> Result<(), GritError> {
    match cmd {
        DaemonCommand::Status => status(cli),
        DaemonCommand::Stop => stop(cli),
    }
}

/// Show daemon status
fn status(cli: &Cli) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;

    // Read daemon lock
    let lock = DaemonLock::read(&ctx.data_dir)
        .map_err(|e| GritError::Internal(format!("Failed to read daemon lock: {}", e)))?;

    if cli.json {
        output_status_json(cli, &lock)?;
    } else {
        output_status_human(cli, &lock)?;
    }

    Ok(())
}

fn output_status_json(cli: &Cli, lock: &Option<DaemonLock>) -> Result<(), GritError> {
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

fn output_status_human(cli: &Cli, lock: &Option<DaemonLock>) -> Result<(), GritError> {
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
fn stop(cli: &Cli) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;

    // Read daemon lock to get IPC endpoint
    let lock = DaemonLock::read(&ctx.data_dir)
        .map_err(|e| GritError::Internal(format!("Failed to read daemon lock: {}", e)))?;

    match lock {
        Some(lock) if !lock.is_expired() => {
            // Try to connect and send stop command
            match libgrit_ipc::IpcClient::connect(&lock.ipc_endpoint) {
                Ok(client) => {
                    let request = libgrit_ipc::IpcRequest::new(
                        uuid::Uuid::new_v4().to_string(),
                        ctx.repo_root().to_string_lossy().to_string(),
                        ctx.actor_id.clone(),
                        ctx.data_dir.to_string_lossy().to_string(),
                        libgrit_ipc::IpcCommand::DaemonStop,
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
