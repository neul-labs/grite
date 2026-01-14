# Actors

Actors identify who authored an event and provide per-agent local state isolation. Each actor has its own data directory under `.git/grit/actors/<actor_id>/`.

## Actor lifecycle

- Created during `grit init` (default actor; also sets repo default)
- Additional actors created via `grit actor init`
- Metadata stored in `.git/grit/actors/<actor_id>/config.toml`

## Actor commands

- `grit actor init [--label <name>]`
- `grit actor list [--json]`
- `grit actor show [<id>] [--json]`
- `grit actor current [--json]`
- `grit actor use <id>` (sets repo default in `.git/grit/config.toml`)

## Actor selection order

Actor context for a command is resolved in this order:

1. `--data-dir` or `GRIT_HOME`
2. `--actor <id>` (resolves to `.git/grit/actors/<id>/`)
3. Repo default in `.git/grit/config.toml`
4. Auto-init a new actor if none exists

## Shared repo guidance

When multiple agents share the same `.git` directory, each agent must use a distinct actor directory to avoid concurrent writes to the same local DB.
