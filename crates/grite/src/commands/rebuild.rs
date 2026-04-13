use libgrite_core::GriteError;
use libgrite_git::{SnapshotManager, WalManager};
use libgrite_ipc::{IpcClient, IpcCommand, IpcRequest};
use serde::Serialize;
use crate::cli::Cli;
use crate::context::{ExecutionMode, GriteContext};
use crate::output::{output_success, print_human};

/// Rebuild can take much longer than normal IPC commands (minutes for large stores).
const REBUILD_TIMEOUT_MS: u64 = 300_000; // 5 minutes

#[derive(Serialize)]
struct RebuildOutput {
    wal_head: Option<String>,
    event_count: usize,
    from_snapshot: Option<String>,
    snapshot_events: Option<usize>,
}

pub fn run(cli: &Cli, use_snapshot: bool) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;

    match ctx.execution_mode(cli.no_daemon) {
        ExecutionMode::Daemon { endpoint, .. } => {
            // The daemon holds the store flock. Route rebuild through it
            // with a generous timeout since rebuilds can be slow.
            rebuild_via_daemon(cli, &ctx, &endpoint)
        }
        ExecutionMode::Blocked { lock } => {
            Err(GriteError::DbBusy(format!(
                "Store is locked by pid {} (expires in {}s). \
                 Try again later or run 'grite daemon stop' first.",
                lock.pid,
                lock.time_remaining_ms() / 1000
            )))
        }
        ExecutionMode::Local => {
            let store = ctx.open_store()?;
            let git_dir = ctx.repo_root().join(".git");
            do_rebuild(cli, &store, &git_dir, use_snapshot)
        }
    }
}

/// Send rebuild command through the daemon's IPC with a long timeout.
fn rebuild_via_daemon(cli: &Cli, ctx: &GriteContext, endpoint: &str) -> Result<(), GriteError> {
    let mut client = IpcClient::connect_with_timeout(endpoint, REBUILD_TIMEOUT_MS)
        .map_err(|e| GriteError::Internal(format!("Failed to connect to daemon: {}", e)))?;

    let request = IpcRequest::new(
        uuid::Uuid::new_v4().to_string(),
        ctx.repo_root().to_string_lossy().to_string(),
        ctx.actor_id.clone(),
        ctx.data_dir.to_string_lossy().to_string(),
        IpcCommand::Rebuild,
    );

    let response = client.send(&request)
        .map_err(|e| GriteError::Internal(format!("Rebuild via daemon failed: {}", e)))?;

    if response.ok {
        if let Some(data) = &response.data {
            if cli.json {
                println!("{}", data);
            } else if !cli.quiet {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    let count = json.get("event_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    print_human(cli, &format!("Rebuilt {} events (via daemon)", count));
                }
            }
        }
        Ok(())
    } else {
        let msg = response.error
            .map(|e| e.message)
            .unwrap_or_else(|| "unknown error".to_string());
        Err(GriteError::Internal(format!("Daemon rebuild failed: {}", msg)))
    }
}

fn do_rebuild(
    cli: &Cli,
    store: &libgrite_core::LockedStore,
    git_dir: &std::path::Path,
    use_snapshot: bool,
) -> Result<(), GriteError> {
    if use_snapshot {
        // Snapshot-based rebuild: load from latest snapshot
        let snap_mgr = SnapshotManager::open(git_dir)
            .map_err(|e| GriteError::Internal(e.to_string()))?;
        let wal_mgr = WalManager::open(git_dir)
            .map_err(|e| GriteError::Internal(e.to_string()))?;

        // Get latest snapshot
        let snapshots = snap_mgr.list()
            .map_err(|e| GriteError::Internal(e.to_string()))?;

        if snapshots.is_empty() {
            print_human(cli, "No snapshots found, falling back to full rebuild");
            let stats = store.rebuild()?;
            output_success(cli, RebuildOutput {
                wal_head: wal_mgr.head().ok().flatten().map(|oid| oid.to_string()),
                event_count: stats.event_count,
                from_snapshot: None,
                snapshot_events: None,
            });
            return Ok(());
        }

        let latest = &snapshots[0]; // List is sorted newest-first
        print_human(cli, &format!("Loading from snapshot: {}", latest.ref_name));

        // Read snapshot events
        let snapshot_events = snap_mgr.read(latest.oid)
            .map_err(|e| GriteError::Internal(e.to_string()))?;

        let snap_count = snapshot_events.len();

        // Rebuild from snapshot events
        // Note: This rebuilds from the snapshot state only. For events added after
        // the snapshot, they should already be in the local store's event log.
        let stats = store.rebuild_from_events(&snapshot_events)?;

        print_human(cli, &format!(
            "Rebuilt from {} snapshot events",
            snap_count
        ));

        output_success(cli, RebuildOutput {
            wal_head: wal_mgr.head().ok().flatten().map(|oid| oid.to_string()),
            event_count: stats.event_count,
            from_snapshot: Some(latest.ref_name.clone()),
            snapshot_events: Some(snap_count),
        });
    } else {
        // Standard rebuild from store events
        let wal_head = WalManager::open(git_dir)
            .ok()
            .and_then(|wal| wal.head().ok().flatten());

        let stats = store.rebuild()?;

        output_success(cli, RebuildOutput {
            wal_head: wal_head.map(|oid| oid.to_string()),
            event_count: stats.event_count,
            from_snapshot: None,
            snapshot_events: None,
        });
    }

    Ok(())
}
