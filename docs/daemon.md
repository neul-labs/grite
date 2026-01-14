# Daemon

The daemon (`gritd`) is optional and exists only to improve performance and coordination. Correctness never depends on it.

## Responsibilities

- Maintain a warm materialized view for fast reads
- Run background sync (`fetch`/`push` of `refs/grit/*`)
- Create snapshots opportunistically when thresholds are met
- Emit notifications (new events, sync status, lock changes)

## Non-responsibilities

- Never rewrites refs or force-pushes
- Never writes to the working tree
- Never becomes required for correctness

## Multi-repo and multi-actor

The daemon may manage multiple repositories and multiple actors simultaneously. Each managed actor is isolated by its data directory.

Key rule:

- Only one process may access a given actor data dir at a time. If the daemon owns a data dir, the CLI must route all commands for that actor through the daemon and must not open the DB directly.

## Ownership and routing rules (tight)

1. **Detect daemon for (repo, actor).** If present, route all commands via IPC.
2. **If no daemon is present**, the CLI takes ownership of the actor data dir and runs locally.
3. **If a daemon lock is present and unexpired but IPC is unreachable**, the CLI must refuse to use that data dir and instruct the user to stop the daemon or select a different actor/data dir.

These rules keep correctness independent of the daemon while preventing multi-process access to the same sled DB.

## Ownership marker (required)

Each actor data dir has an ownership marker file used for mutual exclusion:

- Path: `.git/grit/actors/<actor_id>/daemon.lock`
- Format: JSON

Example:

```json
{
  "pid": 12345,
  "started_ts": 1700000000000,
  "repo_root": "/path/to/repo",
  "actor_id": "<hex-16-bytes>",
  "host_id": "<stable-host-id>",
  "ipc_endpoint": "ipc://.../gritd.sock",
  "lease_ms": 30000,
  "last_heartbeat_ts": 1700000000000,
  "expires_ts": 1700000030000
}
```

Rules:

- The daemon creates the lock on startup and removes it on clean shutdown.
- The daemon refreshes the lock before `expires_ts` (heartbeat).
- The CLI checks for `daemon.lock` before opening the DB.
- If the lock exists and **is unexpired**, the CLI must not open the DB directly.
- If the lock exists, is unexpired, and IPC is reachable, the CLI routes all commands through the daemon.
- If the lock exists but is expired, the CLI may take ownership by replacing the lock.

Recommended model:

- `gritd` runs as a supervisor and spawns one worker per `(repo, actor_id)`.
- Each worker owns:
  - the actor data dir
  - its WAL sync loop
  - its snapshot policy

## IPC behavior

- `REQ/REP` for command execution (`issue create`, `list`, `sync`, `doctor`, `snapshot`)
- `PUB/SUB` for notifications
- `SURVEY` for daemon discovery

If IPC is unavailable and **no daemon lock is present** for the selected data dir, the CLI executes locally using its selected `--data-dir`.

## Discovery and routing

- `grit` checks for a daemon serving the current repo and actor context.
- If present, `grit` routes all commands through it for that actor.
- If absent, `grit` executes locally and takes ownership of the actor data dir.

## CLI integration

- `grit daemon status [--json]` reports daemon presence and managed `(repo, actor)` workers
- `grit daemon stop` requests a graceful shutdown of the global daemon

## Failure behavior

- If a daemon worker crashes, the CLI can fall back to local execution after the lock/ownership is released.
- If background sync fails, the daemon reports the error but never rewrites history.
