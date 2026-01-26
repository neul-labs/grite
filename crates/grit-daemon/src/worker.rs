//! Worker module - handles commands for a single (repo, actor) pair
//!
//! Each worker owns exclusive access to the sled database for its actor.
//! Commands are processed concurrently using tokio tasks, with sled's
//! internal MVCC handling concurrent access safely.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use libgrit_core::types::ids::{hex_to_id, ActorId};
use libgrit_core::{GritError, GritStore, LockedStore};
use libgrit_core::store::IssueFilter;
use libgrit_ipc::{DaemonLock, IpcCommand, IpcResponse, Notification};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::error::DaemonError;

/// Message sent to a worker
pub enum WorkerMessage {
    /// Execute a command
    Command {
        request_id: String,
        command: IpcCommand,
        response_tx: tokio::sync::oneshot::Sender<IpcResponse>,
    },
    /// Refresh the heartbeat
    Heartbeat,
    /// Shutdown the worker
    Shutdown,
}

/// Worker state for a single (repo, actor) pair
pub struct Worker {
    /// Repository root path
    pub repo_root: PathBuf,
    /// Actor ID (hex)
    pub actor_id: String,
    /// Actor ID (bytes)
    actor_id_bytes: ActorId,
    /// Data directory
    pub data_dir: PathBuf,
    /// Sled store path
    sled_path: PathBuf,
    /// Git directory
    git_dir: PathBuf,
    /// Sled store with filesystem lock (shared for concurrent access)
    store: Arc<LockedStore>,
    /// Channel for receiving messages
    rx: mpsc::Receiver<WorkerMessage>,
    /// Notification sender
    notify_tx: mpsc::Sender<Notification>,
    /// Host ID for this daemon
    host_id: String,
    /// IPC endpoint
    ipc_endpoint: String,
}

impl Worker {
    /// Create a new worker
    pub fn new(
        repo_root: PathBuf,
        actor_id: String,
        data_dir: PathBuf,
        rx: mpsc::Receiver<WorkerMessage>,
        notify_tx: mpsc::Sender<Notification>,
        host_id: String,
        ipc_endpoint: String,
    ) -> Result<Self, DaemonError> {
        let git_dir = repo_root.join(".git");
        let sled_path = data_dir.join("sled");

        // Parse actor ID
        let actor_id_bytes = hex_to_id(&actor_id)
            .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;

        // Open store with filesystem lock (blocking with timeout)
        // This ensures exclusive process-level access to the sled database
        let store = Arc::new(GritStore::open_locked_blocking(
            &sled_path,
            Duration::from_secs(5),
        )?);

        Ok(Self {
            repo_root,
            actor_id,
            actor_id_bytes,
            data_dir,
            sled_path,
            git_dir,
            store,
            rx,
            notify_tx,
            host_id,
            ipc_endpoint,
        })
    }

    /// Acquire the daemon lock
    pub fn acquire_lock(&self) -> Result<DaemonLock, DaemonError> {
        DaemonLock::acquire(
            &self.data_dir,
            self.repo_root.to_string_lossy().to_string(),
            self.actor_id.clone(),
            self.host_id.clone(),
            self.ipc_endpoint.clone(),
        )
        .map_err(|e| DaemonError::LockFailed(e.to_string()))
    }

    /// Refresh the daemon lock heartbeat
    pub fn refresh_lock(&self) -> Result<(), DaemonError> {
        if let Ok(Some(mut lock)) = DaemonLock::read(&self.data_dir) {
            if lock.is_owned_by_current_process() {
                lock.refresh();
                lock.write(&self.data_dir)?;
            }
        }
        Ok(())
    }

    /// Run the worker event loop
    pub async fn run(mut self) {
        info!(
            repo = %self.repo_root.display(),
            actor = %self.actor_id,
            "Worker started"
        );

        // Acquire lock
        match self.acquire_lock() {
            Ok(_lock) => {
                debug!("Daemon lock acquired");
            }
            Err(e) => {
                error!("Failed to acquire lock: {}", e);
                return;
            }
        }

        // Notify worker started
        let _ = self
            .notify_tx
            .send(Notification::WorkerStarted {
                repo_root: self.repo_root.to_string_lossy().to_string(),
                actor_id: self.actor_id.clone(),
            })
            .await;

        // Event loop - commands are spawned as concurrent tasks
        while let Some(msg) = self.rx.recv().await {
            match msg {
                WorkerMessage::Command {
                    request_id,
                    command,
                    response_tx,
                } => {
                    // Clone data needed for the spawned task
                    let store = Arc::clone(&self.store);
                    let actor_id_bytes = self.actor_id_bytes;
                    let sled_path = self.sled_path.clone();
                    let data_dir = self.data_dir.clone();
                    let git_dir = self.git_dir.clone();

                    // Spawn task for concurrent command execution
                    // sled's MVCC handles concurrent access safely
                    tokio::spawn(async move {
                        let response = execute_command(
                            &store,
                            actor_id_bytes,
                            &sled_path,
                            &data_dir,
                            &git_dir,
                            &request_id,
                            &command,
                        );
                        let _ = response_tx.send(response);
                    });
                }
                WorkerMessage::Heartbeat => {
                    if let Err(e) = self.refresh_lock() {
                        warn!("Failed to refresh lock: {}", e);
                    }
                }
                WorkerMessage::Shutdown => {
                    info!("Worker shutdown requested");
                    break;
                }
            }
        }

        // Cleanup
        self.shutdown();
    }

    /// Shutdown cleanup
    fn shutdown(&self) {
        // Release lock
        if let Err(e) = DaemonLock::release(&self.data_dir) {
            warn!("Failed to release lock: {}", e);
        }

        // Flush store
        if let Err(e) = self.store.flush() {
            warn!("Failed to flush store: {}", e);
        }

        info!(
            repo = %self.repo_root.display(),
            actor = %self.actor_id,
            "Worker stopped"
        );
    }
}

/// Execute a command with the given context.
///
/// This is a standalone function to enable concurrent execution via tokio::spawn.
fn execute_command(
    store: &LockedStore,
    actor_id_bytes: ActorId,
    sled_path: &PathBuf,
    data_dir: &PathBuf,
    git_dir: &PathBuf,
    request_id: &str,
    command: &IpcCommand,
) -> IpcResponse {
    let result = execute_command_inner(store, actor_id_bytes, sled_path, data_dir, git_dir, command);

    match result {
        Ok(data) => IpcResponse::success(request_id.to_string(), data),
        Err(e) => {
            let (code, message) = error_to_code_message(&e);
            IpcResponse::error(request_id.to_string(), code, message)
        }
    }
}

/// Inner command execution logic
fn execute_command_inner(
    store: &LockedStore,
    actor_id_bytes: ActorId,
    sled_path: &PathBuf,
    data_dir: &PathBuf,
    git_dir: &PathBuf,
    command: &IpcCommand,
) -> Result<Option<String>, DaemonError> {
    use libgrit_core::hash::compute_event_id;
    use libgrit_core::types::event::{Event, EventKind, IssueState};
    use libgrit_core::types::ids::{generate_issue_id, id_to_hex};
    use libgrit_core::types::issue::IssueProjection;
    use libgrit_core::export::{export_json, export_markdown, ExportSince};
    use libgrit_git::SyncManager;

    match command {
        IpcCommand::IssueList { state, label } => {
            let filter = IssueFilter {
                state: state.as_ref().map(|s| match s.as_str() {
                    "open" => IssueState::Open,
                    "closed" => IssueState::Closed,
                    _ => IssueState::Open,
                }),
                label: label.clone(),
            };
            let issues = store.list_issues(&filter)?;
            let json = serde_json::to_string(&serde_json::json!({ "issues": issues }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueShow { issue_id } => {
            let id = hex_to_id(issue_id)
                .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
            let projection = store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;
            let json = serde_json::to_string(&projection)?;
            Ok(Some(json))
        }

        IpcCommand::IssueCreate { title, body, labels } => {
            let issue_id = generate_issue_id();
            let ts = current_time_ms();
            let kind = EventKind::IssueCreated {
                title: title.clone(),
                body: body.clone(),
                labels: labels.clone(),
            };
            let event_id = compute_event_id(&issue_id, &actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, issue_id, actor_id_bytes, ts, None, kind);

            store.insert_event(&event)?;
            store.flush()?;

            let projection = IssueProjection::from_event(&event)?;
            let json = serde_json::to_string(&serde_json::json!({
                "issue_id": id_to_hex(&issue_id),
                "event_id": id_to_hex(&event_id),
                "projection": projection,
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueUpdate { issue_id, title, body } => {
            if title.is_none() && body.is_none() {
                return Err(DaemonError::Grit(GritError::InvalidArgs(
                    "At least one of title or body must be provided".to_string()
                )));
            }

            let id = hex_to_id(issue_id)
                .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

            let ts = current_time_ms();
            let kind = EventKind::IssueUpdated {
                title: title.clone(),
                body: body.clone(),
            };
            let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);

            store.insert_event(&event)?;
            store.flush()?;

            let json = serde_json::to_string(&serde_json::json!({
                "issue_id": issue_id,
                "event_id": id_to_hex(&event_id),
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueComment { issue_id, body } => {
            let id = hex_to_id(issue_id)
                .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

            let ts = current_time_ms();
            let kind = EventKind::CommentAdded { body: body.clone() };
            let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);

            store.insert_event(&event)?;
            store.flush()?;

            let json = serde_json::to_string(&serde_json::json!({
                "issue_id": issue_id,
                "event_id": id_to_hex(&event_id),
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueClose { issue_id } => {
            let id = hex_to_id(issue_id)
                .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

            let ts = current_time_ms();
            let kind = EventKind::StateChanged { state: IssueState::Closed };
            let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);

            store.insert_event(&event)?;
            store.flush()?;

            let json = serde_json::to_string(&serde_json::json!({
                "issue_id": issue_id,
                "event_id": id_to_hex(&event_id),
                "state": "closed",
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueReopen { issue_id } => {
            let id = hex_to_id(issue_id)
                .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

            let ts = current_time_ms();
            let kind = EventKind::StateChanged { state: IssueState::Open };
            let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);

            store.insert_event(&event)?;
            store.flush()?;

            let json = serde_json::to_string(&serde_json::json!({
                "issue_id": issue_id,
                "event_id": id_to_hex(&event_id),
                "state": "open",
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueLabel { issue_id, add, remove } => {
            let id = hex_to_id(issue_id)
                .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

            let mut event_ids = Vec::new();
            let ts = current_time_ms();

            for label in add {
                let kind = EventKind::LabelAdded { label: label.clone() };
                let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
                let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);
                store.insert_event(&event)?;
                event_ids.push(id_to_hex(&event_id));
            }

            for label in remove {
                let kind = EventKind::LabelRemoved { label: label.clone() };
                let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
                let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);
                store.insert_event(&event)?;
                event_ids.push(id_to_hex(&event_id));
            }

            store.flush()?;

            let json = serde_json::to_string(&serde_json::json!({
                "issue_id": issue_id,
                "event_ids": event_ids,
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueAssign { issue_id, add, remove } => {
            let id = hex_to_id(issue_id)
                .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

            let mut event_ids = Vec::new();
            let ts = current_time_ms();

            for user in add {
                let kind = EventKind::AssigneeAdded { user: user.clone() };
                let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
                let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);
                store.insert_event(&event)?;
                event_ids.push(id_to_hex(&event_id));
            }

            for user in remove {
                let kind = EventKind::AssigneeRemoved { user: user.clone() };
                let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
                let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);
                store.insert_event(&event)?;
                event_ids.push(id_to_hex(&event_id));
            }

            store.flush()?;

            let json = serde_json::to_string(&serde_json::json!({
                "issue_id": issue_id,
                "event_ids": event_ids,
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueLink { issue_id, url, note } => {
            let id = hex_to_id(issue_id)
                .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

            let ts = current_time_ms();
            let kind = EventKind::LinkAdded {
                url: url.clone(),
                note: note.clone(),
            };
            let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);

            store.insert_event(&event)?;
            store.flush()?;

            let json = serde_json::to_string(&serde_json::json!({
                "issue_id": issue_id,
                "event_id": id_to_hex(&event_id),
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueAttach { issue_id, file_path } => {
            let id = hex_to_id(issue_id)
                .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

            let parts: Vec<&str> = file_path.splitn(3, ':').collect();
            if parts.len() != 3 {
                return Err(DaemonError::Grit(GritError::InvalidArgs(
                    "file_path must be in format 'name:sha256:mime'".to_string()
                )));
            }

            let name = parts[0].to_string();
            let sha256: [u8; 32] = hex_to_id(parts[1])
                .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
            let mime = parts[2].to_string();

            let ts = current_time_ms();
            let kind = EventKind::AttachmentAdded { name, sha256, mime };
            let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);

            store.insert_event(&event)?;
            store.flush()?;

            let json = serde_json::to_string(&serde_json::json!({
                "issue_id": issue_id,
                "event_id": id_to_hex(&event_id),
            }))?;
            Ok(Some(json))
        }

        IpcCommand::DbStats => {
            let stats = store.stats(sled_path)?;
            let json = serde_json::to_string(&serde_json::json!({
                "path": stats.path,
                "size_bytes": stats.size_bytes,
                "event_count": stats.event_count,
                "issue_count": stats.issue_count,
                "last_rebuild_ts": stats.last_rebuild_ts,
            }))?;
            Ok(Some(json))
        }

        IpcCommand::Rebuild => {
            let stats = store.rebuild()?;
            let json = serde_json::to_string(&serde_json::json!({
                "event_count": stats.event_count,
                "issue_count": stats.issue_count,
            }))?;
            Ok(Some(json))
        }

        IpcCommand::Export { format, since } => {
            let since_opt = since.as_ref()
                .and_then(|s| s.parse::<u64>().ok())
                .map(ExportSince::Timestamp);

            let output = match format.as_str() {
                "json" => {
                    let export = export_json(store, since_opt)?;
                    serde_json::to_string(&export)?
                }
                "md" | "markdown" => export_markdown(store, since_opt)?,
                _ => return Err(DaemonError::Grit(GritError::InvalidArgs(
                    format!("Unknown format: {}", format)
                ))),
            };
            Ok(Some(output))
        }

        IpcCommand::DaemonStatus => {
            let lock = DaemonLock::read(data_dir)
                .map_err(|e| DaemonError::LockFailed(e.to_string()))?;

            let json = match lock {
                Some(l) => serde_json::to_string(&serde_json::json!({
                    "running": !l.is_expired(),
                    "pid": l.pid,
                    "host_id": l.host_id,
                    "ipc_endpoint": l.ipc_endpoint,
                    "started_ts": l.started_ts,
                    "expires_ts": l.expires_ts,
                }))?,
                None => serde_json::to_string(&serde_json::json!({
                    "running": false,
                }))?,
            };
            Ok(Some(json))
        }

        IpcCommand::DaemonStop => {
            Ok(Some(serde_json::json!({"stopping": true}).to_string()))
        }

        IpcCommand::Sync { remote, pull, push } => {
            let sync_mgr = SyncManager::open(git_dir)?;

            // If neither flag is set, do both pull and push
            let do_pull = *pull || (!*pull && !*push);
            let do_push = *push || (!*pull && !*push);

            let mut result = serde_json::json!({});

            if do_pull && !do_push {
                // Pull only
                let pull_result = sync_mgr.pull(remote)?;
                let wal_head: Option<String> = pull_result.new_wal_head.map(|oid| oid.to_string());
                result = serde_json::json!({
                    "pulled": true,
                    "pushed": false,
                    "pull_events": pull_result.events_pulled,
                    "pull_wal_head": wal_head,
                    "message": pull_result.message,
                });
            } else if do_push && !do_pull {
                // Push only with auto-rebase
                let push_result = sync_mgr.push_with_rebase(remote, &actor_id_bytes)?;
                result = serde_json::json!({
                    "pulled": false,
                    "pushed": true,
                    "push_success": push_result.success,
                    "push_rebased": push_result.rebased,
                    "push_events_rebased": push_result.events_rebased,
                    "message": push_result.message,
                });
            } else {
                // Full sync: pull then push with auto-rebase
                let (pull_result, push_result) = sync_mgr.sync_with_rebase(remote, &actor_id_bytes)?;
                let wal_head: Option<String> = pull_result.new_wal_head.map(|oid| oid.to_string());
                result = serde_json::json!({
                    "pulled": true,
                    "pushed": true,
                    "pull_events": pull_result.events_pulled,
                    "pull_wal_head": wal_head,
                    "push_success": push_result.success,
                    "push_rebased": push_result.rebased,
                    "push_events_rebased": push_result.events_rebased,
                    "message": format!("{} / {}", pull_result.message, push_result.message),
                });
            }

            Ok(Some(result.to_string()))
        }

        IpcCommand::SnapshotCreate | IpcCommand::SnapshotList | IpcCommand::SnapshotGc { .. } => {
            Err(DaemonError::Grit(GritError::Internal(
                "Snapshot through daemon not yet implemented - use --no-daemon".to_string()
            )))
        }
    }
}

/// Get current time in milliseconds since Unix epoch
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Convert error to (code, message) for IPC response
fn error_to_code_message(e: &DaemonError) -> (String, String) {
    use libgrit_ipc::error::codes;

    match e {
        DaemonError::Grit(GritError::NotFound(_)) => (codes::NOT_FOUND.to_string(), e.to_string()),
        DaemonError::Grit(GritError::InvalidArgs(_)) => (codes::INVALID_INPUT.to_string(), e.to_string()),
        DaemonError::Grit(GritError::Io(_)) => (codes::IO_ERROR.to_string(), e.to_string()),
        DaemonError::Git(_) => (codes::GIT_ERROR.to_string(), e.to_string()),
        DaemonError::Ipc(_) | DaemonError::Nng(_) => (codes::IPC_ERROR.to_string(), e.to_string()),
        _ => (codes::INTERNAL.to_string(), e.to_string()),
    }
}
