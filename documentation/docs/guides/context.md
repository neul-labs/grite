# Context Store

The context store is a distributed file/symbol index that AI agents can query to understand project structure. It syncs automatically via `grit sync`, giving all actors a shared understanding of the codebase.

## Overview

The context store provides:

- **File indexing**: Extract symbols (functions, classes, structs) from source files
- **Symbol search**: Query for symbols across the project
- **Project metadata**: Key/value store for project-level information
- **Incremental updates**: Only re-indexes changed files (SHA-256 based)
- **Distributed sync**: Context syncs between actors via the WAL

## Indexing Files

### Basic Usage

```bash
# Index all git-tracked files
grit context index

# Index specific paths
grit context index --path src/ --path lib/

# Filter by file pattern
grit context index --path src/ --pattern "*.rs"

# Force re-index even if unchanged
grit context index --force
```

### How Indexing Works

1. Lists files using `git ls-files` (respects .gitignore)
2. Computes SHA-256 of each file
3. Skips files where the hash matches the stored context
4. For changed files: detects language, extracts symbols, generates summary
5. Emits a `ContextUpdated` event for each file

### Supported Languages

| Language | Extensions | Extracted Symbols |
|----------|-----------|-------------------|
| Rust | `.rs` | `fn`, `struct`, `enum`, `trait`, `impl fn`, `const` |
| Python | `.py` | `def`, `class`, `async def` |
| TypeScript/JavaScript | `.ts`, `.tsx`, `.js`, `.jsx` | `function`, `class`, `interface`, `type`, `const`, arrow functions |
| Go | `.go` | `func`, `type struct`, `type interface` |

### Example Output

```json
{
  "indexed": 42,
  "skipped": 15,
  "total_files": 57
}
```

## Querying Symbols

Search for symbols across the indexed codebase:

```bash
grit context query "Config"
```

### Example Output

```json
{
  "query": "Config",
  "matches": [
    { "symbol": "Config", "path": "src/config.rs" },
    { "symbol": "ConfigBuilder", "path": "src/config.rs" },
    { "symbol": "DatabaseConfig", "path": "src/db.rs" }
  ],
  "count": 3
}
```

## Showing File Context

View the extracted context for a specific file:

```bash
grit context show src/main.rs
```

### Example Output

```json
{
  "path": "src/main.rs",
  "language": "rust",
  "summary": "rust file with 3 functions: main, run, setup",
  "content_hash": "a1b2c3d4...",
  "symbols": [
    { "name": "main", "kind": "function", "line_start": 5, "line_end": 15 },
    { "name": "run", "kind": "function", "line_start": 17, "line_end": 30 },
    { "name": "setup", "kind": "function", "line_start": 32, "line_end": 45 }
  ],
  "symbol_count": 3
}
```

## Project Context

A key/value store for project-level metadata that agents can use to share information.

### Setting Values

```bash
grit context set "api_version" "v2"
grit context set "default_branch" "main"
grit context set "test_command" "cargo test"
```

### Reading Values

```bash
# Get a specific key
grit context project "api_version"

# List all entries
grit context project
```

### Example Output

```json
{
  "entries": [
    { "key": "api_version", "value": "v2" },
    { "key": "default_branch", "value": "main" },
    { "key": "test_command", "value": "cargo test" }
  ],
  "count": 3
}
```

## Distributed Behavior

Context events flow through the same WAL as issue events and sync automatically:

- **File context**: Uses LWW (last-writer-wins) per file path
- **Project context**: Uses LWW per key
- No manual conflict resolution needed
- After sync, all actors share the same context view

### Multi-Agent Scenario

```bash
# Agent A indexes backend code
grit context index --path src/api/

# Agent B indexes frontend code
grit context index --path src/ui/

# After sync, both agents can query the full project
grit context query "handleRequest"
```

## Use Cases

### AI Agent Orientation

An AI agent joining a project can quickly understand the codebase:

```bash
# Index the project
grit context index

# Find relevant code for a task
grit context query "authentication"
grit context show src/auth/mod.rs
```

### Shared Project Knowledge

Teams can store project conventions:

```bash
grit context set "orm" "diesel"
grit context set "api_style" "REST"
grit context set "deploy_target" "kubernetes"
```

### Change Detection

Re-index after changes to see what was modified:

```bash
# Only changed files are re-indexed
grit context index
# Output shows: indexed: 3, skipped: 54, total_files: 57
```
