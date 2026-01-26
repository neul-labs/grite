# Grite

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/grite.svg)](https://crates.io/crates/grite)
[![Build Status](https://img.shields.io/github/actions/workflow/status/neul-labs/grite/ci.yml?branch=main)](https://github.com/neul-labs/grite/actions)
[![Documentation](https://img.shields.io/badge/docs-neullabs.com-green.svg)](https://docs.neullabs.com/grite)

**The issue tracker that lives in your repo. Built for AI agents. Works for humans.**

Grite stores issues as an append-only event log inside git refs (`refs/grite/wal`), keeping your working tree clean while enabling seamless multi-agent collaboration through CRDT-based conflict resolution.

## What is Grite?

Grite is a **repo-local, git-backed issue/task system** that solves a fundamental problem: how do multiple AI coding agents (and humans) coordinate work on the same codebase without stepping on each other?

Traditional issue trackers live on external servers, requiring API calls and authentication. Grite embeds directly in your git repository:

- **No external dependencies** — works offline, syncs with `git push/pull`
- **No merge conflicts** — CRDT semantics ensure deterministic, automatic merging
- **No working tree pollution** — all state lives in git refs, not tracked files
- **Agent-native** — designed for AI coding agents from the ground up

## Why Grite?

**The Problem:** AI coding agents need persistent memory and coordination:
- Agents forget context between sessions
- Multiple agents working on the same repo create conflicts
- External issue trackers add latency and require credentials
- Traditional approaches pollute the working tree with state files

**The Solution:** Grite provides:
- **Persistent task memory** — agents can track what they're doing across sessions
- **Multi-agent coordination** — distributed locks prevent conflicting work
- **Zero-config sync** — if you can `git push`, you can sync issues
- **Context extraction** — tree-sitter powered symbol extraction helps agents understand code

## Performance

Grite is designed for speed:

| Operation | Time | Notes |
|-----------|------|-------|
| Issue create | ~5ms | Single event append |
| Issue list (1000 issues) | ~10ms | Materialized view query |
| Full rebuild (10k events) | ~500ms | From git WAL |
| Snapshot rebuild | ~50ms | Skip to last snapshot |
| Sync (1000 new events) | ~200ms | Network-bound |

**Memory footprint:**
- CLI: ~15MB RSS (single operation, exits immediately)
- Daemon: ~30MB RSS (warm cache, handles concurrent requests)
- Database: ~1KB per issue (sled key-value store)

**Concurrency:**
- Local: 50+ concurrent CLI calls via daemon (serialized writes, parallel reads)
- Distributed: 100+ agents across machines (each gets isolated actor ID, CRDT merge on sync)
- Lock contention: Distributed locks with configurable TTL for exclusive resource access

**Scaling:** Tested with 100k+ events, 10k+ issues. The materialized view (sled) provides O(1) lookups; the git WAL provides O(n) rebuild but snapshots reduce this to O(delta).

## Features

- **Git-native storage** - Events stored in `refs/grite/wal`, synced with `git fetch/push`
- **CRDT-based merging** - Deterministic conflict resolution, no manual merge needed
- **Dependency DAG** - Typed issue relationships (blocks, depends_on, related_to) with cycle detection and topological ordering
- **Context store** - Tree-sitter-powered symbol extraction across 10 languages, with distributed sync between agents
- **Per-actor isolation** - Each agent/device gets its own actor ID and local database
- **Optional daemon** - Auto-spawns for performance, not required for correctness
- **Ed25519 signing** - Optional cryptographic signatures on events
- **Team coordination** - Distributed locks for coordinated workflows

## Use Cases

Grite serves different audiences with distinct workflows:

| Audience | Primary Use Cases |
|----------|-------------------|
| [AI Coding Agents](docs/use-cases.md#ai-coding-agents) | Task decomposition, multi-agent coordination, persistent memory |
| [Individual Developers](docs/use-cases.md#individual-developers) | Offline issue tracking, personal task lists, technical debt |
| [Development Teams](docs/use-cases.md#development-teams) | Distributed coordination, code review workflows, knowledge base |
| [Security & Compliance](docs/use-cases.md#security--compliance) | Private vulnerability tracking, incident response, audit trails |
| [DevOps & Release Engineering](docs/use-cases.md#devops--release-engineering) | CI/CD integration, release checklists, deployment tracking |

See [Use Cases](docs/use-cases.md) for detailed workflows and examples.

## Installation

```bash
# Quick install (recommended) — downloads binary to ~/.local/bin/
curl -fsSL https://raw.githubusercontent.com/neul-labs/grite/main/install.sh | bash

# Or use your package manager:
brew install neul-labs/tap/grite    # macOS/Linux
cargo install grite grite-daemon    # Rust
npm install -g @neul-labs/grite     # Node.js
pip install grite-cli               # Python
choco install grite                 # Windows
```

**Prerequisites:** Git 2.38+ and nng library (`apt install libnng-dev` / `brew install nng` — bundled on Windows).

## Quick Start

```bash
# Initialize grite in a git repository
cd your-repo
grite init
# This also creates AGENTS.md with instructions for AI coding agents

# Create an issue
grite issue create --title "Fix login bug" --body "Users can't login"

# List issues
grite issue list

# Add a comment
grite issue comment <issue-id> --body "Working on this"

# Close an issue
grite issue close <issue-id>

# Sync with remote (auto-rebases on conflict)
grite sync

# Run health checks
grite doctor

# Rebuild database (fast mode with snapshots)
grite rebuild --from-snapshot
```

## Architecture

Grite uses a three-layer architecture:

```
+------------------+     +-------------------+     +------------------+
|   Git WAL        | --> | Materialized View | <-- | CLI / Daemon     |
| refs/grite/wal    |     | sled database     |     | grite / grite-daemon     |
| (source of truth)|     | (fast queries)    |     | (user interface) |
+------------------+     +-------------------+     +------------------+
```

### Crate Structure

| Crate | Purpose |
|-------|---------|
| `libgrite-core` | Event types, hashing, projections, sled store, signing |
| `libgrite-git` | WAL commits, ref sync, snapshots, distributed locks |
| `libgrite-ipc` | IPC message schemas (rkyv), daemon lock, client/server |
| `grite` | CLI frontend |
| `grite-daemon` | Optional background daemon |

### ID Types

| Type | Size | Format | Purpose |
|------|------|--------|---------|
| `ActorId` | 128-bit | Random | Identifies device/agent |
| `IssueId` | 128-bit | Random | Identifies issue |
| `EventId` | 256-bit | BLAKE2b hash | Content-addressed event ID |

IDs are stored as byte arrays internally and displayed as lowercase hex strings.

## Daemon

The daemon (`grite-daemon`) is optional and provides:

- **Auto-spawn** - Automatically starts on first CLI command
- **Idle shutdown** - Stops after 5 minutes of inactivity (configurable)
- **Concurrent access** - Multiple CLI calls handled efficiently
- **Warm cache** - Keeps materialized view ready for fast queries

```bash
# Manual daemon control
grite daemon start --idle-timeout 300
grite daemon status
grite daemon stop

# Force local execution (skip daemon)
grite --no-daemon issue list
```

The daemon uses filesystem-level locking (`flock`) to prevent database corruption from concurrent access.

## Storage Layout

```
.git/
  grite/
    config.toml                    # Repo-level config (default actor, lock policy)
    actors/
      <actor_id>/
        config.toml                # Actor config (label, public key)
        sled/                      # Materialized view database
        sled.lock                  # flock for exclusive access
        daemon.lock                # Daemon ownership marker

refs/grite/
  wal                              # Append-only event log
  snapshots/<ts>                   # Periodic snapshots
  locks/<resource_hash>            # Distributed lease locks
```

## Documentation

Full documentation is available at [docs.neullabs.com/grite](https://docs.neullabs.com/grite).

| Document | Description |
|----------|-------------|
| [Architecture](docs/architecture.md) | System design and data flow |
| [Use Cases](docs/use-cases.md) | Workflows for agents, developers, and teams |
| [Data Model](docs/data-model.md) | Event schema, hashing, projections |
| [CLI Reference](docs/cli.md) | Command-line interface |
| [CLI JSON Output](docs/cli-json.md) | JSON output format for scripting |
| [Daemon](docs/daemon.md) | Background daemon details |
| [Actors](docs/actors.md) | Actor identity and isolation |
| [Configuration](docs/configuration.md) | Config files and options |
| [Git WAL](docs/git-wal.md) | WAL format and chunk encoding |
| [IPC Protocol](docs/ipc.md) | Inter-process communication |
| [Locking](docs/locking.md) | Distributed lock coordination |
| [Export Format](docs/export-format.md) | JSON/Markdown export |
| [Hash Vectors](docs/hash-vectors.md) | Canonical hashing test vectors |
| [Operations](docs/operations.md) | Backup, recovery, debugging |
| [Agent Playbook](docs/agent-playbook.md) | Guide for AI coding agents |
| [Comparison](docs/comparison.md) | How Grite compares with Beads, git-bug, and others |

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run --bin grite -- issue list

# Install locally
./install.sh
```

## Design Principles

1. **Git is the source of truth** - All state derivable from `refs/grite/*`
2. **No working tree pollution** - Never writes tracked files (except AGENTS.md for agent discoverability)
3. **Daemon optional** - CLI works standalone, daemon is performance optimization
4. **Deterministic merges** - CRDT semantics, no manual conflict resolution
5. **Per-actor isolation** - Multiple agents can work independently
6. **Agent discoverability** - `grite init` creates AGENTS.md so AI coding agents automatically discover grite

## License

MIT License - see [LICENSE](LICENSE) for details.
