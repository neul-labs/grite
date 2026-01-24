# Comparison with Alternatives

## Overview

| Tool | Storage | Sync | CRDT | Agent-Optimized |
|------|---------|------|------|-----------------|
| **Grit** | Event-sourced WAL in git refs | Git push/pull + CRDT merge | Yes | Yes |
| **Beads** | JSONL files + SQLite | Git commit/export | No | Yes |
| **git-bug** | Git objects | Git push/pull | Partial | No |
| **Trekker** | SQLite only | None (local) | No | Yes (MCP) |

## Grit vs Beads

**Architecture:** Grit uses an append-only event log with formal CRDT merging (LWW + add/remove sets). Beads uses mutable JSONL records with hash-based IDs to avoid collisions.

**Sync:** Grit operates on git refs (never touches worktree), with auto-rebase on conflict. Beads syncs JSONL files in the worktree and has documented race conditions ([#1208](https://github.com/steveyegge/beads/issues/1208), [#1258](https://github.com/steveyegge/beads/issues/1258)).

**Daemon:** Grit's daemon is optional; CLI always works standalone. Beads requires a daemon for sync, which has stack overflow issues on multiple platforms ([#1202](https://github.com/steveyegge/beads/issues/1202), [#1288](https://github.com/steveyegge/beads/issues/1288)).

**Integrity:** Grit uses BLAKE2b-256 content-addressed event IDs with optional Ed25519 signing. Beads uses hash-based IDs without cryptographic verification.

**Dependencies:** Grit has typed edges (blocks, depends_on, related_to) with DFS cycle detection and topological ordering. Beads has four dependency types but no cycle detection.

**Context:** Grit includes a built-in symbol extractor and queryable context store that syncs via CRDT. Beads uses LLM-powered compaction (`bd compact`) for memory management.

**Where Beads is ahead:** Hierarchical IDs (epics/subtasks), LLM compaction, Linear/Jira bridges, larger ecosystem, multi-role modes.

## Grit vs git-bug

git-bug stores issues as git objects (not files) and provides CLI/TUI/Web UI with bridges to GitHub/GitLab. It is not agent-optimized (no JSON-first output, no agent playbook, no context store, no dependency DAG).

## Grit vs Trekker

Trekker is SQLite-only (no distributed sync) with a native MCP plugin for Claude Code. Simpler setup but limited to single-machine, single-agent workflows.

## When to Choose Grit

- CRDT-correct distributed merging for multi-agent workflows
- Cryptographic event integrity (signing, content-addressed IDs)
- Dependency DAG with cycle detection and topological ordering
- Distributed context store for codebase understanding
- Append-only safety (no sync-induced data loss)

## When to Choose Alternatives

- **Beads:** Hierarchical epics, LLM compaction, Linear/Jira bridges
- **git-bug:** GitHub/GitLab bridges, TUI/Web UI
- **Trekker:** Simplest setup, native MCP for Claude Code
