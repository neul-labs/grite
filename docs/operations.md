# Operations

## Doctor

`grit doctor` performs read-only checks and prints a remediation plan. It never rewrites refs.

Checks include:

- WAL ref exists and is monotonic
- Local materialized view matches WAL head
- Actor identity is present
- Remote refs are reachable (optional)
- Locks are not stale (optional)

`grit doctor --apply` only runs safe local actions and never pushes refs:

- rebuild local DB
- fetch refs
- create new WAL commits

If remote sync is needed, the remediation plan explicitly lists `grit sync --pull` and/or `grit sync --push`.

## Rebuild

`grit rebuild` discards the local sled view and replays:

1. Latest snapshot (if present)
2. WAL commits after the snapshot

Rebuilds also compact the local DB because they rewrite it from scratch.

## Limits to be aware of

- Very large WALs will slow rebuilds without recent snapshots.
- High push frequency can increase contention on `refs/grit/wal`; backoff/retry is required.

## Local DB maintenance

The sled DB is a cache and can be safely deleted or rebuilt. Management is done via:

- `grit db stats --json` for size and last rebuild metadata
- `grit rebuild` when the DB appears bloated or after crashes

`grit doctor` may recommend `grit rebuild` if DB size grows beyond configured thresholds.

## Sync

- `grit sync --pull` fetches `refs/grit/*`
- `grit sync --push` pushes `refs/grit/*`

If push is rejected, the client rebases by creating a new WAL commit parented to the remote head.

## Multi-agent concurrency (same repo or remote)

Concurrent agents are supported with a few strict rules:

- WAL appends are safe and monotonic. Locally, `git update-ref` is atomic: if two agents race to advance `refs/grit/wal`, one wins and the other must re-read the new head and append again.
- The local materialized view must not be shared across processes. `sled` is single-writer and not multi-process safe. Use per-agent data dirs under `.git/grit/actors/<actor_id>/` (recommended).
- Remote push races are common. On non-fast-forward push rejection, the client must fetch, re-append on the new head, and retry.

Retry rule (spec-grade):

- On WAL append failure (local race or remote non-fast-forward), the client MUST: read head → create a new append commit on that head → retry push with bounded exponential backoff.

## Snapshots

- `grit snapshot` creates a monotonic snapshot ref
- `grit snapshot gc` prunes old snapshots (local policy)

Snapshots never change WAL history; they are purely a rebuild accelerator.
