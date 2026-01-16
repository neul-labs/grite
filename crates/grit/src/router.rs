//! Command routing through daemon or local execution
//!
//! This module handles the decision of whether to route a command
//! through the daemon or execute it locally.

use libgrit_core::GritError;
use libgrit_ipc::{IpcClient, IpcCommand, IpcRequest, IpcResponse};

use crate::cli::Cli;
use crate::context::{ExecutionMode, GritContext};

/// Result of routing a command
pub enum RouteResult {
    /// Execute locally
    Local,
    /// Routed through daemon, response received
    DaemonResponse(IpcResponse),
    /// Blocked by daemon lock
    Blocked { pid: u32, expires_in_ms: u64 },
}

/// Route an IPC command through the daemon if available
///
/// Returns RouteResult::Local if the command should be executed locally.
/// Returns RouteResult::DaemonResponse if the daemon handled it.
/// Returns RouteResult::Blocked if the data dir is owned by another process.
pub fn route_command(
    ctx: &GritContext,
    cli: &Cli,
    command: IpcCommand,
) -> Result<RouteResult, GritError> {
    match ctx.execution_mode(cli.no_daemon) {
        ExecutionMode::Local => Ok(RouteResult::Local),
        ExecutionMode::Daemon { client, endpoint: _ } => {
            let response = send_to_daemon(ctx, &client, command)?;
            Ok(RouteResult::DaemonResponse(response))
        }
        ExecutionMode::Blocked { lock } => {
            Ok(RouteResult::Blocked {
                pid: lock.pid,
                expires_in_ms: lock.time_remaining_ms(),
            })
        }
    }
}

/// Send a command to the daemon
fn send_to_daemon(
    ctx: &GritContext,
    client: &IpcClient,
    command: IpcCommand,
) -> Result<IpcResponse, GritError> {
    let request = IpcRequest::new(
        uuid::Uuid::new_v4().to_string(),
        ctx.repo_root().to_string_lossy().to_string(),
        ctx.actor_id.clone(),
        ctx.data_dir.to_string_lossy().to_string(),
        command,
    );

    client
        .send_with_retry(&request, 3)
        .map_err(|e| GritError::Internal(format!("IPC error: {}", e)))
}

/// Check if a command should use daemon routing
///
/// Some commands (like init, actor management) should always run locally.
pub fn should_route_through_daemon(cmd: &crate::cli::Command) -> bool {
    use crate::cli::{Command, DbCommand};

    match cmd {
        // Always local - these manage the grit setup itself
        Command::Init => false,
        Command::Actor { .. } => false,

        // Daemon and lock commands are handled specially
        Command::Daemon { .. } => false,
        Command::Lock { .. } => false, // Locks require git ref access

        // Db commands: stats can route, check/verify are local-only
        Command::Db { cmd: db_cmd } => match db_cmd {
            DbCommand::Stats => true,
            DbCommand::Check { .. } => false, // Integrity check is local
            DbCommand::Verify { .. } => false, // Signature verify is local
        },

        // Doctor is local-only (health checks)
        Command::Doctor { .. } => false,

        // These can be routed through daemon
        Command::Issue { .. } => true,
        Command::Export { .. } => true,
        Command::Rebuild => true,
        Command::Sync { .. } => true,
        Command::Snapshot { .. } => true,
    }
}

/// Convert a CLI command to an IPC command
///
/// Returns None for commands that should always run locally.
pub fn cli_to_ipc_command(cmd: &crate::cli::Command) -> Option<IpcCommand> {
    use crate::cli::{Command, ExportFormat};

    match cmd {
        Command::Issue { cmd: issue_cmd } => Some(issue_to_ipc(issue_cmd)),
        Command::Db { cmd: db_cmd } => Some(db_to_ipc(db_cmd)),
        Command::Export { format, since } => Some(IpcCommand::Export {
            format: match format {
                ExportFormat::Json => "json".to_string(),
                ExportFormat::Md => "md".to_string(),
            },
            since: since.clone(),
        }),
        Command::Rebuild => Some(IpcCommand::Rebuild),
        Command::Sync { remote, pull, push } => Some(IpcCommand::Sync {
            remote: remote.clone(),
            pull: *pull,
            push: *push,
        }),
        Command::Snapshot { cmd: snap_cmd } => Some(snapshot_to_ipc(snap_cmd)),
        // These don't route through daemon
        Command::Init | Command::Actor { .. } | Command::Daemon { .. } | Command::Lock { .. } | Command::Doctor { .. } => None,
    }
}

fn issue_to_ipc(cmd: &crate::cli::IssueCommand) -> IpcCommand {
    use crate::cli::{IssueCommand, LabelCommand, AssigneeCommand, LinkCommand, AttachmentCommand};

    match cmd {
        IssueCommand::Create { title, body, label } => IpcCommand::IssueCreate {
            title: title.clone(),
            body: body.clone(),
            labels: label.clone(),
        },
        IssueCommand::List { state, label } => IpcCommand::IssueList {
            state: state.clone(),
            label: label.clone(),
        },
        IssueCommand::Show { id } => IpcCommand::IssueShow {
            issue_id: id.clone(),
        },
        IssueCommand::Update { id, title, body, .. } => IpcCommand::IssueUpdate {
            issue_id: id.clone(),
            title: title.clone(),
            body: body.clone(),
        },
        IssueCommand::Comment { id, body, .. } => IpcCommand::IssueComment {
            issue_id: id.clone(),
            body: body.clone(),
        },
        IssueCommand::Close { id, .. } => IpcCommand::IssueClose {
            issue_id: id.clone(),
        },
        IssueCommand::Reopen { id, .. } => IpcCommand::IssueReopen {
            issue_id: id.clone(),
        },
        IssueCommand::Label { cmd: label_cmd } => match label_cmd {
            LabelCommand::Add { id, label, .. } => IpcCommand::IssueLabel {
                issue_id: id.clone(),
                add: vec![label.clone()],
                remove: vec![],
            },
            LabelCommand::Remove { id, label, .. } => IpcCommand::IssueLabel {
                issue_id: id.clone(),
                add: vec![],
                remove: vec![label.clone()],
            },
        },
        IssueCommand::Assignee { cmd: assign_cmd } => match assign_cmd {
            AssigneeCommand::Add { id, user, .. } => IpcCommand::IssueAssign {
                issue_id: id.clone(),
                add: vec![user.clone()],
                remove: vec![],
            },
            AssigneeCommand::Remove { id, user, .. } => IpcCommand::IssueAssign {
                issue_id: id.clone(),
                add: vec![],
                remove: vec![user.clone()],
            },
        },
        IssueCommand::Link { cmd: link_cmd } => match link_cmd {
            LinkCommand::Add { id, url, note, .. } => IpcCommand::IssueLink {
                issue_id: id.clone(),
                url: url.clone(),
                note: note.clone(),
            },
        },
        IssueCommand::Attachment { cmd: attach_cmd } => match attach_cmd {
            AttachmentCommand::Add { id, name, sha256, mime, .. } => IpcCommand::IssueAttach {
                issue_id: id.clone(),
                file_path: format!("{}:{}:{}", name, sha256, mime),
            },
        },
    }
}

fn db_to_ipc(cmd: &crate::cli::DbCommand) -> IpcCommand {
    use crate::cli::DbCommand;

    match cmd {
        DbCommand::Stats => IpcCommand::DbStats,
        // Check and Verify are local-only, shouldn't reach here
        DbCommand::Check { .. } | DbCommand::Verify { .. } => {
            unreachable!("Check and Verify commands should not be routed through daemon")
        }
    }
}

fn snapshot_to_ipc(cmd: &crate::cli::SnapshotCommand) -> IpcCommand {
    use crate::cli::SnapshotCommand;

    match cmd {
        SnapshotCommand::Create => IpcCommand::SnapshotCreate,
        SnapshotCommand::List => IpcCommand::SnapshotList,
        SnapshotCommand::Gc { keep } => IpcCommand::SnapshotGc {
            keep: *keep as u32,
        },
    }
}
