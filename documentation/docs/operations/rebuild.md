# Rebuilding

The `grite rebuild` command discards local projections and replays events to rebuild the materialized view.

## When to Rebuild

Rebuild when:

- `grite doctor` recommends it
- Database appears corrupted
- Query performance degrades
- After database crashes

## Basic Rebuild

Standard rebuild from local store events:

```bash
grite rebuild
```

This:

1. Clears current projections
2. Reads all events from local store
3. Recomputes projections
4. Compacts the database

## Snapshot-Based Rebuild

Faster rebuild using the latest snapshot:

```bash
grite rebuild --from-snapshot
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
grite rebuild --json
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
    "from_snapshot": "refs/grite/snapshots/1700000000000"
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

1. **Create snapshots regularly**: `grite snapshot`
2. **Use snapshot rebuild**: `grite rebuild --from-snapshot`
3. **Clean old snapshots**: `grite snapshot gc`

## Database Compaction

Rebuild automatically compacts the database because it rewrites all data from scratch. This can:

- Reduce database size
- Improve query performance
- Remove fragmentation

## Checking Rebuild Status

Before rebuild:

```bash
grite db stats --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "path": ".git/grite/actors/.../sled",
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
grite daemon stop

# Delete sled database
rm -rf .git/grite/actors/<actor_id>/sled

# Rebuild from scratch
grite rebuild
```

Or with snapshot:

```bash
grite daemon stop
rm -rf .git/grite/actors/<actor_id>/sled
grite rebuild --from-snapshot
```

## Rebuild After Sync Issues

If sync brought in corrupted data:

```bash
# Verify WAL integrity first
grite db check --json

# If WAL is good, rebuild
grite rebuild

# If WAL is bad, resync from remote
grite sync --pull
grite rebuild
```

## Automation

### Scheduled Rebuild

```bash
#!/bin/bash
# weekly-rebuild.sh

# Check if rebuild needed
stats=$(grite db stats --json)
recommended=$(echo "$stats" | jq -r '.data.rebuild_recommended')

if [ "$recommended" = "true" ]; then
  echo "Rebuild recommended, running..."
  grite rebuild --from-snapshot
else
  echo "Rebuild not needed"
fi
```

### CI Integration

```yaml
- name: Rebuild if needed
  run: |
    if grite db stats --json | jq -e '.data.rebuild_recommended'; then
      grite rebuild --from-snapshot
    fi
```

## Best Practices

### Use Snapshots

For large repositories, always use snapshots:

```bash
# Create regular snapshots
grite snapshot

# Use them for rebuilds
grite rebuild --from-snapshot
```

### Stop Daemon First

For cleanest rebuild:

```bash
grite daemon stop
grite rebuild
```

### Verify After Rebuild

```bash
grite rebuild
grite doctor  # Should show all OK
```

## Troubleshooting

### "Rebuild failed"

Check for:

- Disk space
- Corrupted WAL
- File permissions

```bash
grite db check --json  # Check for corruption
df -h                 # Check disk space
```

### "Snapshot not found"

No snapshots available:

```bash
grite rebuild  # Use standard rebuild instead
grite snapshot # Create a snapshot for next time
```

### "Rebuild takes too long"

Create a snapshot first:

```bash
grite snapshot
grite rebuild --from-snapshot
```

## Next Steps

- [Snapshots](snapshots.md) - Manage snapshots
- [Health Checks](doctor.md) - Verify rebuild success
- [Troubleshooting](troubleshooting.md) - Fix issues
