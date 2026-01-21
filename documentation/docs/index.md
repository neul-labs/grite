# Grit

**A repo-local, git-backed issue/task system for AI coding agents and humans.**

Grit keeps an append-only event log in git refs, builds a fast local materialized view, and never writes tracked state into the working tree. All state syncs via standard `git fetch/push`.

## Features

<div class="grid cards" markdown>

- **Git-native storage** - Events stored in `refs/grit/wal`, synced with standard git
- **CRDT-based merging** - Deterministic conflict resolution, no manual merge needed
- **Per-actor isolation** - Each agent/device gets its own actor ID and local database
- **Optional daemon** - Auto-spawns for performance, not required for correctness
- **Ed25519 signing** - Optional cryptographic signatures on events
- **Team coordination** - Distributed locks for coordinated workflows

</div>

## Quick Install

```bash
curl -fsSL https://raw.githubusercontent.com/neul-labs/grit/main/install.sh | bash
```

See [Installation](getting-started/installation.md) for other methods including Homebrew, Cargo, npm, pip, and gem.

## Quick Start

```bash
# Initialize grit in a git repository
cd your-repo
grit init

# Create an issue
grit issue create --title "Fix login bug" --body "Users can't login"

# List issues
grit issue list

# Sync with remote
grit sync
```

See [Quick Start](getting-started/quickstart.md) for a complete walkthrough.

## Who Is Grit For?

| Audience | Primary Use Cases |
|----------|-------------------|
| [AI Coding Agents](use-cases/ai-agents.md) | Task decomposition, multi-agent coordination, persistent memory |
| [Individual Developers](use-cases/developers.md) | Offline issue tracking, personal task lists, technical debt |
| [Development Teams](use-cases/teams.md) | Distributed coordination, code review workflows, knowledge base |
| [Security & Compliance](use-cases/security.md) | Private vulnerability tracking, incident response, audit trails |
| [DevOps](use-cases/devops.md) | CI/CD integration, release checklists, deployment tracking |

## Architecture

Grit uses a three-layer architecture:

```
+------------------+     +-------------------+     +------------------+
|   Git WAL        | --> | Materialized View | <-- | CLI / Daemon     |
| refs/grit/wal    |     | sled database     |     | grit / gritd     |
| (source of truth)|     | (fast queries)    |     | (user interface) |
+------------------+     +-------------------+     +------------------+
```

Learn more in [Architecture](architecture/index.md).

## Design Principles

1. **Git is the source of truth** - All state derivable from `refs/grit/*`
2. **No working tree pollution** - Never writes tracked files (except AGENTS.md for agent discoverability)
3. **Daemon optional** - CLI works standalone, daemon is performance optimization
4. **Deterministic merges** - CRDT semantics, no manual conflict resolution
5. **Per-actor isolation** - Multiple agents can work independently

## Next Steps

- [Installation](getting-started/installation.md) - Get grit installed on your system
- [Quick Start](getting-started/quickstart.md) - Create your first issue
- [Core Concepts](getting-started/concepts.md) - Understand how grit works
- [CLI Reference](reference/cli.md) - Full command documentation
