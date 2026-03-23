//! Supervisor module - manages workers and IPC sockets
//!
//! The supervisor:
//! - Listens on a Unix socket for commands
//! - Manages worker lifecycle
//! - Routes commands to appropriate workers
//! - Broadcasts notifications via internal channels

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use libgrite_ipc::{
    framing::{read_framed_async, write_framed_async},
    messages::{ArchivedIpcRequest, IpcRequest, IpcResponse},
    IpcCommand, Notification, IPC_SCHEMA_VERSION,
};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

use crate::error::DaemonError;
use crate::worker::{Worker, WorkerMessage};

/// Worker handle for communication
struct WorkerHandle {
    tx: mpsc::Sender<WorkerMessage>,
    join_handle: Option<tokio::task::JoinHandle<()>>,
    repo_root: PathBuf,
    actor_id: String,
}

/// Key for worker lookup
#[derive(Hash, Eq, PartialEq, Clone)]
struct WorkerKey {
    repo_root: String,
    actor_id: String,
}

/// Supervisor manages workers and IPC
pub struct Supervisor {
    /// Daemon ID
    daemon_id: String,
    /// Host ID
    host_id: String,
    /// Unix socket path
    socket_path: String,
    /// Workers by (repo_root, actor_id), behind a Mutex for atomic get-or-create
    workers: Arc<Mutex<HashMap<WorkerKey, WorkerHandle>>>,
    /// Notification channel
    notify_rx: mpsc::Receiver<Notification>,
    /// Notification sender (cloned to workers)
    notify_tx: mpsc::Sender<Notification>,
    /// Shutdown signal
    shutdown_tx: Option<tokio::sync::broadcast::Sender<()>>,
    /// Idle timeout (None = no auto-shutdown)
    idle_timeout: Option<Duration>,
    /// Last activity timestamp (monotonic, as ms since process start)
    last_activity_ms: Arc<AtomicU64>,
    /// Process start instant for relative timing
    start_instant: Instant,
}

impl Supervisor {
    /// Create a new supervisor
    pub fn new(socket_path: String, idle_timeout: Option<Duration>) -> Self {
        let (notify_tx, notify_rx) = mpsc::channel(1000);
        let start_instant = Instant::now();

        Self {
            daemon_id: uuid::Uuid::new_v4().to_string(),
            host_id: get_host_id(),
            socket_path,
            workers: Arc::new(Mutex::new(HashMap::new())),
            notify_rx,
            notify_tx,
            shutdown_tx: None,
            idle_timeout,
            last_activity_ms: Arc::new(AtomicU64::new(0)),
            start_instant,
        }
    }

    /// Update the last activity timestamp
    fn touch_activity(&self) {
        let elapsed_ms = self.start_instant.elapsed().as_millis() as u64;
        self.last_activity_ms.store(elapsed_ms, Ordering::Relaxed);
    }

    /// Trigger shutdown signal (for use from signal handlers)
    pub fn trigger_shutdown(&self) {
        if let Some(ref tx) = self.shutdown_tx {
            let _ = tx.send(());
        }
    }

    /// Run the supervisor
    pub async fn run(&mut self) -> Result<(), DaemonError> {
        info!(
            daemon_id = %self.daemon_id,
            socket_path = %self.socket_path,
            idle_timeout_secs = ?self.idle_timeout.map(|d| d.as_secs()),
            "Supervisor starting"
        );

        // Initialize last activity to now
        self.touch_activity();

        // Create shutdown channel
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        // Clean up stale socket file
        let socket_path = Path::new(&self.socket_path);
        if socket_path.exists() {
            std::fs::remove_file(socket_path).map_err(|e| {
                DaemonError::BindFailed(format!(
                    "Failed to remove stale socket {}: {}",
                    self.socket_path, e
                ))
            })?;
        }

        // Bind Unix listener
        let listener = UnixListener::bind(&self.socket_path).map_err(|e| {
            DaemonError::BindFailed(format!(
                "Failed to bind to {}: {}",
                self.socket_path, e
            ))
        })?;

        info!("Listening on {}", self.socket_path);

        // Spawn heartbeat task (also checks idle timeout)
        let workers_clone = self.workers.clone();
        let last_activity_ms = self.last_activity_ms.clone();
        let idle_timeout = self.idle_timeout;
        let start_instant = self.start_instant;
        let idle_shutdown_tx = shutdown_tx.clone();
        let mut heartbeat_shutdown = shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Send heartbeats to workers
                        let workers = workers_clone.lock().await;
                        for handle in workers.values() {
                            let _ = handle.tx.send(WorkerMessage::Heartbeat).await;
                        }
                        drop(workers);

                        // Check idle timeout
                        if let Some(timeout) = idle_timeout {
                            let last_ms = last_activity_ms.load(Ordering::Relaxed);
                            let now_ms = start_instant.elapsed().as_millis() as u64;
                            let idle_ms = now_ms.saturating_sub(last_ms);
                            if idle_ms >= timeout.as_millis() as u64 {
                                info!("Idle timeout reached ({} ms), shutting down", idle_ms);
                                let _ = idle_shutdown_tx.send(());
                                break;
                            }
                        }
                    }
                    _ = heartbeat_shutdown.recv() => {
                        break;
                    }
                }
            }
        });

        // Spawn notification consumer (just logs for now since PUB socket is removed)
        let mut notify_rx = std::mem::replace(
            &mut self.notify_rx,
            mpsc::channel(1).1,
        );
        let mut notify_shutdown = shutdown_tx.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(notification) = notify_rx.recv() => {
                        debug!(
                            notification_type = %notification.notification_type(),
                            "Notification emitted"
                        );
                    }
                    _ = notify_shutdown.recv() => {
                        break;
                    }
                }
            }
        });

        // Main accept loop — each connection gets its own task
        let mut main_shutdown = shutdown_tx.subscribe();
        loop {
            tokio::select! {
                _ = main_shutdown.recv() => {
                    info!("Shutdown signal received");
                    break;
                }
                result = listener.accept() => {
                    match result {
                        Ok((stream, _addr)) => {
                            let workers = self.workers.clone();
                            let notify_tx = self.notify_tx.clone();
                            let host_id = self.host_id.clone();
                            let ipc_endpoint = self.socket_path.clone();
                            let shutdown_tx_clone = shutdown_tx.clone();
                            let last_activity = self.last_activity_ms.clone();
                            let start = self.start_instant;

                            tokio::spawn(async move {
                                // Update activity timestamp
                                let elapsed_ms = start.elapsed().as_millis() as u64;
                                last_activity.store(elapsed_ms, Ordering::Relaxed);

                                handle_connection(
                                    stream,
                                    workers,
                                    notify_tx,
                                    host_id,
                                    ipc_endpoint,
                                    shutdown_tx_clone,
                                )
                                .await;
                            });
                        }
                        Err(e) => {
                            warn!("Accept error: {}", e);
                        }
                    }
                }
            }
        }

        // Clean up socket file
        let _ = std::fs::remove_file(&self.socket_path);

        // Shutdown all workers
        self.shutdown_workers().await;

        info!("Supervisor stopped");
        Ok(())
    }

    /// Shutdown all workers and wait for them to finish
    pub async fn shutdown_workers(&self) {
        let mut workers = self.workers.lock().await;

        // Send shutdown to all workers
        for handle in workers.values() {
            let _ = handle.tx.send(WorkerMessage::Shutdown).await;
        }

        // Wait for all workers to actually finish (with timeout)
        for handle in workers.values_mut() {
            if let Some(jh) = handle.join_handle.take() {
                match tokio::time::timeout(Duration::from_secs(10), jh).await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => warn!("Worker task panicked: {}", e),
                    Err(_) => warn!(
                        "Worker {}/{} didn't shut down within 10s",
                        handle.repo_root.display(),
                        handle.actor_id
                    ),
                }
            }
        }
    }
}

/// Handle a single client connection: read one request, send one response
async fn handle_connection(
    mut stream: UnixStream,
    workers: Arc<Mutex<HashMap<WorkerKey, WorkerHandle>>>,
    notify_tx: mpsc::Sender<Notification>,
    host_id: String,
    ipc_endpoint: String,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
) {
    // Read request with timeout
    let request_bytes = match tokio::time::timeout(
        Duration::from_secs(30),
        read_framed_async(&mut stream),
    )
    .await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => {
            debug!("Failed to read request: {}", e);
            return;
        }
        Err(_) => {
            debug!("Request read timed out");
            return;
        }
    };

    let response = process_request(
        &request_bytes,
        &workers,
        &notify_tx,
        &host_id,
        &ipc_endpoint,
        &shutdown_tx,
    )
    .await;

    // Serialize and send response
    match rkyv::to_bytes::<rkyv::rancor::Error>(&response) {
        Ok(bytes) => {
            if let Err(e) = tokio::time::timeout(
                Duration::from_secs(5),
                write_framed_async(&mut stream, &bytes),
            )
            .await
            {
                warn!("Failed to send response: {:?}", e);
            }
        }
        Err(e) => {
            warn!("Failed to serialize response: {}", e);
        }
    }
}

/// Process a raw request and return a response
async fn process_request(
    raw: &[u8],
    workers: &Arc<Mutex<HashMap<WorkerKey, WorkerHandle>>>,
    notify_tx: &mpsc::Sender<Notification>,
    host_id: &str,
    ipc_endpoint: &str,
    shutdown_tx: &tokio::sync::broadcast::Sender<()>,
) -> IpcResponse {
    // Deserialize request
    let archived = match rkyv::access::<ArchivedIpcRequest, rkyv::rancor::Error>(raw) {
        Ok(a) => a,
        Err(e) => {
            return IpcResponse::error(
                "unknown".to_string(),
                "deserialization".to_string(),
                format!("Failed to deserialize request: {}", e),
            );
        }
    };

    // Check version
    let version: u32 = archived.ipc_schema_version.into();
    if version != IPC_SCHEMA_VERSION {
        return IpcResponse::error(
            archived.request_id.to_string(),
            "version_mismatch".to_string(),
            format!("Expected version {}, got {}", IPC_SCHEMA_VERSION, version),
        );
    }

    // Deserialize to owned type
    let request: IpcRequest =
        match rkyv::deserialize::<IpcRequest, rkyv::rancor::Error>(archived) {
            Ok(r) => r,
            Err(e) => {
                return IpcResponse::error(
                    archived.request_id.to_string(),
                    "deserialization".to_string(),
                    format!("Failed to deserialize request: {}", e),
                );
            }
        };

    debug!(
        request_id = %request.request_id,
        repo = %request.repo_root,
        actor = %request.actor_id,
        "Handling request"
    );

    // Handle DaemonStop specially
    if matches!(request.command, IpcCommand::DaemonStop) {
        let _ = shutdown_tx.send(());
        return IpcResponse::success(
            request.request_id,
            Some(serde_json::json!({"stopping": true}).to_string()),
        );
    }

    // Route to worker
    route_to_worker(
        request,
        workers,
        notify_tx,
        host_id,
        ipc_endpoint,
    )
    .await
}

/// Route a request to the appropriate worker, creating one if needed
async fn route_to_worker(
    request: IpcRequest,
    workers: &Arc<Mutex<HashMap<WorkerKey, WorkerHandle>>>,
    notify_tx: &mpsc::Sender<Notification>,
    host_id: &str,
    ipc_endpoint: &str,
) -> IpcResponse {
    let key = WorkerKey {
        repo_root: request.repo_root.clone(),
        actor_id: request.actor_id.clone(),
    };

    // Lock the workers map for atomic get-or-create
    let tx = {
        let mut workers_guard = workers.lock().await;

        if let Some(handle) = workers_guard.get(&key) {
            handle.tx.clone()
        } else {
            // Create worker while holding the lock to prevent double-creation
            match create_worker(
                &mut workers_guard,
                CreateWorkerParams {
                    key: key.clone(),
                    repo_root: PathBuf::from(&request.repo_root),
                    actor_id: request.actor_id.clone(),
                    data_dir: PathBuf::from(&request.data_dir),
                    notify_tx: notify_tx.clone(),
                    host_id: host_id.to_string(),
                    ipc_endpoint: ipc_endpoint.to_string(),
                },
            ) {
                Ok(tx) => tx,
                Err(e) => {
                    return IpcResponse::error(
                        request.request_id,
                        "worker_creation_failed".to_string(),
                        e.to_string(),
                    );
                }
            }
        }
    };

    // Send to worker (lock is released)
    send_to_worker(&request, tx).await
}

/// Parameters for creating a new worker
struct CreateWorkerParams {
    key: WorkerKey,
    repo_root: PathBuf,
    actor_id: String,
    data_dir: PathBuf,
    notify_tx: mpsc::Sender<Notification>,
    host_id: String,
    ipc_endpoint: String,
}

/// Create a new worker and insert it into the workers map.
///
/// Must be called while holding the workers lock.
fn create_worker(
    workers: &mut HashMap<WorkerKey, WorkerHandle>,
    params: CreateWorkerParams,
) -> Result<mpsc::Sender<WorkerMessage>, DaemonError> {
    let (tx, rx) = mpsc::channel(100);
    let worker = Worker::new(
        params.repo_root.clone(),
        params.actor_id.clone(),
        params.data_dir,
        rx,
        params.notify_tx,
        params.host_id,
        params.ipc_endpoint,
    )?;

    let join_handle = tokio::spawn(worker.run());

    workers.insert(
        params.key,
        WorkerHandle {
            tx: tx.clone(),
            join_handle: Some(join_handle),
            repo_root: params.repo_root,
            actor_id: params.actor_id,
        },
    );

    Ok(tx)
}

/// Send a request to an existing worker and wait for the response
async fn send_to_worker(
    request: &IpcRequest,
    tx: mpsc::Sender<WorkerMessage>,
) -> IpcResponse {
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let msg = WorkerMessage::Command {
        request_id: request.request_id.clone(),
        command: request.command.clone(),
        response_tx,
    };

    if tx.send(msg).await.is_err() {
        return IpcResponse::error(
            request.request_id.clone(),
            "worker_unavailable".to_string(),
            "Worker channel closed".to_string(),
        );
    }

    // Wait for response with timeout
    match tokio::time::timeout(Duration::from_secs(30), response_rx).await {
        Ok(Ok(response)) => response,
        Ok(Err(_)) => IpcResponse::error(
            request.request_id.clone(),
            "worker_error".to_string(),
            "Worker response channel dropped".to_string(),
        ),
        Err(_) => IpcResponse::error(
            request.request_id.clone(),
            "timeout".to_string(),
            "Worker timed out".to_string(),
        ),
    }
}

/// Get a stable host identifier
fn get_host_id() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string())
}
