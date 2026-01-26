# CLI

## Principles

- Non-interactive by default
- Structured output always available (`--json`)
- No daemon required for correctness

## Command overview

- `grite init [--no-agents-md]`
- `grite actor init [--label <name>]`
- `grite actor list [--json]`
- `grite actor show [<id>] [--json]`
- `grite actor current [--json]`
- `grite actor use <id>`
- `grite issue create --title ... --body ... --label ...`
- `grite issue update <id> [--title ...] [--body ...]`
- `grite issue list --state open --label bug --json`
- `grite issue show <id> --json`
- `grite issue comment <id> --body ...`
- `grite issue close <id>`
- `grite issue reopen <id>`
- `grite issue label add <id> --label <label>`
- `grite issue label remove <id> --label <label>`
- `grite issue assignee add <id> --user <name>`
- `grite issue assignee remove <id> --user <name>`
- `grite issue link add <id> --url ... [--note ...]`
- `grite issue attachment add <id> --name ... --sha256 ... --mime ...`
- `grite issue dep add <id> --target <id> --type blocks|depends_on|related_to`
- `grite issue dep remove <id> --target <id> --type ...`
- `grite issue dep list <id> [--reverse]`
- `grite issue dep topo [--state open] [--label ...]`
- `grite context index [--path ...] [--pattern "*.rs"] [--force]`
- `grite context query <query>`
- `grite context show <path>`
- `grite context project [key]`
- `grite context set <key> <value>`
- `grite sync [--pull] [--push] [--remote <name>]`
- `grite doctor [--fix] [--json]`
- `grite rebuild [--from-snapshot]`
- `grite db stats [--json]`
- `grite export --format md|json`
- `grite snapshot`
- `grite snapshot gc`
- `grite lock acquire --resource <R> --ttl 15m`
- `grite lock renew --resource <R> --ttl 15m`
- `grite lock release --resource <R>`
- `grite lock status [--json]`
- `grite lock gc`
- `grite daemon status [--json]`
- `grite daemon stop`

## JSON output

- `--json` is supported on all commands
- `--quiet` suppresses human output for agents
- Errors are returned with structured details
- JSON schemas and error codes are defined in `docs/cli-json.md`

## Data directory

- `GRIT_HOME` or `--data-dir` sets the local state root for this process
- Default is `.git/grite/actors/<actor_id>/`
- Each concurrent agent should use a distinct data dir
- If a daemon owns the selected data dir, the CLI routes all commands through it and does not open the DB directly

## Git worktrees

Grite fully supports git worktrees. When running grite commands from within a worktree:

- **Shared data**: Issues, events, locks, and context are stored in the main repository's `.git/grite/` directory and shared across all worktrees
- **Shared refs**: The WAL (`refs/grite/wal`), locks (`refs/grite/locks/*`), and snapshots are stored in the main repository
- **Context indexing**: `grite context index` indexes files in the current worktree's working directory using `git ls-files`
- **Actor isolation**: Each worktree can use the same or different actors; actors are shared across worktrees

This enables multi-agent workflows where each agent works on a different branch in its own worktree while sharing the same issue state.

```bash
# Main repo
cd /project
grite init
grite issue create --title "Feature A"

# Create worktree for feature branch
git worktree add ../project-feature -b feature

# Work from worktree - issues are shared
cd ../project-feature
grite issue list          # Shows "Feature A"
grite issue comment <id> --body "Working on this in feature branch"
```

## AGENTS.md

By default, `grite init` creates or updates an `AGENTS.md` file in the repository root with instructions for AI coding agents to use grite as the canonical task and memory system.

- If `AGENTS.md` does not exist, it is created with grite instructions
- If `AGENTS.md` exists but has no `## Grite` section, the section is appended
- If `AGENTS.md` already contains a `## Grite` section, no changes are made
- Use `--no-agents-md` to skip AGENTS.md creation/modification

## Actor identity

- `grite init` creates a default `actor_id`, writes `.git/grite/actors/<actor_id>/config.toml`,
  and sets `default_actor` in `.git/grite/config.toml`
- `grite actor init` creates an additional actor directory and prints the new ID
- If no actor config exists, commands may auto-initialize with a new `actor_id`

## Actor selection

Actor context for a command is resolved in this order:

1. `--data-dir` or `GRIT_HOME`
2. `--actor <id>` (resolves to `.git/grite/actors/<id>/`)
3. Repo default in `.git/grite/config.toml` (set by `grite actor use`)
4. Auto-init a new actor if none exists

## Export

- `grite export --format json` emits a machine-readable export suitable for dashboards
- `grite export --format md` emits a human-readable export
- `grite export --since <ts|event_id>` emits only changes after a point-in-time
- Export output is generated into `.grite/` by default and is never canonical

## Sync

The sync command handles pushing and pulling grite refs with remote repositories.

```bash
# Full sync (pull then push with auto-rebase)
grite sync

# Pull only
grite sync --pull

# Push only (auto-rebases on conflict)
grite sync --push

# Specify remote
grite sync --remote upstream
```

**Auto-rebase:** When a push fails due to non-fast-forward (remote has newer commits), grite automatically:
1. Pulls remote changes
2. Identifies local-only events
3. Re-appends local events on top of remote
4. Pushes again

The sync output reports when conflicts were resolved and how many events were rebased.

## Doctor

Health checks and auto-repair for the grite database.

```bash
# Run health checks
grite doctor

# Auto-repair issues (e.g., rebuild on corruption)
grite doctor --fix
```

**Checks performed:**
- `git_repo`: Git repository validity
- `wal_ref`: WAL ref exists and is readable
- `actor_config`: Actor is properly configured
- `store_integrity`: Database integrity (event hashes)
- `rebuild_threshold`: Warns if too many events since last rebuild

## Rebuild

Rebuild projections from events.

```bash
# Standard rebuild from store events
grite rebuild

# Fast rebuild from latest snapshot (for large repos)
grite rebuild --from-snapshot
```

The `--from-snapshot` flag loads events from the latest snapshot instead of replaying the entire WAL, which is faster for repositories with many events.

## Dependencies

Typed relationships between issues with cycle detection and topological ordering.

```bash
# Add a dependency (issue blocks another)
grite issue dep add <id> --target <target_id> --type blocks

# Types: blocks, depends_on, related_to
grite issue dep add <id> --target <target_id> --type depends_on

# Remove a dependency
grite issue dep remove <id> --target <target_id> --type blocks

# List dependencies for an issue
grite issue dep list <id>

# List issues that depend on this one (reverse)
grite issue dep list <id> --reverse

# Topological ordering (respects dependency DAG)
grite issue dep topo --state open --label sprint-1
```

**Dependency types:**
- `blocks` — "this issue blocks target" (acyclic, enforced)
- `depends_on` — "this issue depends on target" (acyclic, enforced)
- `related_to` — symmetric link, no cycle constraint

**Cycle detection:** Adding a `blocks` or `depends_on` edge that would create a cycle is rejected at command time. The `related_to` type has no acyclicity constraint.

**CRDT notes:** Dependencies are an add/remove set (commutative). Concurrent add+remove of the same edge: add wins. Cycle detection is local validation; concurrent conflicting edges are accepted by the CRDT but flagged by `grite doctor`.

## Context Store

Distributed file/symbol index for AI agents to query project structure.

```bash
# Index files (uses git ls-files, skips unchanged)
grite context index
grite context index --path src/ --pattern "*.rs"
grite context index --force  # re-index even if hash unchanged

# Query symbols
grite context query "Config"

# Show context for a file
grite context show src/main.rs

# Project-level key/value store
grite context project              # list all entries
grite context project "api_version"  # get specific key
grite context set "api_version" "v2" # set key/value
```

**Supported languages:** Rust, Python, TypeScript/TSX, JavaScript, Go, Java, C, C++, Ruby, Elixir (tree-sitter-powered, AST-accurate line ranges)

**Incremental indexing:** Files are SHA-256 hashed; unchanged files are skipped unless `--force` is used.

**CRDT notes:** File context uses last-writer-wins (LWW) per file path. Project context uses LWW per key. Both sync automatically via `grite sync`.

## Error Messages

Errors include actionable suggestions to help resolve issues:

```
error: Issue 'abc123' not found

Suggestions:
  - Run 'grite issue list' to see available issues
```

Common suggestions include:
- **NotFound (issue)**: Run `grite issue list` to see available issues
- **DbBusy**: Try `grite --no-daemon <command>` or stop the daemon
- **Sled errors**: Run `grite doctor --fix` to rebuild
- **IPC errors**: Run `grite daemon stop` and retry
