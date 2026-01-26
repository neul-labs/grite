use std::process::Command as StdCommand;
use sha2::{Sha256, Digest};

use libgrite_core::{
    context::{context_issue_id, PROJECT_CONTEXT_ISSUE_ID},
    context::extractor::{detect_language, extract_symbols, generate_summary},
    hash::compute_event_id,
    types::event::{Event, EventKind},
    types::ids::{id_to_hex},
    GriteError,
};
use crate::cli::{Cli, ContextCommand};
use crate::context::GriteContext;
use crate::output::output_success;
use crate::event_helper::insert_and_append;

pub fn run(cli: &Cli, cmd: ContextCommand) -> Result<(), GriteError> {
    match cmd {
        ContextCommand::Index { path, force, pattern } => run_index(cli, path, force, pattern),
        ContextCommand::Query { query } => run_query(cli, query),
        ContextCommand::Show { path } => run_show(cli, path),
        ContextCommand::Project { key } => run_project(cli, key),
        ContextCommand::Set { key, value } => run_set(cli, key, value),
    }
}

fn current_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn run_index(cli: &Cli, paths: Vec<String>, force: bool, pattern: Option<String>) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let store = ctx.open_store()?;
    let wal = ctx.open_wal().map_err(|e| GriteError::Internal(e.to_string()))?;

    let actor_id_bytes = libgrite_core::types::ids::hex_to_id::<16>(&ctx.actor_id)
        .map_err(|e| GriteError::InvalidArgs(format!("Invalid actor ID: {}", e)))?;

    // Get list of files to index
    let files = get_files_to_index(&paths, &pattern)?;

    let mut indexed = 0u32;
    let mut skipped = 0u32;

    for file_path in &files {
        // Read file content
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => {
                skipped += 1;
                continue; // Skip binary or unreadable files
            }
        };

        // Compute content hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash: [u8; 32] = hasher.finalize().into();

        // Check if already indexed with same hash
        if !force {
            if let Ok(Some(existing)) = store.get_file_context(file_path) {
                if existing.content_hash == content_hash {
                    skipped += 1;
                    continue;
                }
            }
        }

        // Extract symbols
        let language = detect_language(file_path);
        if language == "unknown" {
            skipped += 1;
            continue;
        }

        let symbols = extract_symbols(&content, language);
        let summary = generate_summary(file_path, &symbols, language);

        // Create context event
        let issue_id = context_issue_id(file_path);
        let ts = current_ts();
        let kind = EventKind::ContextUpdated {
            path: file_path.clone(),
            language: language.to_string(),
            symbols,
            summary,
            content_hash,
        };
        let event_id = compute_event_id(&issue_id, &actor_id_bytes, ts, None, &kind);
        let event = Event::new(event_id, issue_id, actor_id_bytes, ts, None, kind);

        insert_and_append(&store, &wal, &actor_id_bytes, &event)?;
        indexed += 1;
    }

    let output = serde_json::json!({
        "indexed": indexed,
        "skipped": skipped,
        "total_files": files.len(),
    });

    output_success(cli, &output);
    Ok(())
}

fn run_query(cli: &Cli, query: String) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let store = ctx.open_store()?;

    let results = store.query_symbols(&query)?;

    let matches: Vec<serde_json::Value> = results.iter().map(|(name, path)| {
        serde_json::json!({
            "symbol": name,
            "path": path,
        })
    }).collect();

    let output = serde_json::json!({
        "query": query,
        "matches": matches,
        "count": matches.len(),
    });

    output_success(cli, &output);
    Ok(())
}

fn run_show(cli: &Cli, path: String) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let store = ctx.open_store()?;

    let file_ctx = store.get_file_context(&path)?
        .ok_or_else(|| GriteError::NotFound(format!("No context found for '{}'", path)))?;

    let symbols: Vec<serde_json::Value> = file_ctx.symbols.iter().map(|s| {
        serde_json::json!({
            "name": s.name,
            "kind": s.kind,
            "line_start": s.line_start,
            "line_end": s.line_end,
        })
    }).collect();

    let output = serde_json::json!({
        "path": file_ctx.path,
        "language": file_ctx.language,
        "summary": file_ctx.summary,
        "content_hash": id_to_hex(&file_ctx.content_hash),
        "symbols": symbols,
        "symbol_count": symbols.len(),
    });

    output_success(cli, &output);
    Ok(())
}

fn run_project(cli: &Cli, key: Option<String>) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let store = ctx.open_store()?;

    if let Some(key) = key {
        let entry = store.get_project_context(&key)?
            .ok_or_else(|| GriteError::NotFound(format!("Project context key '{}' not found", key)))?;

        let output = serde_json::json!({
            "key": key,
            "value": entry.value,
        });
        output_success(cli, &output);
    } else {
        let entries = store.list_project_context()?;
        let list: Vec<serde_json::Value> = entries.iter().map(|(k, v)| {
            serde_json::json!({
                "key": k,
                "value": v.value,
            })
        }).collect();

        let output = serde_json::json!({
            "entries": list,
            "count": list.len(),
        });
        output_success(cli, &output);
    }

    Ok(())
}

fn run_set(cli: &Cli, key: String, value: String) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let store = ctx.open_store()?;
    let wal = ctx.open_wal().map_err(|e| GriteError::Internal(e.to_string()))?;

    let actor_id_bytes = libgrite_core::types::ids::hex_to_id::<16>(&ctx.actor_id)
        .map_err(|e| GriteError::InvalidArgs(format!("Invalid actor ID: {}", e)))?;

    let ts = current_ts();
    let kind = EventKind::ProjectContextUpdated {
        key: key.clone(),
        value: value.clone(),
    };
    let event_id = compute_event_id(&PROJECT_CONTEXT_ISSUE_ID, &actor_id_bytes, ts, None, &kind);
    let event = Event::new(event_id, PROJECT_CONTEXT_ISSUE_ID, actor_id_bytes, ts, None, kind);

    insert_and_append(&store, &wal, &actor_id_bytes, &event)?;

    let output = serde_json::json!({
        "key": key,
        "value": value,
        "action": "set",
    });

    output_success(cli, &output);
    Ok(())
}

/// Get the list of files to index, using git ls-files
fn get_files_to_index(paths: &[String], pattern: &Option<String>) -> Result<Vec<String>, GriteError> {
    let mut cmd = StdCommand::new("git");
    cmd.arg("ls-files");

    if !paths.is_empty() {
        for p in paths {
            cmd.arg(p);
        }
    }

    let output = cmd.output()
        .map_err(|e| GriteError::Internal(format!("Failed to run git ls-files: {}", e)))?;

    if !output.status.success() {
        return Err(GriteError::Internal("git ls-files failed".to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files: Vec<String> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    // Apply glob pattern filter if specified
    if let Some(pat) = pattern {
        let glob = glob::Pattern::new(pat)
            .map_err(|e| GriteError::InvalidArgs(format!("Invalid glob pattern: {}", e)))?;
        files.retain(|f| glob.matches(f));
    }

    Ok(files)
}
