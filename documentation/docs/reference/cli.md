# CLI Reference

Complete command-line interface reference for grite.

## Principles

- **Non-interactive by default**: No prompts or interactive inputs
- **Structured output**: All commands support `--json`
- **No daemon required**: CLI works standalone

## Global Flags

| Flag | Description |
|------|-------------|
| `--json` | Output JSON format |
| `--quiet` | Suppress human-readable output |
| `--no-daemon` | Force local execution, skip daemon |
| `--actor <id>` | Use specific actor |
| `--data-dir <path>` | Override data directory |
| `--help` | Show help |
| `--version` | Show version |

## Commands

### grite init

Initialize grite in a git repository.

```bash
grite init [--no-agents-md]
```

| Flag | Description |
|------|-------------|
| `--no-agents-md` | Skip creating/updating AGENTS.md |

Creates:
- `.git/grite/` directory structure
- Default actor
- `AGENTS.md` file (unless `--no-agents-md`)

---

### grite actor

Manage actor identities.

#### grite actor init

Create a new actor.

```bash
grite actor init [--label <name>] [--generate-key]
```

| Flag | Description |
|------|-------------|
| `--label <name>` | Human-friendly label |
| `--generate-key` | Generate Ed25519 signing key |

#### grite actor list

List all actors.

```bash
grite actor list [--json]
```

#### grite actor show

Show actor details.

```bash
grite actor show [<id>] [--json]
```

If `<id>` is omitted, shows current actor.

#### grite actor current

Show current actor context.

```bash
grite actor current [--json]
```

#### grite actor use

Set default actor for repository.

```bash
grite actor use <id>
```

---

### grite issue

Manage issues.

#### grite issue create

Create a new issue.

```bash
grite issue create --title <title> [--body <body>] [--label <label>]...
```

| Flag | Description |
|------|-------------|
| `--title <title>` | Issue title (required) |
| `--body <body>` | Issue body |
| `--label <label>` | Add label (can repeat) |

#### grite issue list

List issues.

```bash
grite issue list [--state <state>] [--label <label>]... [--json]
```

| Flag | Description |
|------|-------------|
| `--state <state>` | Filter by state: `open`, `closed` |
| `--label <label>` | Filter by label (can repeat) |

#### grite issue show

Show issue details.

```bash
grite issue show <id> [--json]
```

Short ID prefixes work if unique.

#### grite issue update

Update issue title or body.

```bash
grite issue update <id> [--title <title>] [--body <body>]
```

#### grite issue comment

Add a comment.

```bash
grite issue comment <id> --body <body>
```

#### grite issue close

Close an issue.

```bash
grite issue close <id>
```

#### grite issue reopen

Reopen an issue.

```bash
grite issue reopen <id>
```

#### grite issue label

Manage issue labels.

```bash
grite issue label add <id> --label <label>
grite issue label remove <id> --label <label>
```

#### grite issue assignee

Manage issue assignees.

```bash
grite issue assignee add <id> --user <name>
grite issue assignee remove <id> --user <name>
```

#### grite issue link

Add links to issues.

```bash
grite issue link add <id> --url <url> [--note <note>]
```

#### grite issue attachment

Add attachment metadata.

```bash
grite issue attachment add <id> --name <name> --sha256 <hash> --mime <type>
```

#### grite issue dep

Manage typed dependencies between issues.

##### grite issue dep add

Add a dependency.

```bash
grite issue dep add <id> --target <target_id> --type <type>
```

| Flag | Description |
|------|-------------|
| `--target <id>` | Target issue ID (required) |
| `--type <type>` | Dependency type: `blocks`, `depends_on`, `related_to` (required) |

Cycle detection is enforced for `blocks` and `depends_on` types.

##### grite issue dep remove

Remove a dependency.

```bash
grite issue dep remove <id> --target <target_id> --type <type>
```

##### grite issue dep list

List dependencies for an issue.

```bash
grite issue dep list <id> [--reverse]
```

| Flag | Description |
|------|-------------|
| `--reverse` | Show issues that depend on this one |

##### grite issue dep topo

Show topological ordering of issues based on dependency DAG.

```bash
grite issue dep topo [--state <state>] [--label <label>]
```

| Flag | Description |
|------|-------------|
| `--state <state>` | Filter by state: `open`, `closed` |
| `--label <label>` | Filter by label |

---

### grite context

Context store management for file/symbol indexing.

#### grite context index

Index files in the repository.

```bash
grite context index [--path <path>]... [--pattern <glob>] [--force]
```

| Flag | Description |
|------|-------------|
| `--path <path>` | Restrict to specific paths (can repeat) |
| `--pattern <glob>` | Filter files by glob pattern (e.g., `"*.rs"`) |
| `--force` | Re-index even if file hash unchanged |

Uses `git ls-files` for file discovery (respects .gitignore). Tree-sitter-powered symbol extraction supports Rust, Python, TypeScript/TSX, JavaScript, Go, Java, C, C++, Ruby, and Elixir with AST-accurate line ranges.

#### grite context query

Query the symbol index.

```bash
grite context query <query>
```

Searches for symbols matching the query string.

#### grite context show

Show context for a specific file.

```bash
grite context show <path>
```

Displays language, symbols, summary, and content hash.

#### grite context project

Show project-level context entries.

```bash
grite context project [<key>]
```

Without a key, lists all project context entries. With a key, shows that specific entry.

#### grite context set

Set a project-level context entry.

```bash
grite context set <key> <value>
```

---

### grite sync

Synchronize with remote.

```bash
grite sync [--pull] [--push] [--remote <name>]
```

| Flag | Description |
|------|-------------|
| `--pull` | Only pull from remote |
| `--push` | Only push to remote |
| `--remote <name>` | Specify remote (default: `origin`) |

No flags: full sync (pull then push).

---

### grite doctor

Health checks and repair.

```bash
grite doctor [--fix] [--json]
```

| Flag | Description |
|------|-------------|
| `--fix` | Auto-repair issues |

Checks:
- `git_repo`: Git repository validity
- `wal_ref`: WAL ref exists and readable
- `actor_config`: Actor properly configured
- `store_integrity`: Database integrity
- `rebuild_threshold`: Rebuild recommendation

---

### grite rebuild

Rebuild materialized view.

```bash
grite rebuild [--from-snapshot]
```

| Flag | Description |
|------|-------------|
| `--from-snapshot` | Use latest snapshot |

---

### grite db

Database operations.

#### grite db stats

Show database statistics.

```bash
grite db stats [--json]
```

#### grite db check

Verify event hashes.

```bash
grite db check [--verify-parents] [--json]
```

#### grite db verify

Verify event signatures.

```bash
grite db verify [--verbose] [--json]
```

---

### grite export

Export issues.

```bash
grite export --format <format> [--since <ts|event_id>]
```

| Flag | Description |
|------|-------------|
| `--format <format>` | Output format: `json`, `md` |
| `--since <ts>` | Only changes after timestamp or event ID |

---

### grite snapshot

Manage snapshots.

#### grite snapshot

Create a snapshot.

```bash
grite snapshot
```

#### grite snapshot gc

Garbage collect old snapshots.

```bash
grite snapshot gc
```

---

### grite lock

Distributed lock management.

#### grite lock acquire

Acquire a lock.

```bash
grite lock acquire --resource <resource> --ttl <duration>
```

| Flag | Description |
|------|-------------|
| `--resource <resource>` | Resource to lock (e.g., `issue:abc123`) |
| `--ttl <duration>` | Time-to-live (e.g., `15m`, `1h`) |

#### grite lock renew

Renew a lock.

```bash
grite lock renew --resource <resource> --ttl <duration>
```

#### grite lock release

Release a lock.

```bash
grite lock release --resource <resource>
```

#### grite lock status

Show lock status.

```bash
grite lock status [--json]
```

#### grite lock gc

Garbage collect expired locks.

```bash
grite lock gc
```

---

### grite daemon

Daemon control.

#### grite daemon start

Start the daemon.

```bash
grite daemon start [--idle-timeout <seconds>]
```

| Flag | Description |
|------|-------------|
| `--idle-timeout <seconds>` | Auto-shutdown timeout (0 = no timeout) |

#### grite daemon status

Check daemon status.

```bash
grite daemon status [--json]
```

#### grite daemon stop

Stop the daemon.

```bash
grite daemon stop
```

---

## Actor Selection Order

Actor context is resolved in this order:

1. `--data-dir` or `GRIT_HOME`
2. `--actor <id>`
3. `default_actor` in `.git/grite/config.toml`
4. Auto-create new actor if none exists

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success (`ok: true`) |
| `2` | Invalid arguments |
| `3` | Not found |
| `4` | Conflict or lock violation |
| `5` | Environment error (not a git repo, missing config, db busy) |
| `1` | Any other failure |

---

## Error Messages

Errors include actionable suggestions:

```
error: Issue 'abc123' not found

Suggestions:
  - Run 'grite issue list' to see available issues
```

Common suggestions:

| Error | Suggestion |
|-------|------------|
| Issue not found | Run `grite issue list` |
| DbBusy | Try `grite --no-daemon <command>` or stop daemon |
| Sled errors | Run `grite doctor --fix` |
| IPC errors | Run `grite daemon stop` and retry |
