# Three-Layer Design

Grite uses a three-layer architecture that separates the source of truth from the query layer and user interface.

## Overview

```
+------------------+     +-------------------+     +------------------+
|   Git WAL        | --> | Materialized View | <-- | CLI / Daemon     |
| refs/grite/wal    |     | sled database     |     | grite / grite-daemon     |
| (source of truth)|     | (fast queries)    |     | (user interface) |
+------------------+     +-------------------+     +------------------+
```

## Layer 1: Git-backed WAL (Source of Truth)

The Write-Ahead Log (WAL) is the authoritative source of all state.

### Location

```
refs/grite/wal
```

### Characteristics

- **Append-only**: Events are only added, never modified
- **Git-native**: Stored as git refs, synced via `git fetch/push`
- **No working tree**: Never writes to tracked files
- **Portable**: Works with any git remote

### Format

Events are stored in CBOR-encoded chunks within git blobs:

```
refs/grite/wal
  └── points to commit
        └── tree with blob chunks
              └── CBOR-encoded events
```

See [Git WAL](git-wal.md) for detailed format.

### Guarantees

- All state can be derived from the WAL
- WAL can be replayed from any point
- Syncs reliably via standard git operations

## Layer 2: Materialized View (Fast Queries)

The materialized view is a local cache for fast queries.

### Location

```
.git/grite/actors/<actor_id>/sled/
```

### Characteristics

- **Derived**: Built from WAL events
- **Expendable**: Can be deleted and rebuilt
- **Fast**: Indexed for quick lookups
- **Per-actor**: Each actor has its own database

### Storage Engine

Uses [sled](https://sled.rs/), an embedded MVCC database:

- Single-writer, multi-reader
- Crash-safe
- Zero-copy reads

### Key Layout

| Key Pattern | Value | Purpose |
|-------------|-------|---------|
| `event/<event_id>` | Archived Event | Event storage |
| `issue_state/<issue_id>` | IssueProjection | Current issue state |
| `issue_events/<issue_id>/<ts>/<event_id>` | Empty | Event index |
| `label_index/<label>/<issue_id>` | Empty | Label index |

### Rebuilding

The materialized view can be rebuilt at any time:

```bash
grite rebuild                 # From local events
grite rebuild --from-snapshot # From snapshot (faster)
```

## Layer 3: CLI / Daemon (User Interface)

The user interface layer provides access to grite functionality.

### CLI (grite)

The command-line interface:

- Executes commands locally or via daemon
- Non-interactive by default
- JSON output for scripting
- Auto-spawns daemon when needed

### Daemon (grite-daemon)

Optional background process:

- Keeps database warm
- Handles concurrent requests
- Auto-spawns on first command
- Auto-shuts down after idle timeout

### Execution Modes

| Mode | Behavior |
|------|----------|
| Local (no daemon) | CLI acquires flock, executes directly |
| Local (with daemon) | CLI routes through IPC to daemon |
| Remote | Daemon handles multiple CLI clients |

## Concurrency Model

### Without Daemon

Each CLI invocation acquires an exclusive `flock`:

```
CLI 1: acquire flock -> open sled -> execute -> release flock
CLI 2: (waits) -> acquire flock -> open sled -> execute -> release flock
```

Commands serialize at the lock level.

### With Daemon

Daemon holds the flock; CLI routes through IPC:

```
Daemon: acquire flock -> hold sled open
CLI 1: send IPC request -> daemon executes -> receive response
CLI 2: send IPC request -> daemon executes -> receive response
```

The daemon uses tokio for concurrent task execution. Sled handles internal concurrency via MVCC.

### Daemon Auto-Spawn

```
1. CLI starts
2. Check for daemon.lock
3. If no lock, spawn grite-daemon in background
4. Wait for daemon to be ready
5. Route command through IPC
6. Daemon auto-shuts down after idle timeout
```

## Data Flow Examples

### Creating an Issue

```
1. CLI: grite issue create --title "..."
2. Create Event struct
3. Compute event_id (BLAKE2b hash)
4. Sign event (optional)
5. Insert into sled
6. Append to WAL (git commit)
7. Return issue_id
```

### Listing Issues

```
1. CLI: grite issue list
2. Query sled issue_state index
3. Deserialize IssueProjection structs
4. Filter by state/labels
5. Format and return
```

### Syncing

```
1. CLI: grite sync
2. git fetch refs/grite/* from remote
3. Read new WAL entries
4. Insert events into sled
5. Rebuild affected projections
6. git push refs/grite/* to remote
7. Handle conflicts via auto-rebase
```

## Error Handling

| Error | Layer | Behavior |
|-------|-------|----------|
| Sled locked | L2 | Return DbBusy, CLI may route to daemon |
| Daemon unreachable | L3 | Fall back to local execution |
| WAL append fails | L1 | Return error, no partial writes |
| Sync conflict | L1 | Union merge via CRDT, no data loss |
| Signature invalid | L1 | Event flagged but not rejected |

## Security Model

- **Actor isolation**: Each actor has separate data directory
- **Signing optional**: Events can be unsigned, signed, or verified
- **No secrets in WAL**: Private keys never leave local config
- **Lock policy**: Configurable enforcement

## Design Rationale

### Why Git?

- Reliable distributed storage
- Built-in sync protocol
- Works with existing infrastructure
- No external services required

### Why Sled?

- Embedded (no separate server)
- MVCC for concurrent reads
- Crash-safe
- Good performance for this workload

### Why Daemon Optional?

- Correctness shouldn't depend on daemon
- Simple deployments work without it
- Daemon is purely a performance optimization

## Next Steps

- [Data Model](data-model.md) - Event structure and hashing
- [Git WAL](git-wal.md) - WAL format details
- [Storage Layout](storage.md) - File organization
