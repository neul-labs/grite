# IPC Schema

This document specifies the daemon IPC message schema used between `grite`
and `grite-daemon`. The wire format is `rkyv`-serialized structs transported over
NNG sockets. JSON examples are provided for readability; actual bytes are
`rkyv`.

## Versioning

- `ipc_schema_version`: starts at `1` and increments on breaking changes.
- Requests and responses must include the same version.

## Common envelope

### Request

```json
{
  "ipc_schema_version": 1,
  "request_id": "uuid",
  "repo_root": "/path/to/repo",
  "actor_id": "<hex-16-bytes>",
  "data_dir": ".git/grite/actors/<actor_id>",
  "command": { "...": "payload" }
}
```

### Response

```json
{
  "ipc_schema_version": 1,
  "request_id": "uuid",
  "ok": true,
  "data": { }
}
```

Errors:

```json
{
  "ipc_schema_version": 1,
  "request_id": "uuid",
  "ok": false,
  "error": {
    "code": "db_busy",
    "message": "data dir owned by another process",
    "details": { }
  }
}
```

Error codes match `docs/cli-json.md`.

## Command set

The daemon accepts the same logical commands as the CLI. Payloads are
equivalent to CLI flags and the response `data` matches the JSON schemas
in `docs/cli-json.md`.

### Examples

`IssueCreate` request payload:

```json
{ "IssueCreate": { "title": "...", "body": "...", "labels": ["bug"] } }
```

`IssueList` request payload:

```json
{ "IssueList": { "state": "open", "label": "bug" } }
```

`Sync` request payload:

```json
{ "Sync": { "pull": true, "push": true } }
```

## Discovery (SURVEY)

Discovery uses a `SURVEY` socket with a fixed message:

```json
{ "Discover": { "protocol": "grite-ipc", "min_version": 1 } }
```

Response:

```json
{
  "protocol": "grite-ipc",
  "ipc_schema_version": 1,
  "daemon_id": "uuid",
  "endpoint": "ipc://.../grite-daemon.sock",
  "workers": [
    { "repo_root": "/path/to/repo", "actor_id": "...", "data_dir": "..." }
  ]
}
```

## Notifications (PUB/SUB)

The daemon emits asynchronous notifications:

```json
{ "EventApplied": { "issue_id": "...", "event_id": "...", "ts_unix_ms": 0 } }
{ "WalSynced": { "wal_head": "<git-commit-hash>", "remote": "origin" } }
{ "LockChanged": { "resource": "path:docs/", "owner": "...", "expires_unix_ms": 0 } }
{ "SnapshotCreated": { "snapshot_ref": "refs/grite/snapshots/1700000000000" } }
```

Clients must treat unknown notification variants as ignorable.

## Timeouts and retries

- Request/response calls must have a bounded timeout (default 10s).
- On timeout, the CLI must retry using exponential backoff.
- Requests are idempotent only if the command itself is idempotent.
