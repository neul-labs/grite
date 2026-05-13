# Grite

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/grite.svg)](https://crates.io/crates/grite)
[![Build Status](https://img.shields.io/github/actions/workflow/status/neul-labs/grite/ci.yml?branch=main)](https://github.com/neul-labs/grite/actions)
[![Documentation](https://img.shields.io/badge/docs-neullabs.com-green.svg)](https://docs.neullabs.com/grite)
[![npm](https://img.shields.io/npm/v/grite-cli)](https://www.npmjs.com/package/grite-cli)
[![PyPI](https://img.shields.io/pypi/v/grite-cli)](https://pypi.org/project/grite-cli/)
[![RubyGems](https://img.shields.io/gem/v/grite-cli)](https://rubygems.org/gems/grite-cli)

**The issue tracker that lives in your repo. Built for AI agents. Works for humans.**

Grite stores issues as an append-only event log inside git refs (`refs/grite/wal`), keeping your working tree pristine while enabling seamless multi-agent collaboration through deterministic CRDT-based conflict resolution. No servers. No databases. No merge conflicts. Just git.

---

## What is Grite?

Grite is a **repo-local, git-backed issue and task tracking system** that solves one of the most pressing problems in modern software development: how do multiple AI coding agents (and humans) coordinate work on the same codebase without stepping on each other, losing context, or creating conflicting state?

Traditional issue trackers live on external servers. They require API calls, authentication tokens, network connectivity, and manual conflict resolution when two people edit the same issue. They pollute your development workflow with context switches and external dependencies.

Grite embeds directly in your git repository. Your issues travel with your code. They branch when you branch. They merge when you merge. They sync when you `git push`. This is not just convenience — it is a fundamental architectural shift that makes issue tracking native to the development process rather than an external concern.

### The Grite Difference

| | Traditional Trackers | Grite |
|---|---|---|
| **Storage** | External database | Git refs in your repo |
| **Sync** | API calls | `git fetch` / `git push` |
| **Offline** | Requires connectivity | Works entirely offline |
| **Multi-agent** | Rate limits, auth tokens | CRDT merge, no conflicts |
| **Working tree** | N/A | Completely clean |
| **History** | Audit logs (optional) | Immutable event log (built-in) |
| **Agent-native** | Bolted-on integrations | Designed for agents from day one |

---

## Why Grite?

### The Problem: Agents Need Persistent Memory

AI coding agents are transforming software development, but they face a critical limitation: **memory**. An agent starts fresh every session. It forgets what it was working on. It cannot see what other agents have done. It has no way to persist learnings, plans, or task state across invocations.

The existing solutions are inadequate:

- **External issue trackers** add latency, require credentials, and create a hard dependency on internet connectivity and third-party services.
- **State files in the repo** pollute the working tree, create merge conflicts, and clutter code reviews with non-code changes.
- **Chat context windows** are limited, ephemeral, and not shareable between agents or sessions.
- **Database-backed trackers** require setup, maintenance, backups, and network access.

### The Solution: Git-Native Coordination

Grite provides **persistent, shared, conflict-free memory** for any number of agents working on the same codebase:

- **Persistent task memory** — Agents track what they are doing across sessions. Create an issue, shut down, resume later, and the state is exactly as you left it.
- **Multi-agent coordination** — Distributed locks prevent conflicting work. Agent A claims a refactoring task; Agent B sees it is taken and picks something else.
- **Zero-config sync** — If you can `git push`, you can sync issues. No new tools to learn, no new accounts to manage, no new infrastructure to maintain.
- **Context extraction** — Tree-sitter powered symbol extraction across 10 programming languages helps agents understand the codebase and create richer, more contextual issues.
- **Agent discoverability** — `grite init` creates `AGENTS.md`, a convention that AI coding agents read automatically. Agents discover grite without any manual configuration.
- **Memory accumulation** — Agents store learnings, decisions, and architectural notes as labeled issues. These survive forever and are visible to all future agents.

---

## Performance

Grite is engineered for speed. Every millisecond counts when an agent is making hundreds of queries per session.

| Operation | Time | Notes |
|-----------|------|-------|
| Issue create | ~5ms | Single event append to WAL |
| Issue list (1,000 issues) | ~10ms | Materialized view query from sled |
| Full rebuild (10,000 events) | ~500ms | Replay entire WAL from git refs |
| Snapshot rebuild | ~50ms | Jump to last snapshot, replay delta |
| Sync (1,000 new events) | ~200ms | Network-bound `git fetch` + CRDT merge |
| Context extraction | ~20ms | Tree-sitter parse + symbol index |

**Memory footprint:**
- CLI binary: ~15MB RSS (single operation, exits immediately)
- Daemon process: ~30MB RSS (warm cache, handles concurrent requests)
- Per-actor database: ~1KB per issue (sled embedded key-value store)
- Total repo overhead: ~50KB for 100 issues (git refs are extremely compact)

**Concurrency:**
- Local: 50+ concurrent CLI calls via daemon (serialized writes, parallel reads)
- Distributed: 100+ agents across machines (each gets an isolated actor ID, CRDT merge on sync)
- Lock contention: Distributed locks with configurable TTL for exclusive resource access
- WAL throughput: 1,000+ events per second append rate

**Scaling:**
Tested with 100,000+ events and 10,000+ issues. The materialized view (sled) provides O(1) lookups by issue ID. The git WAL provides O(n) rebuild complexity, but periodic snapshots reduce this to O(delta). The CRDT merge algorithm is linear in the number of new events.

---

## Features

### Core Capabilities

- **Git-native storage** — Events stored in `refs/grite/wal`, synced with standard `git fetch` / `git push`. Your issues are version-controlled.
- **CRDT-based merging** — Deterministic conflict resolution using Last-Write-Wins and commutative-set semantics. Two agents edit the same issue simultaneously? Both changes are preserved. No manual merge needed. No data loss.
- **Dependency DAG** — Typed issue relationships (`blocks`, `depends_on`, `related_to`) with automatic cycle detection and topological ordering. Plan complex multi-step refactors with confidence.
- **Context store** — Tree-sitter-powered symbol extraction across 10 languages (Rust, Python, TypeScript, JavaScript, Go, Java, C, C++, Ruby, Elixir). Agents can query "what functions reference X?" and create issues with rich code context.
- **Per-actor isolation** — Each agent or device gets its own actor ID, local database, and signing key. Multiple agents can work independently and merge later without coordination.
- **Optional background daemon** — Auto-spawns for performance, completely optional for correctness. When running, the daemon keeps databases warm and handles concurrent access efficiently.
- **Ed25519 event signing** — Optional cryptographic signatures on every event. Verify who created what, when, with non-repudiable provenance.
- **Distributed locking** — Coordinate exclusive access to resources across agents. Claim a file, a module, or a refactoring task with automatic TTL-based lease expiration.
- **Snapshot optimization** — Periodic snapshots of the materialized view let you rebuild state in milliseconds instead of replaying the entire WAL.
- **Export and reporting** — Export issues to JSON, Markdown, or CSV. Generate changelogs, sprint reports, or audit trails.

### For AI Coding Agents

- **AGENTS.md auto-generation** — `grite init` creates a standard file that tells any AI agent how to use grite as its memory system.
- **JSON output everywhere** — Every CLI command supports `--json` for machine parsing.
- **Checkpoint comments** — Agents post progress updates as comments on issues, creating an automatic audit trail.
- **Memory labels** — Store learnings and architectural decisions with `--label memory` for cross-session knowledge transfer.
- **Agent identity** — Each agent gets a persistent actor ID, so you can track which agent did what.

### For Developers and Teams

- **Offline-first** — Create, edit, and query issues without any network connection. Sync when you are back online.
- **Branch-aware** — Issues created on a feature branch stay on that branch. Merge the branch, merge the issues.
- **Worktree support** — Works seamlessly with `git worktree`. Each worktree gets its own actor database.
- **Fast queries** — The sled materialized view makes `grite issue list` instant even with thousands of issues.
- **Health monitoring** — `grite doctor` checks database integrity, WAL consistency, and actor configuration. Auto-repair with `--fix`.

---

## Use Cases

Grite serves distinct audiences with tailored workflows:

### AI Coding Agents

Agents use grite for **task decomposition**, **multi-agent coordination**, and **persistent memory**:

1. User asks Agent A to "refactor the auth module"
2. Agent A creates an issue, claims the lock, and begins working
3. Agent B (running in parallel) sees the lock and picks a different task
4. Agent A stores learnings about the auth module as a memory issue
5. Next session, Agent C reads the memory and knows exactly how auth works

No external APIs. No lost context. No conflicting edits.

### Individual Developers

Developers use grite for **offline issue tracking**, **personal task lists**, and **technical debt management**:

- Track bugs and TODOs without leaving the terminal
- Maintain a personal knowledge base of architectural decisions
- Create issues while on a plane, sync when you land
- Never lose a bug report because it was in a tab you closed

### Development Teams

Teams use grite for **distributed coordination**, **code review workflows**, and **knowledge sharing**:

- Every developer's issues sync through the same git remote they already use
- No new infrastructure to maintain, no new accounts to provision
- Issues branch and merge with the code they describe
- Full audit trail of who changed what and when, cryptographically signed

### Security and Compliance

Security teams use grite for **private vulnerability tracking**, **incident response**, and **audit trails**:

- Sensitive issues never leave your infrastructure
- Ed25519 signatures provide cryptographic proof of who reported what
- The append-only WAL is tamper-evident — any modification breaks the hash chain
- Export audit trails in standard formats for compliance reporting

### DevOps and Release Engineering

DevOps teams use grite for **CI/CD integration**, **release checklists**, and **deployment tracking**:

- Track deployment steps as a dependency DAG
- Coordinate rollbacks with distributed locks
- Store post-mortems as issues with full context
- Generate changelogs from closed issues between releases

See [Use Cases](docs/use-cases.md) for detailed workflows, command examples, and integration patterns.

---

## Installation

Grite is distributed as a standalone binary with zero runtime dependencies except git.

### Quick Install (Recommended)

```bash
# One-line install — downloads the correct binary for your platform
curl -fsSL https://raw.githubusercontent.com/neul-labs/grite/main/install.sh | bash
```

This installs `grite` and `grite-daemon` to `~/.local/bin/` (or `~/bin/` on macOS).

### Package Managers

```bash
# macOS / Linux via Homebrew
brew install neul-labs/tap/grite

# Rust via Cargo
cargo install grite grite-daemon

# Node.js via npm
npm install -g grite-cli

# Python via pip
pip install grite-cli

# Ruby via RubyGems
gem install grite-cli

# Windows via Chocolatey
choco install grite
```

### Pre-built Binaries

Download pre-built binaries for Linux (x86_64, ARM64, musl), macOS (x86_64, ARM64, Universal), and Windows from the [GitHub Releases](https://github.com/neul-labs/grite/releases) page.

**Prerequisites:** Git 2.38 or later.

### Building from Source

```bash
# Clone the repository
git clone https://github.com/neul-labs/grite.git
cd grite

# Build the release binaries
cargo build --release --package grite --package grite-daemon

# Binaries will be at:
# target/release/grite
# target/release/grite-daemon
```

---

## Quick Start

```bash
# Navigate to any git repository
cd your-project

# Initialize grite (creates AGENTS.md for AI agent discoverability)
grite init

# Create your first issue
grite issue create --title "Fix race condition in WAL append" --body "Intermittent failure under high concurrency."

# List all open issues
grite issue list

# Add a comment with a checkpoint
grite issue comment <issue-id> --body "Reproduced: occurs when two agents append simultaneously."

# Set a label for categorization
grite issue update <issue-id> --label bug --label concurrency

# Create a dependency relationship
grite issue link <issue-a> blocks <issue-b>

# Claim a distributed lock before starting work
grite lock acquire src/wal.rs --ttl 3600

# Close when done
grite issue close <issue-id>

# Sync with remote (automatic CRDT merge on conflict)
grite sync

# Run health checks
grite doctor

# Fast rebuild from snapshot (useful after clone)
grite rebuild --from-snapshot

# Export issues to Markdown for a report
grite export --format markdown --since 7d > sprint-report.md
```

### Agent Workflow Example

```bash
# Agent discovers grite via AGENTS.md and runs startup routine
grite sync --pull
grite issue list --label "agent:todo" --json

# Agent picks a task, claims it, and works
grite issue update <id> --label "agent:in-progress"
grite lock acquire src/parser.rs --ttl 1800

# Agent stores a learning for future sessions
grite issue create --title "Parser edge case: empty structs" \
  --body "The parser fails on `struct {};` — need to handle this." \
  --label memory

# Agent finishes and pushes state
grite issue close <id>
grite sync --push
```

---

## Architecture

Grite uses a three-layer architecture that separates storage, query, and interface concerns:

```
+---------------------+     +----------------------+     +---------------------+
|   Git WAL           | --> | Materialized View    | <-- | CLI / Daemon        |
| refs/grite/wal       |     | sled database        |     | grite / grite-daemon |
| (source of truth)    |     | (fast queries)         |     | (user interface)    |
+---------------------+     +----------------------+     +---------------------+
        |                            |                           |
        v                            v                           v
   Append-only              CRDT projection              IPC via Unix
   event log               with LWW + set              domain sockets
   CBOR-encoded            semantics                 (rkyv zero-copy)
```

### How It Works

1. **Events are appended** to the git-backed WAL as CBOR-encoded chunks, signed with Ed25519 if enabled.
2. **The materialized view** (sled database) rebuilds issue state from events using CRDT semantics. This happens once on startup, then incrementally as new events arrive.
3. **The daemon** (optional) keeps the materialized view warm and handles concurrent IPC requests from the CLI.
4. **Sync** fetches remote WAL refs and merges them deterministically using the CRDT merge function.

### Crate Structure

Grite is organized as a Rust workspace with six crates, each with a single responsibility:

| Crate | Purpose | Standalone Use |
|---|---|---|
| [`libgrite-core`](crates/libgrite-core) | Event types, CRDT projections, hashing, sled storage, Ed25519 signing | Embed the data model and storage engine in any Rust project |
| [`libgrite-git`](crates/libgrite-git) | WAL commits, ref sync, snapshots, distributed locks | Use git as a WAL backend for your own event-sourced system |
| [`libgrite-ipc`](crates/libgrite-ipc) | IPC message schemas (rkyv), daemon lock, client/server | Build custom clients or alternative frontends |
| [`libgrite-cli`](crates/libgrite-cli) | Programmatic API for all CLI operations | Embed grite operations in Rust applications and agent harnesses |
| [`grite`](crates/grite) | CLI frontend binary | End-user command-line interface |
| [`grite-daemon`](crates/grite-daemon) | Background daemon binary | Run the performance daemon independently |

### ID Types

| Type | Size | Format | Purpose |
|------|------|--------|---------|
| `ActorId` | 128-bit | CSPRNG random | Identifies a device or agent instance |
| `IssueId` | 128-bit | CSPRNG random | Identifies an issue or task |
| `EventId` | 256-bit | BLAKE2b hash | Content-addressed, tamper-evident event identifier |

IDs are stored as compact byte arrays internally and displayed as lowercase hex strings. The content-addressed EventId ensures that any modification to an event invalidates its ID, making the WAL tamper-evident.

---

## Daemon

The daemon (`grite-daemon`) is entirely optional but highly recommended for performance. It is not required for correctness — the CLI works standalone, spawning the daemon automatically when beneficial.

### What the Daemon Provides

- **Auto-spawn** — The first CLI command automatically starts the daemon if it is not running. Zero configuration.
- **Idle shutdown** — The daemon stops automatically after 5 minutes of inactivity (configurable). No resource leaks.
- **Concurrent access** — Multiple CLI calls are handled efficiently through a single worker pool. No database lock contention.
- **Warm cache** — The materialized view stays in memory, so queries are instant.
- **Pub/sub notifications** — Future releases will support real-time issue change notifications.

### Manual Control

```bash
# Start with custom idle timeout (seconds)
grite daemon start --idle-timeout 300

# Check status
grite daemon status

# Stop gracefully
grite daemon stop

# Skip daemon for a single command
grite --no-daemon issue list
```

The daemon uses filesystem-level locking (`flock`) to prevent database corruption from concurrent access, even across separate processes.

---

## Storage Layout

Grite stores all state in two places: the `.git/grite/` directory for per-actor materialized views, and git refs for the shared append-only event log.

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
  wal                              # Append-only event log (source of truth)
  snapshots/<ts>                   # Periodic snapshots for fast rebuild
  locks/<resource_hash>            # Distributed lease locks
```

**Key design decisions:**
- The WAL is the only source of truth. The sled database can be deleted and rebuilt at any time.
- Nothing is stored in tracked files (except `AGENTS.md`, which is auto-generated for agent discoverability).
- Each actor has an isolated database, preventing one agent's queries from slowing another.

---

## Documentation

Full documentation is available at [docs.neullabs.com/grite](https://docs.neullabs.com/grite).

| Document | Description |
|----------|-------------|
| [Architecture](docs/architecture.md) | System design, data flow, and component interactions |
| [Use Cases](docs/use-cases.md) | Workflows for agents, developers, teams, and security |
| [Data Model](docs/data-model.md) | Event schema, hashing algorithm, projection semantics |
| [CLI Reference](docs/cli.md) | Complete command-line interface documentation |
| [CLI JSON Output](docs/cli-json.md) | JSON output format for scripting and agent integration |
| [Daemon](docs/daemon.md) | Background daemon architecture and configuration |
| [Actors](docs/actors.md) | Actor identity, isolation, and multi-agent patterns |
| [Configuration](docs/configuration.md) | Config files, environment variables, and defaults |
| [Git WAL](docs/git-wal.md) | WAL format, chunk encoding, and ref structure |
| [IPC Protocol](docs/ipc.md) | Inter-process communication protocol specification |
| [Locking](docs/locking.md) | Distributed lock coordination and lease management |
| [Export Format](docs/export-format.md) | JSON, Markdown, and CSV export specifications |
| [Hash Vectors](docs/hash-vectors.md) | Canonical hashing test vectors for interoperability |
| [Operations](docs/operations.md) | Backup, recovery, debugging, and maintenance |
| [Agent Playbook](docs/agent-playbook.md) | Guide for AI coding agents using grite |
| [Comparison](docs/comparison.md) | How Grite compares with Beads, git-bug, and others |

---

## Design Principles

1. **Git is the source of truth** — All state is derivable from `refs/grite/*`. The materialized view is a cache.
2. **No working tree pollution** — Never writes tracked files (except `AGENTS.md` for agent discoverability).
3. **Daemon optional** — The CLI works standalone. The daemon is a performance optimization, not a requirement.
4. **Deterministic merges** — CRDT semantics guarantee that any two valid WALs can be merged without conflict, and the result is the same regardless of merge order.
5. **Per-actor isolation** — Multiple agents can work independently on the same machine or across the network.
6. **Agent discoverability** — `grite init` creates `AGENTS.md` so AI coding agents automatically discover and use grite.
7. **Offline-first** — Every operation works without network access. Sync is an explicit, optional step.
8. **Cryptographic provenance** — Optional Ed25519 signing makes the audit trail tamper-evident and non-repudiable.

---

## Community and Support

- **GitHub Issues:** [Report bugs and request features](https://github.com/neul-labs/grite/issues)
- **GitHub Discussions:** [Ask questions and share workflows](https://github.com/neul-labs/grite/discussions)
- **Documentation:** [docs.neullabs.com/grite](https://docs.neullabs.com/grite)
- **Crate Docs:** [docs.rs/grite](https://docs.rs/grite)

---

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on code style, testing, and the development workflow.

```bash
# Build everything
cargo build

# Run the full test suite
cargo test

# Run with debug logging
RUST_LOG=debug cargo run --bin grite -- issue list

# Install locally for testing
./install.sh
```

---

## License

MIT License — see [LICENSE](LICENSE) for details.

Grite is built by [Neul Labs](https://neullabs.com).
