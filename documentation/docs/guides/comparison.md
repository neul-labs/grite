# Comparison with Alternatives

This page compares Grit with other git-backed issue trackers, focusing on architectural differences relevant to AI agent workflows.

## Overview

| Tool | Language | Storage | Sync | Agent-Optimized | Distributed CRDT |
|------|----------|---------|------|-----------------|------------------|
| **Grit** | Rust | Event-sourced WAL in git refs | Git push/pull with CRDT merge | Yes | Yes |
| **Beads** | Go | JSONL files + SQLite cache | Git commit/export/import | Yes | No |
| **git-bug** | Go | Git objects (not files) | Git push/pull | No | Partial |
| **Trekker** | TypeScript | SQLite only | None (local only) | Yes | No |
| **git-issue** | Shell | Text files in git | Git push/pull | No | No |

---

## Beads (bd)

[Beads](https://github.com/steveyegge/beads) is a git-backed task graph created by Steve Yegge, designed for AI coding agents. It stores issues as JSONL files in a `.beads/` directory with a SQLite cache for fast queries.

### Architecture Comparison

| Aspect | Grit | Beads |
|--------|------|-------|
| Data model | Event-sourced (append-only event log) | Mutable JSONL records |
| Cache layer | sled (embedded, rebuild-able) | SQLite (primary query layer) |
| Source of truth | Git refs (`refs/grit/wal`) | `.beads/` JSONL files in worktree |
| Merge strategy | Formal CRDT (LWW + add/remove sets) | Hash-based IDs to avoid collisions |
| Event integrity | BLAKE2b-256 content-addressed IDs | Hash-based IDs |
| Signing | Optional Ed25519 per-event | None |
| Daemon | Optional (CLI always works standalone) | Required for auto-sync |
| Dependencies | blocks, depends_on, related_to + cycle detection | blocks, related, parent-child, discovered-from |
| Context/memory | Built-in symbol index (context store) | LLM-powered compaction (`bd compact`) |
| Hierarchy | Flat issues with labels + dependency DAG | Hierarchical IDs (`bd-a3f8.1.1`) |
| Binary size | Single static Rust binary | Go binary + npm package |

### Where Grit's Architecture Differs

**Event sourcing vs mutable records.** Grit's append-only event log means state is always derivable from the event history. There is no "current state" file that can become inconsistent. The sled database is purely a cache and can be deleted and rebuilt at any time (`grit rebuild`).

**Formal CRDT merging.** Grit uses mathematically proven CRDT strategies (last-writer-wins for scalars, add/remove sets for collections) with deterministic tie-breaking by `(timestamp, actor, event_id)`. This guarantees convergence without manual conflict resolution. Beads relies on hash-based IDs to avoid collisions but does not formally resolve concurrent mutations to the same record.

**Sync reliability.** Grit's sync operates on git refs (not worktree files), uses auto-rebase on non-fast-forward pushes, and never touches the user's working directory. Beads has documented sync race conditions ([#1208](https://github.com/steveyegge/beads/issues/1208)) and cases where sync reverts non-beads files ([#1258](https://github.com/steveyegge/beads/issues/1258)).

**Daemon optionality.** Grit's CLI works standalone without a daemon. The daemon is an optional performance optimization. Beads' daemon is required for auto-sync and has experienced stack overflow crashes on multiple platforms ([#1202](https://github.com/steveyegge/beads/issues/1202), [#1224](https://github.com/steveyegge/beads/issues/1224), [#1288](https://github.com/steveyegge/beads/issues/1288)).

**Dependency cycle detection.** Grit performs DFS-based cycle detection at command time for `blocks` and `depends_on` edges, preventing invalid DAG states. It also provides topological ordering (Kahn's algorithm) for execution planning.

**Context store.** Grit includes a built-in regex-based symbol extractor that indexes source files and provides a queryable symbol index. This is CRDT-backed and syncs between agents automatically. Beads relies on LLM-powered compaction (`bd compact`) for memory management but has no structured code understanding.

### Where Beads Is Ahead

**Hierarchical IDs.** Beads supports epics and subtasks with nested IDs (`bd-a3f8.1.1`). Grit uses flat issues organized by labels and the dependency DAG.

**LLM-powered compaction.** `bd compact` uses an LLM to summarize closed issues, reducing context window usage. Grit uses deterministic snapshots but does not (yet) offer semantic summarization.

**Integration bridges.** Beads has Linear and Jira integrations. Grit does not currently bridge to external issue trackers.

**Ecosystem.** Beads has a larger community (12k+ stars, 180+ contributors) with community-built TUIs, web UIs, and editor extensions. Grit is newer and smaller.

**Multi-role modes.** Beads supports stealth (local-only), contributor (separate repo), and maintainer (direct) modes. Grit uses actor isolation for multi-agent scenarios but doesn't have role-based modes.

### Known Beads Issues That Grit's Architecture Avoids

| Beads Issue | Root Cause | Grit's Approach |
|-------------|-----------|-----------------|
| Stack overflow in daemon ([#1202](https://github.com/steveyegge/beads/issues/1202), [#1288](https://github.com/steveyegge/beads/issues/1288)) | Recursive lock acquisition | Optional daemon, flock-based exclusion |
| Sync race condition ([#1208](https://github.com/steveyegge/beads/issues/1208)) | Export-after-commit race | Append-only WAL, atomic ref updates |
| Sync reverts files ([#1258](https://github.com/steveyegge/beads/issues/1258)) | Sync touches worktree | Operates on git refs only, never touches worktree |
| Config ignored ([#1235](https://github.com/steveyegge/beads/issues/1235)) | Complex config resolution | Simple actor-based config in `.git/grit/` |
| Can't import on second machine ([#1275](https://github.com/steveyegge/beads/issues/1275)) | SQLite/JSONL sync mismatch | sled is derived from WAL, rebuild works anywhere |
| Platform lock errors ([#1224](https://github.com/steveyegge/beads/issues/1224)) | SQLite WAL locking on WSL2 | sled + flock, platform-tested |

---

## git-bug

[git-bug](https://github.com/git-bug/git-bug) is a distributed, offline-first bug tracker that stores issues as git objects (commits, trees, blobs) rather than files. It provides CLI, TUI, and Web UI interfaces.

### How It Compares

| Aspect | Grit | git-bug |
|--------|------|---------|
| Storage | CBOR WAL in git refs | Git objects (DAG-based) |
| Language | Rust | Go |
| Merge | Deterministic CRDT with total ordering | Git-native merging |
| Agent support | JSON output, context store, agent playbook | Not agent-optimized |
| Bridges | None yet | GitHub, GitLab, Jira |
| UI | CLI only | CLI + TUI + Web UI |
| Signing | Ed25519 per-event | GPG commit signing |

### Key Differences

**Agent optimization.** git-bug is designed for human developers. It has no JSON-first output mode, no agent playbook, no context store, and no dependency DAG. Grit is designed for AI agents from the ground up.

**CRDT correctness.** git-bug uses git's merge mechanisms. Grit uses formal CRDT semantics with deterministic tie-breaking, guaranteeing convergence regardless of event arrival order.

**Bridges.** git-bug's primary advantage is its bridge system for syncing with GitHub, GitLab, and Jira. Grit does not currently offer this.

**UI.** git-bug has a TUI and Web UI. Grit is CLI-only (planned MCP server for IDE integration).

---

## Trekker

[Trekker](https://github.com/obsfx/trekker) is a lightweight issue tracker built with TypeScript/Bun, using SQLite for storage. It provides a Claude Code MCP plugin for native agent integration.

### How It Compares

| Aspect | Grit | Trekker |
|--------|------|---------|
| Storage | Git-backed (distributed) | SQLite only (local) |
| Sync | Git push/pull with CRDT | None |
| Language | Rust | TypeScript (Bun) |
| Agent integration | CLI + agent playbook | MCP plugin (native) |
| Multi-agent | Actor isolation + CRDT merge | Single-agent only |
| Dependencies | DAG with cycle detection | Basic dependency tracking |
| Output format | JSON | JSON + TOON (token-efficient) |

### Key Differences

**No distributed sync.** Trekker is local-only. There is no way to sync issues between machines or agents on different hosts. Grit syncs via git, making multi-machine and multi-agent workflows possible.

**MCP-native.** Trekker's primary advantage is its MCP plugin for Claude Code, giving agents direct tool access without shelling out to CLI commands. Grit plans MCP support but currently uses CLI.

**Simplicity.** Trekker's philosophy is "a task tracker is a simple application." It trades distributed capabilities for ease of setup and minimal complexity.

---

## Other Tools

### Ticket

A [single-file bash script](https://news.ycombinator.com/item?id=46487580) using flat markdown files. Created by a user frustrated with Beads' growing complexity. Retains graph-based dependencies but drops everything else. Represents the "minimal viable" end of the spectrum.

### git-issue

[git-issue](https://github.com/dspinellis/git-issue) is a shell-based decentralized issue manager. Issues are stored as text files, synced via git. No agent optimization, no structured output, no dependency graph.

### TrackDown

[TrackDown](https://github.com/mgoellnitz/trackdown) embeds issues as markdown in source code. Lightweight but no CLI, no structured queries, no multi-agent support.

---

## When to Choose Grit

- **CRDT-correct distributed merging**: Multiple agents or developers need to work on the same issue set without conflicts
- **Cryptographic event integrity**: Event signing and content-addressed IDs matter (audit trails, compliance)
- **Dependency DAG**: Typed relationships with cycle detection and topological ordering for task prioritization
- **Context store**: Agents need structured codebase understanding that syncs between team members
- **Daemon-optional**: CLI must work reliably standalone without background processes
- **Append-only safety**: No risk of data loss from sync races or file corruption

## When to Choose Alternatives

- **Beads**: You need hierarchical epics/subtasks, LLM-powered context compaction, or Linear/Jira integration
- **git-bug**: You need GitHub/GitLab bridges, a TUI, or a Web UI for human-centric workflows
- **Trekker**: You want the simplest possible setup with native MCP integration for Claude Code, and don't need distributed sync
- **Ticket/git-issue**: You want the absolute minimum tooling with no dependencies beyond bash/git
