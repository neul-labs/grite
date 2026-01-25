# Git Worktrees

Grit fully supports git worktrees, enabling multi-agent workflows where each agent works on a different branch in its own worktree while sharing the same issue state.

## Overview

Git worktrees allow you to have multiple working directories attached to the same repository, each checked out to a different branch. Grit's worktree support means:

- **Shared state**: All worktrees share the same issues, events, locks, and context
- **No configuration needed**: Grit automatically detects worktrees and uses the correct shared storage
- **Daemon compatible**: The daemon works correctly across worktrees

## How It Works

When you run grit from within a worktree, it automatically discovers the main repository's `.git` directory (the "commondir") and stores all data there:

```
/project/                    # Main repo
├── .git/
│   ├── grit/               # Shared grit data
│   │   ├── config.toml
│   │   └── actors/
│   └── worktrees/
│       └── feature/        # Worktree metadata
│
/project-feature/            # Worktree
└── .git                     # File (not directory) pointing to main repo
```

This means:
- `refs/grit/wal` — shared across all worktrees
- `refs/grit/locks/*` — shared, preventing conflicts
- `.git/grit/actors/` — shared actor configurations
- Context store — shared, with LWW (last-writer-wins) semantics

## Basic Usage

### Setting Up

```bash
# Initialize grit in main repo
cd /project
grit init
grit issue create --title "Implement feature X"

# Create a worktree for feature development
git worktree add ../project-feature -b feature-x

# Work from the worktree
cd ../project-feature
grit issue list  # Shows issues from main repo
```

### Multi-Agent Workflow

This is ideal for running multiple AI agents in parallel:

```bash
# Terminal 1: Agent A works on feature-x
cd /project-feature-x
grit issue list --label sprint-1 --json
# Agent picks issue, starts working...

# Terminal 2: Agent B works on feature-y
cd /project-feature-y
grit issue list --label sprint-1 --json
# Agent picks different issue, starts working...

# Both agents share the same issue state
# Comments, status updates, and locks are visible to both
```

### Using Locks for Coordination

When multiple agents work simultaneously, use locks to prevent conflicts:

```bash
# Agent A acquires lock before modifying issue
cd /project-feature-x
grit lock acquire --resource issue:abc123 --ttl 15m
grit issue update abc123 --title "Updated title"
grit lock release --resource issue:abc123

# Agent B sees the lock
cd /project-feature-y
grit lock status  # Shows Agent A's lock
```

## Context Indexing in Worktrees

The context store indexes files from the current working directory:

```bash
# Index files in the main repo
cd /project
grit context index

# Index files in the worktree (may have different files on feature branch)
cd /project-feature
grit context index  # Indexes worktree's files
```

Since context uses LWW (last-writer-wins), the most recently indexed version wins. If you want branch-specific context, consider:

1. Re-indexing when switching worktrees
2. Using different actors for different worktrees
3. Indexing only the files relevant to your task

## Comparison with Beads

Both Grit and Beads support git worktrees with a shared database architecture:

| Aspect | Grit | Beads |
|--------|------|-------|
| Daemon in worktrees | Works normally | Auto-disabled (uses `--no-daemon`) |
| Database location | Main repo's `.git/grit/` | Main repo's `.beads/` |
| Detection | Automatic via `commondir()` | Automatic via worktree resolution |
| Sync branch | N/A (uses refs) | Creates internal worktrees in `.git/beads-worktrees/` |

## Troubleshooting

### "Not a git repository" Error

If grit fails to detect the repository from a worktree:

1. Verify the `.git` file exists and contains a valid `gitdir:` path
2. Check that the main repository's `.git` directory is accessible
3. Try running `git status` to verify git itself works

### Database Lock Errors

If you get "database busy" errors across worktrees:

1. Ensure only one daemon is running per repository
2. Use `grit --no-daemon` for direct access
3. Check for stale lock files with `grit daemon status`

### Context Store Conflicts

If context seems outdated:

1. Re-run `grit context index` in your current worktree
2. Use `grit context index --force` to override cached hashes
3. Remember that context uses LWW — the latest index wins

## Best Practices

1. **Use locks for coordinated work**: When multiple agents modify the same issues, use locks to prevent conflicts.

2. **One daemon per repository**: The daemon is shared across worktrees; don't try to start multiple daemons.

3. **Re-index after branch switches**: If your worktrees are on branches with different file content, re-index to update the context store.

4. **Use distinct actors for different workflows**: While actors are shared, you can create multiple actors for different purposes (e.g., one per worktree).
