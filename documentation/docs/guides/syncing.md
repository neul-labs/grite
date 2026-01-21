# Syncing with Remotes

This guide explains how to synchronize your grit issues with remote repositories.

## Overview

Grit stores all data in git refs (`refs/grit/*`), which sync via standard `git fetch/push`. This means:

- Issues travel with your repository
- Works with any git remote (GitHub, GitLab, etc.)
- Automatic conflict resolution via CRDT merging

## Full Sync

The simplest way to sync is a full sync:

```bash
grit sync
```

This performs:

1. **Pull**: Fetch `refs/grit/*` from the remote
2. **Merge**: Apply new events to local database
3. **Push**: Push local changes to the remote

## Pull Only

To only fetch changes from the remote:

```bash
grit sync --pull
```

Use this when you want to see what others have done without pushing your changes.

## Push Only

To only push your changes:

```bash
grit sync --push
```

Use this when you're confident your changes won't conflict and want a quick push.

## Specifying a Remote

By default, grit syncs with `origin`. To use a different remote:

```bash
grit sync --remote upstream
```

## Handling Conflicts

### Auto-Rebase

When a push fails because the remote has newer commits, grit automatically resolves the conflict:

1. Records your local head
2. Attempts push
3. If rejected (non-fast-forward), pulls remote changes
4. Identifies your local-only events
5. Re-appends your events on top of remote head
6. Pushes again

The output shows when this happens:

```
Conflict resolved: rebased 3 local events on top of remote
Pushed to origin
```

### Why This Works

Grit uses CRDT semantics, so all events are commutative:

- **Last-writer-wins** for title, body, state
- **Add/remove sets** for labels, assignees
- **Append-only** for comments

No manual conflict resolution is ever needed.

## Concurrent Agents

Multiple agents can work on the same repository safely:

### Same Machine

Each agent should use its own actor:

```bash
# Agent 1
grit actor init --label "agent-1"

# Agent 2 (different terminal/process)
grit actor init --label "agent-2"
```

### Different Machines

Just sync regularly:

```bash
# Machine A makes changes
grit issue create --title "Task from A"
grit sync --push

# Machine B pulls and makes changes
grit sync --pull
grit issue create --title "Task from B"
grit sync --push

# Machine A pulls the combined state
grit sync --pull
```

## JSON Output

Get structured sync results:

```bash
grit sync --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "pulled": true,
    "pushed": true,
    "wal_head": "abc123...",
    "remote_wal_head": "abc123..."
  }
}
```

## Retry Behavior

When the remote rejects a push:

1. Grit automatically retries with exponential backoff
2. Maximum retries: 3
3. Backoff: 100ms, 200ms, 400ms

If all retries fail, an error is returned with suggestions.

## Offline Workflow

Grit works fully offline:

```bash
# Work offline
grit issue create --title "Offline task 1"
grit issue create --title "Offline task 2"
grit issue close <id>

# Later, when connected
grit sync
```

All your changes sync when you're back online.

## Best Practices

### Sync Before Starting Work

```bash
grit sync --pull
# Now work with latest state
```

### Sync After Completing Tasks

```bash
grit issue close <id>
grit sync --push
```

### Use Full Sync for Safety

```bash
# Most reliable approach
grit sync  # pull + push
```

### Handle Errors Gracefully

```bash
if ! grit sync --push; then
  echo "Sync failed, trying full sync..."
  grit sync
fi
```

## Common Issues

### "Remote rejected push"

**Cause**: Someone else pushed while you were working.

**Solution**: Run `grit sync` (full sync) to pull and auto-rebase.

### "Network unreachable"

**Cause**: No internet connection.

**Solution**: Work offline and sync later. Your changes are safe in the local WAL.

### "Remote ref not found"

**Cause**: First sync or remote doesn't have grit refs yet.

**Solution**: This is normal for new repositories. Push will create the refs:

```bash
grit sync --push
```

## Next Steps

- [Actor Identity](actors.md) - Multi-agent coordination
- [Distributed Locks](locking.md) - Coordinate work with locks
- [Operations](../operations/index.md) - Troubleshooting sync issues
