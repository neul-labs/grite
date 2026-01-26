//! Grite daemon - background process for improved performance
//!
//! The daemon provides:
//! - IPC interface for CLI commands
//! - Exclusive access to sled databases
//! - Pub/sub notifications
//! - Background sync operations

mod error;
mod supervisor;
mod worker;

use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use supervisor::Supervisor;

#[derive(Parser)]
#[command(name = "grite-daemon", about = "Grite daemon", version)]
struct Cli {
    /// IPC endpoint (e.g., ipc:///tmp/grite-daemon.sock)
    #[arg(long, default_value = "ipc:///tmp/grite-daemon.sock")]
    endpoint: String,

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
    let shutdown = setup_signal_handlers();

    // Create and run supervisor
    let idle_timeout = if cli.idle_timeout > 0 {
        Some(Duration::from_secs(cli.idle_timeout))
    } else {
        None
    };
    let supervisor = Supervisor::new(cli.endpoint, idle_timeout);

    tokio::select! {
        result = supervisor.run() => {
            if let Err(e) = result {
                error!("Supervisor error: {}", e);
            }
        }
        _ = shutdown => {
            info!("Received shutdown signal");
        }
    }

    // Cleanup PID file
    if let Some(ref pid_file) = cli.pid_file {
        let _ = std::fs::remove_file(pid_file);
    }

    info!("grite-daemon stopped");
}

/// Set up signal handlers for graceful shutdown
fn setup_signal_handlers() -> impl std::future::Future<Output = ()> {
    async {
        let ctrl_c = async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {}
            _ = terminate => {}
        }
    }
}
