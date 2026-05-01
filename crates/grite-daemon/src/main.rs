//! Grite daemon - background process for improved performance
//!
//! The daemon provides:
//! - IPC interface for CLI commands
//! - Exclusive access to sled databases
//! - Pub/sub notifications
//! - Background sync operations

use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use grite_daemon::supervisor::Supervisor;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser)]
#[command(name = "grite-daemon", about = "Grite daemon", version)]
struct Cli {
    /// Unix socket path (e.g., /tmp/grite-daemon.sock)
    #[arg(long)]
    endpoint: Option<String>,

    /// Daemonize (run in background)
    #[arg(long, short)]
    daemon: bool,

    /// PID file path (for daemon mode)
    #[arg(long)]
    pid_file: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Idle timeout in seconds (daemon auto-stops after this period of inactivity, 0 = no timeout)
    #[arg(long, default_value = "0")]
    idle_timeout: u64,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize logging
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&cli.log_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("grite-daemon starting");

    // Daemonize if requested
    if cli.daemon {
        info!("Daemonizing...");
        // For full daemonization, we'd use the daemonize crate
        // For now, we just run in foreground
    }

    // Write PID file if specified
    if let Some(ref pid_file) = cli.pid_file {
        let pid = std::process::id();
        if let Err(e) = std::fs::write(pid_file, pid.to_string()) {
            error!("Failed to write PID file: {}", e);
        }
    }

    // Set up signal handlers
    let shutdown = grite_daemon::signals::shutdown_signal();

    // Create and run supervisor
    let idle_timeout = if cli.idle_timeout > 0 {
        Some(Duration::from_secs(cli.idle_timeout))
    } else {
        None
    };
    let endpoint = cli.endpoint.unwrap_or_else(libgrite_ipc::default_socket_path);
    let supervisor = Supervisor::new(endpoint, idle_timeout);

    if let Err(e) = supervisor.run(shutdown).await {
        error!("Supervisor error: {}", e);
    }

    // Cleanup PID file
    if let Some(ref pid_file) = cli.pid_file {
        let _ = std::fs::remove_file(pid_file);
    }

    info!("grite-daemon stopped");
}
