//! Benchmark configuration

use super::scenario::BenchmarkScenario;
use std::path::PathBuf;

/// Configuration for a benchmark run
#[derive(Debug, Clone, Default)]
pub struct BenchmarkConfig {
    /// The scenario to run
    pub scenario: BenchmarkScenario,
    /// Repository path (None = use temp directory)
    pub repo_path: Option<PathBuf>,
    /// Path for JSON report output
    pub json_report_path: Option<PathBuf>,
}
