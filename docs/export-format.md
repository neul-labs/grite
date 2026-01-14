# Export Format

Exports are generated snapshots of the current materialized view and are never canonical. They are intended for dashboards, reports, and integrations.

## Output location

- Default: `.grit/`
- Configurable via CLI in the future

## JSON schema (v1)

```json
{
  "meta": {
    "schema_version": 1,
    "generated_ts": 1700000000000,
    "wal_head": "<git-commit-hash>",
    "event_count": 1234
  },
  "issues": [
    {
      "issue_id": "<hex-16-bytes>",
      "title": "...",
      "state": "open",
      "labels": ["bug", "p0"],
      "assignees": ["alice"],
      "updated_ts": 1700000000000,
      "comment_count": 3
    }
  ],
  "events": [
    {
      "event_id": "<hex-32-bytes>",
      "issue_id": "<hex-16-bytes>",
      "actor": "<hex-16-bytes>",
      "ts_unix_ms": 1700000000000,
      "parent": null,
      "kind": { "IssueCreated": { "title": "...", "body": "...", "labels": ["bug"] } }
    }
  ]
}
```

### Ordering rules

- `issues` sorted by `issue_id` (lexicographic)
- `events` sorted by `(issue_id, ts_unix_ms, event_id)`

## Markdown export

Markdown exports are human-readable summaries and follow the same ordering rules as JSON.

## Incremental exports (`--since`)

`grit export --since <ts|event_id>` limits output to changes after a point-in-time.

- If `--since` is a timestamp: include events with `ts_unix_ms` **greater than** the timestamp.
- If `--since` is an event ID: include events **after** that event in `(issue_id, ts_unix_ms, event_id)` order.

The `meta.event_count` reflects the number of events included in the export.
