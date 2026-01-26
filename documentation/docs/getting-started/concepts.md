# Core Concepts

This page explains the fundamental concepts behind grite's design. Understanding these concepts will help you use grite effectively and troubleshoot issues.

## Three-Layer Architecture

Grite uses a three-layer architecture:

```
+------------------+     +-------------------+     +------------------+
|   Git WAL        | --> | Materialized View | <-- | CLI / Daemon     |
| refs/grite/wal    |     | sled database     |     | grite / grite-daemon     |
| (source of truth)|     | (fast queries)    |     | (user interface) |
+------------------+     +-------------------+     +------------------+
```

### Layer 1: Git WAL (Source of Truth)

All state is stored in an append-only event log within git refs:

- **Location**: `refs/grite/wal`
- **Format**: Append-only event log
- **Sync**: Standard `git fetch/push`

The WAL (Write-Ahead Log) is the authoritative source. Everything else can be rebuilt from it.

### Layer 2: Materialized View (Fast Queries)

A local database caches the current state of all issues:

- **Location**: `.git/grite/actors/<actor_id>/sled/`
- **Purpose**: Fast queries without replaying all events
- **Rebuild**: Can be deleted and rebuilt anytime with `grite rebuild`

### Layer 3: CLI / Daemon (User Interface)

The command-line interface and optional daemon:

- **CLI**: `grite` command for all operations
- **Daemon**: `grite-daemon` for improved performance (optional)

## Events

All changes in grite are recorded as events. Issues are projections of the event stream.

### Event Types

| Event | Description |
|-------|-------------|
| `IssueCreated` | New issue with title, body, and initial labels |
| `IssueUpdated` | Changed title or body |
| `CommentAdded` | New comment on an issue |
| `LabelAdded` | Label added to issue |
| `LabelRemoved` | Label removed from issue |
| `StateChanged` | Issue opened or closed |
| `AssigneeAdded` | User assigned to issue |
| `AssigneeRemoved` | User unassigned from issue |
| `LinkAdded` | URL attached to issue |
| `AttachmentAdded` | File metadata attached |

### Event Properties

Every event has:

- **event_id**: Content-addressed hash (BLAKE2b-256)
- **issue_id**: Which issue this event belongs to
- **actor**: Who created the event
- **ts_unix_ms**: Timestamp in milliseconds
- **parent**: Previous event ID (for ordering)
- **sig**: Optional Ed25519 signature

## Actors

An actor represents a device or agent. Each actor has:

- **actor_id**: 128-bit random identifier
- **label**: Human-friendly name (e.g., "work-laptop")
- **data directory**: Isolated local database

### Why Actors?

- **Isolation**: Multiple agents can work on the same repo without conflicts
- **Attribution**: Track who made each change
- **Independence**: Each actor has its own materialized view

### Creating Actors

```bash
# Create a new actor
grite actor init --label "ci-agent"

# List all actors
grite actor list

# Switch default actor
grite actor use <actor_id>
```

## IDs

Grite uses three types of identifiers:

| Type | Size | Format | Purpose |
|------|------|--------|---------|
| `ActorId` | 128-bit | Random | Identifies device/agent |
| `IssueId` | 128-bit | Random | Identifies issue |
| `EventId` | 256-bit | BLAKE2b hash | Content-addressed event ID |

IDs are displayed as lowercase hex strings:

```
ActorId:  64d15a2c383e2161772f9cea23e87222  (32 hex chars)
IssueId:  8057324b1e03afd613d4b428fdee657a  (32 hex chars)
EventId:  a1b2c3d4...  (64 hex chars)
```

## CRDT Merging

Grite uses CRDT (Conflict-free Replicated Data Type) semantics for deterministic merging. This means:

- **No manual conflict resolution**: Merges are automatic
- **Eventual consistency**: All actors converge to the same state
- **Commutative operations**: Order doesn't matter for the final result

### Merge Strategies

| Field | Strategy |
|-------|----------|
| Title | Last-writer-wins |
| Body | Last-writer-wins |
| State | Last-writer-wins |
| Labels | Add/remove set |
| Assignees | Add/remove set |
| Comments | Append-only list |

### Tie-Breaking

For last-writer-wins fields, ties are broken by:

1. `ts_unix_ms` (higher timestamp wins)
2. `actor` (lexicographic comparison)
3. `event_id` (lexicographic comparison)

This ensures deterministic ordering even with clock skew.

## The Daemon

The daemon (`grite-daemon`) is optional and provides:

- **Warm cache**: Keeps materialized view ready
- **Concurrent access**: Handles multiple CLI calls efficiently
- **Auto-spawn**: Starts automatically on first command
- **Idle shutdown**: Stops after 5 minutes of inactivity

!!! important
    Correctness never depends on the daemon. The CLI can always work standalone.

## Storage Layout

```
.git/
  grite/
    config.toml                    # Repo config (default_actor, lock_policy)
    actors/
      <actor_id>/
        config.toml                # Actor config (label, public key)
        sled/                      # Materialized view database
        sled.lock                  # flock for exclusive access
        daemon.lock                # Daemon ownership marker

refs/grite/
  wal                              # Append-only event log
  snapshots/<ts>                   # Periodic snapshots
  locks/<resource_hash>            # Distributed lease locks
```

## Key Principles

1. **Git is the source of truth**: All state derivable from `refs/grite/*`
2. **No working tree pollution**: Never writes tracked files
3. **Daemon optional**: CLI works standalone
4. **Deterministic merges**: CRDT semantics, no manual conflicts
5. **Per-actor isolation**: Multiple agents can work independently
6. **Offline-first**: Full functionality without network

## Next Steps

- [Working with Issues](../guides/issues.md) - Deep dive into issue management
- [Architecture](../architecture/index.md) - Technical details of the system
- [Data Model](../architecture/data-model.md) - Event schema and hashing
