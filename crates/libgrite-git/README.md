# libgrite-git

Git WAL, sync, snapshot, and lock operations for [Grite](https://github.com/neul-labs/grite).

This crate bridges Grite's event model to git repositories. It writes events as a WAL (write-ahead log) in git refs, manages snapshots of materialized state, and handles multi-actor synchronization and locking.

## Key types

- `WalManager` — append-only WAL backed by git refs (`refs/grite/wal/*`)
- `SnapshotManager` — save and restore sled store snapshots as git notes
- `SyncManager` — pull and merge WALs from remote actors with CRDT semantics
- `LockManager` — check and acquire distributed locks across actors

## Quick example

```rust
use libgrite_git::WalManager;

let wal = WalManager::open(".git")?;
wal.append_event(&event)?;
```

See the [full documentation](https://docs.rs/libgrite-git) and the [Grite repository](https://github.com/neul-labs/grite).
