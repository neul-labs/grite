# Git WAL and Snapshots

## WAL ref

- Ref: `refs/grit/wal`
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
- `chunk_hash` (BLAKE2b-256 of the chunk file)
- `prev_wal` (parent commit hash)

### Chunk encoding

Chunk files contain a small header and a portable CBOR payload:

- magic: `GRITCHNK`
- version: `u16`
- codec: `cbor-v1`
- payload: canonical CBOR array of `Event` records

`Event` record encoding (fixed-order array):

```
[event_id, issue_id, actor, ts_unix_ms, parent, kind_tag, kind_payload, sig]
```

- `event_id`: 32-byte bstr (BLAKE2b-256 of canonical preimage)
- `issue_id`: 16-byte bstr
- `actor`: 16-byte bstr
- `ts_unix_ms`: u64
- `parent`: null or 32-byte bstr
- `kind_tag`/`kind_payload`: same tags and payloads as in `docs/data-model.md`
- `sig`: null or bstr (optional)

Chunk integrity is verified by `chunk_hash`.

## Append algorithm

1. Read current `refs/grit/wal` head (if present).
2. Create a new commit with parent = head, adding a new chunk file.
3. Update `refs/grit/wal` to the new commit.
4. Push the ref (optional).

If the push is rejected because the remote advanced:

1. Fetch `refs/grit/wal`.
2. Create a new commit whose parent is the fetched head, containing the same chunk.
3. Push again (fast-forward only).

History is never rewritten.

## Sync

- Pull: `git fetch <remote> refs/grit/*:refs/grit/*`
- Push: `git push <remote> refs/grit/*:refs/grit/*`

## Snapshots (periodic, no daemon required)

Snapshots are optional, monotonic optimization refs that speed rebuilds without changing the WAL.

- Ref format: `refs/grit/snapshots/<unix_ms>`
- A snapshot commit stores a compacted set of events plus a `snapshot.json` metadata file.
- Rebuild uses the latest snapshot, then replays WAL commits after its `wal_head`.

### When snapshots are created

Snapshots are created opportunistically, even without an always-on daemon:

- During `grit sync --push` if WAL growth exceeds a threshold
- During explicit `grit snapshot` command
- During `grit doctor --apply` if snapshot staleness is detected

When a daemon is running, it may also create snapshots on the same thresholds.

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

Snapshots are never rewritten; older snapshots can be pruned with `grit snapshot gc`.
