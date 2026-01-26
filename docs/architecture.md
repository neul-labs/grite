# Architecture

## Overview

Grit is split into three layers:

1. **Git-backed WAL** (source of truth)
   - Append-only events in `refs/grit/wal`
   - No tracked files in the working tree
   - Synced via standard `git fetch/push`

2. **Materialized view** (fast local query)
   - `sled` embedded database in `.git/grit/actors/<actor_id>/sled/`
   - Deterministic projections from the WAL
   - Can be deleted and rebuilt at any time

3. **Optional daemon** (performance optimization)
   - Auto-spawns on first CLI command
   - Handles concurrent requests efficiently
   - Auto-shuts down after idle timeout

Correctness never depends on the daemon; the CLI can always rebuild state from the WAL.

## Crate Structure

```
grit/
  crates/
    libgrit-core/     # Core library
    libgrit-git/      # Git integration
    libgrit-ipc/      # IPC protocol
    grit/             # CLI binary
    grit-daemon/            # Daemon binary
```

### libgrit-core

Core types and logic, no git or IPC dependencies.

| Module | Purpose |
|--------|---------|
| `types::event` | `Event`, `EventKind`, `IssueState` |
| `types::ids` | `ActorId`, `IssueId`, `EventId`, hex conversion |
| `types::issue` | `IssueProjection`, `IssueSummary`, `Version` |
| `types::actor` | `ActorConfig` |
| `hash` | BLAKE2b-256 canonical event hashing |
| `projection` | CRDT projection logic (LWW, add/remove sets) |
| `store` | Sled database operations, `LockedStore` |
| `signing` | Ed25519 key generation and verification |
| `integrity` | Event hash and signature verification |
| `export` | JSON and Markdown export |
| `config` | Configuration file handling |
| `lock` | Lock policy and conflict detection |

### libgrit-git

Git operations using libgit2.

| Module | Purpose |
|--------|---------|
| `wal` | WAL append/read, chunk encoding (CBOR) |
| `snapshot` | Snapshot creation and garbage collection |
| `sync` | Push/pull of `refs/grit/*` |
| `lock_manager` | Distributed lease locks via git refs |
| `chunk` | Binary chunk format for WAL entries |

### libgrit-ipc

Inter-process communication using nng (nanomsg-next-gen).

| Module | Purpose |
|--------|---------|
| `messages` | `IpcRequest`, `IpcResponse`, `IpcCommand` |
| `client` | IPC client with retry logic |
| `lock` | `DaemonLock` for process coordination |
| `notifications` | Pub/sub notification types |
| `discovery` | Daemon discovery protocol |

### grit (CLI)

Command-line interface.

| Module | Purpose |
|--------|---------|
| `cli` | Clap argument definitions |
| `context` | Actor resolution, execution mode |
| `router` | Command routing (local vs daemon) |
| `commands/*` | Individual command implementations |
| `output` | JSON/human output formatting |

### grit-daemon (Daemon)

Background daemon process.

| Module | Purpose |
|--------|---------|
| `supervisor` | Worker management, IPC socket handling |
| `worker` | Per-(repo, actor) command execution |
| `error` | Daemon-specific error types |

## Data Flow

### Write Path

```
1. CLI creates Event
2. Event signed (optional)
3. Event inserted into sled
4. Event appended to WAL (git commit)
5. Materialized view updated
```

### Read Path

```
1. CLI queries sled
2. Returns IssueProjection
```

### Sync Path

```
1. git fetch refs/grit/*
2. New WAL entries read
3. Events inserted into sled
4. Projections rebuilt
5. git push refs/grit/* (if pushing)
```

## Concurrency Model

### Without Daemon

Each CLI invocation acquires an exclusive `flock` on `sled.lock`:

```
CLI 1: acquire flock -> open sled -> execute -> release flock
CLI 2: acquire flock -> open sled -> execute -> release flock
```

Concurrent CLI calls serialize at the flock level.

### With Daemon

The daemon holds the flock for its lifetime. CLI routes through IPC:

```
Daemon: acquire flock -> hold sled open
CLI 1: send IPC request -> daemon executes -> receive response
CLI 2: send IPC request -> daemon executes -> receive response
```

The daemon spawns concurrent tokio tasks for each request. Sled handles internal concurrency via MVCC.

### Daemon Auto-Spawn

When CLI runs without daemon:

```
1. Check for daemon.lock
2. If no lock, try to spawn grit-daemon
3. Wait for daemon to be ready
4. Route command through IPC
5. Daemon auto-shuts down after idle timeout
```

## Storage Footprint

### Per-Repository

```
.git/grit/
  config.toml                      # Repo config (default_actor, lock_policy)
```

### Per-Actor

```
.git/grit/actors/<actor_id>/
  config.toml                      # Actor config (actor_id, label, public_key)
  sled/                            # Materialized view
  sled.lock                        # flock for exclusive access
  daemon.lock                      # Daemon ownership (JSON)
  keys/
    signing.key                    # Ed25519 private key (optional)
```

### Git Refs

```
refs/grit/
  wal                              # Append-only event log
  snapshots/<timestamp>            # Periodic full snapshots
  locks/<resource_hash>            # Distributed lease locks
```

## Error Handling

| Error | Behavior |
|-------|----------|
| Sled locked | Return `DbBusy`, CLI may retry or route to daemon |
| Daemon unreachable | Fall back to local execution |
| WAL append fails | Return error, no partial writes |
| Sync conflict | Union merge, no data loss |
| Signature invalid | Event flagged but not rejected |

## Security Model

- **Actor isolation**: Each actor has separate data directory
- **Signing optional**: Events can be unsigned, signed, or verified
- **No secrets in WAL**: Private keys never leave local config
- **Lock policy**: Configurable (off, warn, enforce)

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Issue list | O(n) | Scans issue index |
| Issue show | O(1) | Direct sled lookup |
| Event insert | O(1) | Sled + WAL append |
| Rebuild | O(events) | Full projection rebuild |
| Sync | O(new events) | Incremental from WAL |
