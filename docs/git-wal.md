# Git WAL and Snapshots

## WAL ref

- Ref: `refs/gems/wal`
- Each append creates a new commit, parented to the current WAL head.
- Trees contain only WAL data; no working tree files are touched.

## WAL commit tree

```
meta.json
events/YYYY/MM/DD/<chunk>.bin
```

`meta.json` includes:

- `schema_version`
- `actor_id`
- `chunk_hash` (BLAKE3-256 of the chunk file)
- `prev_wal` (parent commit hash)

### Chunk encoding

Chunk files contain a small header and rkyv-encoded event arrays:

- magic: `GEMSCHNK`
- version: `u16`
- codec: `rkyv-v1`
- payload: rkyv-encoded `Vec<Event>`

Chunk integrity is verified by `chunk_hash`.

## Append algorithm

1. Read current `refs/gems/wal` head (if present).
2. Create a new commit with parent = head, adding a new chunk file.
3. Update `refs/gems/wal` to the new commit.
4. Push the ref (optional).

If the push is rejected because the remote advanced:

1. Fetch `refs/gems/wal`.
2. Create a new commit whose parent is the fetched head, containing the same chunk.
3. Push again (fast-forward only).

History is never rewritten.

## Sync

- Pull: `git fetch <remote> refs/gems/*:refs/gems/*`
- Push: `git push <remote> refs/gems/*:refs/gems/*`

## Snapshots (periodic, no daemon required)

Snapshots are optional, monotonic optimization refs that speed rebuilds without changing the WAL.

- Ref format: `refs/gems/snapshots/<unix_ms>`
- A snapshot commit stores a compacted set of events plus a `snapshot.json` metadata file.
- Rebuild uses the latest snapshot, then replays WAL commits after its `wal_head`.

### When snapshots are created

Since there is no always-on daemon, snapshots are created opportunistically:

- During `gems sync --push` if WAL growth exceeds a threshold
- During explicit `gems snapshot` command
- During `gems doctor --apply` if snapshot staleness is detected

Suggested thresholds (configurable):

- WAL events since last snapshot > 10,000
- OR last snapshot older than 7 days

### Snapshot metadata

`snapshot.json` includes:

- `schema_version`
- `created_ts`
- `wal_head` (commit hash)
- `event_count`
- `chunk_hash`

Snapshots are never rewritten; older snapshots can be pruned with `gems snapshot gc`.
