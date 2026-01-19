# CLI

## Principles

- Non-interactive by default
- Structured output always available (`--json`)
- No daemon required for correctness

## Command overview

- `grit init [--no-agents-md]`
- `grit actor init [--label <name>]`
- `grit actor list [--json]`
- `grit actor show [<id>] [--json]`
- `grit actor current [--json]`
- `grit actor use <id>`
- `grit issue create --title ... --body ... --label ...`
- `grit issue update <id> [--title ...] [--body ...]`
- `grit issue list --state open --label bug --json`
- `grit issue show <id> --json`
- `grit issue comment <id> --body ...`
- `grit issue close <id>`
- `grit issue reopen <id>`
- `grit issue label add <id> --label <label>`
- `grit issue label remove <id> --label <label>`
- `grit issue assignee add <id> --user <name>`
- `grit issue assignee remove <id> --user <name>`
- `grit issue link add <id> --url ... [--note ...]`
- `grit issue attachment add <id> --name ... --sha256 ... --mime ...`
- `grit sync [--pull] [--push] [--remote <name>]`
- `grit doctor [--fix] [--json]`
- `grit rebuild [--from-snapshot]`
- `grit db stats [--json]`
- `grit export --format md|json`
- `grit snapshot`
- `grit snapshot gc`
- `grit lock acquire --resource <R> --ttl 15m`
- `grit lock renew --resource <R> --ttl 15m`
- `grit lock release --resource <R>`
- `grit lock status [--json]`
- `grit lock gc`
- `grit daemon status [--json]`
- `grit daemon stop`

## JSON output

- `--json` is supported on all commands
- `--quiet` suppresses human output for agents
- Errors are returned with structured details
- JSON schemas and error codes are defined in `docs/cli-json.md`

## Data directory

- `GRIT_HOME` or `--data-dir` sets the local state root for this process
- Default is `.git/grit/actors/<actor_id>/`
- Each concurrent agent should use a distinct data dir
- If a daemon owns the selected data dir, the CLI routes all commands through it and does not open the DB directly

## AGENTS.md

By default, `grit init` creates or updates an `AGENTS.md` file in the repository root with instructions for AI coding agents to use grit as the canonical task and memory system.

- If `AGENTS.md` does not exist, it is created with grit instructions
- If `AGENTS.md` exists but has no `## Grit` section, the section is appended
- If `AGENTS.md` already contains a `## Grit` section, no changes are made
- Use `--no-agents-md` to skip AGENTS.md creation/modification

## Actor identity

- `grit init` creates a default `actor_id`, writes `.git/grit/actors/<actor_id>/config.toml`,
  and sets `default_actor` in `.git/grit/config.toml`
- `grit actor init` creates an additional actor directory and prints the new ID
- If no actor config exists, commands may auto-initialize with a new `actor_id`

## Actor selection

Actor context for a command is resolved in this order:

1. `--data-dir` or `GRIT_HOME`
2. `--actor <id>` (resolves to `.git/grit/actors/<id>/`)
3. Repo default in `.git/grit/config.toml` (set by `grit actor use`)
4. Auto-init a new actor if none exists

## Export

- `grit export --format json` emits a machine-readable export suitable for dashboards
- `grit export --format md` emits a human-readable export
- `grit export --since <ts|event_id>` emits only changes after a point-in-time
- Export output is generated into `.grit/` by default and is never canonical

## Sync

The sync command handles pushing and pulling grit refs with remote repositories.

```bash
# Full sync (pull then push with auto-rebase)
grit sync

# Pull only
grit sync --pull

# Push only (auto-rebases on conflict)
grit sync --push

# Specify remote
grit sync --remote upstream
```

**Auto-rebase:** When a push fails due to non-fast-forward (remote has newer commits), grit automatically:
1. Pulls remote changes
2. Identifies local-only events
3. Re-appends local events on top of remote
4. Pushes again

The sync output reports when conflicts were resolved and how many events were rebased.

## Doctor

Health checks and auto-repair for the grit database.

```bash
# Run health checks
grit doctor

# Auto-repair issues (e.g., rebuild on corruption)
grit doctor --fix
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
grit rebuild

# Fast rebuild from latest snapshot (for large repos)
grit rebuild --from-snapshot
```

The `--from-snapshot` flag loads events from the latest snapshot instead of replaying the entire WAL, which is faster for repositories with many events.

## Error Messages

Errors include actionable suggestions to help resolve issues:

```
error: Issue 'abc123' not found

Suggestions:
  - Run 'grit issue list' to see available issues
```

Common suggestions include:
- **NotFound (issue)**: Run `grit issue list` to see available issues
- **DbBusy**: Try `grit --no-daemon <command>` or stop the daemon
- **Sled errors**: Run `grit doctor --fix` to rebuild
- **IPC errors**: Run `grit daemon stop` and retry
