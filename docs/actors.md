# Actors

Actors identify who authored an event and provide per-agent local state isolation. Each actor has its own data directory under `.git/gems/actors/<actor_id>/`.

## Actor lifecycle

- Created during `gems init` (default actor)
- Additional actors created via `gems actor init`
- Metadata stored in `.git/gems/actors/<actor_id>/config.toml`

## Actor commands

- `gems actor init [--label <name>]`
- `gems actor list [--json]`
- `gems actor show [<id>] [--json]`
- `gems actor current [--json]`
- `gems actor use <id>` (sets repo default in `.git/gems/config.toml`)

## Actor selection order

Actor context for a command is resolved in this order:

1. `--data-dir` or `GEMS_HOME`
2. `--actor <id>` (resolves to `.git/gems/actors/<id>/`)
3. Repo default in `.git/gems/config.toml`
4. Auto-init a new actor if none exists

## Shared repo guidance

When multiple agents share the same `.git` directory, each agent must use a distinct actor directory to avoid concurrent writes to the same local DB.
