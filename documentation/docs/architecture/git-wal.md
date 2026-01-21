# Git WAL

This document describes the Write-Ahead Log (WAL) format used by grit.

## Overview

The WAL is an append-only event log stored in git refs. It is the source of truth for all grit state.

## Location

```
refs/grit/wal
```

This ref points to a git commit containing the current WAL state.

## Structure

```
refs/grit/wal
  └── commit (HEAD of WAL)
        ├── parent commit (previous WAL state)
        └── tree
              └── blob (CBOR-encoded events)
```

### Commit Chain

Each append creates a new commit:

```
commit-0 (initial) <- commit-1 <- commit-2 <- ... <- commit-N (current)
```

The ref always points to the latest commit.

### Tree Layout

Each commit contains a tree with event data:

```
tree/
  └── events (blob containing CBOR chunks)
```

## Chunk Format

Events are encoded in chunks using the GRITCHNK format.

### Header

```
Magic: GRITCHNK (8 bytes)
Version: 1 (u8)
Count: number of events (u32, big-endian)
```

### Events

After the header, events are CBOR-encoded and concatenated.

### Example

```
GRITCHNK\x01\x00\x00\x00\x03   # Header: 3 events
[CBOR event 1]
[CBOR event 2]
[CBOR event 3]
```

## CBOR Encoding

Each event is encoded as a CBOR array:

```cbor
[
  1,                       # schema version
  h'<issue_id>',           # 16 bytes
  h'<actor_id>',           # 16 bytes
  1700000000000,           # ts_unix_ms
  h'<parent_event_id>',    # 32 bytes or null
  3,                       # kind tag
  ["comment body"],        # kind payload
  h'<signature>'           # optional signature
]
```

### Kind Payloads

| Tag | Kind | Payload |
|-----|------|---------|
| 1 | IssueCreated | `[title, body, [labels...]]` |
| 2 | IssueUpdated | `[title_or_null, body_or_null]` |
| 3 | CommentAdded | `[body]` |
| 4 | LabelAdded | `[label]` |
| 5 | LabelRemoved | `[label]` |
| 6 | StateChanged | `["open" or "closed"]` |
| 7 | LinkAdded | `[url, note_or_null]` |
| 8 | AssigneeAdded | `[user]` |
| 9 | AssigneeRemoved | `[user]` |
| 10 | AttachmentAdded | `[name, h'<sha256>', mime]` |

## Append Algorithm

To append new events:

1. Read current WAL ref to get HEAD commit
2. Create new blob with chunk header + CBOR events
3. Create tree containing the blob
4. Create commit with HEAD as parent
5. Update ref atomically via `git update-ref`

### Atomicity

`git update-ref` is atomic. If two processes race:

- One succeeds
- Other gets "ref already updated" error
- Loser must re-read HEAD and retry

## Read Algorithm

To read all events:

1. Get current WAL ref
2. Walk commit history from HEAD to root
3. For each commit, read blob from tree
4. Parse chunk header
5. Decode CBOR events
6. Return events in chronological order

### Optimization: Snapshots

For large WALs, snapshots accelerate reads:

```
refs/grit/snapshots/<timestamp>
```

Snapshots contain consolidated events up to a point. Reading starts from the latest snapshot instead of the root commit.

## Sync Operations

### Fetch (Pull)

```bash
git fetch origin refs/grit/wal:refs/grit/wal
```

After fetch:

1. Local ref updated to remote HEAD
2. New events read from commits
3. Events inserted into sled
4. Projections updated

### Push

```bash
git push origin refs/grit/wal:refs/grit/wal
```

If rejected (non-fast-forward):

1. Fetch remote changes
2. Identify local-only events (not in remote)
3. Re-append local events on top of remote HEAD
4. Push again

This "rebase" preserves all events from all actors.

## Snapshots

### Purpose

Snapshots accelerate rebuilds for large WALs.

### Location

```
refs/grit/snapshots/<timestamp>
```

### Format

Same as WAL: commit with tree containing CBOR blob.

### Creation

```bash
grit snapshot
```

Creates a snapshot containing all events up to current WAL HEAD.

### Garbage Collection

```bash
grit snapshot gc
```

Removes old snapshots according to policy.

## Distributed Locks

### Location

```
refs/grit/locks/<resource_hash>
```

### Format

Lock data is stored in a commit tree:

```json
{
  "resource": "issue:abc123",
  "owner": "<actor_id>",
  "nonce": "<random>",
  "expires_unix_ms": 1700003600000
}
```

### Acquisition

1. Check if lock ref exists
2. If exists and not expired, fail (conflict)
3. If not exists or expired, create commit with lock data
4. Atomic update-ref to claim lock

### Release

Delete the lock ref.

## Ref Summary

| Ref | Purpose |
|-----|---------|
| `refs/grit/wal` | Append-only event log |
| `refs/grit/snapshots/<ts>` | Point-in-time snapshots |
| `refs/grit/locks/<hash>` | Distributed lease locks |

## Design Rationale

### Why CBOR?

- Compact binary format
- Self-describing
- Cross-language support
- Canonical encoding possible

### Why Append-Only?

- Simple conflict resolution
- Full history preserved
- Easy to sync and merge
- Natural audit trail

### Why Git Refs?

- Atomic updates
- Built-in replication
- Works with any git remote
- No separate storage system

## Next Steps

- [Storage Layout](storage.md) - Local file organization
- [CRDT Merging](crdt-merging.md) - Event merge semantics
- [Syncing Guide](../guides/syncing.md) - Practical sync usage
