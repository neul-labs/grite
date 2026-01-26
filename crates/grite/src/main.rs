mod agents_md;
mod cli;
mod commands;
mod context;
mod output;
mod event_helper;
mod router;

use clap::Parser;
use cli::{Cli, Command};
use libgrite_core::GriteError;

fn main() {
    let cli = Cli::parse();

    let result = run_command(&cli);

    if let Err(e) = result {
        output::output_error(&cli, &e);
        std::process::exit(e.exit_code());
    }
}

fn run_command(cli: &Cli) -> Result<(), GriteError> {
    // Check if this command can be routed through daemon
    if router::should_route_through_daemon(&cli.command) {
        if let Some(ipc_cmd) = router::cli_to_ipc_command(&cli.command) {
            // Try to route through daemon
            if let Some(result) = try_route_through_daemon(cli, ipc_cmd)? {
                return result;
            }
            // Fall through to local execution if daemon not available
        }
    }

    // Execute locally
    match &cli.command {
        Command::Init { no_agents_md } => commands::init::run(cli, *no_agents_md),
        Command::Actor { cmd } => commands::actor::run(cli, cmd.clone()),
        Command::Issue { cmd } => commands::issue::run(cli, cmd.clone()),
        Command::Db { cmd } => commands::db::run(cli, cmd.clone()),
        Command::Export { format, since } => commands::export::run(cli, format.clone(), since.clone()),
        Command::Rebuild { from_snapshot } => commands::rebuild::run(cli, *from_snapshot),
        Command::Sync { remote, pull, push } => commands::sync::run(cli, remote.clone(), *pull, *push),
        Command::Snapshot { cmd } => commands::snapshot::run(cli, cmd.clone()),
        Command::Daemon { cmd } => commands::daemon::run(cli, cmd.clone()),
        Command::Lock { cmd } => commands::lock::run(cli, cmd.clone()),
        Command::Doctor { fix } => commands::doctor::run(cli, *fix),
        Command::Context { cmd } => commands::context::run(cli, cmd.clone()),
    }
}

/// Try to route a command through the daemon.
/// Returns:
/// - Ok(Some(Ok(()))) if daemon handled the command successfully
/// - Ok(Some(Err(_))) if daemon returned an error
/// - Ok(None) if should execute locally (no daemon or --no-daemon)
/// - Err(_) if blocked by another process
fn try_route_through_daemon(
    cli: &Cli,
    ipc_cmd: libgrite_ipc::IpcCommand,
) -> Result<Option<Result<(), GriteError>>, GriteError> {
    // Try to get context - may fail for init command
    let ctx = match context::GriteContext::resolve(cli) {
        Ok(ctx) => ctx,
        Err(_) => return Ok(None), // Execute locally
    };

    match router::route_command(&ctx, cli, ipc_cmd)? {
        router::RouteResult::Local => Ok(None),
        router::RouteResult::DaemonResponse(response) => {
            Ok(Some(handle_daemon_response(cli, response)))
        }
        router::RouteResult::Blocked { pid, expires_in_ms } => {
            Err(GriteError::DbBusy(format!(
                "Data directory locked by daemon (PID {}, expires in {}s). Use --no-daemon to wait or try later.",
                pid,
                expires_in_ms / 1000
            )))
        }
    }
}

/// Handle a response from the daemon
fn handle_daemon_response(cli: &Cli, response: libgrite_ipc::IpcResponse) -> Result<(), GriteError> {
    if response.ok {
        // Output the data
        if let Some(data) = response.data {
            if cli.json {
                // Data is already JSON, just print it
                if !cli.quiet {
                    println!("{}", data);
                }
            } else {
                // Try to format nicely
                output_daemon_data(cli, &data)?;
            }
        }
        Ok(())
    } else {
        // Extract error info
        let (code, message) = match response.error {
            Some(err) => (err.code, err.message),
            None => ("unknown".to_string(), "Unknown error".to_string()),
        };

        // Map error code to GriteError
        match code.as_str() {
            "not_found" => Err(GriteError::NotFound(message)),
            "invalid_input" | "invalid_args" => Err(GriteError::InvalidArgs(message)),
            "conflict" => Err(GriteError::Conflict(message)),
            "db_busy" => Err(GriteError::DbBusy(message)),
            "ipc_error" => Err(GriteError::Ipc(message)),
            _ => Err(GriteError::Internal(message)),
        }
    }
}

/// Output daemon response data in human-readable format
fn output_daemon_data(cli: &Cli, data: &str) -> Result<(), GriteError> {
    if cli.quiet {
        return Ok(());
    }

    // Try to parse as JSON and format nicely
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
        // Handle known response types
        if let Some(issues) = json.get("issues") {
            // Issue list response
            if let Some(arr) = issues.as_array() {
                for issue in arr {
                    let id = issue.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                    let state = issue.get("state").and_then(|v| v.as_str()).unwrap_or("?");
                    let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("?");
                    println!("{} [{}] {}", &id[..8.min(id.len())], state, title);
                }
            }
        } else if json.get("issue_id").is_some() {
            // Issue created response
            let issue_id = json.get("issue_id").and_then(|v| v.as_str()).unwrap_or("?");
            println!("Created issue {}", issue_id);
        } else if json.get("event_count").is_some() {
            // Rebuild response
            let events = json.get("event_count").and_then(|v| v.as_u64()).unwrap_or(0);
            let issues = json.get("issue_count").and_then(|v| v.as_u64()).unwrap_or(0);
            println!("Rebuilt: {} events, {} issues", events, issues);
        } else if json.get("path").is_some() {
            // DB stats response
            println!("{}", serde_json::to_string_pretty(&json)?);
        } else {
            // Unknown format, just print
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    } else {
        // Not JSON, print raw
        println!("{}", data);
    }

    Ok(())
}
