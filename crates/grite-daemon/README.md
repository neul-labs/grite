# grite-daemon

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/grite-daemon.svg)](https://crates.io/crates/grite-daemon)
[![Documentation](https://img.shields.io/badge/docs-docs.rs-green.svg)](https://docs.rs/grite-daemon)
[![Build Status](https://img.shields.io/github/actions/workflow/status/neul-labs/grite/ci.yml?branch=main)](https://github.com/neul-labs/grite/actions)

**High-performance background daemon for Grite — eliminates per-command startup overhead and enables concurrent, multi-process access to the issue database.**

`grite-daemon` is the optional but highly recommended background service that powers the Grite CLI. It keeps materialized views warm in memory, handles concurrent IPC requests, and manages the lifecycle of per-actor databases. It is not required for correctness — the CLI works standalone — but it transforms Grite from a fast tool into a lightning-fast one.

---

## Why a Daemon?

Every time the `grite` CLI runs, it must:

1. Open the sled database
2. Check if the materialized view is stale
3. Replay new events from the git WAL into the view
4. Execute the query
5. Close the database

For simple queries, steps 1-3 dominate the runtime. With the daemon, these happen once. Subsequent commands are pure query execution against an in-memory cache.

### Performance Impact

| Scenario | Without Daemon | With Daemon | Speedup |
|----------|--------------|-------------|---------|
| First `issue list` | ~150ms | ~150ms | 1x |
| Second `issue list` | ~150ms | ~5ms | 30x |
| 50 concurrent `issue list` calls | Serialized, ~7s total | Parallel, ~200ms total | 35x |
| Agent batch query (100 issues) | ~2s | ~50ms | 40x |

The daemon pays for itself on the second command and provides massive speedups for agent workloads that issue hundreds of queries per session.

---

## What the Daemon Does

### Keep Databases Warm

The daemon opens each actor's sled database on first access and keeps it open. This eliminates the open/close overhead on every CLI invocation and allows the operating system's page cache to work effectively.

### Handle Concurrent Requests

Multiple CLI processes can connect to the daemon simultaneously over Unix domain sockets. The daemon routes requests to a pool of async workers, enabling parallel reads while serializing writes to maintain consistency.

### Auto-Spawn and Idle Shutdown

The daemon is completely hands-off:

- **Auto-spawn:** The first CLI command starts the daemon automatically if it is not running.
- **Idle shutdown:** The daemon stops automatically after a configurable period of inactivity (default: 5 minutes). No resource leaks. No manual cleanup.
- **Graceful shutdown:** Responds to `SIGINT` and `SIGTERM` by finishing in-flight requests and flushing state before exiting.

### Database Lock Coordination

The daemon uses filesystem-level locking (`flock`) to prevent database corruption from concurrent access, even when multiple processes try to open the same sled database. If the daemon is running, it holds the lock. If the CLI runs in standalone mode, it acquires the lock directly.

### Pub/Sub Infrastructure

The daemon architecture supports pub/sub notifications (enabled in future releases), allowing real-time issue change notifications for long-running agent sessions or editor integrations.

---

## Architecture

```
+-----------------------------------------------------------+
|                      grite-daemon                          |
|                                                           |
|  +----------------+     +-----------------------------+   |
|  | Supervisor     | --> | Unix Domain Socket Server   |   |
|  |                |     | (bind, accept, route)       |   |
|  +----------------+     +-----------------------------+   |
|         |                           |                     |
|         v                           v                     |
|  +----------------+     +-----------------------------+   |
|  | Signal Handler |     | Worker Pool                |   |
|  | SIGINT/SIGTERM |     | (async tokio tasks)        |   |
|  +----------------+     +-----------------------------+   |
|                                    |                      |
|                                    v                      |
|                           +-------------------+           |
|                           | Per-Actor Stores  |           |
|                           | (sled databases)  |           |
|                           +-------------------+           |
+-----------------------------------------------------------+
              ^
              | Unix domain socket (rkyv zero-copy)
              v
+-----------------------------------------------------------+
|                      grite (CLI)                          |
+-----------------------------------------------------------+
```

### Components

- **`Supervisor`** — Top-level lifecycle manager. Binds the socket, accepts connections, routes messages to workers, and coordinates graceful shutdown.
- **`Worker`** — Per-request async worker that executes CLI operations. Workers are spawned from a tokio task pool and run operations in `spawn_blocking` for CPU-intensive work.
- **`WorkerMessage`** — Command envelope that carries the operation type, arguments, and actor context from the CLI to the worker.
- **`shutdown_signal` / `setup_signal_handlers`** — Cross-platform signal handling for `SIGINT` and `SIGTERM`. Ensures the daemon finishes in-flight requests before exiting.

### IPC Protocol

The daemon communicates with the CLI over Unix domain sockets using [`libgrite-ipc`](../libgrite-ipc):

- **Serialization:** `rkyv` for zero-copy deserialization. Messages are laid out in memory exactly as they are received, with no parsing overhead.
- **Message types:** `IpcCommand` variants for every CLI operation (issue create, list, sync, lock, etc.), and `IpcResponse` variants for success, error, and streaming results.
- **Socket path:** Deterministic path based on the repository and actor ID, discovered via the daemon lock file.

---

## Usage

### As an End User

You rarely need to interact with the daemon directly. The CLI handles it automatically:

```bash
# The daemon starts automatically on first use
grite issue list

# It shuts down automatically after idle timeout
# (configurable, default 300 seconds)

# Manual control when needed
grite daemon start --idle-timeout 600
grite daemon status
grite daemon stop
```

### As a Library

Embed the daemon in your own Rust application:

```rust
use grite_daemon::Supervisor;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a supervisor with a custom socket path and idle timeout
    let supervisor = Supervisor::new(
        "/tmp/grite-daemon.sock",
        Some(Duration::from_secs(600)),
    );

    // Run until SIGINT or SIGTERM
    supervisor.run(shutdown_signal()).await?;

    Ok(())
}
```

### Configuration

Daemon behavior is controlled through the CLI and configuration files:

| Option | Default | Description |
|--------|---------|-------------|
| `--idle-timeout` | 300s | Shutdown after this many seconds of inactivity |
| `--socket-path` | Auto | Unix domain socket path (auto-detected from repo) |

Configuration can also be set in `.git/grite/config.toml`:

```toml
[daemon]
idle_timeout_seconds = 300
```

---

## Performance Characteristics

### Throughput

- **Request handling:** 1,000+ IPC requests per second
- **Concurrent clients:** 50+ simultaneous CLI processes
- **Worker pool:** Auto-scaled based on CPU cores

### Resource Usage

- **Idle memory:** ~10MB RSS (no open databases)
- **Active memory:** ~30MB RSS per actor database (sled cache + materialized view)
- **CPU:** Near-zero when idle; spikes during view rebuilds

### Startup Time

- **Cold start:** ~100ms (open databases, replay WAL delta)
- **Warm start:** ~5ms (database already open, minimal delta)

---

## Reliability

### Crash Safety

If the daemon crashes, no data is lost. The WAL lives in git refs, and the materialized view can be rebuilt at any time. The CLI falls back to standalone mode automatically.

### Zombie Process Prevention

The daemon writes a PID file and heartbeat. If a stale PID file is detected (daemon died without cleanup), it is overwritten on the next startup.

### Graceful Degradation

If the daemon socket is unavailable or corrupted, the CLI transparently falls back to direct database access. The user never sees an error — just slightly higher latency.

---

## See Also

- [Grite Repository](https://github.com/neul-labs/grite) — Main project and documentation
- [grite](../grite) — The CLI frontend that uses this daemon
- [libgrite-ipc](../libgrite-ipc) — IPC protocol and client library
- [docs.rs/grite-daemon](https://docs.rs/grite-daemon) — Rust API documentation

---

## License

MIT License — see [LICENSE](../LICENSE) for details.
