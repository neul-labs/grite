//! grite-bench - TUI benchmark for testing concurrent agent writes

mod app;
mod bench;
mod error;
mod ui;

use std::path::PathBuf;

use clap::Parser;

use app::App;
use bench::{BenchmarkConfig, BenchmarkScenario};
use error::Result;

#[derive(Parser)]
#[command(name = "grite-bench")]
#[command(about = "TUI benchmark for testing concurrent agent writes to grite")]
#[command(version)]
struct Cli {
    /// Number of concurrent agents
    #[arg(short = 'n', long, default_value = "8")]
    agents: usize,

    /// Operations per agent
    #[arg(short = 'o', long, default_value = "100")]
    operations: usize,

    /// Repository path (uses temp directory if not specified)
    #[arg(short = 'r', long)]
    repo: Option<PathBuf>,

    /// Scenario: burst, sustained, ramp
    #[arg(short = 's', long, default_value = "burst")]
    scenario: String,

    /// Output JSON report to file
    #[arg(short = 'j', long)]
    json_report: Option<PathBuf>,

    /// Non-interactive mode (no TUI)
    #[arg(long)]
    headless: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Parse scenario
    let scenario = BenchmarkScenario::from_name(&cli.scenario, cli.agents, cli.operations)
        .ok_or_else(|| error::BenchError::Config(format!(
            "Unknown scenario: '{}'. Use: burst, sustained, or ramp",
            cli.scenario
        )))?;

    let config = BenchmarkConfig {
        scenario,
        repo_path: cli.repo,
        json_report_path: cli.json_report,
    };

    if cli.headless {
        app::run_headless(config)?;
    } else {
        let mut app = App::new(config)?;
        app.run()?;
    }

    Ok(())
}
