//! Error types for grite-bench

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BenchError {
    #[error("Core error: {0}")]
    Core(#[from] libgrite_core::GriteError),

    #[error("Git error: {0}")]
    Git(#[from] libgrite_git::GitError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("Benchmark error: {0}")]
    Bench(String),
}

pub type Result<T> = std::result::Result<T, BenchError>;
