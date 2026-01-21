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

## Quick Reference

| Task | Command |
|------|---------|
| Create issue | `grit issue create --title "..." --body "..."` |
| List issues | `grit issue list` |
| Sync all | `grit sync` |
| Create actor | `grit actor init --label "name"` |
| Acquire lock | `grit lock acquire --resource "..." --ttl 15m` |
| Start daemon | `grit daemon start` |
| Export JSON | `grit export --format json` |
