# grite

Git-backed issue tracker with CRDT merging, designed for AI coding agents and humans.

`grite` is the primary CLI binary. It stores issues as events in your repository's git history, giving you version-controlled issue tracking that works offline, across branches, and in worktrees. No central server required.

## Key commands

- `grite issue create|list|show|update|close|reopen|comment` — issue lifecycle
- `grite actor init|list|show|set-default` — multi-actor identity management
- `grite sync pull|push|merge|snapshot|rebuild` — distributed sync and recovery
- `grite daemon start|stop|status` — background daemon for performance
- `grite install-skill` — install a Claude Code skill for your repo

## Quick example

```bash
# Create an issue tracked in git
grite issue create --title "Fix race in WAL append" --label bug

# Sync with a teammate's actor
grite sync pull origin

# Start the background daemon
grite daemon start
```

See the [full documentation](https://docs.rs/grite) and the [Grite repository](https://github.com/neul-labs/grite).
