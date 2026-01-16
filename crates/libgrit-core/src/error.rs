use thiserror::Error;

/// Main error type for grit operations
#[derive(Debug, Error)]
pub enum GritError {
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("database busy: {0}")]
    DbBusy(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("sled error: {0}")]
    Sled(#[from] sled::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("ID parse error: {0}")]
    IdParse(#[from] crate::types::ids::IdParseError),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("IPC error: {0}")]
    Ipc(String),
}

impl GritError {
    /// Get the error code for JSON output (from cli-json.md)
    pub fn error_code(&self) -> &'static str {
        match self {
            GritError::InvalidArgs(_) => "invalid_args",
            GritError::NotFound(_) => "not_found",
            GritError::Conflict(_) => "conflict",
            GritError::DbBusy(_) => "db_busy",
            GritError::Io(_) => "io_error",
            GritError::Sled(_) => "db_error",
            GritError::Json(_) => "internal_error",
            GritError::TomlParse(_) => "invalid_args",
            GritError::TomlSerialize(_) => "internal_error",
            GritError::IdParse(_) => "invalid_args",
            GritError::Internal(_) => "internal_error",
            GritError::Ipc(_) => "ipc_error",
        }
    }

    /// Get the exit code for CLI (from cli-json.md)
    pub fn exit_code(&self) -> i32 {
        match self {
            GritError::InvalidArgs(_) => 2,
            GritError::NotFound(_) => 3,
            GritError::Conflict(_) => 4,
            GritError::DbBusy(_) => 5,
            GritError::Io(_) => 5,
            GritError::Sled(_) => 5,
            GritError::IdParse(_) => 2,
            GritError::Ipc(_) => 6,
            _ => 1,
        }
    }
}
