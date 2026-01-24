use std::path::Path;
use regex::Regex;

use crate::types::event::SymbolInfo;

/// Detect programming language from file extension
pub fn detect_language(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("go") => "go",
        Some("java") => "java",
        Some("c") | Some("h") => "c",
        Some("cpp") | Some("hpp") | Some("cc") | Some("cxx") => "cpp",
        Some("rb") => "ruby",
        Some("ex") | Some("exs") => "elixir",
        _ => "unknown",
    }
}

/// Extract symbols from source code using language-aware regex patterns
pub fn extract_symbols(content: &str, language: &str) -> Vec<SymbolInfo> {
    match language {
        "rust" => extract_rust_symbols(content),
        "python" => extract_python_symbols(content),
        "typescript" | "javascript" => extract_ts_symbols(content),
        "go" => extract_go_symbols(content),
        _ => vec![],
    }
}

/// Generate a short summary of a file based on its symbols
pub fn generate_summary(path: &str, symbols: &[SymbolInfo], language: &str) -> String {
    if symbols.is_empty() {
        return format!("{} file", language);
    }

    let structs: Vec<&str> = symbols.iter()
        .filter(|s| s.kind == "struct" || s.kind == "class" || s.kind == "interface")
        .map(|s| s.name.as_str())
        .collect();

    let functions: Vec<&str> = symbols.iter()
        .filter(|s| s.kind == "function" || s.kind == "method")
        .map(|s| s.name.as_str())
        .collect();

    let mut parts = Vec::new();
    if !structs.is_empty() {
        let names: String = structs.iter().take(3).copied().collect::<Vec<_>>().join(", ");
        if structs.len() > 3 {
            parts.push(format!("defines {} (+{} more)", names, structs.len() - 3));
        } else {
            parts.push(format!("defines {}", names));
        }
    }
    if !functions.is_empty() {
        parts.push(format!("{} functions", functions.len()));
    }

    if parts.is_empty() {
        format!("{} ({})", Path::new(path).file_name().unwrap_or_default().to_string_lossy(), language)
    } else {
        parts.join("; ")
    }
}

fn extract_rust_symbols(content: &str) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();

    let fn_re = Regex::new(r"(?m)^\s*(?:pub(?:\(.*?\))?\s+)?(?:async\s+)?fn\s+(\w+)").unwrap();
    let struct_re = Regex::new(r"(?m)^\s*(?:pub(?:\(.*?\))?\s+)?struct\s+(\w+)").unwrap();
    let enum_re = Regex::new(r"(?m)^\s*(?:pub(?:\(.*?\))?\s+)?enum\s+(\w+)").unwrap();
    let trait_re = Regex::new(r"(?m)^\s*(?:pub(?:\(.*?\))?\s+)?trait\s+(\w+)").unwrap();
    let impl_re = Regex::new(r"(?m)^\s*impl(?:<[^>]*>)?\s+(\w+)").unwrap();
    let const_re = Regex::new(r"(?m)^\s*(?:pub(?:\(.*?\))?\s+)?(?:const|static)\s+(\w+)").unwrap();

    add_matches(&mut symbols, content, &fn_re, "function");
    add_matches(&mut symbols, content, &struct_re, "struct");
    add_matches(&mut symbols, content, &enum_re, "enum");
    add_matches(&mut symbols, content, &trait_re, "trait");
    add_matches(&mut symbols, content, &impl_re, "impl");
    add_matches(&mut symbols, content, &const_re, "const");

    symbols.sort_by_key(|s| s.line_start);
    symbols
}

fn extract_python_symbols(content: &str) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();

    let fn_re = Regex::new(r"(?m)^(?:\s*)(?:async\s+)?def\s+(\w+)").unwrap();
    let class_re = Regex::new(r"(?m)^class\s+(\w+)").unwrap();

    add_matches(&mut symbols, content, &fn_re, "function");
    add_matches(&mut symbols, content, &class_re, "class");

    symbols.sort_by_key(|s| s.line_start);
    symbols
}

fn extract_ts_symbols(content: &str) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();

    let fn_re = Regex::new(r"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+(\w+)").unwrap();
    let class_re = Regex::new(r"(?m)^\s*(?:export\s+)?class\s+(\w+)").unwrap();
    let interface_re = Regex::new(r"(?m)^\s*(?:export\s+)?interface\s+(\w+)").unwrap();
    let type_re = Regex::new(r"(?m)^\s*(?:export\s+)?type\s+(\w+)").unwrap();
    let const_re = Regex::new(r"(?m)^\s*(?:export\s+)?const\s+(\w+)").unwrap();
    let arrow_re = Regex::new(r"(?m)^\s*(?:export\s+)?(?:const|let)\s+(\w+)\s*=\s*(?:async\s+)?\(").unwrap();

    add_matches(&mut symbols, content, &fn_re, "function");
    add_matches(&mut symbols, content, &class_re, "class");
    add_matches(&mut symbols, content, &interface_re, "interface");
    add_matches(&mut symbols, content, &type_re, "type");
    add_matches(&mut symbols, content, &const_re, "const");
    add_matches(&mut symbols, content, &arrow_re, "function");

    // Deduplicate by name+line (arrow functions may match const pattern too)
    symbols.sort_by_key(|s| (s.line_start, s.name.clone()));
    symbols.dedup_by(|a, b| a.line_start == b.line_start && a.name == b.name);
    symbols
}

fn extract_go_symbols(content: &str) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();

    let fn_re = Regex::new(r"(?m)^func\s+(?:\(\w+\s+\*?\w+\)\s+)?(\w+)").unwrap();
    let struct_re = Regex::new(r"(?m)^type\s+(\w+)\s+struct").unwrap();
    let interface_re = Regex::new(r"(?m)^type\s+(\w+)\s+interface").unwrap();
    let type_re = Regex::new(r"(?m)^type\s+(\w+)\s+\w").unwrap();

    add_matches(&mut symbols, content, &fn_re, "function");
    add_matches(&mut symbols, content, &struct_re, "struct");
    add_matches(&mut symbols, content, &interface_re, "interface");
    add_matches(&mut symbols, content, &type_re, "type");

    // Deduplicate by line - struct/interface matches take priority over generic "type"
    symbols.sort_by_key(|s| (s.line_start, s.name.clone()));
    symbols.dedup_by(|a, b| a.line_start == b.line_start && a.name == b.name);
    symbols
}

fn add_matches(symbols: &mut Vec<SymbolInfo>, content: &str, re: &Regex, kind: &str) {
    for cap in re.captures_iter(content) {
        if let Some(name_match) = cap.get(1) {
            let line_start = content[..name_match.start()].matches('\n').count() as u32 + 1;
            // Estimate end line (next blank line or +10 lines, whichever is smaller)
            let remaining = &content[name_match.end()..];
            let lines_to_end = remaining.find("\n\n")
                .map(|pos| remaining[..pos].matches('\n').count() as u32)
                .unwrap_or(10)
                .min(50);
            let line_end = line_start + lines_to_end;

            symbols.push(SymbolInfo {
                name: name_match.as_str().to_string(),
                kind: kind.to_string(),
                line_start,
                line_end,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("src/main.rs"), "rust");
        assert_eq!(detect_language("app.py"), "python");
        assert_eq!(detect_language("index.ts"), "typescript");
        assert_eq!(detect_language("main.go"), "go");
        assert_eq!(detect_language("README.md"), "unknown");
    }

    #[test]
    fn test_extract_rust_symbols() {
        let content = r#"
pub struct Config {
    pub name: String,
}

pub enum State {
    Open,
    Closed,
}

pub trait Handler {
    fn handle(&self);
}

impl Config {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub async fn load() -> Self {
        todo!()
    }
}

pub const MAX_SIZE: usize = 100;
"#;

        let symbols = extract_rust_symbols(content);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"Config"));
        assert!(names.contains(&"State"));
        assert!(names.contains(&"Handler"));
        assert!(names.contains(&"new"));
        assert!(names.contains(&"load"));
        assert!(names.contains(&"MAX_SIZE"));
    }

    #[test]
    fn test_extract_python_symbols() {
        let content = r#"
class MyClass:
    pass

def my_function():
    pass

async def async_func():
    pass
"#;

        let symbols = extract_python_symbols(content);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"MyClass"));
        assert!(names.contains(&"my_function"));
        assert!(names.contains(&"async_func"));
    }

    #[test]
    fn test_extract_go_symbols() {
        let content = r#"
func main() {
}

func (s *Server) Start() error {
    return nil
}

type Config struct {
    Name string
}

type Handler interface {
    Handle()
}
"#;

        let symbols = extract_go_symbols(content);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"main"));
        assert!(names.contains(&"Start"));
        assert!(names.contains(&"Config"));
        assert!(names.contains(&"Handler"));
    }

    #[test]
    fn test_generate_summary() {
        let symbols = vec![
            SymbolInfo { name: "Config".to_string(), kind: "struct".to_string(), line_start: 1, line_end: 10 },
            SymbolInfo { name: "new".to_string(), kind: "function".to_string(), line_start: 12, line_end: 20 },
            SymbolInfo { name: "load".to_string(), kind: "function".to_string(), line_start: 22, line_end: 30 },
        ];

        let summary = generate_summary("src/config.rs", &symbols, "rust");
        assert!(summary.contains("Config"));
        assert!(summary.contains("2 functions"));
    }
}
