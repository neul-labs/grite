use regex::Regex;

use crate::types::event::SymbolInfo;

/// Regex-based symbol extraction (fallback for unsupported languages or parse failures)
pub fn extract(content: &str, language: &str) -> Vec<SymbolInfo> {
    match language {
        "rust" => extract_rust_symbols(content),
        "python" => extract_python_symbols(content),
        "typescript" | "typescriptreact" | "javascript" => extract_ts_symbols(content),
        "go" => extract_go_symbols(content),
        _ => vec![],
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
