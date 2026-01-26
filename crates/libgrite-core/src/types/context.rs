use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use super::event::SymbolInfo;
use super::issue::Version;

/// Context for a single file in the repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContext {
    pub path: String,
    pub language: String,
    pub symbols: Vec<SymbolInfo>,
    pub summary: String,
    pub content_hash: [u8; 32],
    /// LWW version tracking per file path
    pub version: Version,
}

/// A single entry in the project context store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContextEntry {
    pub value: String,
    pub version: Version,
}

/// Aggregate project-level context (LWW-Map)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectContext {
    pub entries: BTreeMap<String, ProjectContextEntry>,
}
