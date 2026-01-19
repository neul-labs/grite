# Grit

Grit is a repo-local, git-backed issue/task system designed for coding agents and humans. It keeps an append-only event log in git refs, builds a fast local materialized view, and never writes tracked state into the working tree.

## Features

- **Git-native storage** - Events stored in `refs/grit/wal`, synced with `git fetch/push`
- **CRDT-based merging** - Deterministic conflict resolution, no manual merge needed
- **Per-actor isolation** - Each agent/device gets its own actor ID and local database
- **Optional daemon** - Auto-spawns for performance, not required for correctness
- **Ed25519 signing** - Optional cryptographic signatures on events
- **Team coordination** - Distributed locks for coordinated workflows

## Use Cases

Grit serves different audiences with distinct workflows:

| Audience | Primary Use Cases |
|----------|-------------------|
| [AI Coding Agents](docs/use-cases.md#ai-coding-agents) | Task decomposition, multi-agent coordination, persistent memory |
| [Individual Developers](docs/use-cases.md#individual-developers) | Offline issue tracking, personal task lists, technical debt |
| [Development Teams](docs/use-cases.md#development-teams) | Distributed coordination, code review workflows, knowledge base |
| [Security & Compliance](docs/use-cases.md#security--compliance) | Private vulnerability tracking, incident response, audit trails |
| [DevOps & Release Engineering](docs/use-cases.md#devops--release-engineering) | CI/CD integration, release checklists, deployment tracking |

See [Use Cases](docs/use-cases.md) for detailed workflows and examples.

## Installation

### Quick Install (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/neul-labs/grit/main/install.sh | bash
```

This downloads the pre-built binary for your platform and installs to `~/.local/bin/`.

### Package Managers

**Homebrew (macOS/Linux):**
```bash
brew install neul-labs/tap/grit
```

**Cargo (Rust):**
```bash
cargo install grit gritd
```

**npm:**
```bash
npm install -g @neul-labs/grit
```

**pip:**
```bash
pip install grit-cli
```

**gem:**
```bash
gem install grit-cli
```

**Chocolatey (Windows):**
```powershell
choco install grit
```

### From Source

```bash
git clone https://github.com/neul-labs/grit.git
cd grit
./install.sh --source
```

### Prerequisites

- Git 2.38+
- nng library (for IPC)

**Ubuntu/Debian:**
```bash
sudo apt install libnng-dev
```

**macOS:**
```bash
brew install nng
```

**Windows:**
The nng library is bundled with the pre-built binaries.

## Quick Start

```bash
# Initialize grit in a git repository
cd your-repo
grit init
# This also creates AGENTS.md with instructions for AI coding agents

# Create an issue
grit issue create --title "Fix login bug" --body "Users can't login"

# List issues
grit issue list

# Add a comment
grit issue comment <issue-id> --body "Working on this"

# Close an issue
grit issue close <issue-id>

# Sync with remote
grit sync
```

## Architecture

Grit uses a three-layer architecture:

```
+------------------+     +-------------------+     +------------------+
|   Git WAL        | --> | Materialized View | <-- | CLI / Daemon     |
| refs/grit/wal    |     | sled database     |     | grit / gritd     |
| (source of truth)|     | (fast queries)    |     | (user interface) |
+------------------+     +-------------------+     +------------------+
```

### Crate Structure

| Crate | Purpose |
|-------|---------|
| `libgrit-core` | Event types, hashing, projections, sled store, signing |
| `libgrit-git` | WAL commits, ref sync, snapshots, distributed locks |
| `libgrit-ipc` | IPC message schemas (rkyv), daemon lock, client/server |
| `grit` | CLI frontend |
| `gritd` | Optional background daemon |

### ID Types

| Type | Size | Format | Purpose |
|------|------|--------|---------|
| `ActorId` | 128-bit | Random | Identifies device/agent |
| `IssueId` | 128-bit | Random | Identifies issue |
| `EventId` | 256-bit | BLAKE2b hash | Content-addressed event ID |

IDs are stored as byte arrays internally and displayed as lowercase hex strings.

## Daemon

The daemon (`gritd`) is optional and provides:

- **Auto-spawn** - Automatically starts on first CLI command
- **Idle shutdown** - Stops after 5 minutes of inactivity (configurable)
- **Concurrent access** - Multiple CLI calls handled efficiently
- **Warm cache** - Keeps materialized view ready for fast queries

```bash
# Manual daemon control
grit daemon start --idle-timeout 300
grit daemon status
grit daemon stop

# Force local execution (skip daemon)
grit --no-daemon issue list
```

The daemon uses filesystem-level locking (`flock`) to prevent database corruption from concurrent access.

## Storage Layout

```
.git/
  grit/
    config.toml                    # Repo-level config (default actor, lock policy)
    actors/
      <actor_id>/
        config.toml                # Actor config (label, public key)
        sled/                      # Materialized view database
        sled.lock                  # flock for exclusive access
        daemon.lock                # Daemon ownership marker

refs/grit/
  wal                              # Append-only event log
  snapshots/<ts>                   # Periodic snapshots
  locks/<resource_hash>            # Distributed lease locks
```

## Documentation

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

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run --bin grit -- issue list

# Install locally
./install.sh
```

## Design Principles

1. **Git is the source of truth** - All state derivable from `refs/grit/*`
2. **No working tree pollution** - Never writes tracked files (except AGENTS.md for agent discoverability)
3. **Daemon optional** - CLI works standalone, daemon is performance optimization
4. **Deterministic merges** - CRDT semantics, no manual conflict resolution
5. **Per-actor isolation** - Multiple agents can work independently
6. **Agent discoverability** - `grit init` creates AGENTS.md so AI coding agents automatically discover grit

## License

MIT License - see [LICENSE](LICENSE) for details.
