//! Doctor command - health checks and auto-repair

use std::collections::HashSet;
use std::fs;

use libgrite_core::config::{list_actors, actor_sled_path};
use libgrite_core::integrity::check_store_integrity;
use libgrite_core::{EventId, GriteStore, GriteError};
use libgrite_git::WalManager;
use serde::Serialize;

use crate::cli::Cli;
use crate::commands::daemon::{is_daemon_running, start_daemon, stop_daemon};
use crate::context::{ExecutionMode, GriteContext};
use crate::output::output_success;

#[derive(Serialize)]
struct DoctorOutput {
    checks: Vec<CheckResult>,
    applied: Vec<String>,
}

#[derive(Serialize)]
struct CheckResult {
    id: String,
    status: String,
    message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    plan: Vec<String>,
}

impl CheckResult {
    fn ok(id: &str, message: &str) -> Self {
        Self {
            id: id.to_string(),
            status: "ok".to_string(),
            message: message.to_string(),
            plan: vec![],
        }
    }

    fn warn(id: &str, message: &str, plan: Vec<&str>) -> Self {
        Self {
            id: id.to_string(),
            status: "warn".to_string(),
            message: message.to_string(),
            plan: plan.into_iter().map(String::from).collect(),
        }
    }

    fn error(id: &str, message: &str, plan: Vec<&str>) -> Self {
        Self {
            id: id.to_string(),
            status: "error".to_string(),
            message: message.to_string(),
            plan: plan.into_iter().map(String::from).collect(),
        }
    }
}

fn store_held_by_daemon(cli: &Cli) -> bool {
    GriteContext::resolve(cli)
        .map(|ctx| matches!(
            ctx.execution_mode(cli.no_daemon),
            ExecutionMode::Daemon { .. } | ExecutionMode::Blocked { .. }
        ))
        .unwrap_or(false)
}

pub fn run(cli: &Cli, fix: bool) -> Result<(), GriteError> {
    let mut checks = Vec::new();
    let mut applied = Vec::new();

    // Stop daemon before any work if --fix is requested
    // This ensures checks can properly detect issues and fixes can be applied
    let daemon_was_running = fix && is_daemon_running(cli);
    if daemon_was_running {
        if !cli.quiet && !cli.json {
            eprintln!("Stopping daemon for repairs...");
        }
        let _ = stop_daemon(cli);
        // Give the daemon a moment to release the lock
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Check 1: Git repository
    checks.push(check_git_repo(cli));

    // Check 2: WAL ref
    checks.push(check_wal_ref(cli));

    // Check 3: Actor config
    checks.push(check_actor_config(cli));

    // Check 4: Store integrity
    let (store_check, needs_rebuild) = check_store(cli);
    checks.push(store_check);

    // Check 5: Rebuild threshold
    checks.push(check_rebuild_threshold(cli));

    // Check 6: Legacy per-actor sleds
    let (orphan_check, needs_merge) = check_legacy_actor_sleds(cli);
    checks.push(orphan_check);

    // Auto-repair if requested
    if fix && needs_rebuild {
        if let Ok(ctx) = GriteContext::resolve(cli) {
            if let Ok(store) = ctx.open_store() {
                if store.rebuild().is_ok() {
                    applied.push("rebuild".to_string());
                }
            }
        }
    }

    if fix && needs_merge {
        match fix_legacy_actor_sleds(cli) {
            Ok((merged, cleaned)) if merged > 0 || cleaned > 0 => {
                if merged > 0 {
                    applied.push(format!("merged {} legacy event(s)", merged));
                }
                if cleaned > 0 {
                    applied.push(format!("cleaned {} legacy sled(s)", cleaned));
                }
                if let Some(c) = checks.iter_mut().find(|c| c.id == "legacy_actor_sleds") {
                    let msg = match (merged, cleaned) {
                        (m, c) if m > 0 && c > 0 => format!("merged {} event(s), cleaned {} legacy sled(s)", m, c),
                        (m, _) if m > 0 => format!("merged {} legacy event(s) into shared store", m),
                        (_, c) => format!("cleaned {} legacy sled(s)", c),
                    };
                    *c = CheckResult::ok("legacy_actor_sleds", &msg);
                }
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }

    // Restart daemon if we stopped it
    if daemon_was_running {
        if !cli.quiet && !cli.json {
            eprintln!("Restarting daemon...");
        }
        let _ = start_daemon(cli, 300); // Default 5 min idle timeout
    }

    let has_errors = checks.iter().any(|c| c.status == "error");
    let did_repair = !applied.is_empty();

    if cli.json {
        output_success(cli, DoctorOutput { checks, applied });
    } else if !cli.quiet {
        // Human-readable output
        for check in &checks {
            let icon = match check.status.as_str() {
                "ok" => "[ok]",
                "warn" => "[!!]",
                "error" => "[ERR]",
                _ => "[?]",
            };
            println!("{} {}: {}", icon, check.id, check.message);
            for plan_item in &check.plan {
                println!("     -> {}", plan_item);
            }
        }
        if !applied.is_empty() {
            println!("\nApplied fixes: {}", applied.join(", "));
        }
    }

    if has_errors && !did_repair {
        return Err(GriteError::Internal("Health checks failed".to_string()));
    }

    Ok(())
}

fn check_git_repo(cli: &Cli) -> CheckResult {
    match GriteContext::resolve(cli) {
        Ok(ctx) => {
            let git_dir = ctx.repo_root().join(".git");
            if git_dir.exists() {
                CheckResult::ok("git_repo", "Git repository is valid")
            } else {
                CheckResult::error("git_repo", "Not a git repository", vec!["Run 'git init'"])
            }
        }
        Err(_) => CheckResult::error(
            "git_repo",
            "Cannot resolve repository context",
            vec!["Ensure you are in a git repository", "Run 'grite init'"],
        ),
    }
}

fn check_wal_ref(cli: &Cli) -> CheckResult {
    let ctx = match GriteContext::resolve(cli) {
        Ok(ctx) => ctx,
        Err(_) => {
            return CheckResult::warn(
                "wal_ref",
                "Cannot check WAL - no context",
                vec!["Fix git_repo first"],
            )
        }
    };

    let git_dir = ctx.repo_root().join(".git");
    match WalManager::open(&git_dir) {
        Ok(wal) => match wal.head() {
            Ok(Some(_)) => CheckResult::ok("wal_ref", "WAL ref exists and is readable"),
            Ok(None) => CheckResult::ok("wal_ref", "WAL ref not yet created (empty)"),
            Err(e) => CheckResult::error(
                "wal_ref",
                &format!("WAL ref is corrupted: {}", e),
                vec!["Run 'grite doctor --fix' to rebuild"],
            ),
        },
        Err(e) => CheckResult::error(
            "wal_ref",
            &format!("Cannot open WAL manager: {}", e),
            vec!["Check git repository integrity"],
        ),
    }
}

fn check_actor_config(cli: &Cli) -> CheckResult {
    match GriteContext::resolve(cli) {
        Ok(ctx) => {
            if ctx.actor_id.is_empty() {
                CheckResult::warn(
                    "actor_config",
                    "No actor configured",
                    vec!["Run 'grite actor init'"],
                )
            } else {
                CheckResult::ok(
                    "actor_config",
                    &format!("Actor configured: {}", &ctx.actor_id[..8.min(ctx.actor_id.len())]),
                )
            }
        }
        Err(_) => CheckResult::warn(
            "actor_config",
            "Cannot check actor config - no context",
            vec!["Run 'grite init' first"],
        ),
    }
}

fn check_store(cli: &Cli) -> (CheckResult, bool) {
    let ctx = match GriteContext::resolve(cli) {
        Ok(ctx) => ctx,
        Err(_) => {
            return (
                CheckResult::warn(
                    "store_integrity",
                    "Cannot check store - no context",
                    vec!["Fix git_repo first"],
                ),
                false,
            )
        }
    };

    // If the daemon is running it holds the exclusive flock — that's healthy.
    if store_held_by_daemon(cli) {
        return (
            CheckResult::ok("store_integrity", "Store held by running daemon"),
            false,
        );
    }

    let store = match ctx.open_store() {
        Ok(store) => store,
        Err(e) => {
            return (
                CheckResult::error(
                    "store_integrity",
                    &format!("Cannot open store: {}", e),
                    vec!["Run 'grite doctor --fix' to rebuild"],
                ),
                true,
            )
        }
    };

    match check_store_integrity(&store, false) {
        Ok(report) => {
            if report.is_healthy() {
                (
                    CheckResult::ok(
                        "store_integrity",
                        &format!("{} events verified", report.events_checked),
                    ),
                    false,
                )
            } else {
                (
                    CheckResult::error(
                        "store_integrity",
                        &format!(
                            "{} corrupt events found out of {}",
                            report.corruption_count(),
                            report.events_checked
                        ),
                        vec!["Run 'grite doctor --fix' to rebuild from WAL"],
                    ),
                    true,
                )
            }
        }
        Err(e) => (
            CheckResult::error(
                "store_integrity",
                &format!("Integrity check failed: {}", e),
                vec!["Run 'grite doctor --fix' to rebuild"],
            ),
            true,
        ),
    }
}

fn check_rebuild_threshold(cli: &Cli) -> CheckResult {
    let ctx = match GriteContext::resolve(cli) {
        Ok(ctx) => ctx,
        Err(_) => {
            return CheckResult::warn(
                "rebuild_threshold",
                "Cannot check rebuild threshold - no context",
                vec!["Fix git_repo first"],
            )
        }
    };

    // Daemon holds the store; skip this check to avoid lock contention.
    if store_held_by_daemon(cli) {
        return CheckResult::ok(
            "rebuild_threshold",
            "Rebuild threshold managed by running daemon",
        );
    }

    let store = match ctx.open_store() {
        Ok(store) => store,
        Err(_) => {
            return CheckResult::warn(
                "rebuild_threshold",
                "Cannot check rebuild threshold - cannot open store",
                vec!["Fix store_integrity first"],
            )
        }
    };

    let sled_path = ctx.sled_path();
    match store.stats(&sled_path) {
        Ok(stats) => {
            if stats.rebuild_recommended {
                let days_msg = stats
                    .days_since_rebuild
                    .map(|d| format!(" ({} days ago)", d))
                    .unwrap_or_default();
                CheckResult::warn(
                    "rebuild_threshold",
                    &format!(
                        "{} events since last rebuild{}",
                        stats.events_since_rebuild, days_msg
                    ),
                    vec!["Run 'grite rebuild' to optimize performance"],
                )
            } else {
                let events_msg = if stats.events_since_rebuild > 0 {
                    format!("{} events since last rebuild", stats.events_since_rebuild)
                } else {
                    "No events since last rebuild".to_string()
                };
                CheckResult::ok("rebuild_threshold", &events_msg)
            }
        }
        Err(e) => CheckResult::warn(
            "rebuild_threshold",
            &format!("Cannot check rebuild stats: {}", e),
            vec![],
        ),
    }
}

/// Check for legacy per-actor sleds — actor directories under .git/grite/actors/
/// that still contain a sled/ subdirectory with events not yet in the shared store.
///
/// In the shared-sled model all events live in .git/grite/sled; any per-actor
/// sled is a legacy artifact from before the migration and can be merged.
fn check_legacy_actor_sleds(cli: &Cli) -> (CheckResult, bool) {
    let git_dir = match GriteContext::find_git_dir() {
        Ok(d) => d,
        Err(_) => {
            return (
                CheckResult::warn("legacy_actor_sleds", "Cannot check - no git context", vec![]),
                false,
            )
        }
    };

    let ctx = match GriteContext::resolve(cli) {
        Ok(ctx) => ctx,
        Err(_) => {
            return (
                CheckResult::warn(
                    "legacy_actor_sleds",
                    "Cannot check - no actor configured",
                    vec!["Run 'grite init'"],
                ),
                false,
            )
        }
    };

    let actors = match list_actors(&git_dir) {
        Ok(a) => a,
        Err(e) => {
            return (
                CheckResult::warn(
                    "legacy_actor_sleds",
                    &format!("Cannot list actors: {}", e),
                    vec![],
                ),
                false,
            )
        }
    };

    // All per-actor sleds are legacy in the shared-sled model.
    let legacy: Vec<_> = actors
        .into_iter()
        .filter(|a| actor_sled_path(&git_dir, &a.actor_id).exists())
        .collect();

    if legacy.is_empty() {
        return (CheckResult::ok("legacy_actor_sleds", "No legacy per-actor sleds"), false);
    }

    // Can't inspect shared sled directly while the daemon holds the lock.
    if store_held_by_daemon(cli) {
        return (
            CheckResult::warn(
                "legacy_actor_sleds",
                &format!(
                    "{} legacy actor sled(s) found (store held by daemon - cannot count unmerged events)",
                    legacy.len()
                ),
                vec!["Stop the daemon and re-run 'grite doctor' to check for unmerged events"],
            ),
            false,
        );
    }

    // Collect event IDs already present in the shared store.
    let current_event_ids: HashSet<EventId> = match ctx.open_store() {
        Ok(store) => store
            .get_all_events()
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.event_id)
            .collect(),
        Err(_) => {
            return (
                CheckResult::warn(
                    "legacy_actor_sleds",
                    "Cannot open shared store to check for unmerged events",
                    vec!["Fix store_integrity first"],
                ),
                false,
            )
        }
    };

    let mut accessible = 0usize;
    let mut unmerged_events = 0usize;

    for actor in &legacy {
        let sled_path = actor_sled_path(&git_dir, &actor.actor_id);
        if let Ok(store) = GriteStore::open(&sled_path) {
            accessible += 1;
            let events = store.get_all_events().unwrap_or_default();
            unmerged_events += events
                .iter()
                .filter(|e| !current_event_ids.contains(&e.event_id))
                .count();
        }
    }

    if accessible == 0 {
        return (
            CheckResult::ok("legacy_actor_sleds", "No accessible legacy actor sleds"),
            false,
        );
    }

    if unmerged_events == 0 {
        (
            CheckResult::warn(
                "legacy_actor_sleds",
                &format!(
                    "{} legacy actor sled(s), all events already in shared store",
                    accessible
                ),
                vec!["Run 'grite doctor --fix' to clean up legacy sleds"],
            ),
            true, // needs cleanup even though no merge needed
        )
    } else {
        (
            CheckResult::warn(
                "legacy_actor_sleds",
                &format!(
                    "{} legacy actor sled(s) with {} unmerged event(s)",
                    accessible, unmerged_events
                ),
                vec!["Run 'grite doctor --fix' to merge legacy events into the shared store"],
            ),
            true,
        )
    }
}

/// Merge events from all legacy per-actor sleds into the shared store.
/// Returns the number of events merged.
/// Merge events from legacy per-actor sleds into shared store and clean up.
/// Returns (merged_count, cleaned_count).
fn fix_legacy_actor_sleds(cli: &Cli) -> Result<(usize, usize), GriteError> {
    let git_dir = GriteContext::find_git_dir()?;
    let ctx = GriteContext::resolve(cli)?;

    let actors = list_actors(&git_dir)?;
    let current_store = ctx.open_store()?;

    let current_event_ids: HashSet<EventId> = current_store
        .get_all_events()?
        .into_iter()
        .map(|e| e.event_id)
        .collect();

    let mut merged = 0usize;
    let mut cleaned = 0usize;
    let mut paths_to_clean: Vec<std::path::PathBuf> = Vec::new();

    for actor in actors {
        let sled_path = actor_sled_path(&git_dir, &actor.actor_id);
        if !sled_path.exists() {
            continue;
        }
        let legacy_store: GriteStore = match GriteStore::open(&sled_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let events = legacy_store.get_all_events().unwrap_or_default();

        // Check if all events are already in shared store
        let all_merged = events.iter().all(|e| current_event_ids.contains(&e.event_id));

        for event in &events {
            if !current_event_ids.contains(&event.event_id) {
                current_store.insert_event(event)?;
                merged += 1;
            }
        }

        // Track for cleanup - safe to delete if all events are in shared store
        if all_merged || merged > 0 {
            paths_to_clean.push(sled_path);
        }
    }

    // Rebuild projections from scratch so all merged events are applied in
    // correct chronological order.
    if merged > 0 {
        current_store.rebuild()?;
    }

    // Clean up legacy sled directories after successful merge/rebuild
    for path in paths_to_clean {
        if path.exists() {
            if let Ok(canonical) = path.canonicalize() {
                // Safety check: only delete paths under .git/grite/actors/
                if canonical.to_string_lossy().contains("/.git/grite/actors/") {
                    if fs::remove_dir_all(&path).is_ok() {
                        cleaned += 1;
                    }
                }
            }
        }
    }

    Ok((merged, cleaned))
}
