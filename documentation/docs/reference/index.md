# Reference

This section is the authoritative reference for grite's CLI surface, JSON contract, configuration files, and environment variables. It is the canonical place to look up an exact flag, exit code, or schema field.

## Documentation

### [CLI Reference](cli.md)

Every command, subcommand, and flag exposed by the `grite` binary, with examples:

- Global flags (`--json`, `--quiet`, `--no-daemon`, `--actor`, `--data-dir`)
- `grite init`, `grite issue`, `grite comment`, `grite label`, `grite assign`
- `grite sync`, `grite fetch`, `grite push`
- `grite lock`, `grite context`, `grite actor`, `grite export`
- `grite doctor`, `grite rebuild`, `grite snapshot`
- Exit code semantics

### [JSON Output](cli-json.md)

JSON output schemas for scripting and agent integration:

- Response envelope format (`status`, `data`, `error`, `meta`)
- Per-command output schemas
- Error code catalogue
- Stable types shared across commands (`IssueSummary`, `EventRef`, `LockState`)

Use these schemas with `--json` to build reliable automation on top of grite.

### [Configuration](configuration.md)

Configuration files, their location, and every supported key:

- Per-repo config at `.git/grite/config.toml`
- Per-actor config under `.git/grite/actors/<actor_id>/`
- Defaults and precedence
- Migrating config between versions

### [Environment Variables](environment.md)

Every environment variable grite reads, what it overrides, and precedence rules:

- `GRITE_HOME` — data directory override
- `GRITE_ACTOR` — actor selection
- `GRITE_NO_DAEMON` — force CLI-only mode
- `RUST_LOG` — log filtering for diagnostics

## Quick Reference

### Common Commands

| Task | Command |
|------|---------|
| Initialize a repo | `grite init` |
| Create an issue | `grite issue create --title "..." --body "..."` |
| List open issues | `grite issue list` |
| Show one issue | `grite issue show <id>` |
| Comment | `grite comment add <id> --body "..."` |
| Close | `grite issue close <id>` |
| Add a label | `grite label add <id> --label bug` |
| Add a dependency | `grite issue dep add <id> --target <other> --type blocks` |
| Sync with remote | `grite sync` |
| Acquire a lock | `grite lock acquire --resource issue:<id> --ttl 15m` |
| Health check | `grite doctor` |
| Rebuild local view | `grite rebuild` |
| Export to JSON | `grite export --format json` |

### Global Flags

| Flag | Description |
|------|-------------|
| `--json` | Emit machine-readable JSON instead of human output |
| `--quiet` | Suppress non-essential human output |
| `--no-daemon` | Skip the daemon and run the command in-process |
| `--actor <id>` | Use a specific actor identity for this invocation |
| `--data-dir <path>` | Override the data directory (default `.git/grite/`) |

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Other error (catch-all) |
| `2` | Invalid arguments or unknown flag |
| `3` | Resource not found (issue, lock, actor) |
| `4` | Conflict (lock contention, cycle, concurrent edit rejected) |
| `5` | Environment error (not a git repo, missing daemon socket, IO) |

Scripts and agents should branch on the specific code, not just `!= 0`. See [JSON Output](cli-json.md) for the matching `error.code` strings.

## See Also

- [Architecture](../architecture/index.md) — Why the CLI behaves the way it does.
- [Agent Playbook](../agents/playbook.md) — Recommended flag combinations for autonomous agents.
- [Troubleshooting](../operations/troubleshooting.md) — Mapping common errors to fixes.
