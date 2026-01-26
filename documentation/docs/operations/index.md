# Operations

This section covers operational tasks for maintaining grite installations.

## Overview

Grite is designed to be low-maintenance, but occasionally you may need to:

- Run health checks
- Rebuild the database
- Manage snapshots
- Troubleshoot issues

## Documentation

### [Health Checks](doctor.md)

Run diagnostics and auto-repair:

```bash
grite doctor        # Check health
grite doctor --fix  # Auto-repair
```

### [Rebuilding](rebuild.md)

Rebuild the materialized view:

```bash
grite rebuild                 # Standard rebuild
grite rebuild --from-snapshot # Fast rebuild
```

### [Snapshots](snapshots.md)

Manage snapshots for faster rebuilds:

```bash
grite snapshot     # Create snapshot
grite snapshot gc  # Clean old snapshots
```

### [Troubleshooting](troubleshooting.md)

Diagnose and fix common problems:

- Database issues
- Sync problems
- Daemon errors

## Quick Reference

| Task | Command |
|------|---------|
| Health check | `grite doctor` |
| Auto-repair | `grite doctor --fix` |
| Database stats | `grite db stats` |
| Rebuild | `grite rebuild` |
| Fast rebuild | `grite rebuild --from-snapshot` |
| Create snapshot | `grite snapshot` |
| Clean snapshots | `grite snapshot gc` |
| Stop daemon | `grite daemon stop` |

## Maintenance Schedule

### Regular Tasks

| Task | Frequency | Command |
|------|-----------|---------|
| Health check | Weekly | `grite doctor` |
| Snapshot GC | Monthly | `grite snapshot gc` |
| Lock cleanup | As needed | `grite lock gc` |

### Triggered Tasks

| Condition | Action |
|-----------|--------|
| Doctor warns about rebuild | `grite rebuild` |
| Database corruption | `grite doctor --fix` |
| Sync failures | Check [Troubleshooting](troubleshooting.md) |

## Database Management

The sled database is a cache that can be safely deleted and rebuilt:

```bash
# Check database stats
grite db stats --json

# Verify integrity
grite db check --json

# Full rebuild if needed
grite rebuild
```

## Limits to Consider

- Very large WALs slow rebuilds without snapshots
- High push frequency can cause contention on `refs/grite/wal`
- Each actor's sled database should not be shared between processes

## Automation

### CI/CD Health Check

```yaml
- name: Grite Health Check
  run: |
    grite doctor --json | jq '.data.checks[] | select(.status != "ok")'
```

### Scheduled Maintenance

```bash
#!/bin/bash
# weekly-maintenance.sh
grite doctor --fix
grite snapshot gc
grite lock gc
```

## Next Steps

- [Health Checks](doctor.md) - Start with diagnostics
- [Troubleshooting](troubleshooting.md) - Fix common issues
