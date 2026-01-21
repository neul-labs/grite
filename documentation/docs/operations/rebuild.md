# Rebuilding

The `grit rebuild` command discards local projections and replays events to rebuild the materialized view.

## When to Rebuild

Rebuild when:

- `grit doctor` recommends it
- Database appears corrupted
- Query performance degrades
- After database crashes

## Basic Rebuild

Standard rebuild from local store events:

```bash
grit rebuild
```

This:

1. Clears current projections
2. Reads all events from local store
3. Recomputes projections
4. Compacts the database

## Snapshot-Based Rebuild

Faster rebuild using the latest snapshot:

```bash
grit rebuild --from-snapshot
```

This:

1. Finds the latest snapshot ref
2. Loads events from snapshot
3. Rebuilds projections
4. Much faster for large repositories

### When to Use Snapshots

Use `--from-snapshot` when:

- Repository has many events (>10,000)
- Standard rebuild is slow
- Snapshots are available

## What Gets Rebuilt

| Rebuilt | Not Affected |
|---------|--------------|
| Issue projections | WAL (git refs) |
| Issue indexes | Actor config |
| Label indexes | Signing keys |
| Metadata | Repository config |

## JSON Output

```bash
grit rebuild --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "wal_head": "abc123...",
    "event_count": 1234,
    "from_snapshot": null
  }
}
```

With snapshot:

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "wal_head": "abc123...",
    "event_count": 1234,
    "from_snapshot": "refs/grit/snapshots/1700000000000"
  }
}
```

## Performance

### Time Estimates

| Events | Standard Rebuild | Snapshot Rebuild |
|--------|------------------|------------------|
| 1,000 | ~1 second | ~1 second |
| 10,000 | ~10 seconds | ~2 seconds |
| 100,000 | ~2 minutes | ~10 seconds |

Actual times depend on hardware and event complexity.

### Optimizing Rebuild Time

1. **Create snapshots regularly**: `grit snapshot`
2. **Use snapshot rebuild**: `grit rebuild --from-snapshot`
3. **Clean old snapshots**: `grit snapshot gc`

## Database Compaction

Rebuild automatically compacts the database because it rewrites all data from scratch. This can:

- Reduce database size
- Improve query performance
- Remove fragmentation

## Checking Rebuild Status

Before rebuild:

```bash
grit db stats --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "path": ".git/grit/actors/.../sled",
    "size_bytes": 5242880,
    "event_count": 1234,
    "issue_count": 42,
    "last_rebuild_ts": 1699900000000,
    "events_since_rebuild": 500,
    "days_since_rebuild": 7,
    "rebuild_recommended": true
  }
}
```

## Manual Database Reset

For a complete reset (not usually needed):

```bash
# Stop daemon first
grit daemon stop

# Delete sled database
rm -rf .git/grit/actors/<actor_id>/sled

# Rebuild from scratch
grit rebuild
```

Or with snapshot:

```bash
grit daemon stop
rm -rf .git/grit/actors/<actor_id>/sled
grit rebuild --from-snapshot
```

## Rebuild After Sync Issues

If sync brought in corrupted data:

```bash
# Verify WAL integrity first
grit db check --json

# If WAL is good, rebuild
grit rebuild

# If WAL is bad, resync from remote
grit sync --pull
grit rebuild
```

## Automation

### Scheduled Rebuild

```bash
#!/bin/bash
# weekly-rebuild.sh

# Check if rebuild needed
stats=$(grit db stats --json)
recommended=$(echo "$stats" | jq -r '.data.rebuild_recommended')

if [ "$recommended" = "true" ]; then
  echo "Rebuild recommended, running..."
  grit rebuild --from-snapshot
else
  echo "Rebuild not needed"
fi
```

### CI Integration

```yaml
- name: Rebuild if needed
  run: |
    if grit db stats --json | jq -e '.data.rebuild_recommended'; then
      grit rebuild --from-snapshot
    fi
```

## Best Practices

### Use Snapshots

For large repositories, always use snapshots:

```bash
# Create regular snapshots
grit snapshot

# Use them for rebuilds
grit rebuild --from-snapshot
```

### Stop Daemon First

For cleanest rebuild:

```bash
grit daemon stop
grit rebuild
```

### Verify After Rebuild

```bash
grit rebuild
grit doctor  # Should show all OK
```

## Troubleshooting

### "Rebuild failed"

Check for:

- Disk space
- Corrupted WAL
- File permissions

```bash
grit db check --json  # Check for corruption
df -h                 # Check disk space
```

### "Snapshot not found"

No snapshots available:

```bash
grit rebuild  # Use standard rebuild instead
grit snapshot # Create a snapshot for next time
```

### "Rebuild takes too long"

Create a snapshot first:

```bash
grit snapshot
grit rebuild --from-snapshot
```

## Next Steps

- [Snapshots](snapshots.md) - Manage snapshots
- [Health Checks](doctor.md) - Verify rebuild success
- [Troubleshooting](troubleshooting.md) - Fix issues
