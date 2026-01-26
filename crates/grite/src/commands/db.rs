use libgrite_core::{
    config::list_actors,
    integrity::{check_store_integrity, verify_store_signatures, CorruptionKind},
    types::ids::id_to_hex,
    GriteError,
};
use serde::Serialize;
use std::collections::HashMap;
use crate::cli::{Cli, DbCommand};
use crate::context::GriteContext;
use crate::output::output_success;

#[derive(Serialize)]
struct DbStatsOutput {
    path: String,
    size_bytes: u64,
    event_count: usize,
    issue_count: usize,
    last_rebuild_ts: Option<u64>,
    events_since_rebuild: usize,
    days_since_rebuild: Option<u32>,
    rebuild_recommended: bool,
}

pub fn run(cli: &Cli, cmd: DbCommand) -> Result<(), GriteError> {
    match cmd {
        DbCommand::Stats => run_stats(cli),
        DbCommand::Check { verify_parents } => run_check(cli, verify_parents),
        DbCommand::Verify { verbose } => run_verify(cli, verbose),
    }
}

fn run_stats(cli: &Cli) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let store = ctx.open_store()?;
    let sled_path = ctx.sled_path();

    let stats = store.stats(&sled_path)?;

    output_success(cli, DbStatsOutput {
        path: stats.path,
        size_bytes: stats.size_bytes,
        event_count: stats.event_count,
        issue_count: stats.issue_count,
        last_rebuild_ts: stats.last_rebuild_ts,
        events_since_rebuild: stats.events_since_rebuild,
        days_since_rebuild: stats.days_since_rebuild,
        rebuild_recommended: stats.rebuild_recommended,
    });

    Ok(())
}

#[derive(Serialize)]
struct DbCheckOutput {
    events_checked: usize,
    events_valid: usize,
    corrupt_count: usize,
    errors: Vec<CorruptEventJson>,
}

#[derive(Serialize)]
struct CorruptEventJson {
    event_id: String,
    issue_id: String,
    error: String,
}

fn run_check(cli: &Cli, verify_parents: bool) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let store = ctx.open_store()?;

    let report = check_store_integrity(&store, verify_parents)?;

    let errors: Vec<CorruptEventJson> = report
        .corrupt_events
        .iter()
        .map(|e| {
            let error = match &e.kind {
                CorruptionKind::HashMismatch { expected, computed } => {
                    format!(
                        "hash mismatch: expected {}, computed {}",
                        id_to_hex(expected),
                        id_to_hex(computed)
                    )
                }
                CorruptionKind::MissingParent { parent_id } => {
                    format!("missing parent: {}", id_to_hex(parent_id))
                }
            };
            CorruptEventJson {
                event_id: id_to_hex(&e.event_id),
                issue_id: e.issue_id.clone(),
                error,
            }
        })
        .collect();

    output_success(cli, DbCheckOutput {
        events_checked: report.events_checked,
        events_valid: report.events_valid,
        corrupt_count: report.corrupt_events.len(),
        errors,
    });

    if !report.is_healthy() {
        return Err(GriteError::Internal(format!(
            "{} corrupt events found",
            report.corruption_count()
        )));
    }

    Ok(())
}

#[derive(Serialize)]
struct DbVerifyOutput {
    events_checked: usize,
    signatures_checked: usize,
    signatures_valid: usize,
    error_count: usize,
    errors: Vec<SignatureErrorJson>,
}

#[derive(Serialize)]
struct SignatureErrorJson {
    event_id: String,
    actor_id: String,
    error: String,
}

fn run_verify(cli: &Cli, verbose: bool) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let store = ctx.open_store()?;

    // Build a map of actor_id -> public_key from all actors
    let actors = list_actors(&ctx.git_dir)?;
    let mut public_keys: HashMap<String, String> = HashMap::new();

    for actor in &actors {
        if let Some(pk) = &actor.public_key {
            public_keys.insert(actor.actor_id.clone(), pk.clone());
        }
    }

    let get_public_key = |actor_id: &str| -> Option<String> {
        public_keys.get(actor_id).cloned()
    };

    let report = verify_store_signatures(&store, get_public_key)?;

    let errors: Vec<SignatureErrorJson> = if verbose {
        report
            .signature_errors
            .iter()
            .map(|e| SignatureErrorJson {
                event_id: id_to_hex(&e.event_id),
                actor_id: e.actor_id.clone(),
                error: e.error.clone(),
            })
            .collect()
    } else {
        // Only show first 10 errors in non-verbose mode
        report
            .signature_errors
            .iter()
            .take(10)
            .map(|e| SignatureErrorJson {
                event_id: id_to_hex(&e.event_id),
                actor_id: e.actor_id.clone(),
                error: e.error.clone(),
            })
            .collect()
    };

    output_success(cli, DbVerifyOutput {
        events_checked: report.events_checked,
        signatures_checked: report.signatures_checked,
        signatures_valid: report.signatures_valid,
        error_count: report.signature_errors.len(),
        errors,
    });

    if report.signature_error_count() > 0 {
        eprintln!(
            "Warning: {} signature errors found",
            report.signature_error_count()
        );
    }

    Ok(())
}
