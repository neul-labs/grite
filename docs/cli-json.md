# CLI JSON Output

This document defines the JSON output schemas returned by `grit` when `--json`
is provided. Schemas are versioned independently of the WAL schema.

## Envelope

All JSON responses use a common envelope:

```json
{
  "schema_version": 1,
  "ok": true,
  "data": { }
}
```

Errors are returned as:

```json
{
  "schema_version": 1,
  "ok": false,
  "error": {
    "code": "not_found",
    "message": "issue not found",
    "details": { }
  }
}
```

### Error codes (stable)

- `invalid_args`: CLI usage error or invalid flag value
- `not_found`: issue/actor/ref not found
- `conflict`: lock conflict or concurrent WAL update
- `db_busy`: data dir owned by another process or daemon
- `io_error`: filesystem error
- `git_error`: git command or ref failure
- `wal_error`: malformed WAL data or hash mismatch
- `ipc_error`: daemon IPC failure
- `internal_error`: unexpected error

### Exit codes

- `0`: success (`ok: true`)
- `2`: invalid arguments
- `3`: not found
- `4`: conflict or lock violation
- `5`: environment error (not a git repo, missing config, db busy)
- `1`: any other failure

## Common types

All IDs are lowercase hex without `0x`:

- `actor_id`: 16-byte hex
- `issue_id`: 16-byte hex
- `event_id`: 32-byte hex

Times are `ts_unix_ms` (milliseconds since Unix epoch).

For event objects:

- `actor` is an `actor_id`
- `parent` is an `event_id` or `null`

### Issue summary

```json
{
  "issue_id": "...",
  "title": "...",
  "state": "open",
  "labels": ["bug", "p0"],
  "assignees": ["alice"],
  "updated_ts": 1700000000000,
  "comment_count": 3
}
```

### Event

```json
{
  "event_id": "...",
  "issue_id": "...",
  "actor": "...",
  "ts_unix_ms": 1700000000000,
  "parent": null,
  "kind": { "IssueCreated": { "title": "...", "body": "...", "labels": ["bug"] } }
}
```

### Ordering

- Issue lists are sorted by `issue_id` (lexicographic).
- Event lists are sorted by `(issue_id, ts_unix_ms, actor, event_id)`.

## Command outputs

The JSON blocks below describe the `data` payload inside the envelope.

### `grit init`

```json
{
  "actor_id": "...",
  "data_dir": ".git/grit/actors/<actor_id>",
  "repo_config": ".git/grit/config.toml"
}
```

### `grit actor init`

```json
{
  "actor_id": "...",
  "label": "work-laptop",
  "data_dir": ".git/grit/actors/<actor_id>"
}
```

### `grit actor list`

```json
{ "actors": [ { "actor_id": "...", "label": "...", "data_dir": "..." } ] }
```

### `grit actor show`

```json
{ "actor": { "actor_id": "...", "label": "...", "created_ts": 1700000000000 } }
```

### `grit actor current`

```json
{ "actor_id": "...", "data_dir": "...", "source": "repo_default|env|flag|auto" }
```

### `grit actor use`

```json
{ "default_actor": "...", "repo_config": ".git/grit/config.toml" }
```

### `grit issue create`

```json
{ "issue_id": "...", "event_id": "...", "wal_head": "<git-commit-hash>" }
```

### `grit issue update`

```json
{ "issue_id": "...", "event_id": "...", "wal_head": "<git-commit-hash>" }
```

### `grit issue comment`

```json
{ "issue_id": "...", "event_id": "...", "wal_head": "<git-commit-hash>" }
```

### `grit issue close`

```json
{ "issue_id": "...", "event_id": "...", "state": "closed", "wal_head": "<git-commit-hash>" }
```

### `grit issue reopen`

```json
{ "issue_id": "...", "event_id": "...", "state": "open", "wal_head": "<git-commit-hash>" }
```

### `grit issue label add|remove`

```json
{ "issue_id": "...", "event_id": "...", "wal_head": "<git-commit-hash>" }
```

### `grit issue assignee add|remove`

```json
{ "issue_id": "...", "event_id": "...", "wal_head": "<git-commit-hash>" }
```

### `grit issue link add`

```json
{ "issue_id": "...", "event_id": "...", "wal_head": "<git-commit-hash>" }
```

### `grit issue attachment add`

```json
{ "issue_id": "...", "event_id": "...", "wal_head": "<git-commit-hash>" }
```

### `grit issue dep add`

```json
{
  "event_id": "...",
  "issue_id": "...",
  "target": "...",
  "dep_type": "blocks",
  "action": "added"
}
```

### `grit issue dep remove`

```json
{
  "event_id": "...",
  "issue_id": "...",
  "target": "...",
  "dep_type": "blocks",
  "action": "removed"
}
```

### `grit issue dep list`

```json
{
  "issue_id": "...",
  "direction": "dependencies|dependents",
  "deps": [
    { "issue_id": "...", "dep_type": "blocks", "title": "..." }
  ]
}
```

### `grit issue dep topo`

```json
{
  "issues": [
    { "issue_id": "...", "title": "...", "state": "open", "labels": ["..."] }
  ],
  "order": "topological"
}
```

### `grit issue list`

```json
{ "issues": [ { "...": "IssueSummary" } ], "total": 12 }
```

### `grit issue show`

```json
{
  "issue": { "...": "IssueSummary" },
  "events": [ { "...": "Event" } ]
}
```

### `grit sync`

```json
{
  "pulled": true,
  "pushed": true,
  "wal_head": "<git-commit-hash>",
  "remote_wal_head": "<git-commit-hash>"
}
```

### `grit doctor`

```json
{
  "checks": [
    { "id": "wal_ref", "status": "ok|warn|error", "message": "...", "plan": ["..."] }
  ],
  "applied": [ "rebuild", "fetch" ]
}
```

### `grit rebuild`

```json
{
  "wal_head": "<git-commit-hash>",
  "event_count": 1234,
  "from_snapshot": "refs/grit/snapshots/1700000000000"
}
```

### `grit db stats`

```json
{
  "path": ".git/grit/actors/<actor_id>/sled",
  "size_bytes": 1234567,
  "event_count": 1234,
  "issue_count": 12,
  "last_rebuild_ts": 1700000000000,
  "events_since_rebuild": 42,
  "days_since_rebuild": 3,
  "rebuild_recommended": false
}
```

### `grit db check`

```json
{
  "events_checked": 1234,
  "events_valid": 1234,
  "corrupt_count": 0,
  "errors": []
}
```

### `grit db verify`

```json
{
  "events_checked": 1234,
  "signatures_checked": 1000,
  "signatures_valid": 1000,
  "error_count": 0,
  "errors": []
}
```

### `grit export`

```json
{
  "format": "json|md",
  "output_path": ".grit/export.json",
  "wal_head": "<git-commit-hash>",
  "event_count": 1234
}
```

### `grit snapshot`

```json
{
  "snapshot_ref": "refs/grit/snapshots/1700000000000",
  "wal_head": "<git-commit-hash>",
  "event_count": 1234
}
```

### `grit snapshot gc`

```json
{ "deleted": ["refs/grit/snapshots/1690000000000"] }
```

### `grit lock acquire|renew|release`

```json
{
  "lock": {
    "resource": "path:docs/cli.md",
    "owner": "<actor_id>",
    "nonce": "<random>",
    "expires_unix_ms": 1700000000000
  }
}
```

### `grit lock status`

```json
{
  "locks": [ { "...": "lock" } ],
  "conflicts": [ { "resource": "...", "owner": "...", "expires_unix_ms": 1700000000000 } ]
}
```

### `grit lock gc`

```json
{ "expired_pruned": 3 }
```

### `grit daemon status`

```json
{
  "daemon": {
    "running": true,
    "pid": 12345,
    "endpoint": "ipc://.../grit-daemon.sock",
    "workers": [
      { "repo_root": "/path/to/repo", "actor_id": "...", "data_dir": "..." }
    ]
  }
}
```

### `grit daemon stop`

```json
{ "stopped": true }
```

### `grit context index`

```json
{
  "indexed": 42,
  "skipped": 15,
  "total_files": 57
}
```

### `grit context query`

```json
{
  "query": "Config",
  "matches": [
    { "symbol": "Config", "path": "src/config.rs" }
  ],
  "count": 1
}
```

### `grit context show`

```json
{
  "path": "src/main.rs",
  "language": "rust",
  "summary": "rust file with 2 functions: main, setup",
  "content_hash": "a1b2c3d4...",
  "symbols": [
    { "name": "main", "kind": "function", "line_start": 1, "line_end": 10 }
  ],
  "symbol_count": 1
}
```

### `grit context project`

```json
{
  "entries": [
    { "key": "api_version", "value": "v2" }
  ],
  "count": 1
}
```

### `grit context project <key>`

```json
{
  "key": "api_version",
  "value": "v2"
}
```

### `grit context set`

```json
{
  "key": "api_version",
  "value": "v2",
  "action": "set"
}
