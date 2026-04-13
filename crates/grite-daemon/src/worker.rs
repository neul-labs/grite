//! Worker module - handles commands for a single repo.
//!
//! Each worker owns exclusive access to the shared sled database for its
//! repository. Commands are processed concurrently using tokio tasks, with
//! sled's internal MVCC handling concurrent access safely.
//!
//! Actor ID is supplied per-command rather than being fixed at worker
//! creation time, reflecting the shared-sled model where actor identity
//! is authorship metadata rather than a storage partition.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use libgrite_core::config::repo_sled_path;
use libgrite_core::types::ids::{hex_to_id, ActorId};
use libgrite_core::{GriteError, GriteStore, LockedStore};
use libgrite_core::store::IssueFilter;
use libgrite_ipc::{DaemonLock, IpcCommand, IpcResponse, Notification};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::error::DaemonError;

/// Message sent to a worker
pub enum WorkerMessage {
    /// Execute a command
    Command {
        request_id: String,
        /// Actor ID (hex) for event authorship
        actor_id: String,
        command: IpcCommand,
        response_tx: tokio::sync::oneshot::Sender<IpcResponse>,
    },
    /// Refresh the heartbeat
    Heartbeat,
    /// Shutdown the worker
    Shutdown,
}

/// Worker state for a single repository
pub struct Worker {
    /// Repository root path
    pub repo_root: PathBuf,
    /// Git directory (.git or worktree commondir)
    git_dir: PathBuf,
    /// Grite data directory (.git/grite) — used for daemon lock
    grite_dir: PathBuf,
    /// Sled store path
    sled_path: PathBuf,
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
    /// Owner actor ID used when acquiring the daemon lock
    owner_actor_id: String,
}

impl Worker {
    /// Create a new worker
    pub fn new(
        repo_root: PathBuf,
        owner_actor_id: String,
        rx: mpsc::Receiver<WorkerMessage>,
        notify_tx: mpsc::Sender<Notification>,
        host_id: String,
        ipc_endpoint: String,
    ) -> Result<Self, DaemonError> {
        let git_dir = repo_root.join(".git");
        let grite_dir = git_dir.join("grite");
        let sled_path = repo_sled_path(&git_dir);

        // Open store with filesystem lock (blocking with timeout)
        // This ensures exclusive process-level access to the sled database
        let store = Arc::new(GriteStore::open_locked_blocking(
            &sled_path,
            Duration::from_secs(5),
        )?);

        Ok(Self {
            repo_root,
            git_dir,
            grite_dir,
            sled_path,
            store,
            rx,
            notify_tx,
            host_id,
            ipc_endpoint,
            owner_actor_id,
        })
    }

    /// Acquire the daemon lock
    pub fn acquire_lock(&self) -> Result<DaemonLock, DaemonError> {
        DaemonLock::acquire(
            &self.grite_dir,
            self.repo_root.to_string_lossy().to_string(),
            self.owner_actor_id.clone(),
            self.host_id.clone(),
            self.ipc_endpoint.clone(),
        )
        .map_err(|e| DaemonError::LockFailed(e.to_string()))
    }

    /// Refresh the daemon lock heartbeat
    pub fn refresh_lock(&self) -> Result<(), DaemonError> {
        if let Ok(Some(mut lock)) = DaemonLock::read(&self.grite_dir) {
            if lock.is_owned_by_current_process() {
                lock.refresh();
                lock.write(&self.grite_dir)?;
            }
        }
        Ok(())
    }

    /// Run the worker event loop
    pub async fn run(mut self) {
        info!(
            repo = %self.repo_root.display(),
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
                actor_id: self.owner_actor_id.clone(),
            })
            .await;

        // Track in-flight commands so we can wait for them on shutdown
        let in_flight = Arc::new(AtomicUsize::new(0));

        // Event loop - commands are spawned as concurrent tasks
        while let Some(msg) = self.rx.recv().await {
            match msg {
                WorkerMessage::Command {
                    request_id,
                    actor_id,
                    command,
                    response_tx,
                } => {
                    // Parse actor ID bytes for event authorship
                    let actor_id_bytes: ActorId = match hex_to_id(&actor_id) {
                        Ok(b) => b,
                        Err(e) => {
                            let resp = IpcResponse::error(
                                request_id,
                                "invalid_actor".to_string(),
                                format!("Invalid actor ID: {}", e),
                            );
                            let _ = response_tx.send(resp);
                            continue;
                        }
                    };

                    // Clone data needed for the spawned task
                    let store = Arc::clone(&self.store);
                    let sled_path = self.sled_path.clone();
                    let git_dir = self.git_dir.clone();
                    let in_flight = Arc::clone(&in_flight);

                    in_flight.fetch_add(1, Ordering::SeqCst);

                    // Run on the blocking thread pool — sled and git2 do
                    // synchronous I/O that must not starve the async runtime.
                    tokio::task::spawn_blocking(move || {
                        let response = execute_command(
                            &store,
                            actor_id_bytes,
                            &sled_path,
                            &git_dir,
                            &request_id,
                            &command,
                        );
                        let _ = response_tx.send(response);
                        in_flight.fetch_sub(1, Ordering::SeqCst);
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

        // Wait for in-flight commands to complete (with timeout)
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while in_flight.load(Ordering::SeqCst) > 0 {
            if tokio::time::Instant::now() >= deadline {
                warn!(
                    "Timed out waiting for {} in-flight commands",
                    in_flight.load(Ordering::SeqCst)
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Cleanup
        self.shutdown();
    }

    /// Shutdown cleanup
    fn shutdown(&self) {
        // Release lock
        if let Err(e) = DaemonLock::release(&self.grite_dir) {
            warn!("Failed to release lock: {}", e);
        }

        // Flush store
        if let Err(e) = self.store.flush() {
            warn!("Failed to flush store: {}", e);
        }

        info!(
            repo = %self.repo_root.display(),
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
    git_dir: &PathBuf,
    request_id: &str,
    command: &IpcCommand,
) -> IpcResponse {
    let result = execute_command_inner(store, actor_id_bytes, sled_path, git_dir, command);

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
    git_dir: &PathBuf,
    command: &IpcCommand,
) -> Result<Option<String>, DaemonError> {
    use libgrite_core::hash::compute_event_id;
    use libgrite_core::types::event::{Event, EventKind, IssueState};
    use libgrite_core::types::ids::{generate_issue_id, id_to_hex};
    use libgrite_core::types::issue::IssueProjection;
    use libgrite_core::export::{export_json, export_markdown, ExportSince};
    use libgrite_git::SyncManager;

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
            let summaries: Vec<serde_json::Value> = issues.iter().map(|s| serde_json::json!({
                "issue_id": id_to_hex(&s.issue_id),
                "title": s.title,
                "state": format!("{:?}", s.state).to_lowercase(),
                "labels": s.labels,
                "assignees": s.assignees,
                "updated_ts": s.updated_ts,
                "comment_count": s.comment_count,
            })).collect();
            let json = serde_json::to_string(&serde_json::json!({ "issues": summaries }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueShow { issue_id } => {
            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            let p = store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Issue {} not found", issue_id))))?;

            let json = serde_json::to_string(&projection_to_json(&p))?;
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
            let mut json_val = projection_to_json(&projection);
            json_val["event_id"] = serde_json::Value::String(id_to_hex(&event_id));
            json_val["action"] = serde_json::Value::String(libgrite_ipc::issue_action::CREATED.to_string());
            let json = serde_json::to_string(&json_val)?;
            Ok(Some(json))
        }

        IpcCommand::IssueUpdate { issue_id, title, body } => {
            if title.is_none() && body.is_none() {
                return Err(DaemonError::Core(GriteError::InvalidArgs(
                    "At least one of title or body must be provided".to_string()
                )));
            }

            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Issue {} not found", issue_id))))?;

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
            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Issue {} not found", issue_id))))?;

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
            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Issue {} not found", issue_id))))?;

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
                "action": libgrite_ipc::issue_action::CLOSED,
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueReopen { issue_id } => {
            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Issue {} not found", issue_id))))?;

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
                "action": libgrite_ipc::issue_action::REOPENED,
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueLabel { issue_id, add, remove } => {
            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Issue {} not found", issue_id))))?;

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
            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Issue {} not found", issue_id))))?;

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
            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Issue {} not found", issue_id))))?;

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
            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Issue {} not found", issue_id))))?;

            let parts: Vec<&str> = file_path.splitn(3, ':').collect();
            if parts.len() != 3 {
                return Err(DaemonError::Core(GriteError::InvalidArgs(
                    "file_path must be in format 'name:sha256:mime'".to_string()
                )));
            }

            let name = parts[0].to_string();
            let sha256: [u8; 32] = hex_to_id(parts[1])
                .map_err(|e| DaemonError::Core(GriteError::InvalidArgs(e.to_string())))?;
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
                _ => return Err(DaemonError::Core(GriteError::InvalidArgs(
                    format!("Unknown format: {}", format)
                ))),
            };
            Ok(Some(output))
        }

        IpcCommand::IssueDepAdd { issue_id, target_id, dep_type } => {
            use libgrite_core::hash::compute_event_id;
            use libgrite_core::types::event::{Event, EventKind, DependencyType};
            use libgrite_core::types::ids::id_to_hex;

            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            let target = store.resolve_issue_id(target_id)
                .map_err(DaemonError::Core)?;
            let dep = DependencyType::from_str(dep_type).ok_or_else(|| {
                DaemonError::Core(GriteError::InvalidArgs(format!("Invalid dep type: {}", dep_type)))
            })?;

            store.get_issue(&id)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Issue {} not found", issue_id))))?;
            store.get_issue(&target)?
                .ok_or_else(|| DaemonError::Core(GriteError::NotFound(format!("Target {} not found", target_id))))?;

            if store.would_create_cycle(&id, &target, &dep)? {
                return Err(DaemonError::Core(GriteError::InvalidArgs(format!(
                    "Adding this dependency would create a cycle in the {} graph", dep.as_str()
                ))));
            }

            let ts = current_time_ms();
            let kind = EventKind::DependencyAdded { target, dep_type: dep };
            let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);
            store.insert_event(&event)?;
            store.flush()?;

            let json = serde_json::to_string(&serde_json::json!({
                "event_id": id_to_hex(&event_id),
                "issue_id": issue_id,
                "target": target_id,
                "dep_type": dep_type,
                "action": "added",
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueDepRemove { issue_id, target_id, dep_type } => {
            use libgrite_core::hash::compute_event_id;
            use libgrite_core::types::event::{Event, EventKind, DependencyType};
            use libgrite_core::types::ids::id_to_hex;

            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            let target = store.resolve_issue_id(target_id)
                .map_err(DaemonError::Core)?;
            let dep = DependencyType::from_str(dep_type).ok_or_else(|| {
                DaemonError::Core(GriteError::InvalidArgs(format!("Invalid dep type: {}", dep_type)))
            })?;

            let ts = current_time_ms();
            let kind = EventKind::DependencyRemoved { target, dep_type: dep };
            let event_id = compute_event_id(&id, &actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, actor_id_bytes, ts, None, kind);
            store.insert_event(&event)?;
            store.flush()?;

            let json = serde_json::to_string(&serde_json::json!({
                "event_id": id_to_hex(&event_id),
                "issue_id": issue_id,
                "target": target_id,
                "dep_type": dep_type,
                "action": "removed",
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueDepList { issue_id, reverse } => {
            use libgrite_core::types::ids::id_to_hex;

            let id = store.resolve_issue_id(issue_id)
                .map_err(DaemonError::Core)?;
            let deps = if *reverse {
                store.get_dependents(&id)?
            } else {
                store.get_dependencies(&id)?
            };
            let dep_list: Vec<serde_json::Value> = deps.iter().map(|(target, dep_type)| {
                let title = store.get_issue(target).ok().flatten()
                    .map(|p| p.title.clone()).unwrap_or_else(|| "?".to_string());
                serde_json::json!({
                    "issue_id": id_to_hex(target),
                    "dep_type": dep_type.as_str(),
                    "title": title,
                })
            }).collect();
            let json = serde_json::to_string(&serde_json::json!({
                "issue_id": issue_id,
                "direction": if *reverse { "dependents" } else { "dependencies" },
                "deps": dep_list,
            }))?;
            Ok(Some(json))
        }

        IpcCommand::IssueDepTopo { state, label } => {
            use libgrite_core::types::event::IssueState;
            use libgrite_core::types::ids::id_to_hex;

            let filter = IssueFilter {
                state: state.as_deref().map(|s| match s {
                    "closed" => IssueState::Closed,
                    _ => IssueState::Open,
                }),
                label: label.clone(),
            };
            let sorted = store.topological_order(&filter)?;
            let issues: Vec<serde_json::Value> = sorted.iter().map(|s| serde_json::json!({
                "issue_id": id_to_hex(&s.issue_id),
                "title": s.title,
                "state": format!("{:?}", s.state).to_lowercase(),
                "labels": s.labels,
            })).collect();
            let json = serde_json::to_string(&serde_json::json!({
                "issues": issues,
                "order": "topological",
            }))?;
            Ok(Some(json))
        }

        // DaemonStatus and DaemonStop are handled at the supervisor level
        // in process_request() and never reach the worker.
        IpcCommand::DaemonStatus | IpcCommand::DaemonStop => {
            unreachable!("handled by supervisor before routing to worker")
        }

        IpcCommand::Sync { remote, pull, push } => {
            let sync_mgr = SyncManager::open(git_dir)?;

            // If neither flag is set, do both pull and push
            let do_pull = *pull || !*push;
            let do_push = *push || !*pull;

            let result = if do_pull && !do_push {
                // Pull only
                let pull_result = sync_mgr.pull(remote)?;
                let wal_head: Option<String> = pull_result.new_wal_head.map(|oid| oid.to_string());
                serde_json::json!({
                    "pulled": true,
                    "pushed": false,
                    "pull_events": pull_result.events_pulled,
                    "pull_wal_head": wal_head,
                    "message": pull_result.message,
                })
            } else if do_push && !do_pull {
                // Push only with auto-rebase
                let push_result = sync_mgr.push_with_rebase(remote, &actor_id_bytes)?;
                serde_json::json!({
                    "pulled": false,
                    "pushed": true,
                    "push_success": push_result.success,
                    "push_rebased": push_result.rebased,
                    "push_events_rebased": push_result.events_rebased,
                    "message": push_result.message,
                })
            } else {
                // Full sync: pull then push with auto-rebase
                let (pull_result, push_result) = sync_mgr.sync_with_rebase(remote, &actor_id_bytes)?;
                let wal_head: Option<String> = pull_result.new_wal_head.map(|oid| oid.to_string());
                serde_json::json!({
                    "pulled": true,
                    "pushed": true,
                    "pull_events": pull_result.events_pulled,
                    "pull_wal_head": wal_head,
                    "push_success": push_result.success,
                    "push_rebased": push_result.rebased,
                    "push_events_rebased": push_result.events_rebased,
                    "message": format!("{} / {}", pull_result.message, push_result.message),
                })
            };

            Ok(Some(result.to_string()))
        }

        IpcCommand::SnapshotCreate | IpcCommand::SnapshotList | IpcCommand::SnapshotGc { .. } => {
            Err(DaemonError::Core(GriteError::Internal(
                "Snapshot through daemon not yet implemented - use --no-daemon".to_string()
            )))
        }
    }
}

/// Convert an IssueProjection to a JSON value with hex-encoded IDs
fn projection_to_json(p: &libgrite_core::types::issue::IssueProjection) -> serde_json::Value {
    use libgrite_core::types::ids::id_to_hex;

    let comments: Vec<serde_json::Value> = p.comments.iter().map(|c| serde_json::json!({
        "event_id": id_to_hex(&c.event_id),
        "actor": id_to_hex(&c.actor),
        "ts_unix_ms": c.ts_unix_ms,
        "body": c.body,
    })).collect();
    let links: Vec<serde_json::Value> = p.links.iter().map(|l| serde_json::json!({
        "event_id": id_to_hex(&l.event_id),
        "url": l.url,
        "note": l.note,
    })).collect();
    let attachments: Vec<serde_json::Value> = p.attachments.iter().map(|a| serde_json::json!({
        "event_id": id_to_hex(&a.event_id),
        "name": a.name,
        "sha256": hex::encode(a.sha256),
        "mime": a.mime,
    })).collect();
    let deps: Vec<serde_json::Value> = p.dependencies.iter().map(|d| serde_json::json!({
        "target": id_to_hex(&d.target),
        "dep_type": d.dep_type.as_str(),
    })).collect();

    serde_json::json!({
        "issue_id": id_to_hex(&p.issue_id),
        "title": p.title,
        "body": p.body,
        "state": format!("{:?}", p.state).to_lowercase(),
        "labels": p.labels,
        "assignees": p.assignees,
        "comments": comments,
        "links": links,
        "attachments": attachments,
        "dependencies": deps,
        "created_ts": p.created_ts,
        "updated_ts": p.updated_ts,
    })
}

/// Get current time in milliseconds since Unix epoch
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Convert error to (code, message) for IPC response
fn error_to_code_message(e: &DaemonError) -> (String, String) {
    use libgrite_ipc::error::codes;

    match e {
        DaemonError::Core(GriteError::NotFound(_)) => (codes::NOT_FOUND.to_string(), e.to_string()),
        DaemonError::Core(GriteError::InvalidArgs(_)) => (codes::INVALID_INPUT.to_string(), e.to_string()),
        DaemonError::Core(GriteError::Io(_)) => (codes::IO_ERROR.to_string(), e.to_string()),
        DaemonError::Git(_) => (codes::GIT_ERROR.to_string(), e.to_string()),
        DaemonError::Ipc(_) => (codes::IPC_ERROR.to_string(), e.to_string()),
        _ => (codes::INTERNAL.to_string(), e.to_string()),
    }
}
