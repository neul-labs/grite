//! Grite daemon library
//!
//! Exposes the daemon's core components for integration testing and embedding
//! in other applications. The supervisor manages per-repo workers and IPC
//! communication over Unix domain sockets.
//!
//! # Example
//!
//! ```rust,no_run
//! use grite_daemon::supervisor::Supervisor;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let supervisor = Supervisor::new(
//!         "/tmp/grite-daemon.sock".to_string(),
//!         Some(Duration::from_secs(300)),
//!     );
//!     supervisor.run(std::future::pending()).await.unwrap();
//! }
//! ```

pub mod error;
pub mod supervisor;
pub mod worker;
pub mod signals;

pub use error::DaemonError;
pub use supervisor::Supervisor;
pub use worker::{Worker, WorkerMessage};
pub use signals::{setup_signal_handlers, shutdown_signal};
