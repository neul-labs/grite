# CLI Reference

Complete command-line interface reference for grit.

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

### grit init

Initialize grit in a git repository.

```bash
grit init [--no-agents-md]
```

| Flag | Description |
|------|-------------|
| `--no-agents-md` | Skip creating/updating AGENTS.md |

Creates:
- `.git/grit/` directory structure
- Default actor
- `AGENTS.md` file (unless `--no-agents-md`)

---

### grit actor

Manage actor identities.

#### grit actor init

Create a new actor.

```bash
grit actor init [--label <name>] [--generate-key]
```

| Flag | Description |
|------|-------------|
| `--label <name>` | Human-friendly label |
| `--generate-key` | Generate Ed25519 signing key |

#### grit actor list

List all actors.

```bash
grit actor list [--json]
```

#### grit actor show

Show actor details.

```bash
grit actor show [<id>] [--json]
```

If `<id>` is omitted, shows current actor.

#### grit actor current

Show current actor context.

```bash
grit actor current [--json]
```

#### grit actor use

Set default actor for repository.

```bash
grit actor use <id>
```

---

### grit issue

Manage issues.

#### grit issue create

Create a new issue.

```bash
grit issue create --title <title> [--body <body>] [--label <label>]...
```

| Flag | Description |
|------|-------------|
| `--title <title>` | Issue title (required) |
| `--body <body>` | Issue body |
| `--label <label>` | Add label (can repeat) |

#### grit issue list

List issues.

```bash
grit issue list [--state <state>] [--label <label>]... [--json]
```

| Flag | Description |
|------|-------------|
| `--state <state>` | Filter by state: `open`, `closed` |
| `--label <label>` | Filter by label (can repeat) |

#### grit issue show

Show issue details.

```bash
grit issue show <id> [--json]
```

Short ID prefixes work if unique.

#### grit issue update

Update issue title or body.

```bash
grit issue update <id> [--title <title>] [--body <body>]
```

#### grit issue comment

Add a comment.

```bash
grit issue comment <id> --body <body>
```

#### grit issue close

Close an issue.

```bash
grit issue close <id>
```

#### grit issue reopen

Reopen an issue.

```bash
grit issue reopen <id>
```

#### grit issue label

Manage issue labels.

```bash
grit issue label add <id> --label <label>
grit issue label remove <id> --label <label>
```

#### grit issue assignee

Manage issue assignees.

```bash
grit issue assignee add <id> --user <name>
grit issue assignee remove <id> --user <name>
```

#### grit issue link

Add links to issues.

```bash
grit issue link add <id> --url <url> [--note <note>]
```

#### grit issue attachment

Add attachment metadata.

```bash
grit issue attachment add <id> --name <name> --sha256 <hash> --mime <type>
```

---

### grit sync

Synchronize with remote.

```bash
grit sync [--pull] [--push] [--remote <name>]
```

| Flag | Description |
|------|-------------|
| `--pull` | Only pull from remote |
| `--push` | Only push to remote |
| `--remote <name>` | Specify remote (default: `origin`) |

No flags: full sync (pull then push).

---

### grit doctor

Health checks and repair.

```bash
grit doctor [--fix] [--json]
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

### grit rebuild

Rebuild materialized view.

```bash
grit rebuild [--from-snapshot]
```

| Flag | Description |
|------|-------------|
| `--from-snapshot` | Use latest snapshot |

---

### grit db

Database operations.

#### grit db stats

Show database statistics.

```bash
grit db stats [--json]
```

#### grit db check

Verify event hashes.

```bash
grit db check [--verify-parents] [--json]
```

#### grit db verify

Verify event signatures.

```bash
grit db verify [--verbose] [--json]
```

---

### grit export

Export issues.

```bash
grit export --format <format> [--since <ts|event_id>]
```

| Flag | Description |
|------|-------------|
| `--format <format>` | Output format: `json`, `md` |
| `--since <ts>` | Only changes after timestamp or event ID |

---

### grit snapshot

Manage snapshots.

#### grit snapshot

Create a snapshot.

```bash
grit snapshot
```

#### grit snapshot gc

Garbage collect old snapshots.

```bash
grit snapshot gc
```

---

### grit lock

Distributed lock management.

#### grit lock acquire

Acquire a lock.

```bash
grit lock acquire --resource <resource> --ttl <duration>
```

| Flag | Description |
|------|-------------|
| `--resource <resource>` | Resource to lock (e.g., `issue:abc123`) |
| `--ttl <duration>` | Time-to-live (e.g., `15m`, `1h`) |

#### grit lock renew

Renew a lock.

```bash
grit lock renew --resource <resource> --ttl <duration>
```

#### grit lock release

Release a lock.

```bash
grit lock release --resource <resource>
```

#### grit lock status

Show lock status.

```bash
grit lock status [--json]
```

#### grit lock gc

Garbage collect expired locks.

```bash
grit lock gc
```

---

### grit daemon

Daemon control.

#### grit daemon start

Start the daemon.

```bash
grit daemon start [--idle-timeout <seconds>]
```

| Flag | Description |
|------|-------------|
| `--idle-timeout <seconds>` | Auto-shutdown timeout (0 = no timeout) |

#### grit daemon status

Check daemon status.

```bash
grit daemon status [--json]
```

#### grit daemon stop

Stop the daemon.

```bash
grit daemon stop
```

---

## Actor Selection Order

Actor context is resolved in this order:

1. `--data-dir` or `GRIT_HOME`
2. `--actor <id>`
3. `default_actor` in `.git/grit/config.toml`
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
  - Run 'grit issue list' to see available issues
```

Common suggestions:

| Error | Suggestion |
|-------|------------|
| Issue not found | Run `grit issue list` |
| DbBusy | Try `grit --no-daemon <command>` or stop daemon |
| Sled errors | Run `grit doctor --fix` |
| IPC errors | Run `grit daemon stop` and retry |
