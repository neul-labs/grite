# Troubleshooting

This guide helps diagnose and fix common grite issues.

## Quick Diagnostics

Start with a health check:

```bash
grite doctor --json
```

Check for errors:

```bash
grite doctor --json | jq '.data.checks[] | select(.status != "ok")'
```

## Common Issues

### Database Busy (DbBusy)

**Symptom:**

```
error: Database busy - another process or daemon owns the data directory
```

**Causes:**

- Daemon is running
- Another grite process has the lock
- Stale lock file

**Solutions:**

1. Route through daemon:
   ```bash
   grite issue list  # Will use daemon if running
   ```

2. Stop the daemon:
   ```bash
   grite daemon stop
   grite issue list
   ```

3. Skip daemon:
   ```bash
   grite --no-daemon issue list
   ```

4. Remove stale lock (if process crashed):
   ```bash
   rm .git/grite/actors/<actor_id>/sled.lock
   ```

### Issue Not Found

**Symptom:**

```
error: Issue 'abc123' not found
```

**Causes:**

- Issue ID is wrong
- Issue not synced yet
- Database needs rebuild

**Solutions:**

1. Check available issues:
   ```bash
   grite issue list
   ```

2. Sync from remote:
   ```bash
   grite sync --pull
   ```

3. Rebuild database:
   ```bash
   grite rebuild
   ```

### Sync Conflicts

**Symptom:**

```
error: Push rejected (non-fast-forward)
```

**Causes:**

- Remote has newer commits
- Normal during concurrent work

**Solution:**

Use full sync (auto-rebases):

```bash
grite sync  # Pulls, then pushes with auto-rebase
```

### WAL Corruption

**Symptom:**

```
error: WAL data malformed or hash mismatch
```

**Causes:**

- Interrupted write
- Git corruption
- Disk error

**Solutions:**

1. Try syncing from remote:
   ```bash
   grite sync --pull
   grite rebuild
   ```

2. Verify git integrity:
   ```bash
   git fsck
   ```

3. Re-clone if necessary:
   ```bash
   git clone <remote> fresh-copy
   cd fresh-copy
   grite rebuild
   ```

### IPC Errors

**Symptom:**

```
error: Failed to connect to daemon (IPC error)
```

**Causes:**

- Daemon crashed
- Socket file missing
- Permission issues

**Solutions:**

1. Restart daemon:
   ```bash
   grite daemon stop
   grite daemon start
   ```

2. Check daemon status:
   ```bash
   grite daemon status
   ```

3. Use local execution:
   ```bash
   grite --no-daemon issue list
   ```

4. Remove stale daemon lock:
   ```bash
   rm .git/grite/actors/<actor_id>/daemon.lock
   ```

### Signature Verification Failed

**Symptom:**

```
warning: Invalid signature on event abc123...
```

**Causes:**

- Signing key changed
- Event tampered with
- Wrong public key

**Solutions:**

1. Check verification policy:
   ```bash
   cat .git/grite/config.toml
   ```

2. Set policy to `warn` to continue:
   ```toml
   lock_policy = "warn"
   ```

3. Verify specific events:
   ```bash
   grite db verify --verbose --json
   ```

### Slow Performance

**Symptom:**

- Commands take a long time
- List queries are slow

**Causes:**

- Database needs rebuild
- No daemon running
- Large WAL without snapshots

**Solutions:**

1. Check database stats:
   ```bash
   grite db stats --json
   ```

2. Rebuild if recommended:
   ```bash
   grite rebuild --from-snapshot
   ```

3. Create snapshots:
   ```bash
   grite snapshot
   ```

4. Use daemon for repeated commands:
   ```bash
   grite daemon start
   ```

### Out of Disk Space

**Symptom:**

```
error: No space left on device
```

**Solutions:**

1. Clean up snapshots:
   ```bash
   grite snapshot gc
   ```

2. Clean expired locks:
   ```bash
   grite lock gc
   ```

3. Rebuild to compact:
   ```bash
   grite rebuild
   ```

### Actor Configuration Missing

**Symptom:**

```
error: Actor configuration not found
```

**Solutions:**

1. Create a new actor:
   ```bash
   grite actor init --label "my-actor"
   ```

2. Check existing actors:
   ```bash
   grite actor list
   ```

3. Re-initialize grite:
   ```bash
   grite init
   ```

## Diagnostic Commands

### Check Health

```bash
grite doctor --json
```

### Database Statistics

```bash
grite db stats --json
```

### Verify Integrity

```bash
grite db check --json
```

### Daemon Status

```bash
grite daemon status --json
```

### Lock Status

```bash
grite lock status --json
```

### Current Actor

```bash
grite actor current --json
```

## Debug Logging

Enable verbose logging:

```bash
RUST_LOG=debug grite issue list
```

More specific:

```bash
RUST_LOG=debug,libgrit_core=trace grite issue list
```

## Recovery Procedures

### Full Reset

If all else fails:

```bash
# Stop daemon
grite daemon stop

# Backup current state
grite export --format json
cp .grite/export.json ~/backup-$(date +%Y%m%d).json

# Remove local state
rm -rf .git/grite/actors/<actor_id>/sled
rm -f .git/grite/actors/<actor_id>/*.lock

# Sync and rebuild
grite sync --pull
grite rebuild
```

### Re-clone Repository

For severe corruption:

```bash
# In a different directory
git clone <remote> fresh-repo
cd fresh-repo
grite init
grite sync --pull
grite rebuild
```

### Restore from Export

If you have a JSON export:

```bash
# Manual restoration may be needed
# Export is for backup/migration, not direct restore
```

## Getting Help

If these steps don't resolve your issue:

1. Check the [GitHub Issues](https://github.com/neul-labs/grite/issues)
2. Enable debug logging and capture output
3. Report the issue with:
   - grite version (`grite --version`)
   - Error message
   - Debug log output
   - Steps to reproduce

## Next Steps

- [Health Checks](doctor.md) - Regular diagnostics
- [Rebuilding](rebuild.md) - Database recovery
- [CLI Reference](../reference/cli.md) - Command details
