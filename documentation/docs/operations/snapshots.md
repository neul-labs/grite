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
grite snapshot
```

Output:

```
Created snapshot refs/grite/snapshots/1700000000000
  Events: 5432
  WAL head: abc123...
```

## JSON Output

```bash
grite snapshot --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "snapshot_ref": "refs/grite/snapshots/1700000000000",
    "wal_head": "abc123...",
    "event_count": 5432
  }
}
```

## Garbage Collection

Remove old snapshots:

```bash
grite snapshot gc
```

This removes snapshots according to the retention policy:

- Keeps most recent snapshot
- Removes snapshots older than threshold

### JSON Output

```bash
grite snapshot gc --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "deleted": [
      "refs/grite/snapshots/1690000000000",
      "refs/grite/snapshots/1695000000000"
    ]
  }
}
```

## Snapshot Location

Snapshots are stored as git refs:

```
refs/grite/snapshots/<timestamp>
```

Where `<timestamp>` is Unix milliseconds.

List snapshots:

```bash
git for-each-ref refs/grite/snapshots/
```

## Using Snapshots

### For Rebuilds

Use the latest snapshot for fast rebuilds:

```bash
grite rebuild --from-snapshot
```

### Automatic Selection

Grite automatically selects the most recent snapshot.

### Manual Inspection

View snapshot contents:

```bash
# Get snapshot ref
git show refs/grite/snapshots/1700000000000
```

## Configuration

Snapshot creation thresholds are configured in `.git/grite/config.toml`:

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
grite snapshot

# Monthly cleanup
grite snapshot gc
```

### CI/CD

```bash
# After significant changes
grite snapshot

# Before deployment
grite rebuild --from-snapshot
```

### Large Teams

```bash
# Coordinator creates snapshot after sync
grite sync --pull
grite snapshot
grite sync --push
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
git for-each-ref --format='%(refname) %(objectsize:disk)' refs/grite/snapshots/

# Clean up
grite snapshot gc
```

## Syncing Snapshots

Snapshots sync with the repository:

```bash
# Push snapshots
git push origin refs/grite/snapshots/*:refs/grite/snapshots/*

# Fetch snapshots
git fetch origin refs/grite/snapshots/*:refs/grite/snapshots/*
```

By default, `grite sync` handles snapshot refs.

## Automation

### Cron Job

```bash
# Create weekly snapshots
0 0 * * 0 cd /path/to/repo && grite snapshot

# Monthly cleanup
0 0 1 * * cd /path/to/repo && grite snapshot gc
```

### CI Pipeline

```yaml
- name: Create snapshot
  run: |
    grite snapshot
    grite sync --push
```

### Script

```bash
#!/bin/bash
# snapshot-if-needed.sh

stats=$(grite db stats --json)
events=$(echo "$stats" | jq '.data.events_since_rebuild')
days=$(echo "$stats" | jq '.data.days_since_rebuild')

if [ "$events" -gt 5000 ] || [ "$days" -gt 3 ]; then
  echo "Creating snapshot..."
  grite snapshot
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
grite snapshot gc
```

### Verify Snapshots Work

Test that snapshot-based rebuild works:

```bash
grite rebuild --from-snapshot
grite doctor
```

## Troubleshooting

### "No snapshot found"

No snapshots exist yet:

```bash
grite snapshot  # Create one
```

### "Snapshot corrupt"

Rebuild without snapshot, then create new one:

```bash
grite rebuild          # Standard rebuild
grite snapshot         # Create fresh snapshot
```

### "Too many snapshots"

Run garbage collection:

```bash
grite snapshot gc
```

## Next Steps

- [Rebuilding](rebuild.md) - Use snapshots for rebuilds
- [Troubleshooting](troubleshooting.md) - Fix snapshot issues
