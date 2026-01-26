# Using the Daemon

This guide explains the grite daemon and how to use it effectively.

## Overview

The daemon (`grite-daemon`) is an optional background process that:

- Keeps the materialized view warm for fast queries
- Handles concurrent CLI requests efficiently
- Reduces database open/close overhead

!!! important
    Correctness never depends on the daemon. The CLI works standalone.

## Auto-Spawn

By default, the daemon auto-spawns when you run CLI commands:

```bash
grite issue list  # Daemon spawns automatically
```

The process:

1. CLI checks for running daemon
2. If none, spawns `grite-daemon` in background
3. Waits for daemon to be ready (up to 5 seconds)
4. Routes command through IPC
5. Daemon runs until idle timeout (default: 5 minutes)

## Manual Control

### Start Daemon

```bash
grite daemon start
```

With custom idle timeout:

```bash
grite daemon start --idle-timeout 600  # 10 minutes
```

### Check Status

```bash
grite daemon status
```

Output:

```
Daemon is running
  PID:            12345
  Host ID:        my-laptop
  IPC Endpoint:   ipc:///tmp/grite-daemon.sock
  Started:        2024-01-15 10:30:00 UTC
  Expires in:     4m 30s
```

### Stop Daemon

```bash
grite daemon stop
```

## Idle Timeout

The daemon shuts down automatically after a period of inactivity.

### Default Behavior

- Default timeout: 5 minutes (300 seconds)
- Timer resets on each command
- When timeout reached, daemon exits gracefully

### Custom Timeout

```bash
# Start with 10-minute timeout
grite daemon start --idle-timeout 600

# No timeout (runs until stopped)
grite daemon start --idle-timeout 0
```

## Skipping the Daemon

Force local execution without the daemon:

```bash
grite --no-daemon issue list
```

Use this when:

- Debugging daemon issues
- Running in minimal environments
- Avoiding daemon startup latency for one-off commands

## Performance Benefits

| Aspect | Without Daemon | With Daemon |
|--------|---------------|-------------|
| First command latency | Higher (open sled DB) | Lower (DB already open) |
| Concurrent commands | Serialize at flock | Concurrent in daemon |
| Memory usage | Per-process | Shared in daemon |
| Subsequent commands | Reopen DB each time | DB stays warm |

The daemon is most beneficial when:

- Running many commands in sequence
- Multiple processes issue commands concurrently
- Working with large databases

## How It Works

### Architecture

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

### Components

**Supervisor**:

- Listens on IPC socket
- Routes requests to workers
- Tracks idle time for auto-shutdown

**Worker**:

- One per (repo, actor) pair
- Holds exclusive database lock
- Executes commands concurrently via tokio

## Database Locking

### Without Daemon

Each CLI invocation acquires an exclusive `flock`:

```
CLI 1: acquire flock -> open sled -> execute -> release flock
CLI 2: acquire flock -> open sled -> execute -> release flock
```

Commands serialize at the lock.

### With Daemon

Daemon holds the lock for its lifetime:

```
Daemon: acquire flock -> hold sled open
CLI 1: send IPC request -> daemon executes -> receive response
CLI 2: send IPC request -> daemon executes -> receive response
```

Commands execute concurrently within the daemon.

## Status JSON Output

```bash
grite daemon status --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "daemon": {
      "running": true,
      "pid": 12345,
      "endpoint": "ipc:///tmp/grite-daemon.sock",
      "workers": [
        {
          "repo_root": "/path/to/repo",
          "actor_id": "64d15a2c383e2161772f9cea23e87222",
          "data_dir": ".git/grite/actors/64d15a2c.../"
        }
      ]
    }
  }
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

Control daemon log level:

```bash
grite-daemon --log-level debug
```

Log levels: `trace`, `debug`, `info`, `warn`, `error`

When auto-spawned, daemon runs with `--log-level info` and output redirected to `/dev/null`.

## Troubleshooting

### "DbBusy" Error

The database is locked by another process.

```bash
# Check daemon status
grite daemon status

# If daemon is stuck, stop it
grite daemon stop

# Or use --no-daemon
grite --no-daemon issue list
```

### "IPC Error"

Can't connect to daemon.

```bash
# Restart daemon
grite daemon stop
grite daemon start
```

### Daemon Not Starting

Check for stale lock files:

```bash
ls -la .git/grite/actors/*/daemon.lock

# Remove stale locks
rm .git/grite/actors/*/daemon.lock
```

### High Memory Usage

Daemon keeps database in memory. Stop it when not needed:

```bash
grite daemon stop
```

Or use shorter idle timeout:

```bash
grite daemon start --idle-timeout 60  # 1 minute
```

## Best Practices

### Let It Auto-Spawn

For most use cases, auto-spawn is fine. Don't manually manage the daemon unless needed.

### Use Appropriate Timeouts

| Use Case | Suggested Timeout |
|----------|-------------------|
| Active development | 5-10 minutes |
| CI/CD | 1-2 minutes |
| Batch scripts | 30 seconds |
| Long-running services | 0 (no timeout) |

### Stop When Not Needed

```bash
# End of work session
grite daemon stop
```

### Use --no-daemon for Scripts

In scripts that run once and exit:

```bash
#!/bin/bash
grite --no-daemon issue list --json | process_issues.py
```

## Next Steps

- [Actor Identity](actors.md) - Multiple actors with daemon
- [Operations](../operations/index.md) - Daemon troubleshooting
- [Architecture](../architecture/index.md) - Technical details
