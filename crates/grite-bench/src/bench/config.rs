//! Benchmark configuration

use std::path::PathBuf;
use super::scenario::BenchmarkScenario;

/// Configuration for a benchmark run
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// The scenario to run
    pub scenario: BenchmarkScenario,
    /// Repository path (None = use temp directory)
    pub repo_path: Option<PathBuf>,
    /// Path for JSON report output
    pub json_report_path: Option<PathBuf>,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            scenario: BenchmarkScenario::default(),
            repo_path: None,
            json_report_path: None,
        }
    }
}
