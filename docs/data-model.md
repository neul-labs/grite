# Data Model

## Event schema

All state changes are events. Issues are projections of the event stream.

```rust
pub enum EventKind {
  IssueCreated { title: String, body: String, labels: Vec<String> },
  IssueUpdated { title: Option<String>, body: Option<String> },
  CommentAdded { body: String },
  LabelAdded { label: String },
  LabelRemoved { label: String },
  StateChanged { state: IssueState },
  LinkAdded { url: String, note: Option<String> },
  AssigneeAdded { user: String },
  AssigneeRemoved { user: String },
  AttachmentAdded { name: String, sha256: [u8; 32], mime: String },
}

pub struct Event {
  pub event_id: [u8; 32],
  pub issue_id: [u8; 16],
  pub actor: [u8; 16],
  pub ts_unix_ms: u64,
  pub parent: Option<[u8; 32]>,
  pub kind: EventKind,
  pub sig: Option<Vec<u8>>,
}
```

## IDs

- `actor` is a random 128-bit ID generated per actor (typically one per device or agent).
- `issue_id` is a random 128-bit ID generated for `IssueCreated`.
- `event_id` is content-addressed and deterministic.

Actor IDs are assigned during `gems init` (or first run if missing) and stored in `.git/gems/actors/<actor_id>/config.toml`. Each agent should have its own `actor` and its own local data directory under `.git/gems/actors/<actor_id>/`.

## Canonical encoding and event hashing

**Goal:** stable, cross-language hashing regardless of platform or serializer.

- **Hash**: BLAKE3-256 (`[u8; 32]`)
- **Preimage**: canonical CBOR encoding of a fixed-order array

Hashing is independent of storage. The WAL chunk format may use `rkyv` for speed, but `event_id` is always computed from the canonical CBOR preimage described below.

The hash input is the following array (no maps):

```
[
  1,                 // schema version
  issue_id,          // 16-byte bstr
  actor,             // 16-byte bstr
  ts_unix_ms,        // u64
  parent,            // null or 32-byte bstr
  kind_tag,          // u32
  kind_payload       // array (see below)
]
```

### Kind tags and payloads

```
1: IssueCreated   => [title, body, labels]
2: IssueUpdated   => [title_opt, body_opt]
3: CommentAdded   => [body]
4: LabelAdded     => [label]
5: LabelRemoved   => [label]
6: StateChanged   => [state]
7: LinkAdded      => [url, note_opt]
8: AssigneeAdded  => [user]
9: AssigneeRemoved=> [user]
10: AttachmentAdded => [name, sha256, mime]
```

### Canonicalization rules

- CBOR is encoded using canonical rules (RFC 8949). Arrays are encoded in order.
- Strings are UTF-8 as provided.
- For hashing only, `labels` in `IssueCreated` are sorted lexicographically to treat them as a set.
- `sig` is **not** included in the hash; it may sign the `event_id` instead.

## Deterministic projection

`IssueProjection` is computed by folding events for an issue:

- Title/body: last-writer-wins by `(ts_unix_ms, actor)`
- Labels/assignees: commutative add/remove operations
- State: last-writer-wins by `(ts_unix_ms, actor)`
- Comments: append-only list

Clock skew is handled by ordering ties by `actor` as a stable secondary key.

## Materialized view

Key layout in `sled` (example):

- `event/<event_id>` -> archived `Event`
- `issue_state/<issue_id>` -> `IssueProjection`
- `issue_events/<issue_id>/<ts>/<event_id>` -> empty
- `label_index/<label>/<issue_id>` -> empty
