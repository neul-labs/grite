//! Grit daemon - background process for improved performance
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

use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use supervisor::Supervisor;

#[derive(Parser)]
#[command(name = "gritd", about = "Grit daemon", version)]
struct Cli {
    /// IPC endpoint (e.g., ipc:///tmp/gritd.sock)
    #[arg(long, default_value = "ipc:///tmp/gritd.sock")]
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

    info!("gritd starting");

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
    let supervisor = Supervisor::new(cli.endpoint);

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

    info!("gritd stopped");
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
