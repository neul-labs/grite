# Architecture

This section covers the technical architecture of grite.

## Overview

Grite is split into three layers:

```
+------------------+     +-------------------+     +------------------+
|   Git WAL        | --> | Materialized View | <-- | CLI / Daemon     |
| refs/grite/wal    |     | sled database     |     | grite / grite-daemon     |
| (source of truth)|     | (fast queries)    |     | (user interface) |
+------------------+     +-------------------+     +------------------+
```

1. **[Git-backed WAL](three-layer.md)** - Append-only events in `refs/grite/wal`
2. **[Materialized View](three-layer.md#layer-2-materialized-view)** - Fast local queries via sled database
3. **[Optional Daemon](three-layer.md#layer-3-cli--daemon)** - Performance optimization

Correctness never depends on the daemon; the CLI can always rebuild state from the WAL.

## Key Design Principles

1. **Git is the source of truth** - All state derivable from `refs/grite/*`
2. **No working tree pollution** - Never writes tracked files
3. **Daemon optional** - CLI works standalone
4. **Deterministic merges** - CRDT semantics, no manual conflicts
5. **Per-actor isolation** - Multiple agents work independently

## Documentation

### [Three-Layer Design](three-layer.md)

Detailed explanation of the three-layer architecture:

- Git WAL (source of truth)
- Materialized view (fast queries)
- CLI and daemon (user interface)

### [Data Model](data-model.md)

Event schema and data structures:

- Event types and schema
- ID types (ActorId, IssueId, EventId)
- Canonical encoding and hashing

### [CRDT Merging](crdt-merging.md)

Conflict-free merge semantics:

- Last-writer-wins fields
- Add/remove sets
- Deterministic projection

### [Git WAL](git-wal.md)

Write-ahead log format:

- WAL structure
- Chunk encoding
- Sync operations

### [Storage Layout](storage.md)

File and directory structure:

- Repository files
- Actor files
- Git refs

## Crate Structure

```
grite/
  crates/
    libgrite-core/     # Core library (no git/IPC deps)
    libgrite-git/      # Git integration
    libgrite-ipc/      # IPC protocol
    grite/             # CLI binary
    grite-daemon/            # Daemon binary
```

| Crate | Purpose |
|-------|---------|
| `libgrite-core` | Event types, hashing, projections, sled store, signing |
| `libgrite-git` | WAL commits, ref sync, snapshots, distributed locks |
| `libgrite-ipc` | IPC message schemas (rkyv), daemon lock, client/server |
| `grite` | CLI frontend |
| `grite-daemon` | Optional background daemon |

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
1. git fetch refs/grite/*
2. New WAL entries read
3. Events inserted into sled
4. Projections rebuilt
5. git push refs/grite/* (if pushing)
```

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Issue list | O(n) | Scans issue index |
| Issue show | O(1) | Direct sled lookup |
| Event insert | O(1) | Sled + WAL append |
| Rebuild | O(events) | Full projection rebuild |
| Sync | O(new events) | Incremental from WAL |

## Next Steps

- [Three-Layer Design](three-layer.md) - Start with the architecture overview
- [Data Model](data-model.md) - Understand event structure
- [CRDT Merging](crdt-merging.md) - Learn about conflict resolution
