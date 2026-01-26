use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};
use tree_sitter_language::LanguageFn;

use crate::types::event::SymbolInfo;

/// Attempt tree-sitter-based symbol extraction.
/// Returns None if language is unsupported or parsing fails (triggers regex fallback).
pub fn extract(content: &str, language: &str) -> Option<Vec<SymbolInfo>> {
    let (lang_fn, query_source, kinds) = language_config(language)?;
    let lang: Language = Language::from(lang_fn);

    let mut parser = Parser::new();
    parser.set_language(&lang).ok()?;

    let tree = parser.parse(content, None)?;
    let query = Query::new(&lang, query_source).ok()?;

    let name_idx = query.capture_index_for_name("name")?;
    let def_idx = query.capture_index_for_name("definition");

    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();

    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
    while let Some(m) = matches.next() {
        let mut name: Option<&str> = None;
        let mut def_start: u32 = 0;
        let mut def_end: u32 = 0;
        let mut name_start: u32 = 0;
        let mut name_end: u32 = 0;

        for capture in m.captures {
            if capture.index == name_idx {
                let start = capture.node.start_byte();
                let end = capture.node.end_byte();
                if start <= end && end <= content.len() {
                    name = Some(&content[start..end]);
                }
                name_start = capture.node.start_position().row as u32 + 1;
                name_end = capture.node.end_position().row as u32 + 1;
            }
            if let Some(di) = def_idx {
                if capture.index == di {
                    def_start = capture.node.start_position().row as u32 + 1;
                    def_end = capture.node.end_position().row as u32 + 1;
                }
            }
        }

        // Use definition span if available, otherwise use name node span
        let (line_start, line_end) = if def_idx.is_some() && def_start > 0 {
            (def_start, def_end)
        } else {
            (name_start, name_end)
        };

        if let Some(symbol_name) = name {
            let kind: &str = kinds.get(m.pattern_index).copied().unwrap_or("unknown");
            symbols.push(SymbolInfo {
                name: symbol_name.to_string(),
                kind: kind.to_string(),
                line_start,
                line_end,
            });
        }
    }

    // Sort by line, then by kind specificity (prefer struct/class/interface over generic "type")
    symbols.sort_by(|a, b| {
        a.line_start.cmp(&b.line_start)
            .then_with(|| kind_priority(&a.kind).cmp(&kind_priority(&b.kind)))
    });
    // Deduplicate: same name+line keeps the more specific kind (first after priority sort)
    symbols.dedup_by(|a, b| a.line_start == b.line_start && a.name == b.name);
    Some(symbols)
}

/// Priority for deduplication: lower = more specific, preferred.
fn kind_priority(kind: &str) -> u8 {
    match kind {
        "struct" | "class" | "interface" | "enum" | "trait" | "module" | "namespace" => 0,
        "function" | "method" | "impl" | "const" | "static" => 1,
        "type" => 2,
        _ => 3,
    }
}

/// Returns (LanguageFn, query_source, pattern_kinds) for a given language string.
fn language_config(language: &str) -> Option<(LanguageFn, &'static str, &'static [&'static str])> {
    match language {
        "rust" => Some((tree_sitter_rust::LANGUAGE, RUST_QUERY, RUST_KINDS)),
        "python" => Some((tree_sitter_python::LANGUAGE, PYTHON_QUERY, PYTHON_KINDS)),
        "typescript" => Some((tree_sitter_typescript::LANGUAGE_TYPESCRIPT, TYPESCRIPT_QUERY, TYPESCRIPT_KINDS)),
        "typescriptreact" => Some((tree_sitter_typescript::LANGUAGE_TSX, TYPESCRIPT_QUERY, TYPESCRIPT_KINDS)),
        "javascript" => Some((tree_sitter_javascript::LANGUAGE, JAVASCRIPT_QUERY, JAVASCRIPT_KINDS)),
        "go" => Some((tree_sitter_go::LANGUAGE, GO_QUERY, GO_KINDS)),
        "java" => Some((tree_sitter_java::LANGUAGE, JAVA_QUERY, JAVA_KINDS)),
        "c" => Some((tree_sitter_c::LANGUAGE, C_QUERY, C_KINDS)),
        "cpp" => Some((tree_sitter_cpp::LANGUAGE, CPP_QUERY, CPP_KINDS)),
        "ruby" => Some((tree_sitter_ruby::LANGUAGE, RUBY_QUERY, RUBY_KINDS)),
        "elixir" => Some((tree_sitter_elixir::LANGUAGE, ELIXIR_QUERY, ELIXIR_KINDS)),
        _ => None,
    }
}

// --- Rust ---

const RUST_QUERY: &str = r#"
(function_item name: (identifier) @name) @definition
(struct_item name: (type_identifier) @name) @definition
(enum_item name: (type_identifier) @name) @definition
(trait_item name: (type_identifier) @name) @definition
(impl_item type: (type_identifier) @name) @definition
(const_item name: (identifier) @name) @definition
(type_item name: (type_identifier) @name) @definition
(static_item name: (identifier) @name) @definition
"#;

const RUST_KINDS: &[&str] = &[
    "function", // function_item
    "struct",   // struct_item
    "enum",     // enum_item
    "trait",    // trait_item
    "impl",     // impl_item
    "const",    // const_item
    "type",     // type_item
    "static",   // static_item
];

// --- Python ---

const PYTHON_QUERY: &str = r#"
(function_definition name: (identifier) @name) @definition
(class_definition name: (identifier) @name) @definition
"#;

const PYTHON_KINDS: &[&str] = &[
    "function", // function_definition
    "class",    // class_definition
];

// --- TypeScript (works for both TS and TSX grammars) ---

const TYPESCRIPT_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @definition
(class_declaration name: (type_identifier) @name) @definition
(interface_declaration name: (type_identifier) @name) @definition
(type_alias_declaration name: (type_identifier) @name) @definition
(enum_declaration name: (identifier) @name) @definition
(lexical_declaration
  (variable_declarator
    name: (identifier) @name
    value: (arrow_function)) @definition)
"#;

const TYPESCRIPT_KINDS: &[&str] = &[
    "function",  // function_declaration
    "class",     // class_declaration
    "interface", // interface_declaration
    "type",      // type_alias_declaration
    "enum",      // enum_declaration
    "function",  // arrow function in variable
];

// --- JavaScript ---

const JAVASCRIPT_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @definition
(class_declaration name: (identifier) @name) @definition
(lexical_declaration
  (variable_declarator
    name: (identifier) @name
    value: (arrow_function)) @definition)
"#;

const JAVASCRIPT_KINDS: &[&str] = &[
    "function", // function_declaration
    "class",    // class_declaration
    "function", // arrow function in variable
];

// --- Go ---

const GO_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @definition
(method_declaration name: (field_identifier) @name) @definition
(type_declaration (type_spec name: (type_identifier) @name type: (struct_type))) @definition
(type_declaration (type_spec name: (type_identifier) @name type: (interface_type))) @definition
(type_declaration (type_spec name: (type_identifier) @name)) @definition
"#;

const GO_KINDS: &[&str] = &[
    "function",  // function_declaration
    "function",  // method_declaration
    "struct",    // struct type
    "interface", // interface type
    "type",      // other type alias
];

// --- Java ---

const JAVA_QUERY: &str = r#"
(method_declaration name: (identifier) @name) @definition
(class_declaration name: (identifier) @name) @definition
(interface_declaration name: (identifier) @name) @definition
(enum_declaration name: (identifier) @name) @definition
(constructor_declaration name: (identifier) @name) @definition
"#;

const JAVA_KINDS: &[&str] = &[
    "method",    // method_declaration
    "class",     // class_declaration
    "interface", // interface_declaration
    "enum",      // enum_declaration
    "method",    // constructor_declaration
];

// --- C ---

const C_QUERY: &str = r#"
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition
(struct_specifier
  name: (type_identifier) @name) @definition
(enum_specifier
  name: (type_identifier) @name) @definition
(type_definition
  declarator: (type_identifier) @name) @definition
"#;

const C_KINDS: &[&str] = &[
    "function", // function_definition
    "struct",   // struct_specifier
    "enum",     // enum_specifier
    "type",     // type_definition (typedef)
];

// --- C++ ---

const CPP_QUERY: &str = r#"
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition
(class_specifier
  name: (type_identifier) @name) @definition
(struct_specifier
  name: (type_identifier) @name) @definition
(enum_specifier
  name: (type_identifier) @name) @definition
(namespace_definition
  name: (namespace_identifier) @name) @definition
"#;

const CPP_KINDS: &[&str] = &[
    "function",  // function_definition
    "class",     // class_specifier
    "struct",    // struct_specifier
    "enum",      // enum_specifier
    "namespace", // namespace_definition
];

// --- Ruby ---

const RUBY_QUERY: &str = r#"
(method name: (identifier) @name) @definition
(class name: (constant) @name) @definition
(module name: (constant) @name) @definition
(singleton_method name: (identifier) @name) @definition
"#;

const RUBY_KINDS: &[&str] = &[
    "function", // method
    "class",    // class
    "module",   // module
    "function", // singleton_method
];

// --- Elixir ---

const ELIXIR_QUERY: &str = r#"
(call
  target: (identifier) @_kw
  (arguments
    (call target: (identifier) @name))
  (#match? @_kw "^(def|defp)$")) @definition

(call
  target: (identifier) @_kw
  (arguments
    (identifier) @name)
  (#match? @_kw "^(def|defp)$")) @definition

(call
  target: (identifier) @_kw
  (arguments
    (alias) @name)
  (#match? @_kw "^defmodule$")) @definition
"#;

const ELIXIR_KINDS: &[&str] = &[
    "function", // def/defp with call target (e.g. def foo(args))
    "function", // def/defp with simple identifier
    "module",   // defmodule
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_extraction() {
        let content = r#"pub struct Config {
    pub name: String,
    pub value: u32,
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
        Self { name, value: 0 }
    }

    pub async fn load() -> Self {
        todo!()
    }
}

pub const MAX_SIZE: usize = 100;

pub type Result<T> = std::result::Result<T, Error>;
"#;

        let symbols = extract(content, "rust").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"Config"), "missing Config, got: {:?}", names);
        assert!(names.contains(&"State"), "missing State, got: {:?}", names);
        assert!(names.contains(&"Handler"), "missing Handler, got: {:?}", names);
        assert!(names.contains(&"new"), "missing new, got: {:?}", names);
        assert!(names.contains(&"load"), "missing load, got: {:?}", names);
        assert!(names.contains(&"MAX_SIZE"), "missing MAX_SIZE, got: {:?}", names);

        // Check accurate line ranges
        let config = symbols.iter().find(|s| s.name == "Config" && s.kind == "struct").unwrap();
        assert_eq!(config.line_start, 1);
        assert_eq!(config.line_end, 4);
    }

    #[test]
    fn test_python_extraction() {
        let content = r#"class MyClass:
    def __init__(self):
        self.x = 0

    def method(self):
        return self.x

def standalone():
    pass

async def async_func():
    pass
"#;

        let symbols = extract(content, "python").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"MyClass"), "missing MyClass, got: {:?}", names);
        assert!(names.contains(&"__init__"), "missing __init__, got: {:?}", names);
        assert!(names.contains(&"method"), "missing method, got: {:?}", names);
        assert!(names.contains(&"standalone"), "missing standalone, got: {:?}", names);
        assert!(names.contains(&"async_func"), "missing async_func, got: {:?}", names);

        // Check accurate line ranges
        let class = symbols.iter().find(|s| s.name == "MyClass").unwrap();
        assert_eq!(class.line_start, 1);
        assert_eq!(class.line_end, 6);
    }

    #[test]
    fn test_typescript_extraction() {
        let content = r#"export function greet(name: string): string {
    return `Hello, ${name}!`;
}

export class UserService {
    constructor() {}
    getName(): string { return ""; }
}

export interface Config {
    name: string;
    value: number;
}

type UserId = string;

enum Status {
    Active,
    Inactive
}

const fetchData = async (url: string) => {
    return fetch(url);
};
"#;

        let symbols = extract(content, "typescript").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"greet"), "missing greet, got: {:?}", names);
        assert!(names.contains(&"UserService"), "missing UserService, got: {:?}", names);
        assert!(names.contains(&"Config"), "missing Config, got: {:?}", names);
        assert!(names.contains(&"UserId"), "missing UserId, got: {:?}", names);
        assert!(names.contains(&"Status"), "missing Status, got: {:?}", names);
        assert!(names.contains(&"fetchData"), "missing fetchData, got: {:?}", names);
    }

    #[test]
    fn test_javascript_extraction() {
        let content = r#"function hello(name) {
    console.log(`Hello, ${name}!`);
}

class Animal {
    constructor(name) {
        this.name = name;
    }
}

const greet = (name) => {
    return `Hi ${name}`;
};
"#;

        let symbols = extract(content, "javascript").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"hello"), "missing hello, got: {:?}", names);
        assert!(names.contains(&"Animal"), "missing Animal, got: {:?}", names);
        assert!(names.contains(&"greet"), "missing greet, got: {:?}", names);
    }

    #[test]
    fn test_go_extraction() {
        let content = r#"package main

func main() {
    fmt.Println("hello")
}

func (s *Server) Start() error {
    return nil
}

type Config struct {
    Name string
    Port int
}

type Handler interface {
    Handle() error
}

type UserID string
"#;

        let symbols = extract(content, "go").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"main"), "missing main, got: {:?}", names);
        assert!(names.contains(&"Start"), "missing Start, got: {:?}", names);
        assert!(names.contains(&"Config"), "missing Config, got: {:?}", names);
        assert!(names.contains(&"Handler"), "missing Handler, got: {:?}", names);
        assert!(names.contains(&"UserID"), "missing UserID, got: {:?}", names);

        // Check kinds
        let config = symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(config.kind, "struct");
        let handler = symbols.iter().find(|s| s.name == "Handler").unwrap();
        assert_eq!(handler.kind, "interface");
    }

    #[test]
    fn test_java_extraction() {
        let content = r#"public class UserService {
    private String name;

    public UserService(String name) {
        this.name = name;
    }

    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }
}

public interface Repository {
    void save(Object entity);
    Object find(String id);
}

public enum Status {
    ACTIVE,
    INACTIVE
}
"#;

        let symbols = extract(content, "java").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"UserService"), "missing UserService, got: {:?}", names);
        assert!(names.contains(&"getName"), "missing getName, got: {:?}", names);
        assert!(names.contains(&"setName"), "missing setName, got: {:?}", names);
        assert!(names.contains(&"Repository"), "missing Repository, got: {:?}", names);
        assert!(names.contains(&"Status"), "missing Status, got: {:?}", names);
    }

    #[test]
    fn test_c_extraction() {
        let content = r#"struct Point {
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

void helper(int n) {
    printf("%d\n", n);
}
"#;

        let symbols = extract(content, "c").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"Point"), "missing Point, got: {:?}", names);
        assert!(names.contains(&"Color"), "missing Color, got: {:?}", names);
        assert!(names.contains(&"main"), "missing main, got: {:?}", names);
        assert!(names.contains(&"helper"), "missing helper, got: {:?}", names);
    }

    #[test]
    fn test_cpp_extraction() {
        let content = r#"namespace mylib {

class Widget {
public:
    Widget();
    void draw();
};

struct Point {
    double x, y;
};

enum Color {
    Red, Green, Blue
};

}

void process(int n) {
    return;
}
"#;

        let symbols = extract(content, "cpp").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"mylib"), "missing mylib, got: {:?}", names);
        assert!(names.contains(&"Widget"), "missing Widget, got: {:?}", names);
        assert!(names.contains(&"Point"), "missing Point, got: {:?}", names);
        assert!(names.contains(&"Color"), "missing Color, got: {:?}", names);
        assert!(names.contains(&"process"), "missing process, got: {:?}", names);
    }

    #[test]
    fn test_ruby_extraction() {
        let content = r#"module Authentication
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

        let symbols = extract(content, "ruby").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"Authentication"), "missing Authentication, got: {:?}", names);
        assert!(names.contains(&"User"), "missing User, got: {:?}", names);
        assert!(names.contains(&"initialize"), "missing initialize, got: {:?}", names);
        assert!(names.contains(&"greet"), "missing greet, got: {:?}", names);
    }

    #[test]
    fn test_elixir_extraction() {
        let content = r#"defmodule MyApp.Users do
  def get_user(id) do
    Repo.get(User, id)
  end

  defp validate(user) do
    # private function
    :ok
  end
end
"#;

        let symbols = extract(content, "elixir").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"MyApp.Users"), "missing MyApp.Users, got: {:?}", names);
        assert!(names.contains(&"get_user"), "missing get_user, got: {:?}", names);
        assert!(names.contains(&"validate"), "missing validate, got: {:?}", names);
    }

    #[test]
    fn test_tsx_extraction() {
        let content = r#"interface Props {
    name: string;
}

export function Component(props: Props): JSX.Element {
    return <div>{props.name}</div>;
}

type Theme = "light" | "dark";
"#;

        let symbols = extract(content, "typescriptreact").unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"Props"), "missing Props, got: {:?}", names);
        assert!(names.contains(&"Component"), "missing Component, got: {:?}", names);
        assert!(names.contains(&"Theme"), "missing Theme, got: {:?}", names);
    }

    #[test]
    fn test_unknown_language_returns_none() {
        assert!(extract("anything", "brainfuck").is_none());
    }
}
