//! Snapshot command implementation

use libgrit_core::GritError;
use serde::Serialize;
use crate::cli::{Cli, SnapshotCommand};
use crate::context::GritContext;
use crate::output::output_success;

#[derive(Serialize)]
struct SnapshotCreateOutput {
    oid: String,
    event_count: usize,
    wal_head: String,
}

#[derive(Serialize)]
struct SnapshotListOutput {
    snapshots: Vec<SnapshotInfo>,
    total: usize,
}

#[derive(Serialize)]
struct SnapshotInfo {
    oid: String,
    timestamp: u64,
    ref_name: String,
}

#[derive(Serialize)]
struct SnapshotGcOutput {
    deleted: usize,
    kept: usize,
}

pub fn run(cli: &Cli, cmd: SnapshotCommand) -> Result<(), GritError> {
    match cmd {
        SnapshotCommand::Create => run_create(cli),
        SnapshotCommand::List => run_list(cli),
        SnapshotCommand::Gc { keep } => run_gc(cli, keep),
    }
}

fn run_create(cli: &Cli) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let wal = ctx.open_wal().map_err(|e| GritError::Internal(e.to_string()))?;
    let snapshot_mgr = ctx.open_snapshot().map_err(|e| GritError::Internal(e.to_string()))?;

    // Get current WAL head
    let wal_head = wal.head().map_err(|e| GritError::Internal(e.to_string()))?
        .ok_or_else(|| GritError::NotFound("No WAL commits found".to_string()))?;

    // Read all events from WAL
    let events = wal.read_all().map_err(|e| GritError::Internal(e.to_string()))?;

    if events.is_empty() {
        return Err(GritError::InvalidArgs("No events to snapshot".to_string()));
    }

    // Create snapshot
    let oid = snapshot_mgr.create(wal_head, &events)
        .map_err(|e| GritError::Internal(e.to_string()))?;

    output_success(cli, SnapshotCreateOutput {
        oid: oid.to_string(),
        event_count: events.len(),
        wal_head: wal_head.to_string(),
    });

    Ok(())
}

fn run_list(cli: &Cli) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let snapshot_mgr = ctx.open_snapshot().map_err(|e| GritError::Internal(e.to_string()))?;

    let snapshots = snapshot_mgr.list().map_err(|e| GritError::Internal(e.to_string()))?;
    let total = snapshots.len();

    let snapshot_infos: Vec<SnapshotInfo> = snapshots.into_iter()
        .map(|s| SnapshotInfo {
            oid: s.oid.to_string(),
            timestamp: s.timestamp,
            ref_name: s.ref_name,
        })
        .collect();

    output_success(cli, SnapshotListOutput {
        snapshots: snapshot_infos,
        total,
    });

    Ok(())
}

fn run_gc(cli: &Cli, keep: usize) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let snapshot_mgr = ctx.open_snapshot().map_err(|e| GritError::Internal(e.to_string()))?;

    let stats = snapshot_mgr.gc(keep).map_err(|e| GritError::Internal(e.to_string()))?;

    output_success(cli, SnapshotGcOutput {
        deleted: stats.deleted,
        kept: stats.kept,
    });

    Ok(())
}
