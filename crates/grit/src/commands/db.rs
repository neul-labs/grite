use libgrit_core::GritError;
use serde::Serialize;
use crate::cli::{Cli, DbCommand};
use crate::context::GritContext;
use crate::output::output_success;

#[derive(Serialize)]
struct DbStatsOutput {
    path: String,
    size_bytes: u64,
    event_count: usize,
    issue_count: usize,
    last_rebuild_ts: Option<u64>,
}

pub fn run(cli: &Cli, cmd: DbCommand) -> Result<(), GritError> {
    match cmd {
        DbCommand::Stats => run_stats(cli),
    }
}

fn run_stats(cli: &Cli) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let store = ctx.open_store()?;
    let sled_path = ctx.sled_path();

    let stats = store.stats(&sled_path)?;

    output_success(cli, DbStatsOutput {
        path: stats.path,
        size_bytes: stats.size_bytes,
        event_count: stats.event_count,
        issue_count: stats.issue_count,
        last_rebuild_ts: stats.last_rebuild_ts,
    });

    Ok(())
}
