# Guides

This section contains how-to guides for common grite tasks. Each guide focuses on a specific feature and includes practical examples.

## Available Guides

### [Working with Issues](issues.md)

Complete guide to creating, updating, and managing issues. Covers:

- Creating issues with titles, bodies, and labels
- Updating issue content
- Adding comments
- Managing labels and assignees
- Closing and reopening issues
- Filtering and searching

### [Dependencies](dependencies.md)

Manage typed relationships between issues:

- Dependency types: blocks, depends_on, related_to
- Cycle detection for acyclic types
- Topological ordering for execution planning
- Distributed sync with CRDT semantics

### [Context Store](context.md)

Distributed file/symbol index for AI agents:

- Incremental file indexing with symbol extraction
- Symbol search across the project
- Project-level key/value metadata
- Tree-sitter-powered symbol extraction (Rust, Python, TypeScript/TSX, JavaScript, Go, Java, C, C++, Ruby, Elixir)

### [Git Worktrees](worktrees.md)

Use grite with multiple working directories:

- Shared state across all worktrees
- Multi-agent workflows with parallel development
- Daemon compatibility
- Context indexing per worktree

### [Syncing with Remotes](syncing.md)

Learn how to synchronize your issues with remote repositories:

- Full sync (pull and push)
- Pull-only and push-only operations
- Handling conflicts with auto-rebase
- Working with multiple remotes

### [Actor Identity](actors.md)

Understand and manage actor identities:

- What actors are and why they matter
- Creating new actors
- Switching between actors
- Multi-agent scenarios

### [Distributed Locks](locking.md)

Coordinate work across agents and team members:

- Lock namespaces and resources
- Acquiring and releasing locks
- Lock policies
- Best practices for coordination

### [Using the Daemon](daemon.md)

Get the most out of the optional daemon:

- Auto-spawn behavior
- Manual daemon control
- Idle timeout configuration
- Performance benefits

### [Exporting Data](export.md)

Export issues for external use:

- JSON export for dashboards
- Markdown export for documentation
- Incremental exports

### [Comparison with Alternatives](comparison.md)

How Grite compares with other tools in this space:

- Beads: Architecture differences, sync reliability, CRDT vs hash IDs
- git-bug: Git objects, bridges, not agent-optimized
- Trekker: SQLite-only, MCP-native, no distributed sync
- When to choose each tool

## Quick Reference

| Task | Command |
|------|---------|
| Create issue | `grite issue create --title "..." --body "..."` |
| List issues | `grite issue list` |
| Add dependency | `grite issue dep add <id> --target <id> --type blocks` |
| Topo order | `grite issue dep topo --state open` |
| Index files | `grite context index` |
| Query symbols | `grite context query "SymbolName"` |
| Sync all | `grite sync` |
| Create actor | `grite actor init --label "name"` |
| Acquire lock | `grite lock acquire --resource "..." --ttl 15m` |
| Start daemon | `grite daemon start` |
| Export JSON | `grite export --format json` |
