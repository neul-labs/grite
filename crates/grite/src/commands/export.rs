use libgrite_core::{
    export::{export_json, export_markdown, ExportSince},
    types::ids::hex_to_id,
    GriteError,
};
use serde::Serialize;
use crate::cli::{Cli, ExportFormat};
use crate::context::GriteContext;
use crate::output::output_success;

#[derive(Serialize)]
struct ExportOutput {
    format: String,
    output_path: String,
    wal_head: Option<String>,
    event_count: usize,
}

pub fn run(cli: &Cli, format: ExportFormat, since: Option<String>) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;
    let store = ctx.open_store()?;

    // Parse since filter
    let since_filter = match since {
        Some(s) => {
            // Try to parse as event_id first, then as timestamp
            if s.len() == 64 {
                let event_id = hex_to_id(&s)?;
                Some(ExportSince::EventId(event_id))
            } else {
                let ts: u64 = s.parse()
                    .map_err(|_| GriteError::InvalidArgs(format!("Invalid since value: {}", s)))?;
                Some(ExportSince::Timestamp(ts))
            }
        }
        None => None,
    };

    // Create .grite directory if needed
    let repo_root = ctx.git_dir.parent()
        .ok_or_else(|| GriteError::Internal(
            "Cannot determine repository root from git directory".to_string()
        ))?;
    let grite_export_dir = repo_root.join(".grite");
    std::fs::create_dir_all(&grite_export_dir)?;

    let (format_str, output_path, event_count) = match format {
        ExportFormat::Json => {
            let export = export_json(&store, since_filter)?;
            let output_path = grite_export_dir.join("export.json");
            let content = serde_json::to_string_pretty(&export)?;
            std::fs::write(&output_path, &content)?;
            ("json".to_string(), output_path, export.meta.event_count)
        }
        ExportFormat::Md => {
            let md = export_markdown(&store, since_filter)?;
            let output_path = grite_export_dir.join("export.md");
            std::fs::write(&output_path, &md)?;
            // Count events by parsing (approximate)
            let event_count = md.lines().filter(|l| l.starts_with("**ID:**")).count();
            ("md".to_string(), output_path, event_count)
        }
    };

    output_success(cli, ExportOutput {
        format: format_str,
        output_path: output_path.to_string_lossy().to_string(),
        wal_head: None,
        event_count,
    });

    Ok(())
}
