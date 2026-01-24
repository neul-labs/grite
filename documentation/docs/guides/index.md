# Guides

This section contains how-to guides for common grit tasks. Each guide focuses on a specific feature and includes practical examples.

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
- Supports Rust, Python, TypeScript/JavaScript, Go

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

How Grit compares with other tools in this space:

- Beads: Architecture differences, sync reliability, CRDT vs hash IDs
- git-bug: Git objects, bridges, not agent-optimized
- Trekker: SQLite-only, MCP-native, no distributed sync
- When to choose each tool

## Quick Reference

| Task | Command |
|------|---------|
| Create issue | `grit issue create --title "..." --body "..."` |
| List issues | `grit issue list` |
| Add dependency | `grit issue dep add <id> --target <id> --type blocks` |
| Topo order | `grit issue dep topo --state open` |
| Index files | `grit context index` |
| Query symbols | `grit context query "SymbolName"` |
| Sync all | `grit sync` |
| Create actor | `grit actor init --label "name"` |
| Acquire lock | `grit lock acquire --resource "..." --ttl 15m` |
| Start daemon | `grit daemon start` |
| Export JSON | `grit export --format json` |
