# Data Model

This document describes grit's data model, including event schema, ID types, and encoding.

## Event Schema

All state changes are events. Issues are projections of the event stream.

### Event Structure

```rust
pub struct Event {
  pub event_id: EventId,      // [u8; 32] - content-addressed
  pub issue_id: IssueId,      // [u8; 16] - random
  pub actor: ActorId,         // [u8; 16] - random
  pub ts_unix_ms: u64,
  pub parent: Option<EventId>,
  pub kind: EventKind,
  pub sig: Option<Vec<u8>>,   // Ed25519 signature (optional)
}
```

### Event Kinds

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

pub enum IssueState {
  Open,
  Closed,
}
```

### Event Kind Details

| Kind | Fields | Purpose |
|------|--------|---------|
| `IssueCreated` | title, body, labels | Create new issue |
| `IssueUpdated` | title?, body? | Update title/body |
| `CommentAdded` | body | Add comment |
| `LabelAdded` | label | Add label |
| `LabelRemoved` | label | Remove label |
| `StateChanged` | state | Open/close issue |
| `LinkAdded` | url, note? | Attach URL |
| `AssigneeAdded` | user | Assign user |
| `AssigneeRemoved` | user | Unassign user |
| `AttachmentAdded` | name, sha256, mime | Attach file metadata |

!!! note
    `AttachmentAdded` carries only metadata and a content hash. Binary storage is out of scope for the WAL.

## ID Types

### Overview

| Type | Rust Type | Size | Format | Generation |
|------|-----------|------|--------|------------|
| `ActorId` | `[u8; 16]` | 128-bit | Random | `rand::thread_rng().gen()` |
| `IssueId` | `[u8; 16]` | 128-bit | Random | `rand::thread_rng().gen()` |
| `EventId` | `[u8; 32]` | 256-bit | Hash | BLAKE2b-256 of event content |

### Why Byte Arrays?

1. **Compact storage**: 16 bytes vs 32 chars (hex string), 50% smaller
2. **Type safety**: `[u8; 16]` vs `[u8; 32]` catches ID mixups at compile time
3. **Hash-native**: EventId directly holds BLAKE2b-256 output
4. **Crypto-friendly**: Works directly with signing/hashing operations
5. **No allocations**: Fixed-size arrays on stack

### Display Format

All IDs are displayed as lowercase hex strings:

```
ActorId:  64d15a2c383e2161772f9cea23e87222  (32 hex chars)
IssueId:  8057324b1e03afd613d4b428fdee657a  (32 hex chars)
EventId:  a1b2c3d4...                        (64 hex chars)
```

### ActorId

- Generated once per device/agent during `grit init`
- Stored in `.git/grit/actors/<actor_id>/config.toml`
- Identifies the source of events
- Multiple actors can exist per repository

### IssueId

- Generated when creating a new issue
- Random to avoid coordination between actors
- Appears in all events for that issue

### EventId

- Content-addressed: computed from event content
- Ensures event integrity (any change produces different ID)
- Used for deduplication during sync

## Canonical Encoding

### Goal

Stable, cross-language hashing regardless of platform or serializer.

### Approach

- **Hash**: BLAKE2b-256 (`[u8; 32]`)
- **Preimage**: Canonical CBOR encoding of a fixed-order array

Hashing is independent of storage. WAL chunks use portable CBOR encoding, while sled uses `rkyv` for compact on-disk values. The `event_id` is always computed from the canonical CBOR preimage.

### Hash Input

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

### Kind Tags and Payloads

| Tag | Kind | Payload |
|-----|------|---------|
| 1 | IssueCreated | `[title, body, labels]` |
| 2 | IssueUpdated | `[title_opt, body_opt]` |
| 3 | CommentAdded | `[body]` |
| 4 | LabelAdded | `[label]` |
| 5 | LabelRemoved | `[label]` |
| 6 | StateChanged | `[state]` |
| 7 | LinkAdded | `[url, note_opt]` |
| 8 | AssigneeAdded | `[user]` |
| 9 | AssigneeRemoved | `[user]` |
| 10 | AttachmentAdded | `[name, sha256, mime]` |

### IssueState Encoding

`IssueState` values are encoded as lowercase strings:

- `"open"`
- `"closed"`

### Canonicalization Rules

- CBOR is encoded using canonical rules (RFC 8949)
- Arrays are encoded in order
- Strings are UTF-8 as provided
- For hashing only, `labels` in `IssueCreated` are sorted lexicographically
- `sig` is **not** included in the hash; it signs the `event_id`

## Signing and Verification

### Overview

Signatures are optional. If present, `sig` is a detached Ed25519 signature over the raw 32-byte `event_id`.

### Key Generation

```bash
grit actor init --generate-key
```

Creates:

- Private key: `.git/grit/actors/<actor_id>/keys/signing.key`
- Public key: stored in actor `config.toml`

### Verification Flow

1. Compute `event_id` from canonical CBOR preimage
2. Verify `sig` against `event_id` using actor's public key

### Verification Policy

Configurable in repo config:

| Policy | Behavior |
|--------|----------|
| `off` | No verification |
| `warn` | Log warning on invalid signatures |
| `reject` | Reject events with invalid signatures |

## Issue Projection

`IssueProjection` is computed by folding events for an issue. See [CRDT Merging](crdt-merging.md) for merge strategy details.

```rust
pub struct IssueProjection {
  pub issue_id: IssueId,
  pub title: String,
  pub body: String,
  pub state: IssueState,
  pub labels: BTreeSet<String>,
  pub assignees: BTreeSet<String>,
  pub comments: Vec<Comment>,
  pub links: Vec<Link>,
  pub attachments: Vec<Attachment>,
  pub created_ts: u64,
  pub updated_ts: u64,
  pub version: Version,
}
```

## Next Steps

- [CRDT Merging](crdt-merging.md) - How projections are computed
- [Git WAL](git-wal.md) - How events are stored
- [Storage Layout](storage.md) - File organization
