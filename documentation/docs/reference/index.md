# Reference

This section contains complete reference documentation for grit.

## Documentation

### [CLI Reference](cli.md)

Complete command-line interface documentation:

- All commands and subcommands
- Global flags
- Exit codes
- Examples

### [JSON Output](cli-json.md)

JSON output schemas for scripting:

- Response envelope format
- Error codes
- Per-command output schemas
- Common types

### [Configuration](configuration.md)

Configuration file reference:

- Repository configuration
- Actor configuration
- All configuration options

### [Environment Variables](environment.md)

Environment variable reference:

- `GRIT_HOME`
- `RUST_LOG`
- Precedence rules

## Quick Reference

### Common Commands

| Task | Command |
|------|---------|
| Initialize | `grit init` |
| Create issue | `grit issue create --title "..." --body "..."` |
| List issues | `grit issue list` |
| Show issue | `grit issue show <id>` |
| Close issue | `grit issue close <id>` |
| Sync | `grit sync` |
| Health check | `grit doctor` |

### Global Flags

| Flag | Description |
|------|-------------|
| `--json` | Output JSON |
| `--quiet` | Suppress human output |
| `--no-daemon` | Skip daemon |
| `--actor <id>` | Use specific actor |
| `--data-dir <path>` | Override data directory |

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `2` | Invalid arguments |
| `3` | Not found |
| `4` | Conflict |
| `5` | Environment error |
| `1` | Other error |
