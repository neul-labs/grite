# Operations

This section covers operational tasks for maintaining grit installations.

## Overview

Grit is designed to be low-maintenance, but occasionally you may need to:

- Run health checks
- Rebuild the database
- Manage snapshots
- Troubleshoot issues

## Documentation

### [Health Checks](doctor.md)

Run diagnostics and auto-repair:

```bash
grit doctor        # Check health
grit doctor --fix  # Auto-repair
```

### [Rebuilding](rebuild.md)

Rebuild the materialized view:

```bash
grit rebuild                 # Standard rebuild
grit rebuild --from-snapshot # Fast rebuild
```

### [Snapshots](snapshots.md)

Manage snapshots for faster rebuilds:

```bash
grit snapshot     # Create snapshot
grit snapshot gc  # Clean old snapshots
```

### [Troubleshooting](troubleshooting.md)

Diagnose and fix common problems:

- Database issues
- Sync problems
- Daemon errors

## Quick Reference

| Task | Command |
|------|---------|
| Health check | `grit doctor` |
| Auto-repair | `grit doctor --fix` |
| Database stats | `grit db stats` |
| Rebuild | `grit rebuild` |
| Fast rebuild | `grit rebuild --from-snapshot` |
| Create snapshot | `grit snapshot` |
| Clean snapshots | `grit snapshot gc` |
| Stop daemon | `grit daemon stop` |

## Maintenance Schedule

### Regular Tasks

| Task | Frequency | Command |
|------|-----------|---------|
| Health check | Weekly | `grit doctor` |
| Snapshot GC | Monthly | `grit snapshot gc` |
| Lock cleanup | As needed | `grit lock gc` |

### Triggered Tasks

| Condition | Action |
|-----------|--------|
| Doctor warns about rebuild | `grit rebuild` |
| Database corruption | `grit doctor --fix` |
| Sync failures | Check [Troubleshooting](troubleshooting.md) |

## Database Management

The sled database is a cache that can be safely deleted and rebuilt:

```bash
# Check database stats
grit db stats --json

# Verify integrity
grit db check --json

# Full rebuild if needed
grit rebuild
```

## Limits to Consider

- Very large WALs slow rebuilds without snapshots
- High push frequency can cause contention on `refs/grit/wal`
- Each actor's sled database should not be shared between processes

## Automation

### CI/CD Health Check

```yaml
- name: Grit Health Check
  run: |
    grit doctor --json | jq '.data.checks[] | select(.status != "ok")'
```

### Scheduled Maintenance

```bash
#!/bin/bash
# weekly-maintenance.sh
grit doctor --fix
grit snapshot gc
grit lock gc
```

## Next Steps

- [Health Checks](doctor.md) - Start with diagnostics
- [Troubleshooting](troubleshooting.md) - Fix common issues
