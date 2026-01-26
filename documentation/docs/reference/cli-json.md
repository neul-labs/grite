# JSON Output

This document defines the JSON output schemas returned by `grite` when `--json` is provided.

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

### grite init

```json
{
  "actor_id": "64d15a2c383e2161772f9cea23e87222",
  "data_dir": ".git/grite/actors/64d15a2c.../",
  "repo_config": ".git/grite/config.toml"
}
```

### grite actor init

```json
{
  "actor_id": "64d15a2c383e2161772f9cea23e87222",
  "label": "work-laptop",
  "data_dir": ".git/grite/actors/64d15a2c.../"
}
```

### grite actor list

```json
{
  "actors": [
    {
      "actor_id": "64d15a2c383e2161772f9cea23e87222",
      "label": "work-laptop",
      "data_dir": ".git/grite/actors/64d15a2c.../"
    }
  ]
}
```

### grite actor show

```json
{
  "actor": {
    "actor_id": "64d15a2c383e2161772f9cea23e87222",
    "label": "work-laptop",
    "created_ts": 1700000000000
  }
}
```

### grite actor current

```json
{
  "actor_id": "64d15a2c383e2161772f9cea23e87222",
  "data_dir": ".git/grite/actors/64d15a2c.../",
  "source": "repo_default"
}
```

Source values: `repo_default`, `env`, `flag`, `auto`

### grite actor use

```json
{
  "default_actor": "64d15a2c383e2161772f9cea23e87222",
  "repo_config": ".git/grite/config.toml"
}
```

### grite issue create

```json
{
  "issue_id": "8057324b1e03afd613d4b428fdee657a",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grite issue update

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grite issue comment

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grite issue close

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "state": "closed",
  "wal_head": "abc123..."
}
```

### grite issue reopen

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "state": "open",
  "wal_head": "abc123..."
}
```

### grite issue label add/remove

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grite issue assignee add/remove

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grite issue link add

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grite issue attachment add

```json
{
  "issue_id": "8057324b...",
  "event_id": "a1b2c3d4...",
  "wal_head": "abc123..."
}
```

### grite issue dep add

```json
{
  "event_id": "a1b2c3d4...",
  "issue_id": "8057324b...",
  "target": "c4d5e6f7...",
  "dep_type": "blocks",
  "action": "added"
}
```

### grite issue dep remove

```json
{
  "event_id": "a1b2c3d4...",
  "issue_id": "8057324b...",
  "target": "c4d5e6f7...",
  "dep_type": "blocks",
  "action": "removed"
}
```

### grite issue dep list

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

### grite issue dep topo

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

### grite issue list

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

### grite issue show

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

### grite sync

```json
{
  "pulled": true,
  "pushed": true,
  "wal_head": "abc123...",
  "remote_wal_head": "abc123..."
}
```

### grite doctor

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

### grite rebuild

```json
{
  "wal_head": "abc123...",
  "event_count": 1234,
  "from_snapshot": "refs/grite/snapshots/1700000000000"
}
```

### grite db stats

```json
{
  "path": ".git/grite/actors/.../sled",
  "size_bytes": 1234567,
  "event_count": 1234,
  "issue_count": 12,
  "last_rebuild_ts": 1700000000000,
  "events_since_rebuild": 42,
  "days_since_rebuild": 3,
  "rebuild_recommended": false
}
```

### grite db check

```json
{
  "events_checked": 1234,
  "events_valid": 1234,
  "corrupt_count": 0,
  "errors": []
}
```

### grite db verify

```json
{
  "events_checked": 1234,
  "signatures_checked": 1000,
  "signatures_valid": 1000,
  "error_count": 0,
  "errors": []
}
```

### grite export

```json
{
  "format": "json",
  "output_path": ".grite/export.json",
  "wal_head": "abc123...",
  "event_count": 1234
}
```

### grite snapshot

```json
{
  "snapshot_ref": "refs/grite/snapshots/1700000000000",
  "wal_head": "abc123...",
  "event_count": 1234
}
```

### grite snapshot gc

```json
{
  "deleted": ["refs/grite/snapshots/1690000000000"]
}
```

### grite lock acquire/renew/release

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

### grite lock status

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

### grite lock gc

```json
{
  "expired_pruned": 3
}
```

### grite daemon status

```json
{
  "daemon": {
    "running": true,
    "pid": 12345,
    "endpoint": "ipc:///tmp/grite-daemon.sock",
    "workers": [
      {
        "repo_root": "/path/to/repo",
        "actor_id": "64d15a2c...",
        "data_dir": ".git/grite/actors/64d15a2c.../"
      }
    ]
  }
}
```

### grite daemon stop

```json
{
  "stopped": true
}
```

---

## Context Commands

### grite context index

```json
{
  "indexed": 42,
  "skipped": 15,
  "total_files": 57
}
```

### grite context query

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

### grite context show

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

### grite context project

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

### grite context set

```json
{
  "key": "api_version",
  "value": "v2",
  "action": "set"
}
```
