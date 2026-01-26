# Operations

## Doctor

`grite doctor` performs health checks and prints remediation suggestions.

```bash
# Run health checks
grite doctor

# Auto-fix issues
grite doctor --fix
```

Checks performed:

| Check | Description |
|-------|-------------|
| `git_repo` | Git repository is valid |
| `wal_ref` | WAL ref exists and is readable |
| `actor_config` | Actor is properly configured |
| `store_integrity` | Database integrity (event hashes match) |
| `rebuild_threshold` | Warns if rebuild is recommended |

**Rebuild threshold:** The doctor checks if too many events have accumulated since the last rebuild (default: 10,000 events or 7 days). When exceeded, it suggests running `grite rebuild`.

`grite doctor --fix` runs safe local repairs:

- Rebuilds local DB on corruption
- Does not modify git refs
- Does not push to remote

If remote sync is needed, the remediation plan explicitly lists `grite sync --pull` and/or `grite sync --push`.

## Rebuild

`grite rebuild` discards the local sled projections and replays all events.

```bash
# Standard rebuild from local store events
grite rebuild

# Fast rebuild from latest snapshot
grite rebuild --from-snapshot
```

**Standard rebuild:** Clears projections and replays all events from the local store. Use when projections are corrupted but events are intact.

**Snapshot-based rebuild:** Loads events from the latest snapshot instead of the local store. Faster for large repositories because snapshots are pre-consolidated:

1. Loads events from latest snapshot ref
2. Rebuilds projections from those events
3. Updates rebuild timestamp

Rebuilds compact the local DB because they rewrite projections from scratch.

## Limits to be aware of

- Very large WALs will slow rebuilds without recent snapshots.
- High push frequency can increase contention on `refs/grite/wal`; backoff/retry is required.

## Local DB maintenance

The sled DB is a cache and can be safely deleted or rebuilt. Management is done via:

- `grite db stats --json` for size and last rebuild metadata
- `grite rebuild` when the DB appears bloated or after crashes

`grite doctor` may recommend `grite rebuild` if DB size grows beyond configured thresholds.

## Sync

```bash
# Full sync (pull then push)
grite sync

# Pull only
grite sync --pull

# Push only
grite sync --push

# Specify remote
grite sync --remote upstream
```

- `grite sync --pull` fetches `refs/grite/*` from the remote
- `grite sync --push` pushes `refs/grite/*` to the remote
- `grite sync` (no flags) does both: pull first, then push

### Auto-rebase on conflict

When a push is rejected due to non-fast-forward (remote has commits you don't have), grite automatically resolves the conflict:

1. Records local head before attempting push
2. Attempts push
3. If rejected, pulls remote changes (local ref now points to remote head)
4. Identifies events that were local-only (not in remote)
5. Re-appends local events on top of remote head
6. Pushes again

The sync output reports when conflicts were resolved:

```
Conflict resolved: rebased 3 local events on top of remote
Pushed to origin
```

This automatic rebase ensures CRDT semantics are preserved - all events from all actors are included in the final WAL.

## Multi-agent concurrency (same repo or remote)

Concurrent agents are supported with a few strict rules:

- WAL appends are safe and monotonic. Locally, `git update-ref` is atomic: if two agents race to advance `refs/grite/wal`, one wins and the other must re-read the new head and append again.
- The local materialized view must not be shared across processes. `sled` is single-writer and not multi-process safe. Use per-agent data dirs under `.git/grite/actors/<actor_id>/` (recommended).
- Remote push races are common. On non-fast-forward push rejection, the client must fetch, re-append on the new head, and retry.

Retry rule (spec-grade):

- On WAL append failure (local race or remote non-fast-forward), the client MUST: read head → create a new append commit on that head → retry push with bounded exponential backoff.

## Snapshots

- `grite snapshot` creates a monotonic snapshot ref
- `grite snapshot gc` prunes old snapshots (local policy)

Snapshots never change WAL history; they are purely a rebuild accelerator.
