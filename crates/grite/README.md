# grite

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/grite.svg)](https://crates.io/crates/grite)
[![Documentation](https://img.shields.io/badge/docs-docs.rs-green.svg)](https://docs.rs/grite)
[![Build Status](https://img.shields.io/github/actions/workflow/status/neul-labs/grite/ci.yml?branch=main)](https://github.com/neul-labs/grite/actions)

**The command-line interface for Grite — git-backed issue tracking that works offline, syncs with `git push`, and coordinates AI agents without conflicts.**

`grite` is the primary user-facing binary of the Grite ecosystem. It provides a fast, ergonomic CLI for creating, querying, and managing issues stored as an append-only event log inside your repository's git refs. Every operation works offline. Sync happens through the git remote you already use. Multi-agent coordination is handled automatically by CRDT semantics.

---

## What Makes the Grite CLI Different?

Most issue trackers force you to leave your terminal, open a browser, and context-switch to a web application. Grite brings issue tracking to where you already are — the command line, inside your repository, on your branch.

### Zero-Config, Zero-Setup

Run `grite init` in any git repository and you have a fully functional issue tracker. No database setup. No server to run. No account to create. No API tokens to manage. Your issues live in `refs/grite/wal`, travel with your code, and sync when you `git push`.

### Agent-Native Design

The CLI was designed for both humans and machines:

- **Human-friendly** — Pretty-printed tables, colored output, sensible defaults, and intuitive commands.
- **Machine-friendly** — Every command supports `--json` for structured output. JSON is stable, documented, and tested. Build scripts, CI pipelines, and AI agents can parse it reliably.
- **Agent-discoverable** — `grite init` generates `AGENTS.md`, a convention that AI coding agents read automatically. Agents discover grite without any manual configuration.

### Performance by Default

The CLI auto-detects and auto-spawns a background daemon on first use. This eliminates per-command startup overhead (database open, WAL replay, view rebuild) and enables concurrent access. The daemon shuts down automatically after idle timeout. You never think about it, but you always benefit from it.

### Conflict-Free Collaboration

Two agents edit the same issue at the same time? Both changes are preserved. Grite uses CRDT (Conflict-free Replicated Data Type) semantics to merge events deterministically, with no manual intervention and no data loss. This is not eventual consistency with conflicts — it is mathematically guaranteed convergence.

---

## Installation

```bash
# Quick install (all platforms)
curl -fsSL https://raw.githubusercontent.com/neul-labs/grite/main/install.sh | bash

# macOS / Linux via Homebrew
brew install neul-labs/tap/grite

# Rust via Cargo
cargo install grite

# Node.js via npm
npm install -g grite

# Python via pip
pip install grite

# Ruby via RubyGems
gem install grite
```

**Prerequisites:** Git 2.38+.

---

## Commands

### Issue Lifecycle

Manage the full lifecycle of issues and tasks directly from your terminal:

```bash
# Create a new issue
grite issue create --title "Fix race in WAL append" --body "Occurs under high load."

# List open issues (fast — queries the materialized view)
grite issue list
grite issue list --label bug --label concurrency
grite issue list --json | jq '.[] | select(.status == "open")'

# Show full issue details
grite issue show <issue-id>

# Update issue properties
grite issue update <issue-id> --title "Fix race in WAL append (critical)" --label critical

# Close an issue
grite issue close <issue-id>

# Reopen a closed issue
grite issue reopen <issue-id>

# Add a comment (great for agent checkpoints)
grite issue comment <issue-id> --body "Reproduced on commit abc123."
```

### Actor Management

Each installation of grite gets an actor ID. Multiple actors (agents, developers, CI systems) can work on the same repository and merge their changes later.

```bash
# Initialize a new actor for this machine/agent
grite actor init --label "CI Runner"

# List all known actors
grite actor list

# Show actor details
grite actor show <actor-id>

# Set the default actor for this repository
grite actor set-default <actor-id>
```

### Sync and Recovery

Synchronize state between actors and rebuild the materialized view:

```bash
# Pull remote WAL and merge
grite sync pull origin

# Push local WAL to remote
grite sync push origin

# Pull, merge, and push in one command
grite sync

# Create a snapshot for fast rebuilds
grite sync snapshot

# Rebuild the materialized view from WAL
grite rebuild

# Fast rebuild from latest snapshot
grite rebuild --from-snapshot
```

### Distributed Locks

Coordinate exclusive access to resources across agents:

```bash
# Acquire a lock on a file or module
grite lock acquire src/parser.rs --ttl 3600

# Check if a resource is locked
grite lock status src/parser.rs

# Release a lock early
grite lock release src/parser.rs

# List all active locks
grite lock list
```

### Daemon Control

The background daemon is optional but recommended for performance:

```bash
# Start the daemon manually
grite daemon start --idle-timeout 300

# Check daemon status
grite daemon status

# Stop the daemon
grite daemon stop

# Run a command without using the daemon
grite --no-daemon issue list
```

### Health and Diagnostics

```bash
# Run comprehensive health checks
grite doctor

# Auto-repair detected issues
grite doctor --fix

# Export issues to Markdown
grite export --format markdown --since 7d

# Export to JSON for programmatic processing
grite export --format json --label memory
```

### Context Extraction

Extract code context using tree-sitter for richer issues:

```bash
# Extract symbols from the current codebase
grite context extract src/main.rs

# Query symbol references
grite context query --symbol "parse_expression"
```

---

## Quick Examples

### Daily Developer Workflow

```bash
# Morning standup — see what is open
grite issue list

# Pick a task, add an in-progress label
grite issue update <id> --label in-progress

# Claim the lock on the file you are editing
grite lock acquire src/auth.rs --ttl 7200

# Work, work, work...

# Add a checkpoint comment
grite issue comment <id> --body "Implemented token validation. Need to add tests."

# Close when done
grite issue close <id>
grite lock release src/auth.rs

# Push everything
grite sync --push
```

### Agent Workflow

```bash
# Agent startup routine (from AGENTS.md)
grite sync --pull
grite issue list --label "agent:todo" --json > /tmp/tasks.json

# Agent claims a task
grite issue update <id> --label "agent:in-progress"

# Agent stores a memory
grite issue create --title "Auth module: token rotation" \
  --body "Tokens rotate every 15 minutes. Refresh endpoint is /auth/refresh." \
  --label memory

# Agent pushes state
grite sync --push
```

---

## Architecture

The `grite` binary is a thin CLI frontend that delegates to the [`libgrite-cli`](../libgrite-cli) programmatic API. It handles argument parsing, output formatting, and daemon lifecycle management.

```
grite (CLI binary)
  |
  +-- libgrite-cli (programmatic API)
        |
        +-- libgrite-core (data model, storage, CRDTs)
        +-- libgrite-git (WAL, sync, snapshots)
        +-- libgrite-ipc (daemon communication)
```

- **Single operation mode:** If the daemon is not running, the CLI opens the database directly, executes the command, and exits. No daemon required.
- **Daemon mode:** If the daemon is running (or auto-spawned), commands are sent via IPC over Unix domain sockets for better performance and concurrency.

---

## Configuration

Grite reads configuration from multiple sources, in order of precedence:

1. Command-line flags (highest priority)
2. Environment variables (`GRITE_*`)
3. `.git/grite/config.toml` (repository-level)
4. `~/.config/grite/config.toml` (user-level)
5. Built-in defaults (lowest priority)

See the [Configuration](https://docs.neullabs.com/grite/configuration) documentation for all options.

---

## Integration

### Shell Completions

```bash
# Bash
grite completions bash > /etc/bash_completion.d/grite

# Zsh
grite completions zsh > /usr/local/share/zsh/site-functions/_grite

# Fish
grite completions fish > ~/.config/fish/completions/grite.fish
```

### CI/CD Integration

```yaml
# Example GitHub Actions step
- name: Track deployment
  run: |
    grite issue create --title "Deploy v${{ github.ref_name }}" \
      --label deployment \
      --label "release:${{ github.ref_name }}"
    grite sync --push
```

### Editor Integration

Because grite outputs JSON, integrating with editors is straightforward:

```bash
# Vim: list issues in quickfix
grite issue list --json | jq -r '.[] | "\(.id): \(.title)"'

# VS Code: use in tasks.json
grite issue show $(grite issue list --json | jq -r '.[0].id')
```

---

## See Also

- [Grite Repository](https://github.com/neul-labs/grite) — Main project, architecture, and documentation
- [libgrite-cli](../libgrite-cli) — Programmatic API for embedding in Rust applications
- [grite-daemon](../grite-daemon) — Background daemon for performance
- [docs.rs/grite](https://docs.rs/grite) — Rust API documentation

---

## License

MIT License — see [LICENSE](../LICENSE) for details.
