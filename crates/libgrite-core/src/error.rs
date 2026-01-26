use thiserror::Error;

/// Main error type for grit operations
#[derive(Debug, Error)]
pub enum GriteError {
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

impl GriteError {
    /// Get the error code for JSON output (from cli-json.md)
    pub fn error_code(&self) -> &'static str {
        match self {
            GriteError::InvalidArgs(_) => "invalid_args",
            GriteError::NotFound(_) => "not_found",
            GriteError::Conflict(_) => "conflict",
            GriteError::DbBusy(_) => "db_busy",
            GriteError::Io(_) => "io_error",
            GriteError::Sled(_) => "db_error",
            GriteError::Json(_) => "internal_error",
            GriteError::TomlParse(_) => "invalid_args",
            GriteError::TomlSerialize(_) => "internal_error",
            GriteError::IdParse(_) => "invalid_args",
            GriteError::Internal(_) => "internal_error",
            GriteError::Ipc(_) => "ipc_error",
        }
    }

    /// Get the exit code for CLI (from cli-json.md)
    pub fn exit_code(&self) -> i32 {
        match self {
            GriteError::InvalidArgs(_) => 2,
            GriteError::NotFound(_) => 3,
            GriteError::Conflict(_) => 4,
            GriteError::DbBusy(_) => 5,
            GriteError::Io(_) => 5,
            GriteError::Sled(_) => 5,
            GriteError::IdParse(_) => 2,
            GriteError::Ipc(_) => 6,
            _ => 1,
        }
    }

    /// Get actionable suggestions for fixing the error
    pub fn suggestions(&self) -> Vec<&'static str> {
        match self {
            GriteError::NotFound(msg) => {
                if msg.contains("issue") || msg.starts_with("Issue") {
                    vec!["Run 'grit issue list' to see available issues"]
                } else if msg.contains("actor") {
                    vec!["Run 'grit actor init' to create an actor"]
                } else {
                    vec![]
                }
            }
            GriteError::DbBusy(_) => vec![
                "Try 'grit --no-daemon <command>' to bypass the daemon",
                "Or wait for the other process to finish",
                "Or run 'grit daemon stop' to stop the daemon",
            ],
            GriteError::Sled(_) => vec![
                "Run 'grit doctor --fix' to rebuild the database",
                "If problem persists, check disk space and permissions",
            ],
            GriteError::Ipc(_) => vec![
                "Run 'grit daemon stop' and retry",
                "Or use 'grit --no-daemon <command>' to bypass IPC",
            ],
            GriteError::Conflict(_) => vec![
                "Run 'grit sync' to pull latest changes",
            ],
            GriteError::IdParse(_) => vec![
                "IDs should be hex strings (e.g., 'abc123...')",
                "Use 'grit issue list' to see valid issue IDs",
            ],
            _ => vec![],
        }
    }

    /// Create a NotFound error for an issue with helpful context
    pub fn issue_not_found(issue_id: &str) -> Self {
        GriteError::NotFound(format!(
            "Issue '{}' not found",
            if issue_id.len() > 16 {
                &issue_id[..16]
            } else {
                issue_id
            }
        ))
    }

    /// Create a DbBusy error with process info
    pub fn database_locked(details: Option<&str>) -> Self {
        let msg = match details {
            Some(d) => format!("Database is locked ({})", d),
            None => "Database is locked by another process".to_string(),
        };
        GriteError::DbBusy(msg)
    }
}
