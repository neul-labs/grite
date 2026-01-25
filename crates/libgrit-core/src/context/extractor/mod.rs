mod ts_engine;
mod regex_fallback;

use std::path::Path;

use crate::types::event::SymbolInfo;

/// Detect programming language from file extension
pub fn detect_language(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("js") => "javascript",
        Some("jsx") => "javascript",
        Some("go") => "go",
        Some("java") => "java",
        Some("c") | Some("h") => "c",
        Some("cpp") | Some("hpp") | Some("cc") | Some("cxx") => "cpp",
        Some("rb") => "ruby",
        Some("ex") | Some("exs") => "elixir",
        _ => "unknown",
    }
}

/// Extract symbols from source code using tree-sitter (with regex fallback)
pub fn extract_symbols(content: &str, language: &str) -> Vec<SymbolInfo> {
    match ts_engine::extract(content, language) {
        Some(symbols) => symbols,
        None => regex_fallback::extract(content, language),
    }
}

/// Generate a short summary of a file based on its symbols
pub fn generate_summary(path: &str, symbols: &[SymbolInfo], language: &str) -> String {
    let display_language = match language {
        "typescriptreact" => "typescript",
        other => other,
    };

    if symbols.is_empty() {
        return format!("{} file", display_language);
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
        format!("{} ({})", Path::new(path).file_name().unwrap_or_default().to_string_lossy(), display_language)
    } else {
        parts.join("; ")
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
        assert_eq!(detect_language("component.tsx"), "typescriptreact");
        assert_eq!(detect_language("main.go"), "go");
        assert_eq!(detect_language("Main.java"), "java");
        assert_eq!(detect_language("main.c"), "c");
        assert_eq!(detect_language("main.cpp"), "cpp");
        assert_eq!(detect_language("app.rb"), "ruby");
        assert_eq!(detect_language("lib.ex"), "elixir");
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

        let symbols = extract_symbols(content, "rust");
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

        let symbols = extract_symbols(content, "python");
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

        let symbols = extract_symbols(content, "go");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"main"));
        assert!(names.contains(&"Start"));
        assert!(names.contains(&"Config"));
        assert!(names.contains(&"Handler"));
    }

    #[test]
    fn test_extract_typescript_symbols() {
        let content = r#"
export function greet(name: string): string {
    return `Hello, ${name}!`;
}

export class UserService {
    constructor() {}
}

export interface Config {
    name: string;
}

type UserId = string;

const fetchData = async (url: string) => {
    return fetch(url);
};
"#;

        let symbols = extract_symbols(content, "typescript");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"greet"));
        assert!(names.contains(&"UserService"));
        assert!(names.contains(&"Config"));
        assert!(names.contains(&"UserId"));
        assert!(names.contains(&"fetchData"));
    }

    #[test]
    fn test_extract_java_symbols() {
        let content = r#"
public class UserService {
    private String name;

    public UserService(String name) {
        this.name = name;
    }

    public String getName() {
        return name;
    }
}

public interface Repository {
    void save(Object entity);
}

public enum Status {
    ACTIVE,
    INACTIVE
}
"#;

        let symbols = extract_symbols(content, "java");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"UserService"));
        assert!(names.contains(&"getName"));
        assert!(names.contains(&"Repository"));
        assert!(names.contains(&"Status"));
    }

    #[test]
    fn test_extract_c_symbols() {
        let content = r#"
struct Point {
    int x;
    int y;
};

enum Color {
    RED,
    GREEN,
    BLUE
};

typedef unsigned long ulong;

int main(int argc, char** argv) {
    return 0;
}
"#;

        let symbols = extract_symbols(content, "c");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"Point"));
        assert!(names.contains(&"Color"));
        assert!(names.contains(&"main"));
    }

    #[test]
    fn test_extract_ruby_symbols() {
        let content = r#"
module Authentication
  class User
    def initialize(name)
      @name = name
    end

    def self.find(id)
      new("user_#{id}")
    end

    def greet
      "Hello, #{@name}"
    end
  end
end
"#;

        let symbols = extract_symbols(content, "ruby");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"Authentication"));
        assert!(names.contains(&"User"));
        assert!(names.contains(&"initialize"));
        assert!(names.contains(&"greet"));
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

    #[test]
    fn test_generate_summary_tsx() {
        let symbols = vec![
            SymbolInfo { name: "App".to_string(), kind: "function".to_string(), line_start: 1, line_end: 10 },
        ];

        let summary = generate_summary("src/App.tsx", &symbols, "typescriptreact");
        assert!(summary.contains("1 functions"));
    }

    #[test]
    fn test_fallback_for_unknown_language() {
        let symbols = extract_symbols("fn main() {}", "brainfuck");
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_rust_accurate_line_ranges() {
        let content = r#"pub struct Config {
    pub name: String,
    pub value: u32,
}

pub fn process(config: &Config) -> String {
    format!("{}: {}", config.name, config.value)
}
"#;

        let symbols = extract_symbols(content, "rust");

        let config = symbols.iter().find(|s| s.name == "Config" && s.kind == "struct").unwrap();
        assert_eq!(config.line_start, 1);
        assert_eq!(config.line_end, 4);

        let process = symbols.iter().find(|s| s.name == "process").unwrap();
        assert_eq!(process.line_start, 6);
        assert_eq!(process.line_end, 8);
    }

    #[test]
    fn test_python_accurate_line_ranges() {
        let content = r#"class MyClass:
    def __init__(self):
        self.x = 0

    def method(self):
        return self.x

def standalone():
    pass
"#;

        let symbols = extract_symbols(content, "python");

        let class = symbols.iter().find(|s| s.name == "MyClass").unwrap();
        assert_eq!(class.line_start, 1);
        assert_eq!(class.line_end, 6);

        let standalone = symbols.iter().find(|s| s.name == "standalone").unwrap();
        assert_eq!(standalone.line_start, 8);
        assert_eq!(standalone.line_end, 9);
    }
}
