use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Attribute, Cell, Color,
                    Table, ContentArrangement};
use libgrite_core::GriteError;
use regex::Regex;
use serde::Serialize;
use crate::cli::Cli;

/// JSON response envelope (from cli-json.md)
#[derive(Serialize)]
pub struct JsonResponse<T: Serialize> {
    pub schema_version: u32,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonError>,
}

#[derive(Serialize)]
pub struct JsonError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub details: serde_json::Value,
}

/// Output a successful result
pub fn output_success<T: Serialize>(cli: &Cli, data: T) {
    if cli.json {
        let response = JsonResponse {
            schema_version: 1,
            ok: true,
            data: Some(data),
            error: None,
        };
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
    } else if !cli.quiet {
        // For human output, serialize to JSON and print nicely
        println!("{}", serde_json::to_string_pretty(&data).unwrap());
    }
}

/// Output an error
pub fn output_error(cli: &Cli, err: &GriteError) {
    if cli.json {
        // Include suggestions in JSON details
        let suggestions = err.suggestions();
        let details = if suggestions.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::json!({ "suggestions": suggestions })
        };

        let response: JsonResponse<()> = JsonResponse {
            schema_version: 1,
            ok: false,
            data: None,
            error: Some(JsonError {
                code: err.error_code().to_string(),
                message: err.to_string(),
                details,
            }),
        };
        eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
    } else {
        eprintln!("error: {}", err);
        // Print suggestions for human-readable output
        let suggestions = err.suggestions();
        if !suggestions.is_empty() {
            eprintln!();
            eprintln!("Suggestions:");
            for suggestion in suggestions {
                eprintln!("  - {}", suggestion);
            }
        }
    }
}

/// Print human-readable output (ignored in quiet mode)
pub fn print_human(cli: &Cli, msg: &str) {
    if !cli.json && !cli.quiet {
        println!("{}", msg);
    }
}

/// Strip basic markdown formatting from a string for plain terminal display.
fn strip_markdown(input: &str) -> String {
    // Order matters: links before bold/italic to avoid partial matches
    let re_link = Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap();
    let re_bold = Regex::new(r"\*\*(.+?)\*\*").unwrap();
    let re_italic = Regex::new(r"(^|[^*])\*([^*]+?)\*([^*]|$)").unwrap();
    let re_code = Regex::new(r"`([^`]+)`").unwrap();
    let re_heading = Regex::new(r"^#+\s+").unwrap();

    let s = re_link.replace_all(input, "$1");
    let s = re_bold.replace_all(&s, "$1");
    let s = re_italic.replace_all(&s, "$1");
    let s = re_code.replace_all(&s, "$1");
    re_heading.replace_all(&s, "").to_string()
}

/// A single issue row for table formatting.
pub struct IssueRow {
    pub id: String,
    pub state: String,
    pub title: String,
    pub created_ts: u64,
}

/// Format a Unix millisecond timestamp as local date/time.
/// Example: "Apr 5, 26 2:35 pm"
fn format_local_date(ts_ms: u64) -> String {
    let secs = (ts_ms / 1000) as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%b %-e, %y %-l:%M %P").to_string())
        .unwrap_or_else(|| ts_ms.to_string())
}

/// Format a list of issues as a colored table.
pub fn format_issue_table(issues: &[IssueRow]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["ID", "Status", "Created", "Title"]);

    for issue in issues {
        let state_cell = match issue.state.as_str() {
            "open" => Cell::new(&issue.state)
                .fg(Color::Blue)
                .add_attribute(Attribute::Bold),
            _ => Cell::new(&issue.state)
                .fg(Color::DarkYellow),
        };

        let title = strip_markdown(&issue.title);
        table.add_row(vec![
            Cell::new(&issue.id[..8.min(issue.id.len())]),
            state_cell,
            Cell::new(format_local_date(issue.created_ts)),
            Cell::new(title),
        ]);
    }

    let open_count = issues.iter().filter(|i| i.state == "open").count();
    let closed_count = issues.len() - open_count;

    let mut table_str = table.to_string();
    if !issues.is_empty() {
        table_str.push_str(&format!(
            "\n{} issues total ({} open, {} closed)",
            issues.len(),
            open_count,
            closed_count,
        ));
    }
    table_str
}
