use std::collections::HashMap;

use libgrite_core::{
    config::list_actors,
    integrity::{check_store_integrity, verify_store_signatures, CorruptionKind},
    types::ids::id_to_hex,
    GriteError,
};

use crate::context::GriteContext;
use crate::types::*;

/// Show database statistics.
pub fn db_stats(ctx: &GriteContext) -> Result<DbStatsResult, GriteError> {
    let store = ctx.open_store()?;
    let sled_path = ctx.sled_path();

    let stats = store.stats(&sled_path)?;

    Ok(DbStatsResult {
        path: sled_path,
        size_bytes: stats.size_bytes,
        event_count: stats.event_count,
        issue_count: stats.issue_count,
        last_rebuild_ts: stats.last_rebuild_ts,
        events_since_rebuild: stats.events_since_rebuild,
        days_since_rebuild: stats.days_since_rebuild,
        rebuild_recommended: stats.rebuild_recommended,
    })
}

/// Check database integrity.
pub fn db_check(ctx: &GriteContext, opts: &DbCheckOptions) -> Result<DbCheckResult, GriteError> {
    let store = ctx.open_store()?;

    let report = check_store_integrity(&store, opts.verify_parents)?;

    let hash_mismatches: Vec<String> = report
        .corrupt_events
        .iter()
        .filter_map(|e| {
            if let CorruptionKind::HashMismatch { expected, computed } = &e.kind {
                Some(format!(
                    "{}: hash mismatch expected {} computed {}",
                    id_to_hex(&e.event_id),
                    id_to_hex(expected),
                    id_to_hex(computed)
                ))
            } else {
                None
            }
        })
        .collect();

    let parent_errors: Vec<String> = report
        .corrupt_events
        .iter()
        .filter_map(|e| {
            if let CorruptionKind::MissingParent { parent_id } = &e.kind {
                Some(format!(
                    "{}: missing parent {}",
                    id_to_hex(&e.event_id),
                    id_to_hex(parent_id)
                ))
            } else {
                None
            }
        })
        .collect();

    Ok(DbCheckResult {
        checked_events: report.events_checked,
        hash_mismatches,
        parent_errors,
    })
}

/// Verify event signatures.
pub fn db_verify(ctx: &GriteContext, opts: &DbVerifyOptions) -> Result<DbVerifyResult, GriteError> {
    let store = ctx.open_store()?;

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

    let invalid_signatures: Vec<String> = report
        .signature_errors
        .iter()
        .map(|e| {
            format!(
                "{} (actor {}): {}",
                id_to_hex(&e.event_id),
                e.actor_id,
                e.error
            )
        })
        .collect();

    Ok(DbVerifyResult {
        checked_events: report.events_checked,
        invalid_signatures,
    })
}
