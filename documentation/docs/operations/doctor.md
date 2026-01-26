# Health Checks

The `grite doctor` command performs health checks and provides remediation suggestions.

## Basic Usage

Run health checks:

```bash
grite doctor
```

Output:

```
Checking grite health...

[OK] git_repo: Git repository is valid
[OK] wal_ref: WAL ref exists and is readable
[OK] actor_config: Actor is properly configured
[OK] store_integrity: Database integrity verified
[WARN] rebuild_threshold: 15000 events since last rebuild (threshold: 10000)

Suggestions:
  - Run 'grite rebuild' to improve query performance
```

## Auto-Repair

Automatically fix issues:

```bash
grite doctor --fix
```

This runs safe local repairs:

- Rebuilds local DB on corruption
- Does **not** modify git refs
- Does **not** push to remote

If remote sync is needed, the remediation plan explicitly lists `grite sync --pull` and/or `grite sync --push`.

## Checks Performed

| Check | Description | Auto-Fix |
|-------|-------------|----------|
| `git_repo` | Git repository is valid | No |
| `wal_ref` | WAL ref exists and is readable | No |
| `actor_config` | Actor is properly configured | No |
| `store_integrity` | Event hashes match | Yes (rebuild) |
| `rebuild_threshold` | Events since last rebuild | Yes (rebuild) |

### git_repo

Verifies the current directory is a valid git repository.

**Failures:**

- Not a git repository
- Git directory corrupted

**Resolution:** Initialize git or fix git directory.

### wal_ref

Checks that `refs/grite/wal` exists and is readable.

**Failures:**

- WAL ref doesn't exist
- WAL commits are corrupted

**Resolution:** Run `grite sync --pull` to fetch from remote, or `grite init` for new repositories.

### actor_config

Verifies actor configuration is valid.

**Failures:**

- Actor config file missing
- Actor config malformed

**Resolution:** Run `grite actor init` to create a new actor.

### store_integrity

Verifies event hashes in the database match computed hashes.

**Failures:**

- Hash mismatches indicate corruption
- Missing events

**Resolution:** `grite doctor --fix` rebuilds the database automatically.

### rebuild_threshold

Checks if rebuild is recommended based on:

- Events since last rebuild (default threshold: 10,000)
- Days since last rebuild (default threshold: 7)

**Warnings:**

- Too many events accumulated
- Too long since last rebuild

**Resolution:** Run `grite rebuild` for better performance.

## JSON Output

```bash
grite doctor --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "checks": [
      {
        "id": "git_repo",
        "status": "ok",
        "message": "Git repository is valid",
        "plan": []
      },
      {
        "id": "wal_ref",
        "status": "ok",
        "message": "WAL ref exists and is readable",
        "plan": []
      },
      {
        "id": "store_integrity",
        "status": "ok",
        "message": "Database integrity verified",
        "plan": []
      },
      {
        "id": "rebuild_threshold",
        "status": "warn",
        "message": "15000 events since last rebuild",
        "plan": ["grite rebuild"]
      }
    ],
    "applied": []
  }
}
```

### Status Values

| Status | Meaning |
|--------|---------|
| `ok` | Check passed |
| `warn` | Advisory warning |
| `error` | Problem detected |

## Verify Event Hashes

For deeper integrity verification:

```bash
grite db check --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "events_checked": 1234,
    "events_valid": 1234,
    "corrupt_count": 0,
    "errors": []
  }
}
```

## Verify Signatures

If events are signed:

```bash
grite db verify --verbose --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "events_checked": 1234,
    "signatures_checked": 1000,
    "signatures_valid": 1000,
    "error_count": 0,
    "errors": []
  }
}
```

## Automation

### CI Pipeline

```yaml
- name: Grite Health Check
  run: |
    result=$(grite doctor --json)
    if echo "$result" | jq -e '.data.checks[] | select(.status == "error")' > /dev/null; then
      echo "Grite health check failed"
      exit 1
    fi
```

### Cron Job

```bash
# Run weekly health check
0 0 * * 0 cd /path/to/repo && grite doctor --fix >> /var/log/grite-doctor.log 2>&1
```

## Best Practices

### Regular Checks

Run `grite doctor` periodically:

- After major changes
- Before important syncs
- As part of CI/CD

### Don't Ignore Warnings

Warnings indicate potential issues:

```bash
# Check for warnings
grite doctor --json | jq '.data.checks[] | select(.status == "warn")'
```

### Fix Before Sync

Run doctor before syncing with others:

```bash
grite doctor --fix
grite sync
```

## Next Steps

- [Rebuilding](rebuild.md) - Rebuild the database
- [Troubleshooting](troubleshooting.md) - Common issues
