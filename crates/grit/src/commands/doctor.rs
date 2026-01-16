//! Doctor command - health checks and auto-repair

use libgrit_core::integrity::check_store_integrity;
use libgrit_core::GritError;
use libgrit_git::WalManager;
use serde::Serialize;

use crate::cli::Cli;
use crate::context::GritContext;
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

pub fn run(cli: &Cli, fix: bool) -> Result<(), GritError> {
    let mut checks = Vec::new();
    let mut applied = Vec::new();

    // Check 1: Git repository
    checks.push(check_git_repo(cli));

    // Check 2: WAL ref
    checks.push(check_wal_ref(cli));

    // Check 3: Actor config
    checks.push(check_actor_config(cli));

    // Check 4: Store integrity
    let (store_check, needs_rebuild) = check_store(cli);
    checks.push(store_check);

    // Auto-repair if requested
    if fix && needs_rebuild {
        if let Ok(ctx) = GritContext::resolve(cli) {
            if let Ok(store) = ctx.open_store() {
                if store.rebuild().is_ok() {
                    applied.push("rebuild".to_string());
                }
            }
        }
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
        return Err(GritError::Internal("Health checks failed".to_string()));
    }

    Ok(())
}

fn check_git_repo(cli: &Cli) -> CheckResult {
    match GritContext::resolve(cli) {
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
            vec!["Ensure you are in a git repository", "Run 'grit init'"],
        ),
    }
}

fn check_wal_ref(cli: &Cli) -> CheckResult {
    let ctx = match GritContext::resolve(cli) {
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
                vec!["Run 'grit doctor --fix' to rebuild"],
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
    match GritContext::resolve(cli) {
        Ok(ctx) => {
            if ctx.actor_id.is_empty() {
                CheckResult::warn(
                    "actor_config",
                    "No actor configured",
                    vec!["Run 'grit actor init'"],
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
            vec!["Run 'grit init' first"],
        ),
    }
}

fn check_store(cli: &Cli) -> (CheckResult, bool) {
    let ctx = match GritContext::resolve(cli) {
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

    let store = match ctx.open_store() {
        Ok(store) => store,
        Err(e) => {
            return (
                CheckResult::error(
                    "store_integrity",
                    &format!("Cannot open store: {}", e),
                    vec!["Run 'grit doctor --fix' to rebuild"],
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
                        vec!["Run 'grit doctor --fix' to rebuild from WAL"],
                    ),
                    true,
                )
            }
        }
        Err(e) => (
            CheckResult::error(
                "store_integrity",
                &format!("Integrity check failed: {}", e),
                vec!["Run 'grit doctor --fix' to rebuild"],
            ),
            true,
        ),
    }
}
