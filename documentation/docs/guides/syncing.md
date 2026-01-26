# Syncing with Remotes

This guide explains how to synchronize your grite issues with remote repositories.

## Overview

Grite stores all data in git refs (`refs/grite/*`), which sync via standard `git fetch/push`. This means:

- Issues travel with your repository
- Works with any git remote (GitHub, GitLab, etc.)
- Automatic conflict resolution via CRDT merging

## Full Sync

The simplest way to sync is a full sync:

```bash
grite sync
```

This performs:

1. **Pull**: Fetch `refs/grite/*` from the remote
2. **Merge**: Apply new events to local database
3. **Push**: Push local changes to the remote

## Pull Only

To only fetch changes from the remote:

```bash
grite sync --pull
```

Use this when you want to see what others have done without pushing your changes.

## Push Only

To only push your changes:

```bash
grite sync --push
```

Use this when you're confident your changes won't conflict and want a quick push.

## Specifying a Remote

By default, grite syncs with `origin`. To use a different remote:

```bash
grite sync --remote upstream
```

## Handling Conflicts

### Auto-Rebase

When a push fails because the remote has newer commits, grite automatically resolves the conflict:

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

Grite uses CRDT semantics, so all events are commutative:

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
grite actor init --label "agent-1"

# Agent 2 (different terminal/process)
grite actor init --label "agent-2"
```

### Different Machines

Just sync regularly:

```bash
# Machine A makes changes
grite issue create --title "Task from A"
grite sync --push

# Machine B pulls and makes changes
grite sync --pull
grite issue create --title "Task from B"
grite sync --push

# Machine A pulls the combined state
grite sync --pull
```

## JSON Output

Get structured sync results:

```bash
grite sync --json
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

1. Grite automatically retries with exponential backoff
2. Maximum retries: 3
3. Backoff: 100ms, 200ms, 400ms

If all retries fail, an error is returned with suggestions.

## Offline Workflow

Grite works fully offline:

```bash
# Work offline
grite issue create --title "Offline task 1"
grite issue create --title "Offline task 2"
grite issue close <id>

# Later, when connected
grite sync
```

All your changes sync when you're back online.

## Best Practices

### Sync Before Starting Work

```bash
grite sync --pull
# Now work with latest state
```

### Sync After Completing Tasks

```bash
grite issue close <id>
grite sync --push
```

### Use Full Sync for Safety

```bash
# Most reliable approach
grite sync  # pull + push
```

### Handle Errors Gracefully

```bash
if ! grite sync --push; then
  echo "Sync failed, trying full sync..."
  grite sync
fi
```

## Common Issues

### "Remote rejected push"

**Cause**: Someone else pushed while you were working.

**Solution**: Run `grite sync` (full sync) to pull and auto-rebase.

### "Network unreachable"

**Cause**: No internet connection.

**Solution**: Work offline and sync later. Your changes are safe in the local WAL.

### "Remote ref not found"

**Cause**: First sync or remote doesn't have grite refs yet.

**Solution**: This is normal for new repositories. Push will create the refs:

```bash
grite sync --push
```

## Next Steps

- [Actor Identity](actors.md) - Multi-agent coordination
- [Distributed Locks](locking.md) - Coordinate work with locks
- [Operations](../operations/index.md) - Troubleshooting sync issues
