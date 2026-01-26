# Daemon

The daemon (`grite-daemon`) is optional and exists only to improve performance and coordination. Correctness never depends on it.

## Quick Start

```bash
# Daemon auto-spawns on first command (no manual start needed)
grite issue list

# Manual control
grite daemon start --idle-timeout 300
grite daemon status
grite daemon stop

# Force local execution (skip daemon)
grite --no-daemon issue list
```

## Auto-Spawn

The daemon automatically spawns when you run CLI commands:

1. CLI checks for running daemon
2. If no daemon, spawns `grite-daemon` in background
3. Waits for daemon to become ready (up to 5 seconds)
4. Routes command through IPC
5. Daemon runs until idle timeout

Default idle timeout is 5 minutes (300 seconds).

### Disabling Auto-Spawn

Use `--no-daemon` to force local execution:

```bash
grite --no-daemon issue list
```

## Idle Timeout

The daemon automatically shuts down after a period of inactivity:

```bash
# Start with 10-minute idle timeout
grite daemon start --idle-timeout 600

# Start with no timeout (runs until stopped)
grite daemon start --idle-timeout 0
```

The idle timer resets on each command. When timeout is reached:

1. Daemon logs "Idle timeout reached"
2. Workers shut down gracefully
3. All locks released
4. Process exits

## Responsibilities

- Maintain a warm materialized view for fast reads
- Handle concurrent CLI requests efficiently
- Refresh daemon lock heartbeat
- Release locks on shutdown

## Non-Responsibilities

- Never rewrites refs or force-pushes
- Never writes to the working tree
- Never becomes required for correctness
- No background sync (sync is explicit via `grite sync`)

## Architecture

```
+----------------+     +----------------+     +----------------+
|    CLI         | --> |   Supervisor   | --> |    Worker      |
|  (grite)        |     |  (manages IPC) |     | (per repo/actor)|
+----------------+     +----------------+     +----------------+
        |                      |                      |
        v                      v                      v
   IPC Request          Route to Worker         Execute Command
                                                      |
                                                      v
                                               +-------------+
                                               | LockedStore |
                                               |   (sled)    |
                                               +-------------+
```

### Supervisor

- Listens on IPC socket (`ipc:///tmp/grite-daemon.sock`)
- Routes requests to appropriate worker
- Manages worker lifecycle
- Tracks idle time for auto-shutdown

### Worker

- One worker per (repo, actor) pair
- Holds exclusive `flock` on sled database
- Spawns concurrent tokio tasks for commands
- Refreshes daemon lock heartbeat

## Concurrency

The daemon handles concurrent requests efficiently:

1. Supervisor receives IPC request
2. Routes to worker for (repo, actor)
3. Worker spawns tokio task
4. Sled MVCC handles concurrent access
5. Response sent back via IPC

Multiple CLI processes can issue commands simultaneously. The daemon serializes database access internally while allowing concurrent execution.

## Database Locking

### Filesystem Lock (flock)

The daemon acquires an exclusive `flock` on `sled.lock`:

```
.git/grite/actors/<actor_id>/sled.lock
```

This prevents other processes from opening the sled database while the daemon is running.

### Daemon Lock (ownership marker)

The daemon creates a JSON lock file for coordination:

```
.git/grite/actors/<actor_id>/daemon.lock
```

Example:

```json
{
  "pid": 12345,
  "started_ts": 1700000000000,
  "repo_root": "/path/to/repo",
  "actor_id": "64d15a2c383e2161772f9cea23e87222",
  "host_id": "hostname",
  "ipc_endpoint": "ipc:///tmp/grite-daemon.sock",
  "lease_ms": 30000,
  "last_heartbeat_ts": 1700000000000,
  "expires_ts": 1700000030000
}
```

### Lock Rules

| Scenario | CLI Behavior |
|----------|--------------|
| No daemon lock | Execute locally or auto-spawn |
| Lock valid, IPC reachable | Route through daemon |
| Lock valid, IPC unreachable | Error (daemon may have crashed) |
| Lock expired | Take over, execute locally |

## CLI Integration

### Status

```bash
$ grite daemon status
Daemon is running
  PID:            12345
  Host ID:        my-laptop
  IPC Endpoint:   ipc:///tmp/grite-daemon.sock
  Started:        2024-01-15 10:30:00 UTC
  Expires in:     25s
```

### JSON Output

```bash
$ grite daemon status --json
{
  "running": true,
  "pid": 12345,
  "host_id": "my-laptop",
  "ipc_endpoint": "ipc:///tmp/grite-daemon.sock",
  "started_ts": 1705315800000,
  "expires_ts": 1705315830000,
  "time_remaining_ms": 25000
}
```

## Failure Behavior

| Failure | Recovery |
|---------|----------|
| Daemon crashes | Lock expires, CLI can take over |
| IPC timeout | CLI retries 3 times, then errors |
| Worker panics | Supervisor continues, worker restarted on next request |
| Command error | Error returned via IPC, daemon continues |

## Logging

The daemon logs to stderr. Control verbosity with `--log-level`:

```bash
grite-daemon --log-level debug
```

Log levels: `trace`, `debug`, `info`, `warn`, `error`

When auto-spawned, daemon runs with `--log-level info` and stdout/stderr redirected to `/dev/null`.

## IPC Protocol

- Socket: `ipc:///tmp/grite-daemon.sock`
- Serialization: rkyv (zero-copy)
- Pattern: REQ/REP (request/response)

See [IPC Protocol](ipc.md) for message format details.

## Configuration

The daemon reads configuration from:

1. Command-line arguments
2. Environment variables (for log level)

No configuration file is used. The daemon is stateless except for the workers it manages.

## Comparison: With vs Without Daemon

| Aspect | Without Daemon | With Daemon |
|--------|---------------|-------------|
| First command latency | Higher (open sled) | Lower (sled warm) |
| Concurrent commands | Serialize at flock | Concurrent in daemon |
| Memory usage | Per-process | Shared in daemon |
| Complexity | Simple | More moving parts |
| Correctness | Same | Same |
