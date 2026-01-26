# JSON Output

This document defines the JSON output schemas returned by `grit` when `--json` is provided.

## Response Envelope

All JSON responses use a common envelope:

### Success

```json
{
  "schema_version": 1,
  "ok": true,
  "data": { ... }
}
```

### Error

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

## Error Codes

| Code | Description |
|------|-------------|
| `invalid_args` | CLI usage error or invalid flag value |
| `not_found` | Issue/actor/ref not found |
| `conflict` | Lock conflict or concurrent WAL update |
| `db_busy` | Data dir owned by another process or daemon |
| `io_error` | Filesystem error |
| `git_error` | Git command or ref failure |
| `wal_error` | Malformed WAL data or hash mismatch |
| `ipc_error` | Daemon IPC failure |
| `internal_error` | Unexpected error |

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success (`ok: true`) |
| `2` | Invalid arguments |
| `3` | Not found |
| `4` | Conflict or lock violation |
| `5` | Environment error |
| `1` | Any other failure |

## Common Types

### IDs

All IDs are lowercase hex without `0x`:

- `actor_id`: 16-byte hex (32 characters)
- `issue_id`: 16-byte hex (32 characters)
- `event_id`: 32-byte hex (64 characters)

### Timestamps

`ts_unix_ms`: milliseconds since Unix epoch

### Issue Summary

```json
{
  "issue_id": "8057324b1e03afd613d4b428fdee657a",
  "title": "Fix login bug",
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
  "event_id": "a1b2c3d4...",
  "issue_id": "8057324b...",
  "actor": "64d15a2c...",
  "ts_unix_ms": 1700000000000,
  "parent": null,
  "kind": {
    "IssueCreated": {
      "title": "Fix login bug",
      "body": "Users can't login",
      "labels": ["bug"]
    }
  }
}
```

### Ordering

- Issue lists: sorted by `issue_id` (lexicographic)
- Event lists: sorted by `(issue_id, ts_unix_ms, actor, event_id)`

---

## Command Outputs

The `data` payload for each command:

### grit init

```json
{
  "actor_id": "64d15a2c383e2161772f9cea23e87222",
  "data_dir": ".git/grit/actors/64d15a2c.../",
  "repo_config": ".git/grit/config.toml"
}
```

### grit actor init

```json
{
  "actor_id": "64d15a2c383e2161772f9cea23e87222",
  "label": "work-laptop",
  "data_dir": ".git/grit/actors/64d15a2c.../"
}
```

### grit actor list

```json
{
  "actors": [
    {
      "actor_id": "64d15a2c383e2161772f9cea23e87222",
      "label": "work-laptop",
      "data_dir": ".git/grit/actors/64d15a2c.../"
    }
  ]
}
```

### grit actor show

```json
{
  "actor": {
    "actor_id": "64d15a2c383e2161772f9cea23e87222",
    "label": "work-laptop",
    "created_ts": 1700000000000
  }
}
```

### grit actor current

```json
{
  "actor_id": "64d15a2c383e2161772f9cea23e87222",
  "data_dir": ".git/grit/actors/64d15a2c.../",
  "source": "repo_default"
}
```

Source values: `repo_default`, `env`, `flag`, `auto`

### grit actor use

```json
{
  "default_actor": "64d15a2c383e2161772f9cea23e87222",
  "repo_config": ".git/grit/config.toml"
}
```

### grit issue create

```json
{
  "issue_id": "8057324b1e03afd613d4b428fdee657a",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grit issue update

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grit issue comment

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grit issue close

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "state": "closed",
  "wal_head": "abc123..."
}
```

### grit issue reopen

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "state": "open",
  "wal_head": "abc123..."
}
```

### grit issue label add/remove

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grit issue assignee add/remove

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grit issue link add

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grit issue attachment add

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grit issue dep add

```json
{
  "event_id": "a1b2c3d4...",
  "issue_id": "8057324b...",
  "target": "c4d5e6f7...",
  "dep_type": "blocks",
  "action": "added"
}
```

### grit issue dep remove

```json
{
  "event_id": "a1b2c3d4...",
  "issue_id": "8057324b...",
  "target": "c4d5e6f7...",
  "dep_type": "blocks",
  "action": "removed"
}
```

### grit issue dep list

```json
{
  "issue_id": "8057324b...",
  "direction": "dependencies",
  "deps": [
    {
      "issue_id": "c4d5e6f7...",
      "dep_type": "blocks",
      "title": "Fix login page"
    }
  ]
}
```

### grit issue dep topo

```json
{
  "issues": [
    {
      "issue_id": "8057324b...",
      "title": "Design API",
      "state": "open",
      "labels": ["sprint-1"]
    }
  ],
  "order": "topological"
}
```

### grit issue list

```json
{
  "issues": [
    {
      "issue_id": "...",
      "title": "...",
      "state": "open",
      "labels": [],
      "assignees": [],
      "updated_ts": 1700000000000,
      "comment_count": 0
    }
  ],
  "total": 12
}
```

### grit issue show

```json
{
  "issue": {
    "issue_id": "...",
    "title": "...",
    "state": "open",
    "labels": [],
    "assignees": [],
    "updated_ts": 1700000000000,
    "comment_count": 0
  },
  "events": [
    {
      "event_id": "...",
      "issue_id": "...",
      "actor": "...",
      "ts_unix_ms": 1700000000000,
      "parent": null,
      "kind": { "IssueCreated": { ... } }
    }
  ]
}
```

### grit sync

```json
{
  "pulled": true,
  "pushed": true,
  "wal_head": "abc123...",
  "remote_wal_head": "abc123..."
}
```

### grit doctor

```json
{
  "checks": [
    {
      "id": "wal_ref",
      "status": "ok",
      "message": "WAL ref exists",
      "plan": []
    }
  ],
  "applied": []
}
```

Status values: `ok`, `warn`, `error`

### grit rebuild

```json
{
  "wal_head": "abc123...",
  "event_count": 1234,
  "from_snapshot": "refs/grit/snapshots/1700000000000"
}
```

### grit db stats

```json
{
  "path": ".git/grit/actors/.../sled",
  "size_bytes": 1234567,
  "event_count": 1234,
  "issue_count": 12,
  "last_rebuild_ts": 1700000000000,
  "events_since_rebuild": 42,
  "days_since_rebuild": 3,
  "rebuild_recommended": false
}
```

### grit db check

```json
{
  "events_checked": 1234,
  "events_valid": 1234,
  "corrupt_count": 0,
  "errors": []
}
```

### grit db verify

```json
{
  "events_checked": 1234,
  "signatures_checked": 1000,
  "signatures_valid": 1000,
  "error_count": 0,
  "errors": []
}
```

### grit export

```json
{
  "format": "json",
  "output_path": ".grit/export.json",
  "wal_head": "abc123...",
  "event_count": 1234
}
```

### grit snapshot

```json
{
  "snapshot_ref": "refs/grit/snapshots/1700000000000",
  "wal_head": "abc123...",
  "event_count": 1234
}
```

### grit snapshot gc

```json
{
  "deleted": ["refs/grit/snapshots/1690000000000"]
}
```

### grit lock acquire/renew/release

```json
{
  "lock": {
    "resource": "issue:8057324b...",
    "owner": "64d15a2c...",
    "nonce": "abc123...",
    "expires_unix_ms": 1700003600000
  }
}
```

### grit lock status

```json
{
  "locks": [
    {
      "resource": "issue:8057324b...",
      "owner": "64d15a2c...",
      "expires_unix_ms": 1700003600000
    }
  ],
  "conflicts": []
}
```

### grit lock gc

```json
{
  "expired_pruned": 3
}
```

### grit daemon status

```json
{
  "daemon": {
    "running": true,
    "pid": 12345,
    "endpoint": "ipc:///tmp/grit-daemon.sock",
    "workers": [
      {
        "repo_root": "/path/to/repo",
        "actor_id": "64d15a2c...",
        "data_dir": ".git/grit/actors/64d15a2c.../"
      }
    ]
  }
}
```

### grit daemon stop

```json
{
  "stopped": true
}
```

---

## Context Commands

### grit context index

```json
{
  "indexed": 42,
  "skipped": 15,
  "total_files": 57
}
```

### grit context query

```json
{
  "query": "Config",
  "matches": [
    {
      "symbol": "Config",
      "path": "src/config.rs"
    }
  ],
  "count": 1
}
```

### grit context show

```json
{
  "path": "src/main.rs",
  "language": "rust",
  "summary": "rust file with 2 functions: main, setup",
  "content_hash": "a1b2c3d4...",
  "symbols": [
    {
      "name": "main",
      "kind": "function",
      "line_start": 1,
      "line_end": 10
    }
  ],
  "symbol_count": 1
}
```

### grit context project

Without key (list all):

```json
{
  "entries": [
    { "key": "api_version", "value": "v2" }
  ],
  "count": 1
}
```

With key:

```json
{
  "key": "api_version",
  "value": "v2"
}
```

### grit context set

```json
{
  "key": "api_version",
  "value": "v2",
  "action": "set"
}
```
