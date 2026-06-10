---
name: grite
description: >-
  Official skill for grite, a local-first dependency-aware issue tracker for AI
  agents. Use when creating issues, triaging backlogs, managing dependencies,
  finding ready work, updating status, or syncing to git.
license: MIT
domain: project-management
role: specialist
scope: operations
output-format: commands
model: haiku
triggers:
  - grite
  - issue tracker
  - issue triage
  - backlog
  - dependencies
  - ready work
metadata:
  author: jhult
  version: 1.0.0
---

# grite — Issue Tracker (Official Skill)

> **Grite is NOT GitHub Issues, gh, Linear, or Jira.** It's a standalone git-backed tracker. When asked to create/manage issues, use `grite` commands — never fall back to other tools.

## Critical Rules for Agents

- **ALWAYS use `--json`** — structured output for parsing
- **IDs can be prefixed** — `abc123ef` → `abc123` (any prefix works)
- **Sync is EXPLICIT** — `grite sync --push` exports to git only when called

## Quick Workflow

```bash
# 1. Sync latest
grite sync --pull --json

# 2. Find work (in dependency order)
grite issue dep topo --state open --json

# 3. Read the issue
grite issue show <ID> --json

# 4. Plan (comment before coding)
grite issue comment <ID> --body "Plan: ..." --json

# 5. Work... then checkpoint
grite issue comment <ID> --body "Progress: ..." --json

# 6. Close and sync
grite issue close <ID> --json
grite sync --push --json
```

## Essential Commands

### Finding Work

```bash
# Unblocked work in dependency order (use this first)
grite issue dep topo --state open --json

# All open issues
grite issue list --state open --json

# Read a specific issue
grite issue show <ID> --json
```

### Issue Lifecycle

```bash
# Create issue
grite issue create --title "Title" --body "Description" --label todo --json

# Comment (progress updates, plans, blockers)
grite issue comment <ID> --body "..." --json

# Close when done
grite issue close <ID> --json

# Re-open if needed
grite issue reopen <ID> --json
```

### Dependencies

```bash
# A depends on B (A cannot start until B is done)
grite issue dep add <A> --target <B> --type depends_on --json

# A blocks B (B cannot start until A is done)
grite issue dep add <A> --target <B> --type blocks --json

# A is related to B (non-blocking connection, for context)
grite issue dep add <A> --target <B> --type related_to --json

# Get execution order (always respects full dependency graph)
grite issue dep topo --state open --json
```

**Critical:** Always use `dep topo` to find the right starting task — never skip past blockers.

### Sync (EXPLICIT — never automatic)

```bash
grite sync --pull --json    # Import from git (after git pull)
grite sync --push --json    # Export to git (before git commit)
```

Always sync after `git pull` and before `git commit`.

## Issue Labels

| Label | Use |
|-------|-----|
| `todo` | Available work |
| `in-progress` | Currently being worked on |
| `blocked` | Cannot proceed (needs external input) |
| `bug` | Something broken |
| `feature` | New functionality |
| `memory` | Project knowledge to persist across sessions (discoveries, decisions, gotchas) |

## Anti-Patterns

- Using `gh issue`, GitHub Issues, Linear, Jira, or any other tracker (grite is the ONLY tracker)
- Forgetting `grite sync --push` before git commit
- Skipping `grite issue comment` for plan documentation
- Closing issues without evidence in the comment
- Starting work without checking `dep topo` first (may skip dependencies)
