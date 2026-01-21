# Troubleshooting

This guide helps diagnose and fix common grit issues.

## Quick Diagnostics

Start with a health check:

```bash
grit doctor --json
```

Check for errors:

```bash
grit doctor --json | jq '.data.checks[] | select(.status != "ok")'
```

## Common Issues

### Database Busy (DbBusy)

**Symptom:**

```
error: Database busy - another process or daemon owns the data directory
```

**Causes:**

- Daemon is running
- Another grit process has the lock
- Stale lock file

**Solutions:**

1. Route through daemon:
   ```bash
   grit issue list  # Will use daemon if running
   ```

2. Stop the daemon:
   ```bash
   grit daemon stop
   grit issue list
   ```

3. Skip daemon:
   ```bash
   grit --no-daemon issue list
   ```

4. Remove stale lock (if process crashed):
   ```bash
   rm .git/grit/actors/<actor_id>/sled.lock
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
   grit issue list
   ```

2. Sync from remote:
   ```bash
   grit sync --pull
   ```

3. Rebuild database:
   ```bash
   grit rebuild
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
grit sync  # Pulls, then pushes with auto-rebase
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
   grit sync --pull
   grit rebuild
   ```

2. Verify git integrity:
   ```bash
   git fsck
   ```

3. Re-clone if necessary:
   ```bash
   git clone <remote> fresh-copy
   cd fresh-copy
   grit rebuild
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
   grit daemon stop
   grit daemon start
   ```

2. Check daemon status:
   ```bash
   grit daemon status
   ```

3. Use local execution:
   ```bash
   grit --no-daemon issue list
   ```

4. Remove stale daemon lock:
   ```bash
   rm .git/grit/actors/<actor_id>/daemon.lock
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
   cat .git/grit/config.toml
   ```

2. Set policy to `warn` to continue:
   ```toml
   lock_policy = "warn"
   ```

3. Verify specific events:
   ```bash
   grit db verify --verbose --json
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
   grit db stats --json
   ```

2. Rebuild if recommended:
   ```bash
   grit rebuild --from-snapshot
   ```

3. Create snapshots:
   ```bash
   grit snapshot
   ```

4. Use daemon for repeated commands:
   ```bash
   grit daemon start
   ```

### Out of Disk Space

**Symptom:**

```
error: No space left on device
```

**Solutions:**

1. Clean up snapshots:
   ```bash
   grit snapshot gc
   ```

2. Clean expired locks:
   ```bash
   grit lock gc
   ```

3. Rebuild to compact:
   ```bash
   grit rebuild
   ```

### Actor Configuration Missing

**Symptom:**

```
error: Actor configuration not found
```

**Solutions:**

1. Create a new actor:
   ```bash
   grit actor init --label "my-actor"
   ```

2. Check existing actors:
   ```bash
   grit actor list
   ```

3. Re-initialize grit:
   ```bash
   grit init
   ```

## Diagnostic Commands

### Check Health

```bash
grit doctor --json
```

### Database Statistics

```bash
grit db stats --json
```

### Verify Integrity

```bash
grit db check --json
```

### Daemon Status

```bash
grit daemon status --json
```

### Lock Status

```bash
grit lock status --json
```

### Current Actor

```bash
grit actor current --json
```

## Debug Logging

Enable verbose logging:

```bash
RUST_LOG=debug grit issue list
```

More specific:

```bash
RUST_LOG=debug,libgrit_core=trace grit issue list
```

## Recovery Procedures

### Full Reset

If all else fails:

```bash
# Stop daemon
grit daemon stop

# Backup current state
grit export --format json
cp .grit/export.json ~/backup-$(date +%Y%m%d).json

# Remove local state
rm -rf .git/grit/actors/<actor_id>/sled
rm -f .git/grit/actors/<actor_id>/*.lock

# Sync and rebuild
grit sync --pull
grit rebuild
```

### Re-clone Repository

For severe corruption:

```bash
# In a different directory
git clone <remote> fresh-repo
cd fresh-repo
grit init
grit sync --pull
grit rebuild
```

### Restore from Export

If you have a JSON export:

```bash
# Manual restoration may be needed
# Export is for backup/migration, not direct restore
```

## Getting Help

If these steps don't resolve your issue:

1. Check the [GitHub Issues](https://github.com/neul-labs/grit/issues)
2. Enable debug logging and capture output
3. Report the issue with:
   - grit version (`grit --version`)
   - Error message
   - Debug log output
   - Steps to reproduce

## Next Steps

- [Health Checks](doctor.md) - Regular diagnostics
- [Rebuilding](rebuild.md) - Database recovery
- [CLI Reference](../reference/cli.md) - Command details
