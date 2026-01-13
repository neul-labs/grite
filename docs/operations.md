# Operations

## Doctor

`gems doctor` performs read-only checks and prints a remediation plan. It never rewrites refs.

Checks include:

- WAL ref exists and is monotonic
- Local materialized view matches WAL head
- Actor identity is present
- Remote refs are reachable (optional)
- Locks are not stale (optional)

`gems doctor --apply` only runs safe actions:

- rebuild local DB
- fetch refs
- create new WAL commits

## Rebuild

`gems rebuild` discards the local sled view and replays:

1. Latest snapshot (if present)
2. WAL commits after the snapshot

## Sync

- `gems sync --pull` fetches `refs/gems/*`
- `gems sync --push` pushes `refs/gems/*`

If push is rejected, the client rebases by creating a new WAL commit parented to the remote head.

## Multi-agent concurrency (same repo or remote)

Concurrent agents are supported with a few strict rules:

- WAL appends are safe and monotonic. Locally, `git update-ref` is atomic: if two agents race to advance `refs/gems/wal`, one wins and the other must re-read the new head and append again.
- The local materialized view must not be shared across processes. `sled` is single-writer and not multi-process safe. Use per-agent data dirs under `.git/gems/actors/<actor_id>/` (recommended).
- Remote push races are common. On non-fast-forward push rejection, the client must fetch, re-append on the new head, and retry.

Retry rule (spec-grade):

- On WAL append failure (local race or remote non-fast-forward), the client MUST: read head → create a new append commit on that head → retry push with bounded exponential backoff.

## Snapshots

- `gems snapshot` creates a monotonic snapshot ref
- `gems snapshot gc` prunes old snapshots (local policy)

Snapshots never change WAL history; they are purely a rebuild accelerator.
