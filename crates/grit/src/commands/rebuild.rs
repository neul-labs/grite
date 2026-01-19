use libgrit_core::GritError;
use libgrit_git::{SnapshotManager, WalManager};
use serde::Serialize;
use crate::cli::Cli;
use crate::context::GritContext;
use crate::output::{output_success, print_human};

#[derive(Serialize)]
struct RebuildOutput {
    wal_head: Option<String>,
    event_count: usize,
    from_snapshot: Option<String>,
    snapshot_events: Option<usize>,
}

pub fn run(cli: &Cli, use_snapshot: bool) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let store = ctx.open_store()?;
    let git_dir = ctx.repo_root().join(".git");

    if use_snapshot {
        // Snapshot-based rebuild: load from latest snapshot
        let snap_mgr = SnapshotManager::open(&git_dir)
            .map_err(|e| GritError::Internal(e.to_string()))?;
        let wal_mgr = WalManager::open(&git_dir)
            .map_err(|e| GritError::Internal(e.to_string()))?;

        // Get latest snapshot
        let snapshots = snap_mgr.list()
            .map_err(|e| GritError::Internal(e.to_string()))?;

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
            .map_err(|e| GritError::Internal(e.to_string()))?;

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
        let wal_head = WalManager::open(&git_dir)
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
