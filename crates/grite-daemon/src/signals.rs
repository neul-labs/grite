//! Signal handling utilities for graceful daemon shutdown

use std::future::Future;

/// Set up signal handlers for graceful shutdown
///
/// Returns a future that resolves when either SIGINT or SIGTERM is received.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!("Failed to install Ctrl+C handler: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => tracing::warn!("Failed to install SIGTERM handler: {}", e),
        }
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
