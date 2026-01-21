# Exporting Data

This guide explains how to export grit data for external use.

## Overview

Grit can export issues in two formats:

- **JSON**: Machine-readable for dashboards and integrations
- **Markdown**: Human-readable for documentation

## Export Formats

### JSON Export

Export all issues as JSON:

```bash
grit export --format json
```

Output is written to `.grit/export.json` by default.

### Markdown Export

Export as readable Markdown:

```bash
grit export --format md
```

Output is written to `.grit/export.md` by default.

## Export Output

### JSON Format

```json
{
  "schema_version": 1,
  "exported_at": 1700000000000,
  "wal_head": "abc123...",
  "issues": [
    {
      "issue_id": "8057324b...",
      "title": "Fix login bug",
      "body": "Users can't login",
      "state": "open",
      "labels": ["bug"],
      "assignees": ["alice"],
      "created_ts": 1699990000000,
      "updated_ts": 1700000000000,
      "comments": [
        {
          "body": "Investigating now",
          "actor": "64d15a2c...",
          "ts": 1699995000000
        }
      ],
      "links": [],
      "events": [...]
    }
  ]
}
```

### Markdown Format

```markdown
# Grit Export

Exported: 2024-01-15 10:30:00 UTC

## Open Issues

### Fix login bug

**ID:** 8057324b...
**State:** open
**Labels:** bug
**Assignees:** alice

Users can't login

#### Comments

- **64d15a2c** (2024-01-15 10:00:00): Investigating now

---

## Closed Issues

...
```

## Command Output

The export command returns metadata:

```bash
grit export --format json --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "format": "json",
    "output_path": ".grit/export.json",
    "wal_head": "abc123...",
    "event_count": 42
  }
}
```

## Incremental Exports

Export only changes since a point in time:

### Since Timestamp

```bash
grit export --format json --since 1699990000000
```

### Since Event ID

```bash
grit export --format json --since abc123def456...
```

This is useful for:

- Syncing to external dashboards
- Building change logs
- Incremental backups

## Use Cases

### Dashboard Integration

Export JSON and process with your dashboard tool:

```bash
grit export --format json
cat .grit/export.json | upload_to_dashboard.py
```

### Documentation Generation

Include issue summaries in docs:

```bash
grit export --format md
cat .grit/export.md >> docs/current-issues.md
```

### Backup

Create periodic backups:

```bash
grit export --format json
cp .grit/export.json "backups/export-$(date +%Y%m%d).json"
```

### CI Reports

Generate issue reports in CI:

```bash
# In CI pipeline
grit export --format md
# Attach .grit/export.md as artifact
```

## Scripting Examples

### Filter Open Issues

```bash
grit export --format json
cat .grit/export.json | jq '.issues[] | select(.state == "open")'
```

### Count by Label

```bash
grit export --format json
cat .grit/export.json | jq '.issues | group_by(.labels[]) | map({label: .[0].labels[0], count: length})'
```

### Recent Activity

```bash
grit export --format json
cat .grit/export.json | jq '.issues | sort_by(.updated_ts) | reverse | .[:5]'
```

## Output Location

By default, exports go to `.grit/` directory:

- `.grit/export.json`
- `.grit/export.md`

!!! note
    The `.grit/` directory is for exports only and is never canonical. The source of truth is always `refs/grit/wal`.

## Best Practices

### Regular Exports for Dashboards

```bash
# Cron job for hourly export
0 * * * * cd /path/to/repo && grit export --format json && upload.sh
```

### Incremental for Large Repos

For repositories with many issues, use incremental exports:

```bash
# Store last export timestamp
LAST_TS=$(cat .grit/last_export_ts 2>/dev/null || echo 0)
grit export --format json --since "$LAST_TS"
date +%s000 > .grit/last_export_ts
```

### Archive Before Major Changes

```bash
# Before migration or major refactor
grit export --format json
cp .grit/export.json "archives/pre-migration-$(date +%Y%m%d).json"
```

## Next Steps

- [CLI Reference](../reference/cli.md) - Full export command options
- [JSON Output](../reference/cli-json.md) - JSON schema details
- [Operations](../operations/index.md) - Backup and recovery
