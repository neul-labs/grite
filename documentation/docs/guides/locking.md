# Distributed Locks

This guide explains how to use grite's distributed lock system to coordinate work across agents and team members.

## Overview

Grite provides lease-based distributed locks stored as git refs. Locks help coordinate:

- Which agent is working on an issue
- Exclusive access to a file or directory
- Preventing duplicate work

## Lock Concepts

### Lease-Based

Locks have a time-to-live (TTL):

- Locks expire automatically after TTL
- Crashed processes don't hold locks forever
- Active processes must renew locks

### Git-Backed

Locks are stored in `refs/grite/locks/<hash>`:

- Synced via `git fetch/push`
- Visible to all agents
- Survives restarts

## Lock Namespaces

Resources are prefixed with a namespace:

| Namespace | Example | Use Case |
|-----------|---------|----------|
| `issue:` | `issue:8057324b` | Claim an issue |
| `path:` | `path:src/auth.rs` | File editing |
| `repo:` | `repo:deploy` | Repository-wide operations |

## Acquiring Locks

### Basic Acquisition

```bash
grite lock acquire --resource "issue:8057324b" --ttl 30m
```

Output:

```
Lock acquired
  Resource: issue:8057324b
  Expires: 2024-01-15 11:30:00 UTC
```

### TTL Formats

- `15m` - 15 minutes
- `1h` - 1 hour
- `30s` - 30 seconds

### JSON Output

```bash
grite lock acquire --resource "path:src/api.rs" --ttl 1h --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "lock": {
      "resource": "path:src/api.rs",
      "owner": "64d15a2c383e2161772f9cea23e87222",
      "nonce": "abc123...",
      "expires_unix_ms": 1700003600000
    }
  }
}
```

### Handling Conflicts

If the resource is already locked:

```bash
grite lock acquire --resource "issue:8057324b" --ttl 30m
```

```
Error: Lock conflict
  Resource: issue:8057324b
  Owner: e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0
  Expires: 2024-01-15 11:00:00 UTC
```

## Renewing Locks

Extend a lock before it expires:

```bash
grite lock renew --resource "issue:8057324b" --ttl 30m
```

!!! warning
    You can only renew locks you own. Renewing a lock held by another actor fails.

## Releasing Locks

Explicitly release when done:

```bash
grite lock release --resource "issue:8057324b"
```

This is important for good coordination. Don't rely solely on TTL expiration.

## Checking Lock Status

### All Locks

```bash
grite lock status
```

Output:

```
Active Locks:
  issue:8057324b  64d15a2c  expires in 25m
  path:src/api.rs e5f6a7b8  expires in 55m
```

### JSON Output

```bash
grite lock status --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "locks": [
      {
        "resource": "issue:8057324b",
        "owner": "64d15a2c383e2161772f9cea23e87222",
        "expires_unix_ms": 1700001500000
      }
    ],
    "conflicts": []
  }
}
```

## Lock Garbage Collection

Remove expired locks:

```bash
grite lock gc
```

This cleans up `refs/grite/locks/` refs that have expired.

## Lock Policy

Configure lock behavior in `.git/grite/config.toml`:

```toml
lock_policy = "warn"  # off, warn, require
```

| Policy | Behavior |
|--------|----------|
| `off` | No lock warnings or enforcement |
| `warn` | Warn when working on locked resources |
| `require` | Require lock before modifying resources |

## Practical Examples

### Claiming an Issue

```bash
# Check for existing lock
if grite lock acquire --resource "issue:$ID" --ttl 30m --json | jq -e '.ok'; then
  echo "Claimed issue $ID"

  # Do work...
  grite issue comment $ID --body "Working on this"

  # Release when done
  grite issue close $ID
  grite lock release --resource "issue:$ID"
else
  echo "Issue already claimed"
fi
```

### File Editing Coordination

```bash
# Lock file before editing
grite lock acquire --resource "path:config/settings.json" --ttl 15m

# Edit file...

# Release after commit
git add config/settings.json
git commit -m "Update settings"
grite lock release --resource "path:config/settings.json"
```

### Long-Running Task

```bash
# Acquire with longer TTL
grite lock acquire --resource "repo:migration" --ttl 2h

# Periodically renew during long task
while migration_in_progress; do
  sleep 300  # 5 minutes
  grite lock renew --resource "repo:migration" --ttl 2h
done

grite lock release --resource "repo:migration"
```

## Multi-Agent Coordination

### Coordinator Pattern

A coordinator agent assigns work:

```bash
# Coordinator creates task and assigns
grite issue create --title "Process batch" --label "todo" --json
grite lock acquire --resource "issue:$ID" --ttl 30m

# Worker agent checks for available work
for id in $(grite issue list --label todo --json | jq -r '.data.issues[].issue_id'); do
  if grite lock acquire --resource "issue:$id" --ttl 30m 2>/dev/null; then
    # Got the lock, do the work
    process_issue "$id"
    grite lock release --resource "issue:$id"
    break
  fi
done
```

### Work Stealing

Agents can steal expired locks:

```bash
# Lock expired? Acquire it
if ! grite lock status --json | jq -e ".data.locks[] | select(.resource == \"$RESOURCE\")"; then
  grite lock acquire --resource "$RESOURCE" --ttl 30m
fi
```

## Best Practices

### Always Release Locks

```bash
# Use trap to ensure release on exit
trap 'grite lock release --resource "issue:$ID"' EXIT

grite lock acquire --resource "issue:$ID" --ttl 30m
# Do work...
```

### Use Appropriate TTLs

| Task Type | Suggested TTL |
|-----------|---------------|
| Quick edit | 5-15 minutes |
| Feature work | 30-60 minutes |
| Long migration | 2-4 hours |

### Renew Before Expiry

Set up renewal at 50% of TTL:

```bash
# 30 minute lock, renew every 15 minutes
grite lock acquire --resource "..." --ttl 30m

while working; do
  sleep 900  # 15 minutes
  grite lock renew --resource "..." --ttl 30m
done
```

### Handle Lock Failures Gracefully

```bash
if ! grite lock acquire --resource "..." --ttl 30m; then
  echo "Could not acquire lock, trying later"
  exit 0  # Don't fail, just skip
fi
```

## Next Steps

- [Actor Identity](actors.md) - Understand lock ownership
- [Using the Daemon](daemon.md) - Lock behavior with daemon
- [AI Agents Use Case](../use-cases/ai-agents.md) - Lock patterns for agents
