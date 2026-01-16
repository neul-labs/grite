//! Worker module - handles commands for a single (repo, actor) pair
//!
//! Each worker owns exclusive access to the sled database for its actor.

use std::path::PathBuf;

use libgrit_core::types::ids::{hex_to_id, ActorId};
use libgrit_core::{GritError, GritStore};
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
    /// Sled store (owned exclusively)
    store: GritStore,
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

        // Open store (this acquires exclusive access)
        let store = GritStore::open(&sled_path)?;

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

        // Event loop
        while let Some(msg) = self.rx.recv().await {
            match msg {
                WorkerMessage::Command {
                    request_id,
                    command,
                    response_tx,
                } => {
                    let response = self.handle_command(&request_id, command);
                    let _ = response_tx.send(response);
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

    /// Handle a command
    fn handle_command(&mut self, request_id: &str, command: IpcCommand) -> IpcResponse {
        let result = self.execute_command(&command);

        match result {
            Ok(data) => IpcResponse::success(request_id.to_string(), data),
            Err(e) => {
                let (code, message) = error_to_code_message(&e);
                IpcResponse::error(request_id.to_string(), code, message)
            }
        }
    }

    /// Execute a command and return the result
    fn execute_command(&mut self, command: &IpcCommand) -> Result<Option<String>, DaemonError> {
        match command {
            // Issue commands
            IpcCommand::IssueList { state, label } => {
                self.issue_list(state.as_deref(), label.as_deref())
            }
            IpcCommand::IssueShow { issue_id } => self.issue_show(issue_id),
            IpcCommand::IssueCreate { title, body, labels } => {
                self.issue_create(title, body, labels)
            }
            IpcCommand::IssueUpdate { issue_id, title, body } => {
                self.issue_update(issue_id, title.as_deref(), body.as_deref())
            }
            IpcCommand::IssueComment { issue_id, body } => {
                self.issue_comment(issue_id, body)
            }
            IpcCommand::IssueClose { issue_id } => self.issue_close(issue_id),
            IpcCommand::IssueReopen { issue_id } => self.issue_reopen(issue_id),
            IpcCommand::IssueLabel { issue_id, add, remove } => {
                self.issue_label(issue_id, add, remove)
            }
            IpcCommand::IssueAssign { issue_id, add, remove } => {
                self.issue_assign(issue_id, add, remove)
            }
            IpcCommand::IssueLink { issue_id, url, note } => {
                self.issue_link(issue_id, url, note.as_deref())
            }
            IpcCommand::IssueAttach { issue_id, file_path } => {
                self.issue_attach(issue_id, file_path)
            }

            // Database commands
            IpcCommand::DbStats => self.db_stats(),
            IpcCommand::Rebuild => self.rebuild(),
            IpcCommand::Export { format, since } => {
                self.export(format, since.as_deref())
            }

            // Sync commands
            IpcCommand::Sync { remote, pull, push } => {
                self.sync(remote, *pull, *push)
            }

            // Snapshot commands
            IpcCommand::SnapshotCreate => self.snapshot_create(),
            IpcCommand::SnapshotList => self.snapshot_list(),
            IpcCommand::SnapshotGc { keep } => self.snapshot_gc(*keep),

            // Daemon commands
            IpcCommand::DaemonStatus => self.daemon_status(),
            IpcCommand::DaemonStop => {
                Ok(Some(serde_json::json!({"stopping": true}).to_string()))
            }
        }
    }

    // Command implementations

    fn issue_list(&self, state: Option<&str>, label: Option<&str>) -> Result<Option<String>, DaemonError> {
        use libgrit_core::types::event::IssueState;

        let filter = IssueFilter {
            state: state.map(|s| match s {
                "open" => IssueState::Open,
                "closed" => IssueState::Closed,
                _ => IssueState::Open,
            }),
            label: label.map(String::from),
        };

        let issues = self.store.list_issues(&filter)?;
        let json = serde_json::to_string(&serde_json::json!({ "issues": issues }))?;
        Ok(Some(json))
    }

    fn issue_show(&self, issue_id: &str) -> Result<Option<String>, DaemonError> {
        let id = hex_to_id(issue_id)
            .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;
        let projection = self.store.get_issue(&id)?
            .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;
        let json = serde_json::to_string(&projection)?;
        Ok(Some(json))
    }

    fn issue_create(&mut self, title: &str, body: &str, labels: &[String]) -> Result<Option<String>, DaemonError> {
        use libgrit_core::hash::compute_event_id;
        use libgrit_core::types::event::{Event, EventKind};
        use libgrit_core::types::ids::generate_issue_id;
        use libgrit_core::types::issue::IssueProjection;

        let issue_id = generate_issue_id();
        let ts = current_time_ms();
        let kind = EventKind::IssueCreated {
            title: title.to_string(),
            body: body.to_string(),
            labels: labels.to_vec(),
        };
        let event_id = compute_event_id(&issue_id, &self.actor_id_bytes, ts, None, &kind);
        let event = Event::new(event_id, issue_id, self.actor_id_bytes, ts, None, kind);

        // Insert into store
        self.store.insert_event(&event)?;
        self.store.flush()?;

        // Create projection for response
        let projection = IssueProjection::from_event(&event)?;

        let json = serde_json::to_string(&serde_json::json!({
            "issue_id": libgrit_core::types::ids::id_to_hex(&issue_id),
            "event_id": libgrit_core::types::ids::id_to_hex(&event_id),
            "projection": projection,
        }))?;
        Ok(Some(json))
    }

    fn db_stats(&self) -> Result<Option<String>, DaemonError> {
        let stats = self.store.stats(&self.sled_path)?;
        let json = serde_json::to_string(&serde_json::json!({
            "path": stats.path,
            "size_bytes": stats.size_bytes,
            "event_count": stats.event_count,
            "issue_count": stats.issue_count,
            "last_rebuild_ts": stats.last_rebuild_ts,
        }))?;
        Ok(Some(json))
    }

    fn rebuild(&mut self) -> Result<Option<String>, DaemonError> {
        let stats = self.store.rebuild()?;
        let json = serde_json::to_string(&serde_json::json!({
            "event_count": stats.event_count,
            "issue_count": stats.issue_count,
        }))?;
        Ok(Some(json))
    }

    fn daemon_status(&self) -> Result<Option<String>, DaemonError> {
        let lock = DaemonLock::read(&self.data_dir)
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

    fn issue_update(&mut self, issue_id: &str, title: Option<&str>, body: Option<&str>) -> Result<Option<String>, DaemonError> {
        use libgrit_core::hash::compute_event_id;
        use libgrit_core::types::event::{Event, EventKind};
        use libgrit_core::types::ids::id_to_hex;

        if title.is_none() && body.is_none() {
            return Err(DaemonError::Grit(GritError::InvalidArgs(
                "At least one of title or body must be provided".to_string()
            )));
        }

        let id = hex_to_id(issue_id)
            .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;

        // Verify issue exists
        self.store.get_issue(&id)?
            .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

        let ts = current_time_ms();
        let kind = EventKind::IssueUpdated {
            title: title.map(String::from),
            body: body.map(String::from),
        };
        let event_id = compute_event_id(&id, &self.actor_id_bytes, ts, None, &kind);
        let event = Event::new(event_id, id, self.actor_id_bytes, ts, None, kind);

        self.store.insert_event(&event)?;
        self.store.flush()?;

        let json = serde_json::to_string(&serde_json::json!({
            "issue_id": issue_id,
            "event_id": id_to_hex(&event_id),
        }))?;
        Ok(Some(json))
    }

    fn issue_comment(&mut self, issue_id: &str, body: &str) -> Result<Option<String>, DaemonError> {
        use libgrit_core::hash::compute_event_id;
        use libgrit_core::types::event::{Event, EventKind};
        use libgrit_core::types::ids::id_to_hex;

        let id = hex_to_id(issue_id)
            .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;

        // Verify issue exists
        self.store.get_issue(&id)?
            .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

        let ts = current_time_ms();
        let kind = EventKind::CommentAdded { body: body.to_string() };
        let event_id = compute_event_id(&id, &self.actor_id_bytes, ts, None, &kind);
        let event = Event::new(event_id, id, self.actor_id_bytes, ts, None, kind);

        self.store.insert_event(&event)?;
        self.store.flush()?;

        let json = serde_json::to_string(&serde_json::json!({
            "issue_id": issue_id,
            "event_id": id_to_hex(&event_id),
        }))?;
        Ok(Some(json))
    }

    fn issue_close(&mut self, issue_id: &str) -> Result<Option<String>, DaemonError> {
        use libgrit_core::hash::compute_event_id;
        use libgrit_core::types::event::{Event, EventKind, IssueState};
        use libgrit_core::types::ids::id_to_hex;

        let id = hex_to_id(issue_id)
            .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;

        // Verify issue exists
        self.store.get_issue(&id)?
            .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

        let ts = current_time_ms();
        let kind = EventKind::StateChanged { state: IssueState::Closed };
        let event_id = compute_event_id(&id, &self.actor_id_bytes, ts, None, &kind);
        let event = Event::new(event_id, id, self.actor_id_bytes, ts, None, kind);

        self.store.insert_event(&event)?;
        self.store.flush()?;

        let json = serde_json::to_string(&serde_json::json!({
            "issue_id": issue_id,
            "event_id": id_to_hex(&event_id),
            "state": "closed",
        }))?;
        Ok(Some(json))
    }

    fn issue_reopen(&mut self, issue_id: &str) -> Result<Option<String>, DaemonError> {
        use libgrit_core::hash::compute_event_id;
        use libgrit_core::types::event::{Event, EventKind, IssueState};
        use libgrit_core::types::ids::id_to_hex;

        let id = hex_to_id(issue_id)
            .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;

        // Verify issue exists
        self.store.get_issue(&id)?
            .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

        let ts = current_time_ms();
        let kind = EventKind::StateChanged { state: IssueState::Open };
        let event_id = compute_event_id(&id, &self.actor_id_bytes, ts, None, &kind);
        let event = Event::new(event_id, id, self.actor_id_bytes, ts, None, kind);

        self.store.insert_event(&event)?;
        self.store.flush()?;

        let json = serde_json::to_string(&serde_json::json!({
            "issue_id": issue_id,
            "event_id": id_to_hex(&event_id),
            "state": "open",
        }))?;
        Ok(Some(json))
    }

    fn issue_label(&mut self, issue_id: &str, add: &[String], remove: &[String]) -> Result<Option<String>, DaemonError> {
        use libgrit_core::hash::compute_event_id;
        use libgrit_core::types::event::{Event, EventKind};
        use libgrit_core::types::ids::id_to_hex;

        let id = hex_to_id(issue_id)
            .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;

        // Verify issue exists
        self.store.get_issue(&id)?
            .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

        let mut event_ids = Vec::new();
        let ts = current_time_ms();

        // Add labels
        for label in add {
            let kind = EventKind::LabelAdded { label: label.clone() };
            let event_id = compute_event_id(&id, &self.actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, self.actor_id_bytes, ts, None, kind);
            self.store.insert_event(&event)?;
            event_ids.push(id_to_hex(&event_id));
        }

        // Remove labels
        for label in remove {
            let kind = EventKind::LabelRemoved { label: label.clone() };
            let event_id = compute_event_id(&id, &self.actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, self.actor_id_bytes, ts, None, kind);
            self.store.insert_event(&event)?;
            event_ids.push(id_to_hex(&event_id));
        }

        self.store.flush()?;

        let json = serde_json::to_string(&serde_json::json!({
            "issue_id": issue_id,
            "event_ids": event_ids,
        }))?;
        Ok(Some(json))
    }

    fn issue_assign(&mut self, issue_id: &str, add: &[String], remove: &[String]) -> Result<Option<String>, DaemonError> {
        use libgrit_core::hash::compute_event_id;
        use libgrit_core::types::event::{Event, EventKind};
        use libgrit_core::types::ids::id_to_hex;

        let id = hex_to_id(issue_id)
            .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;

        // Verify issue exists
        self.store.get_issue(&id)?
            .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

        let mut event_ids = Vec::new();
        let ts = current_time_ms();

        // Add assignees
        for user in add {
            let kind = EventKind::AssigneeAdded { user: user.clone() };
            let event_id = compute_event_id(&id, &self.actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, self.actor_id_bytes, ts, None, kind);
            self.store.insert_event(&event)?;
            event_ids.push(id_to_hex(&event_id));
        }

        // Remove assignees
        for user in remove {
            let kind = EventKind::AssigneeRemoved { user: user.clone() };
            let event_id = compute_event_id(&id, &self.actor_id_bytes, ts, None, &kind);
            let event = Event::new(event_id, id, self.actor_id_bytes, ts, None, kind);
            self.store.insert_event(&event)?;
            event_ids.push(id_to_hex(&event_id));
        }

        self.store.flush()?;

        let json = serde_json::to_string(&serde_json::json!({
            "issue_id": issue_id,
            "event_ids": event_ids,
        }))?;
        Ok(Some(json))
    }

    fn issue_link(&mut self, issue_id: &str, url: &str, note: Option<&str>) -> Result<Option<String>, DaemonError> {
        use libgrit_core::hash::compute_event_id;
        use libgrit_core::types::event::{Event, EventKind};
        use libgrit_core::types::ids::id_to_hex;

        let id = hex_to_id(issue_id)
            .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;

        // Verify issue exists
        self.store.get_issue(&id)?
            .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

        let ts = current_time_ms();
        let kind = EventKind::LinkAdded {
            url: url.to_string(),
            note: note.map(String::from),
        };
        let event_id = compute_event_id(&id, &self.actor_id_bytes, ts, None, &kind);
        let event = Event::new(event_id, id, self.actor_id_bytes, ts, None, kind);

        self.store.insert_event(&event)?;
        self.store.flush()?;

        let json = serde_json::to_string(&serde_json::json!({
            "issue_id": issue_id,
            "event_id": id_to_hex(&event_id),
        }))?;
        Ok(Some(json))
    }

    fn issue_attach(&mut self, issue_id: &str, file_path: &str) -> Result<Option<String>, DaemonError> {
        use libgrit_core::hash::compute_event_id;
        use libgrit_core::types::event::{Event, EventKind};
        use libgrit_core::types::ids::id_to_hex;

        let id = hex_to_id(issue_id)
            .map_err(|e| DaemonError::Grit(GritError::InvalidArgs(e.to_string())))?;

        // Verify issue exists
        self.store.get_issue(&id)?
            .ok_or_else(|| DaemonError::Grit(GritError::NotFound(format!("Issue {} not found", issue_id))))?;

        // Parse file_path as "name:sha256:mime"
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
        let event_id = compute_event_id(&id, &self.actor_id_bytes, ts, None, &kind);
        let event = Event::new(event_id, id, self.actor_id_bytes, ts, None, kind);

        self.store.insert_event(&event)?;
        self.store.flush()?;

        let json = serde_json::to_string(&serde_json::json!({
            "issue_id": issue_id,
            "event_id": id_to_hex(&event_id),
        }))?;
        Ok(Some(json))
    }

    fn export(&self, format: &str, since: Option<&str>) -> Result<Option<String>, DaemonError> {
        use libgrit_core::export::{export_json, export_markdown, ExportSince};

        // Parse since timestamp
        let since_opt = since.and_then(|s| s.parse::<u64>().ok()).map(ExportSince::Timestamp);

        let output = match format {
            "json" => {
                let export = export_json(&self.store, since_opt)?;
                serde_json::to_string(&export)?
            }
            "md" | "markdown" => export_markdown(&self.store, since_opt)?,
            _ => return Err(DaemonError::Grit(GritError::InvalidArgs(
                format!("Unknown format: {}", format)
            ))),
        };

        Ok(Some(output))
    }

    fn sync(&self, _remote: &str, _pull: bool, _push: bool) -> Result<Option<String>, DaemonError> {
        // Sync requires WAL access which we don't have in the worker yet
        // Return a stub response for now
        Err(DaemonError::Grit(GritError::Internal(
            "Sync through daemon not yet implemented - use --no-daemon".to_string()
        )))
    }

    fn snapshot_create(&self) -> Result<Option<String>, DaemonError> {
        // Snapshot requires git access which we don't have in the worker yet
        Err(DaemonError::Grit(GritError::Internal(
            "Snapshot through daemon not yet implemented - use --no-daemon".to_string()
        )))
    }

    fn snapshot_list(&self) -> Result<Option<String>, DaemonError> {
        // Snapshot requires git access
        Err(DaemonError::Grit(GritError::Internal(
            "Snapshot through daemon not yet implemented - use --no-daemon".to_string()
        )))
    }

    fn snapshot_gc(&self, _keep: u32) -> Result<Option<String>, DaemonError> {
        // Snapshot requires git access
        Err(DaemonError::Grit(GritError::Internal(
            "Snapshot through daemon not yet implemented - use --no-daemon".to_string()
        )))
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
