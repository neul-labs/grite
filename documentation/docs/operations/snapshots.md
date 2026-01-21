# Snapshots

Snapshots are point-in-time copies of the event log that accelerate rebuilds.

## Overview

Snapshots:

- Consolidate events for faster reads
- Enable quick database rebuilds
- Are stored as git refs
- Never modify WAL history

## Creating Snapshots

Create a snapshot of the current state:

```bash
grit snapshot
```

Output:

```
Created snapshot refs/grit/snapshots/1700000000000
  Events: 5432
  WAL head: abc123...
```

## JSON Output

```bash
grit snapshot --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "snapshot_ref": "refs/grit/snapshots/1700000000000",
    "wal_head": "abc123...",
    "event_count": 5432
  }
}
```

## Garbage Collection

Remove old snapshots:

```bash
grit snapshot gc
```

This removes snapshots according to the retention policy:

- Keeps most recent snapshot
- Removes snapshots older than threshold

### JSON Output

```bash
grit snapshot gc --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "deleted": [
      "refs/grit/snapshots/1690000000000",
      "refs/grit/snapshots/1695000000000"
    ]
  }
}
```

## Snapshot Location

Snapshots are stored as git refs:

```
refs/grit/snapshots/<timestamp>
```

Where `<timestamp>` is Unix milliseconds.

List snapshots:

```bash
git for-each-ref refs/grit/snapshots/
```

## Using Snapshots

### For Rebuilds

Use the latest snapshot for fast rebuilds:

```bash
grit rebuild --from-snapshot
```

### Automatic Selection

Grit automatically selects the most recent snapshot.

### Manual Inspection

View snapshot contents:

```bash
# Get snapshot ref
git show refs/grit/snapshots/1700000000000
```

## Configuration

Snapshot creation thresholds are configured in `.git/grit/config.toml`:

```toml
[snapshot]
max_events = 10000    # Create snapshot at this event count
max_age_days = 7      # Create snapshot if older than this
```

### max_events

Create a snapshot when the event count since last snapshot exceeds this threshold.

- **Default**: 10000
- **Recommendation**: Lower for faster rebuilds, higher for less storage

### max_age_days

Create a snapshot when the last snapshot is older than this many days.

- **Default**: 7
- **Recommendation**: More frequent for active repositories

## Snapshot Workflow

### Development

```bash
# Weekly snapshot
grit snapshot

# Monthly cleanup
grit snapshot gc
```

### CI/CD

```bash
# After significant changes
grit snapshot

# Before deployment
grit rebuild --from-snapshot
```

### Large Teams

```bash
# Coordinator creates snapshot after sync
grit sync --pull
grit snapshot
grit sync --push
```

## Snapshot vs WAL

| Aspect | WAL | Snapshot |
|--------|-----|----------|
| Purpose | Source of truth | Rebuild accelerator |
| Append-only | Yes | No (point-in-time) |
| Required | Yes | No |
| Modifiable | No | Can be deleted |

## Storage Impact

### Snapshot Size

Snapshots contain all events up to a point:

- Size ≈ WAL size at snapshot time
- Compressed by git

### Managing Storage

```bash
# Check snapshot refs
git for-each-ref --format='%(refname) %(objectsize:disk)' refs/grit/snapshots/

# Clean up
grit snapshot gc
```

## Syncing Snapshots

Snapshots sync with the repository:

```bash
# Push snapshots
git push origin refs/grit/snapshots/*:refs/grit/snapshots/*

# Fetch snapshots
git fetch origin refs/grit/snapshots/*:refs/grit/snapshots/*
```

By default, `grit sync` handles snapshot refs.

## Automation

### Cron Job

```bash
# Create weekly snapshots
0 0 * * 0 cd /path/to/repo && grit snapshot

# Monthly cleanup
0 0 1 * * cd /path/to/repo && grit snapshot gc
```

### CI Pipeline

```yaml
- name: Create snapshot
  run: |
    grit snapshot
    grit sync --push
```

### Script

```bash
#!/bin/bash
# snapshot-if-needed.sh

stats=$(grit db stats --json)
events=$(echo "$stats" | jq '.data.events_since_rebuild')
days=$(echo "$stats" | jq '.data.days_since_rebuild')

if [ "$events" -gt 5000 ] || [ "$days" -gt 3 ]; then
  echo "Creating snapshot..."
  grit snapshot
fi
```

## Best Practices

### Regular Snapshots

Create snapshots regularly for fast rebuilds:

- After bulk operations
- Weekly for active repos
- Before major syncs

### Clean Up Old Snapshots

Don't accumulate unnecessary snapshots:

```bash
grit snapshot gc
```

### Verify Snapshots Work

Test that snapshot-based rebuild works:

```bash
grit rebuild --from-snapshot
grit doctor
```

## Troubleshooting

### "No snapshot found"

No snapshots exist yet:

```bash
grit snapshot  # Create one
```

### "Snapshot corrupt"

Rebuild without snapshot, then create new one:

```bash
grit rebuild          # Standard rebuild
grit snapshot         # Create fresh snapshot
```

### "Too many snapshots"

Run garbage collection:

```bash
grit snapshot gc
```

## Next Steps

- [Rebuilding](rebuild.md) - Use snapshots for rebuilds
- [Troubleshooting](troubleshooting.md) - Fix snapshot issues
