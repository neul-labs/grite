use libgrit_core::GritError;
use serde::Serialize;
use crate::cli::Cli;
use crate::context::GritContext;
use crate::output::output_success;

#[derive(Serialize)]
struct RebuildOutput {
    wal_head: Option<String>,
    event_count: usize,
    from_snapshot: Option<String>,
}

pub fn run(cli: &Cli) -> Result<(), GritError> {
    let ctx = GritContext::resolve(cli)?;
    let store = ctx.open_store()?;

    let stats = store.rebuild()?;

    output_success(cli, RebuildOutput {
        wal_head: None, // M1 has no WAL
        event_count: stats.event_count,
        from_snapshot: None, // M1 has no snapshots
    });

    Ok(())
}
