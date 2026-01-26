# Exporting Data

This guide explains how to export grite data for external use.

## Overview

Grite can export issues in two formats:

- **JSON**: Machine-readable for dashboards and integrations
- **Markdown**: Human-readable for documentation

## Export Formats

### JSON Export

Export all issues as JSON:

```bash
grite export --format json
```

Output is written to `.grite/export.json` by default.

### Markdown Export

Export as readable Markdown:

```bash
grite export --format md
```

Output is written to `.grite/export.md` by default.

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
# Grite Export

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
grite export --format json --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "format": "json",
    "output_path": ".grite/export.json",
    "wal_head": "abc123...",
    "event_count": 42
  }
}
```

## Incremental Exports

Export only changes since a point in time:

### Since Timestamp

```bash
grite export --format json --since 1699990000000
```

### Since Event ID

```bash
grite export --format json --since abc123def456...
```

This is useful for:

- Syncing to external dashboards
- Building change logs
- Incremental backups

## Use Cases

### Dashboard Integration

Export JSON and process with your dashboard tool:

```bash
grite export --format json
cat .grite/export.json | upload_to_dashboard.py
```

### Documentation Generation

Include issue summaries in docs:

```bash
grite export --format md
cat .grite/export.md >> docs/current-issues.md
```

### Backup

Create periodic backups:

```bash
grite export --format json
cp .grite/export.json "backups/export-$(date +%Y%m%d).json"
```

### CI Reports

Generate issue reports in CI:

```bash
# In CI pipeline
grite export --format md
# Attach .grite/export.md as artifact
```

## Scripting Examples

### Filter Open Issues

```bash
grite export --format json
cat .grite/export.json | jq '.issues[] | select(.state == "open")'
```

### Count by Label

```bash
grite export --format json
cat .grite/export.json | jq '.issues | group_by(.labels[]) | map({label: .[0].labels[0], count: length})'
```

### Recent Activity

```bash
grite export --format json
cat .grite/export.json | jq '.issues | sort_by(.updated_ts) | reverse | .[:5]'
```

## Output Location

By default, exports go to `.grite/` directory:

- `.grite/export.json`
- `.grite/export.md`

!!! note
    The `.grite/` directory is for exports only and is never canonical. The source of truth is always `refs/grite/wal`.

## Best Practices

### Regular Exports for Dashboards

```bash
# Cron job for hourly export
0 * * * * cd /path/to/repo && grite export --format json && upload.sh
```

### Incremental for Large Repos

For repositories with many issues, use incremental exports:

```bash
# Store last export timestamp
LAST_TS=$(cat .grite/last_export_ts 2>/dev/null || echo 0)
grite export --format json --since "$LAST_TS"
date +%s000 > .grite/last_export_ts
```

### Archive Before Major Changes

```bash
# Before migration or major refactor
grite export --format json
cp .grite/export.json "archives/pre-migration-$(date +%Y%m%d).json"
```

## Next Steps

- [CLI Reference](../reference/cli.md) - Full export command options
- [JSON Output](../reference/cli-json.md) - JSON schema details
- [Operations](../operations/index.md) - Backup and recovery
