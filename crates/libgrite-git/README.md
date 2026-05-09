# libgrite-git

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/libgrite-git.svg)](https://crates.io/crates/libgrite-git)
[![Documentation](https://img.shields.io/badge/docs-docs.rs-green.svg)](https://docs.rs/libgrite-git)
[![Build Status](https://img.shields.io/github/actions/workflow/status/neul-labs/grite/ci.yml?branch=main)](https://github.com/neul-labs/grite/actions)

**Git-backed write-ahead log, distributed synchronization, and snapshot management for event-sourced applications.**

`libgrite-git` bridges Grite's event model to git repositories. It writes events as an append-only WAL in git refs, manages periodic snapshots of materialized state, handles multi-actor synchronization with CRDT semantics, and coordinates distributed locks across agents.

If you are building an application that needs **append-only logs in git**, **distributed sync through existing git remotes**, or **deterministic CRDT merging**, this crate provides the infrastructure.

---

## What This Crate Provides

### Git-Backed Write-Ahead Log (WAL)

The `WalManager` stores events as git commits in `refs/grite/wal`, with each commit containing:

- A `meta.json` blob with actor ID, chunk hash, and parent reference
- A CBOR-encoded chunk of events in a date-based tree structure (`events/YYYY/MM/DD/<hash>.bin`)

This design gives you:

- **Version-controlled history** — Every event batch is a git commit. Use standard git tools to inspect, diff, and revert.
- **Content-addressed chunks** — Each chunk is identified by its BLAKE2b hash. Corruption is immediately detectable.
- **Efficient storage** — CBOR encoding is compact. Date-based tree sharding prevents giant directories.
- **Atomic appends** — A WAL commit is all-or-nothing. No partial writes.

### Distributed Synchronization

The `SyncManager` pulls WAL refs from remote repositories and merges them using the CRDT semantics defined in [`libgrite-core`](../libgrite-core):

- **Pull remote WALs** — `git fetch` retrieves remote `refs/grite/wal` references.
- **CRDT merge** — New events are merged into the local WAL using deterministic ordering. No conflicts, no manual resolution.
- **Push local WALs** — `git push` publishes your events to the shared remote.
- **Branch awareness** — Issues created on a feature branch stay on that branch. Merge the branch, merge the issues.

This means your issue tracker syncs through the exact same mechanism as your code. If you can collaborate on code with git, you can collaborate on issues with Grite. No new protocols, no new infrastructure.

### Snapshot Management

The `SnapshotManager` creates and restores snapshots of the materialized view:

- **Fast rebuild** — Snapshots capture the full sled database state as git tree objects. Rebuild from a snapshot in ~50ms instead of replaying thousands of events.
- **Automatic creation** — Snapshots are created automatically when the event count exceeds a threshold.
- **Garbage collection** — Old snapshots are pruned to keep storage bounded.

### Distributed Locks

The `LockManager` provides lease-based distributed locking across actors:

- **Resource claims** — Lock any string resource (file path, module name, task ID) with a configurable TTL.
- **Lease expiration** — Locks automatically expire, preventing deadlocks when agents crash.
- **Reentrant safety** — The same actor can renew or extend an existing lock.
- **Git-backed storage** — Lock state lives in `refs/grite/locks/`, synced with the rest of the WAL.

---

## Key Types

### WAL Manager

```rust
use libgrite_git::WalManager;

// Open the WAL for a repository
let wal = WalManager::open(".git")?;

// Append events (creates a new git commit)
let oid = wal.append(&actor_id, &events)?;

// Read all events from the WAL
let all_events = wal.read_all()?;

// Read events since a specific commit
let new_events = wal.read_since(oid)?;
```

### Snapshot Manager

```rust
use libgrite_git::SnapshotManager;

let snapshots = SnapshotManager::open(".git")?;

// Create a snapshot from the current materialized state
let snapshot_oid = snapshots.create(wal_head, &events)?;

// List all snapshots (newest first)
let list = snapshots.list()?;

// Read events from a snapshot
let events = snapshots.read(list[0].oid)?;

// GC old snapshots, keeping the 3 most recent
let stats = snapshots.gc(3)?;
```

### Sync Manager

```rust
use libgrite_git::SyncManager;

let sync = SyncManager::open(".git")?;

// Pull and merge remote WAL
sync.pull("origin")?;

// Push local WAL to remote
sync.push("origin")?;
```

### Lock Manager

```rust
use libgrite_git::LockManager;

let locks = LockManager::open(".git")?;

// Acquire a lock with a 1-hour TTL
locks.acquire("src/parser.rs", &actor_id, 3600)?;

// Check if a resource is locked
if let Some(lock) = locks.check("src/parser.rs")? {
    println!("Locked by {} until {}", lock.actor_id, lock.expires_at);
}

// Release a lock
locks.release("src/parser.rs", &actor_id)?;
```

---

## Quick Example

```rust
use libgrite_git::WalManager;
use libgrite_core::{
    Event, EventKind,
    types::ids::{generate_issue_id, generate_actor_id},
    hash::compute_event_id,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wal = WalManager::open(".git")?;
    let actor_id = generate_actor_id();
    let issue_id = generate_issue_id();

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis() as u64;

    let kind = EventKind::IssueCreated {
        title: "Fix race condition".into(),
        body: "Under high concurrency, WAL appends collide.".into(),
        labels: vec!["bug".into(), "concurrency".into()],
    };

    let event_id = compute_event_id(&issue_id, &actor_id, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor_id, ts, None, kind);

    // Append to the WAL (creates a git commit)
    let commit_oid = wal.append(&actor_id, &[event])?;
    println!("Appended event as commit {}", commit_oid);

    // Read back
    let events = wal.read_all()?;
    println!("WAL contains {} events", events.len());

    Ok(())
}
```

---

## WAL Format

Each WAL commit is a git commit with the following tree structure:

```
<commit>
├── meta.json          # Actor ID, chunk hash, parent WAL ref
└── events/
    └── 2026/
        └── 05/
            └── 09/
                └── <blake2b-hash>.bin   # CBOR-encoded event chunk
```

### meta.json

```json
{
  "schema_version": 1,
  "actor_id": "a1b2c3d4...",
  "chunk_hash": "e5f6g7h8...",
  "prev_wal": "abc123..."
}
```

### Chunk Encoding

Events are grouped into chunks and encoded with CBOR:

- **Compact** — CBOR is a binary JSON alternative that is smaller and faster to parse.
- **Schema-evolvable** — New event kinds can be added without breaking existing readers.
- **Hash-verified** — The chunk hash in `meta.json` ensures integrity.

---

## Design Decisions

### Why Git Refs Instead of Files?

We store the WAL in git refs (`refs/grite/wal`) rather than tracked files for several reasons:

1. **No working tree pollution** — The working tree stays clean. No `.grite/` directory full of JSON files.
2. **Automatic versioning** — Every WAL append is a git commit. You get history, diffs, and revert for free.
3. **Native sync** — `git fetch` and `git push` sync the WAL alongside your code. No new protocols.
4. **Branch isolation** — WAL refs are per-branch. Issues on a feature branch do not leak into main until merged.

### Why Not Pack Files?

Git pack files are great for code but not ideal for append-only event logs:

- **Random access** — We need to read the latest events without unpacking the entire history.
- **Independent chunks** — Each WAL commit contains only its own events, not the full history.
- **Snapshot optimization** — Snapshots provide fast random access to materialized state without replay.

### Why Date-Based Tree Sharding?

The `events/YYYY/MM/DD/<hash>.bin` structure prevents any single git tree from growing too large:

- Git trees with thousands of entries become slow.
- Date sharding keeps each directory to a manageable size.
- The hash in the filename ensures uniqueness even if multiple chunks occur on the same day.

---

## Use Cases Beyond Grite

`libgrite-git` is designed for Grite but its primitives are reusable:

- **Audit logging in git** — Append audit events to a git ref for tamper-evident, version-controlled logs.
- **Distributed state machines** — Use the WAL + CRDT merge pattern for any collaborative state that needs to sync through git.
- **Git-backed databases** — The snapshot + WAL pattern is a general-purpose embedded database architecture.
- **Lock coordination** — The distributed lock manager works for any resource that needs cross-agent coordination.

---

## See Also

- [Grite Repository](https://github.com/neul-labs/grite) — Main project and documentation
- [libgrite-core](../libgrite-core) — Data model and CRDT projections (used by this crate)
- [libgrite-ipc](../libgrite-ipc) — IPC protocol for daemon communication
- [docs.rs/libgrite-git](https://docs.rs/libgrite-git) — Full Rust API documentation

---

## License

MIT License — see [LICENSE](../LICENSE) for details.
