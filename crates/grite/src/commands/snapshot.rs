//! Snapshot command implementation

use crate::cli::{Cli, SnapshotCommand};
use crate::context::GriteContext;
use crate::output::output_success;
use libgrite_core::GriteError;
use serde::Serialize;

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

pub fn run(cli: &Cli, cmd: SnapshotCommand) -> Result<(), GriteError> {
    match cmd {
        SnapshotCommand::Create => run_create(cli),
        SnapshotCommand::List => run_list(cli),
        SnapshotCommand::Gc { keep } => run_gc(cli, keep),
    }
}

fn run_create(cli: &Cli) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let wal = ctx.open_wal()?;
    let snapshot_mgr = ctx.open_snapshot()?;

    // Get current WAL head
    let wal_head = wal
        .head()?
        .ok_or_else(|| GriteError::NotFound("No WAL commits found".to_string()))?;

    // Read all events from WAL
    let events = wal.read_all()?;

    if events.is_empty() {
        return Err(GriteError::InvalidArgs("No events to snapshot".to_string()));
    }

    // Create snapshot
    let oid = snapshot_mgr.create(wal_head, &events)?;

    output_success(
        cli,
        SnapshotCreateOutput {
            oid: oid.to_string(),
            event_count: events.len(),
            wal_head: wal_head.to_string(),
        },
    );

    Ok(())
}

fn run_list(cli: &Cli) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let snapshot_mgr = ctx.open_snapshot()?;

    let snapshots = snapshot_mgr.list()?;
    let total = snapshots.len();

    let snapshot_infos: Vec<SnapshotInfo> = snapshots
        .into_iter()
        .map(|s| SnapshotInfo {
            oid: s.oid.to_string(),
            timestamp: s.timestamp,
            ref_name: s.ref_name,
        })
        .collect();

    output_success(
        cli,
        SnapshotListOutput {
            snapshots: snapshot_infos,
            total,
        },
    );

    Ok(())
}

fn run_gc(cli: &Cli, keep: usize) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let snapshot_mgr = ctx.open_snapshot()?;

    let stats = snapshot_mgr.gc(keep)?;

    output_success(
        cli,
        SnapshotGcOutput {
            deleted: stats.deleted,
            kept: stats.kept,
        },
    );

    Ok(())
}
