use libgrit_core::GritError;
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
pub fn output_error(cli: &Cli, err: &GritError) {
    if cli.json {
        let response: JsonResponse<()> = JsonResponse {
            schema_version: 1,
            ok: false,
            data: None,
            error: Some(JsonError {
                code: err.error_code().to_string(),
                message: err.to_string(),
                details: serde_json::Value::Null,
            }),
        };
        eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
    } else {
        eprintln!("error: {}", err);
    }
}

/// Print human-readable output (ignored in quiet mode)
pub fn print_human(cli: &Cli, msg: &str) {
    if !cli.json && !cli.quiet {
        println!("{}", msg);
    }
}
