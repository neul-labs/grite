//! Supervisor module - manages workers and IPC sockets
//!
//! The supervisor:
//! - Listens on a Unix socket for commands
//! - Manages worker lifecycle
//! - Routes commands to appropriate workers
//! - Broadcasts notifications via internal channels

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use libgrite_ipc::{
    framing::{read_framed_async, write_framed_async},
    messages::{ArchivedIpcRequest, IpcRequest, IpcResponse},
    IpcCommand, Notification, IPC_SCHEMA_VERSION,
};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, Mutex, Semaphore};
use tracing::{debug, info, warn};

use crate::error::DaemonError;
use crate::state::{AtomicSupervisorState, SupervisorState};
use crate::worker::{Worker, WorkerMessage};

/// Maximum concurrent connections the daemon will handle
const MAX_CONNECTIONS: usize = 256;

/// Worker handle for communication
struct WorkerHandle {
    tx: mpsc::Sender<WorkerMessage>,
    join_handle: Option<tokio::task::JoinHandle<()>>,
    repo_root: PathBuf,
    state: Option<Arc<crate::state::AtomicWorkerState>>,
}

/// Key for worker lookup — one worker per repository
#[derive(Hash, Eq, PartialEq, Clone)]
struct WorkerKey {
    repo_root: String,
}

/// Shared daemon state accessible from all connection tasks.
///
/// Wrapped in `Arc` and passed to every spawned connection task,
/// replacing the previous pattern of cloning 8+ individual values.
struct DaemonState {
    daemon_id: String,
    host_id: String,
    pid: u32,
    started_ts: u64,
    socket_path: String,
    workers: Mutex<HashMap<WorkerKey, WorkerHandle>>,
    notify_tx: mpsc::Sender<Notification>,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
    conn_semaphore: Arc<Semaphore>,
    last_activity_ms: AtomicU64,
    start_instant: Instant,
    idle_timeout: Option<Duration>,
    supervisor_state: AtomicSupervisorState,
}

impl DaemonState {
    fn touch_activity(&self) {
        let elapsed_ms = self.start_instant.elapsed().as_millis() as u64;
        self.last_activity_ms.store(elapsed_ms, Ordering::Relaxed);
    }
}

/// Supervisor manages workers and IPC
pub struct Supervisor {
    state: Arc<DaemonState>,
    notify_rx: mpsc::Receiver<Notification>,
}

impl Supervisor {
    /// Create a new supervisor
    pub fn new(socket_path: String, idle_timeout: Option<Duration>) -> Self {
        let (notify_tx, notify_rx) = mpsc::channel(1000);
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let start_instant = Instant::now();

        let started_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let state = Arc::new(DaemonState {
            daemon_id: uuid::Uuid::new_v4().to_string(),
            host_id: get_host_id(),
            pid: std::process::id(),
            started_ts,
            socket_path,
            workers: Mutex::new(HashMap::new()),
            notify_tx,
            shutdown_tx,
            conn_semaphore: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            last_activity_ms: AtomicU64::new(0),
            start_instant,
            idle_timeout,
            supervisor_state: AtomicSupervisorState::new(SupervisorState::Starting),
        });

        Self { state, notify_rx }
    }

    /// Run the supervisor until shutdown.
    ///
    /// Shutdown is triggered by either:
    /// - The external `shutdown_signal` future resolving (e.g. SIGTERM)
    /// - An internal trigger (idle timeout, DaemonStop command)
    ///
    /// All cleanup (socket removal, worker shutdown) is handled here.
    pub async fn run(
        mut self,
        shutdown_signal: impl Future<Output = ()> + Send,
    ) -> Result<(), DaemonError> {
        info!(
            daemon_id = %self.state.daemon_id,
            socket_path = %self.state.socket_path,
            idle_timeout_secs = ?self.state.idle_timeout.map(|d| d.as_secs()),
            "Supervisor starting"
        );

        // Initialize last activity to now
        self.state.touch_activity();

        // Clean up stale socket file, but only if no live supervisor owns it
        let socket_path = Path::new(&self.state.socket_path);
        if socket_path.exists() {
            if std::os::unix::net::UnixStream::connect(socket_path).is_ok() {
                return Err(DaemonError::BindFailed(format!(
                    "Another supervisor is already listening on {}",
                    self.state.socket_path,
                )));
            }
            std::fs::remove_file(socket_path).map_err(|e| {
                DaemonError::BindFailed(format!(
                    "Failed to remove stale socket {}: {}",
                    self.state.socket_path, e
                ))
            })?;
        }

        // Bind Unix listener
        let listener = UnixListener::bind(&self.state.socket_path).map_err(|e| {
            DaemonError::BindFailed(format!(
                "Failed to bind to {}: {}",
                self.state.socket_path, e
            ))
        })?;

        info!("Listening on {}", self.state.socket_path);
        self.state
            .supervisor_state
            .transition(SupervisorState::Running, Ordering::SeqCst)
            .ok();

        // Spawn heartbeat task (also checks idle timeout)
        let state_hb = self.state.clone();
        let mut heartbeat_shutdown = self.state.shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Send heartbeats to workers
                        let workers = state_hb.workers.lock().await;
                        for handle in workers.values() {
                            let _ = handle.tx.send(WorkerMessage::Heartbeat).await;
                        }
                        drop(workers);

                        // Check idle timeout
                        if let Some(timeout) = state_hb.idle_timeout {
                            let last_ms = state_hb.last_activity_ms.load(Ordering::Relaxed);
                            let now_ms = state_hb.start_instant.elapsed().as_millis() as u64;
                            let idle_ms = now_ms.saturating_sub(last_ms);
                            if idle_ms >= timeout.as_millis() as u64 {
                                info!("Idle timeout reached ({} ms), shutting down", idle_ms);
                                let _ = state_hb.shutdown_tx.send(());
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
        let mut notify_shutdown = self.state.shutdown_tx.subscribe();
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

        // Main accept loop
        let mut internal_shutdown = self.state.shutdown_tx.subscribe();
        tokio::pin!(shutdown_signal);

        loop {
            tokio::select! {
                _ = &mut shutdown_signal => {
                    info!("Received shutdown signal");
                    break;
                }
                _ = internal_shutdown.recv() => {
                    info!("Internal shutdown signal received");
                    break;
                }
                result = listener.accept() => {
                    match result {
                        Ok((stream, _addr)) => {
                            let permit = match self.state.conn_semaphore.clone().try_acquire_owned() {
                                Ok(permit) => permit,
                                Err(_) => {
                                    warn!("Connection limit reached ({}), dropping connection", MAX_CONNECTIONS);
                                    continue;
                                }
                            };
                            let state = self.state.clone();
                            tokio::spawn(async move {
                                state.touch_activity();
                                handle_connection(stream, &state).await;
                                state.touch_activity();
                                drop(permit);
                            });
                        }
                        Err(e) => {
                            warn!("Accept error: {}", e);
                        }
                    }
                }
            }
        }

        // === Single cleanup path ===
        self.state
            .supervisor_state
            .transition(SupervisorState::ShuttingDown, Ordering::SeqCst)
            .ok();

        // Signal background tasks (heartbeat, notifications) to stop.
        // This is a no-op if shutdown was already triggered internally
        // (idle timeout / DaemonStop), since those tasks already received
        // the broadcast — the second send simply has no receivers.
        let _ = self.state.shutdown_tx.send(());

        // Clean up socket file
        let _ = std::fs::remove_file(&self.state.socket_path);

        // Stop accepting new connections so no new tasks can spawn.
        drop(listener);

        // Wait for all in-flight connection tasks to finish (they each
        // hold a semaphore permit). This prevents the race where a
        // connection task inserts a new worker after we drain the map.
        let _ = tokio::time::timeout(
            Duration::from_secs(10),
            self.state.conn_semaphore.acquire_many(MAX_CONNECTIONS as u32),
        )
        .await;

        // Now safe to drain — no connection tasks are running
        shutdown_workers(&self.state).await;

        self.state
            .supervisor_state
            .transition(SupervisorState::Stopped, Ordering::SeqCst)
            .ok();

        info!("Supervisor stopped");
        Ok(())
    }
}

/// Drain workers from the map and shut them down.
///
/// The mutex is released before sending shutdown messages or awaiting
/// join handles, preventing deadlocks with in-flight connection tasks
/// that may be waiting to insert new workers.
async fn shutdown_workers(state: &DaemonState) {
    let handles: Vec<WorkerHandle> = {
        let mut workers = state.workers.lock().await;
        workers.drain().map(|(_, h)| h).collect()
    };
    // Mutex released — in-flight connection tasks can now complete

    for handle in &handles {
        let _ = handle.tx.send(WorkerMessage::Shutdown).await;
    }

    for mut handle in handles {
        if let Some(jh) = handle.join_handle.take() {
            match tokio::time::timeout(Duration::from_secs(10), jh).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => warn!("Worker task panicked: {}", e),
                Err(_) => warn!(
                    "Worker {} didn't shut down within 10s",
                    handle.repo_root.display()
                ),
            }
        }
    }
}

/// Handle a single client connection: read one request, send one response
async fn handle_connection(mut stream: UnixStream, state: &DaemonState) {
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

    let response = process_request(&request_bytes, state).await;

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
async fn process_request(raw: &[u8], state: &DaemonState) -> IpcResponse {
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

    // Handle daemon-level commands at the supervisor, not in workers
    match &request.command {
        IpcCommand::DaemonStop => {
            let _ = state.shutdown_tx.send(());
            return IpcResponse::success(
                request.request_id,
                Some(serde_json::json!({"stopping": true}).to_string()),
            );
        }
        IpcCommand::DaemonStatus => {
            let workers_guard = state.workers.lock().await;
            let worker_count = workers_guard.len();
            drop(workers_guard);

            let supervisor_state = format!("{:?}", state.supervisor_state.load(Ordering::SeqCst));
            return IpcResponse::success(
                request.request_id,
                Some(
                    serde_json::json!({
                        "running": true,
                        "daemon_id": state.daemon_id,
                        "pid": state.pid,
                        "host_id": state.host_id,
                        "ipc_endpoint": state.socket_path,
                        "started_ts": state.started_ts,
                        "worker_count": worker_count,
                        "state": supervisor_state,
                    })
                    .to_string(),
                ),
            );
        }
        _ => {}
    }

    // Route to worker
    route_to_worker(request, state).await
}

/// Route a request to the appropriate worker, creating one if needed.
///
/// If the worker's channel is dead (task panicked or exited), the stale
/// handle is removed and a fresh worker is spawned automatically.
///
/// Uses double-checked locking: the workers mutex is NOT held during
/// `Worker::new` (which does blocking sled I/O). If two tasks race to
/// create the same worker, the loser finds the winner's entry on re-check.
async fn route_to_worker(request: IpcRequest, state: &DaemonState) -> IpcResponse {
    let key = WorkerKey {
        repo_root: request.repo_root.clone(),
    };

    // Fast path: check for existing live worker (mutex held briefly)
    {
        let mut workers_guard = state.workers.lock().await;

        // Remove dead worker handles
        if let Some(handle) = workers_guard.get(&key) {
            if handle.tx.is_closed() {
                warn!(
                    repo = %handle.repo_root.display(),
                    "Removing dead worker handle"
                );
                workers_guard.remove(&key);
            }
        }

        if let Some(handle) = workers_guard.get(&key) {
            let tx = handle.tx.clone();
            drop(workers_guard);
            return send_to_worker(&request, tx).await;
        }
    }
    // Mutex released — slow path: create worker on blocking thread pool.
    // Worker::new opens the sled store which can block for seconds.
    let (tx, rx) = mpsc::channel(100);
    let repo_root = PathBuf::from(&request.repo_root);
    let actor_id = request.actor_id.clone();
    let ntx = state.notify_tx.clone();
    let hid = state.host_id.clone();
    let ipc = state.socket_path.clone();

    let worker_result = tokio::task::spawn_blocking(move || {
        Worker::new(repo_root, actor_id, rx, ntx, hid, ipc)
    })
    .await;

    let worker = match worker_result {
        Ok(Ok(w)) => w,
        Ok(Err(e)) => {
            // Creation failed — another task may have won the race.
            // Re-check the map before returning an error.
            let workers_guard = state.workers.lock().await;
            if let Some(handle) = workers_guard.get(&key) {
                if !handle.tx.is_closed() {
                    let tx = handle.tx.clone();
                    drop(workers_guard);
                    return send_to_worker(&request, tx).await;
                }
            }
            return IpcResponse::error(
                request.request_id,
                "worker_creation_failed".to_string(),
                e.to_string(),
            );
        }
        Err(e) => {
            return IpcResponse::error(
                request.request_id,
                "worker_creation_failed".to_string(),
                format!("Worker creation panicked: {}", e),
            );
        }
    };

    // Re-acquire lock and insert (double-check for races)
    {
        let mut workers_guard = state.workers.lock().await;

        // Another task may have created a worker for this key while we
        // were blocked. If so, use theirs and drop ours.
        if let Some(handle) = workers_guard.get(&key) {
            if !handle.tx.is_closed() {
                let tx = handle.tx.clone();
                drop(workers_guard);
                // Our worker is dropped here — its sled lock releases on Drop
                return send_to_worker(&request, tx).await;
            }
            workers_guard.remove(&key);
        }

        let repo_root = worker.repo_root.clone();
        let worker_state = Some(worker.state.clone());
        let join_handle = tokio::spawn(worker.run());

        workers_guard.insert(
            key,
            WorkerHandle {
                tx: tx.clone(),
                join_handle: Some(join_handle),
                repo_root,
                state: worker_state,
            },
        );
    }

    send_to_worker(&request, tx).await
}

/// Send a request to an existing worker and wait for the response
async fn send_to_worker(
    request: &IpcRequest,
    tx: mpsc::Sender<WorkerMessage>,
) -> IpcResponse {
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let msg = WorkerMessage::Command {
        request_id: request.request_id.clone(),
        actor_id: request.actor_id.clone(),
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
