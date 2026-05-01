//! Signal handling utilities for graceful daemon shutdown

use std::future::Future;

/// Set up signal handlers for graceful shutdown
///
/// Returns a future that resolves when either SIGINT or SIGTERM is received.
pub async fn shutdown_signal() {
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

/// Convenience function that returns a future usable directly with `Supervisor::run`.
pub fn setup_signal_handlers() -> impl Future<Output = ()> + Send {
    shutdown_signal()
}
