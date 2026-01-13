# CLI

## Principles

- Non-interactive by default
- Structured output always available (`--json`)
- No daemon required for correctness

## Command overview

- `gems init`
- `gems actor init [--label <name>]`
- `gems actor list [--json]`
- `gems actor show [<id>] [--json]`
- `gems actor current [--json]`
- `gems actor use <id>`
- `gems issue create --title ... --body ... --label ...`
- `gems issue list --state open --label bug --json`
- `gems issue show <id> --json`
- `gems issue comment <id> --body ...`
- `gems issue close <id> --reason done`
- `gems sync [--pull] [--push]`
- `gems doctor [--json] [--apply]`
- `gems export --format md|json`
- `gems snapshot`
- `gems snapshot gc`
- `gems lock acquire --resource <R> --ttl 15m`
- `gems lock renew --resource <R> --ttl 15m`
- `gems lock release --resource <R>`
- `gems lock gc`

## JSON output

- `--json` is supported on all read commands
- `--quiet` suppresses human output for agents
- Errors are returned with structured details

## Data directory

- `GEMS_HOME` or `--data-dir` sets the local state root for this process
- Default is `.git/gems/actors/<actor_id>/`
- Each concurrent agent should use a distinct data dir

## Actor identity

- `gems init` creates a default `actor_id` and writes `.git/gems/actors/<actor_id>/config.toml`
- `gems actor init` creates an additional actor directory and prints the new ID
- If no actor config exists, commands may auto-initialize with a new `actor_id`

## Actor selection

Actor context for a command is resolved in this order:

1. `--data-dir` or `GEMS_HOME`
2. `--actor <id>` (resolves to `.git/gems/actors/<id>/`)
3. Repo default in `.git/gems/config.toml` (set by `gems actor use`)
4. Auto-init a new actor if none exists

## Export

- `gems export --format json` emits a machine-readable export suitable for dashboards
- `gems export --format md` emits a human-readable export
- Export output is generated into `.gems/` by default and is never canonical
