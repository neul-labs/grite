//! Supervisor module - manages workers and IPC sockets
//!
//! The supervisor:
//! - Listens on REQ/REP socket for commands
//! - Manages worker lifecycle
//! - Routes commands to appropriate workers
//! - Handles discovery requests
//! - Broadcasts notifications via PUB socket

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use libgrit_ipc::{
    messages::{ArchivedIpcRequest, IpcRequest, IpcResponse},
    IpcCommand, Notification, IPC_SCHEMA_VERSION,
};
use nng::{options::Options, Message, Protocol, Socket};
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, error, info, warn};

use crate::error::DaemonError;
use crate::worker::{Worker, WorkerMessage};

/// Worker handle for communication
struct WorkerHandle {
    tx: mpsc::Sender<WorkerMessage>,
    repo_root: PathBuf,
    actor_id: String,
    data_dir: PathBuf,
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
    /// IPC endpoint
    ipc_endpoint: String,
    /// Workers by (repo_root, actor_id)
    workers: Arc<RwLock<HashMap<WorkerKey, WorkerHandle>>>,
    /// Notification channel
    notify_rx: mpsc::Receiver<Notification>,
    /// Notification sender (cloned to workers)
    notify_tx: mpsc::Sender<Notification>,
    /// Shutdown signal
    shutdown_tx: Option<tokio::sync::broadcast::Sender<()>>,
}

impl Supervisor {
    /// Create a new supervisor
    pub fn new(ipc_endpoint: String) -> Self {
        let (notify_tx, notify_rx) = mpsc::channel(1000);

        Self {
            daemon_id: uuid::Uuid::new_v4().to_string(),
            host_id: get_host_id(),
            ipc_endpoint,
            workers: Arc::new(RwLock::new(HashMap::new())),
            notify_rx,
            notify_tx,
            shutdown_tx: None,
        }
    }

    /// Run the supervisor
    pub async fn run(mut self) -> Result<(), DaemonError> {
        info!(
            daemon_id = %self.daemon_id,
            endpoint = %self.ipc_endpoint,
            "Supervisor starting"
        );

        // Create shutdown channel
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        // Create REQ/REP socket for commands
        let rep_socket = Socket::new(Protocol::Rep0)?;
        rep_socket
            .set_opt::<nng::options::RecvTimeout>(Some(Duration::from_millis(100)))
            .map_err(|e| DaemonError::BindFailed(e.to_string()))?;
        rep_socket
            .listen(&self.ipc_endpoint)
            .map_err(|e| DaemonError::BindFailed(format!("Failed to bind to {}: {}", self.ipc_endpoint, e)))?;

        info!("Listening on {}", self.ipc_endpoint);

        // Create PUB socket for notifications
        let pub_endpoint = format!("{}-pub", self.ipc_endpoint);
        let pub_socket = Socket::new(Protocol::Pub0)?;
        let _ = pub_socket.listen(&pub_endpoint); // Optional - may fail if not supported

        // Spawn heartbeat task
        let workers_clone = self.workers.clone();
        let mut heartbeat_shutdown = shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let workers = workers_clone.read().await;
                        for handle in workers.values() {
                            let _ = handle.tx.send(WorkerMessage::Heartbeat).await;
                        }
                    }
                    _ = heartbeat_shutdown.recv() => {
                        break;
                    }
                }
            }
        });

        // Take notify_rx for the notification publisher
        let mut notify_rx = std::mem::replace(
            &mut self.notify_rx,
            mpsc::channel(1).1, // Replace with dummy receiver
        );
        let mut pub_shutdown = shutdown_tx.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(notification) = notify_rx.recv() => {
                        // Serialize and publish
                        if let Ok(bytes) = rkyv::to_bytes::<rkyv::rancor::Error>(&notification) {
                            let msg = Message::from(bytes.as_slice());
                            let _ = pub_socket.send(msg);
                        }
                    }
                    _ = pub_shutdown.recv() => {
                        break;
                    }
                }
            }
        });

        // Main command loop
        let mut shutdown_rx = shutdown_tx.subscribe();
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    break;
                }
                result = tokio::task::spawn_blocking({
                    let socket = rep_socket.clone();
                    move || socket.recv()
                }) => {
                    match result {
                        Ok(Ok(msg)) => {
                            let response = self.handle_request(&msg).await;
                            if let Ok(bytes) = rkyv::to_bytes::<rkyv::rancor::Error>(&response) {
                                let reply = Message::from(bytes.as_slice());
                                if let Err(e) = rep_socket.send(reply) {
                                    warn!("Failed to send response: {:?}", e);
                                }
                            }
                        }
                        Ok(Err(nng::Error::TimedOut)) => {
                            // Normal timeout, continue loop
                            continue;
                        }
                        Ok(Err(e)) => {
                            warn!("Receive error: {}", e);
                        }
                        Err(e) => {
                            error!("Task join error: {}", e);
                        }
                    }
                }
            }
        }

        // Shutdown all workers
        self.shutdown_workers().await;

        info!("Supervisor stopped");
        Ok(())
    }

    /// Handle an incoming request
    async fn handle_request(&self, msg: &Message) -> IpcResponse {
        // Deserialize request
        let archived = match rkyv::access::<ArchivedIpcRequest, rkyv::rancor::Error>(msg) {
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
        let request: IpcRequest = match rkyv::deserialize::<IpcRequest, rkyv::rancor::Error>(archived) {
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
            if let Some(ref tx) = self.shutdown_tx {
                let _ = tx.send(());
            }
            return IpcResponse::success(
                request.request_id,
                Some(serde_json::json!({"stopping": true}).to_string()),
            );
        }

        // Route to worker
        self.route_to_worker(request).await
    }

    /// Route a request to the appropriate worker
    async fn route_to_worker(&self, request: IpcRequest) -> IpcResponse {
        let key = WorkerKey {
            repo_root: request.repo_root.clone(),
            actor_id: request.actor_id.clone(),
        };

        // Get or create worker
        let tx = {
            let workers = self.workers.read().await;
            workers.get(&key).map(|h| h.tx.clone())
        };

        let tx = match tx {
            Some(tx) => tx,
            None => {
                // Try to create worker
                match self.create_worker(
                    PathBuf::from(&request.repo_root),
                    request.actor_id.clone(),
                    PathBuf::from(&request.data_dir),
                ).await {
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

        // Send command to worker
        let (response_tx, response_rx) = oneshot::channel();
        let msg = WorkerMessage::Command {
            request_id: request.request_id.clone(),
            command: request.command,
            response_tx,
        };

        if let Err(_) = tx.send(msg).await {
            return IpcResponse::error(
                request.request_id,
                "worker_unavailable".to_string(),
                "Worker channel closed".to_string(),
            );
        }

        // Wait for response
        match tokio::time::timeout(Duration::from_secs(30), response_rx).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => IpcResponse::error(
                request.request_id,
                "worker_error".to_string(),
                "Worker response channel dropped".to_string(),
            ),
            Err(_) => IpcResponse::error(
                request.request_id,
                "timeout".to_string(),
                "Worker timed out".to_string(),
            ),
        }
    }

    /// Create a new worker
    async fn create_worker(
        &self,
        repo_root: PathBuf,
        actor_id: String,
        data_dir: PathBuf,
    ) -> Result<mpsc::Sender<WorkerMessage>, DaemonError> {
        let key = WorkerKey {
            repo_root: repo_root.to_string_lossy().to_string(),
            actor_id: actor_id.clone(),
        };

        // Check if already exists
        {
            let workers = self.workers.read().await;
            if let Some(handle) = workers.get(&key) {
                return Ok(handle.tx.clone());
            }
        }

        // Create worker
        let (tx, rx) = mpsc::channel(100);
        let worker = Worker::new(
            repo_root.clone(),
            actor_id.clone(),
            data_dir.clone(),
            rx,
            self.notify_tx.clone(),
            self.host_id.clone(),
            self.ipc_endpoint.clone(),
        )?;

        // Spawn worker task using spawn_blocking since worker does sync I/O
        tokio::task::spawn_blocking(move || {
            // Create a new tokio runtime for the worker
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(worker.run());
        });

        // Store handle
        {
            let mut workers = self.workers.write().await;
            workers.insert(
                key,
                WorkerHandle {
                    tx: tx.clone(),
                    repo_root,
                    actor_id,
                    data_dir,
                },
            );
        }

        Ok(tx)
    }

    /// Shutdown all workers
    async fn shutdown_workers(&self) {
        let workers = self.workers.read().await;
        for handle in workers.values() {
            let _ = handle.tx.send(WorkerMessage::Shutdown).await;
        }

        // Give workers time to cleanup
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Get a stable host identifier
fn get_host_id() -> String {
    // Try to get hostname, fallback to random UUID
    std::env::var("HOSTNAME")
        .or_else(|_| {
            std::fs::read_to_string("/etc/hostname")
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string())
}
