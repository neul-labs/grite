# Data Model

## Event Schema

All state changes are events. Issues are projections of the event stream.

```rust
pub enum IssueState {
  Open,
  Closed,
}

pub enum DependencyType {
  Blocks,      // "this issue blocks target"
  DependsOn,   // "this issue depends on target"
  RelatedTo,   // symmetric, no cycle constraint
}

pub struct SymbolInfo {
  pub name: String,
  pub kind: String,       // "function", "struct", "trait", "class", etc.
  pub line_start: u32,
  pub line_end: u32,
}

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
  DependencyAdded { target: IssueId, dep_type: DependencyType },
  DependencyRemoved { target: IssueId, dep_type: DependencyType },
  ContextUpdated { path: String, language: String, symbols: Vec<SymbolInfo>, summary: String, content_hash: [u8; 32] },
  ProjectContextUpdated { key: String, value: String },
}

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

`AttachmentAdded` carries only metadata and a content hash. Binary storage is out of scope for the WAL.

`ContextUpdated` and `ProjectContextUpdated` events use derived IssueIds rather than random ones:
- File context: `IssueId = blake2b("grite:context:file:" + path)[..16]`
- Project context: `IssueId = [0xFF; 16]` (sentinel)

This allows context events to flow through the standard WAL and sync for free.

## ID Types

### Overview

| Type | Rust Type | Size | Format | Generation |
|------|-----------|------|--------|------------|
| `ActorId` | `[u8; 16]` | 128-bit | Random | `rand::thread_rng().gen()` |
| `IssueId` | `[u8; 16]` | 128-bit | Random | `rand::thread_rng().gen()` |
| `EventId` | `[u8; 32]` | 256-bit | Hash | BLAKE2b-256 of event content |

### Why Byte Arrays?

1. **Compact storage** - 16 bytes vs 32 chars (hex string), 50% smaller in sled/rkyv
2. **Type safety** - `[u8; 16]` vs `[u8; 32]` catches ID mixups at compile time
3. **Hash-native** - EventId directly holds BLAKE2b-256 output
4. **Crypto-friendly** - Works directly with signing/hashing operations
5. **No allocations** - Fixed-size arrays on stack, no heap allocation

### Display Format

All IDs are displayed as lowercase hex strings:

```
ActorId:  64d15a2c383e2161772f9cea23e87222  (32 hex chars)
IssueId:  8057324b1e03afd613d4b428fdee657a  (32 hex chars)
EventId:  a1b2c3d4...  (64 hex chars)
```

### Conversion Functions

```rust
// Byte array to hex string
pub fn id_to_hex<const N: usize>(id: &[u8; N]) -> String;

// Hex string to byte array
pub fn hex_to_id<const N: usize>(hex_str: &str) -> Result<[u8; N], IdParseError>;

// Convenience wrappers
pub fn parse_actor_id(hex: &str) -> Result<ActorId, IdParseError>;
pub fn parse_issue_id(hex: &str) -> Result<IssueId, IdParseError>;
pub fn parse_event_id(hex: &str) -> Result<EventId, IdParseError>;
```

### ActorId

- Generated once per device/agent during `grite init`
- Stored in `.git/grite/actors/<actor_id>/config.toml`
- Used to identify the source of events
- Multiple actors can exist per repository (one per agent)

### IssueId

- Generated when creating a new issue
- Random to avoid coordination between actors
- Appears in all events for that issue

### EventId

- Content-addressed: deterministically computed from event content
- Ensures event integrity (any change produces different ID)
- Used for deduplication during sync

## Canonical Encoding and Event Hashing

**Goal:** Stable, cross-language hashing regardless of platform or serializer.

- **Hash**: BLAKE2b-256 (`[u8; 32]`)
- **Preimage**: Canonical CBOR encoding of a fixed-order array

Hashing is independent of storage. WAL chunks use portable CBOR encoding, while the local sled DB uses `rkyv` for compact on-disk values. `event_id` is always computed from the canonical CBOR preimage.

Canonical test vectors are in `docs/hash-vectors.md`.

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

```
1:  IssueCreated           => [title, body, labels]
2:  IssueUpdated           => [title_opt, body_opt]
3:  CommentAdded           => [body]
4:  LabelAdded             => [label]
5:  LabelRemoved           => [label]
6:  StateChanged           => [state]
7:  LinkAdded              => [url, note_opt]
8:  AssigneeAdded          => [user]
9:  AssigneeRemoved        => [user]
10: AttachmentAdded        => [name, sha256, mime]
11: DependencyAdded        => [target_bytes, dep_type_str]
12: DependencyRemoved      => [target_bytes, dep_type_str]
13: ContextUpdated         => [path, language, sorted_symbols_array, summary, content_hash_bytes]
14: ProjectContextUpdated  => [key, value]
```

### IssueState Encoding

`IssueState` values are encoded as lowercase strings:

- `open`
- `closed`

### Canonicalization Rules

- CBOR is encoded using canonical rules (RFC 8949)
- Arrays are encoded in order
- Strings are UTF-8 as provided
- For hashing only, `labels` in `IssueCreated` are sorted lexicographically
- `sig` is **not** included in the hash; it signs the `event_id`

## Signing and Verification

Signatures are optional. If present, `sig` is a detached Ed25519 signature over the raw 32-byte `event_id`.

### Key Generation

```bash
grite actor init --generate-key
```

Creates an Ed25519 keypair:
- Private key: `.git/grite/actors/<actor_id>/keys/signing.key`
- Public key: stored in `config.toml`

### Verification Flow

1. Compute `event_id` from canonical CBOR preimage
2. Verify `sig` against `event_id` using actor's public key

### Verification Policy

Configurable in repo config:

- `off` - No verification
- `warn` - Log warning on invalid signatures
- `reject` - Reject events with invalid signatures

## Deterministic Projection

`IssueProjection` is computed by folding events for an issue:

| Field | Merge Strategy |
|-------|---------------|
| Title | Last-writer-wins by `(ts, actor, event_id)` |
| Body | Last-writer-wins by `(ts, actor, event_id)` |
| State | Last-writer-wins by `(ts, actor, event_id)` |
| Labels | Add/remove set (commutative) |
| Assignees | Add/remove set (commutative) |
| Dependencies | Add/remove set (commutative) |
| Comments | Append-only list by event order |
| Links | Append-only list by event order |
| Attachments | Append-only list by event order |

### Tie-Breaking

For last-writer-wins fields, ties are broken by:

1. `ts_unix_ms` (higher wins)
2. `actor` (lexicographic comparison)
3. `event_id` (lexicographic comparison)

This ensures total ordering even with clock skew.

### Output Ordering

For deterministic output:
- `labels` sorted lexicographically
- `assignees` sorted lexicographically

## Materialized View

Key layout in `sled`:

| Key Pattern | Value |
|-------------|-------|
| `event/<event_id>` | Archived `Event` |
| `issue_state/<issue_id>` | `IssueProjection` |
| `issue_events/<issue_id>/<ts>/<event_id>` | Empty (index) |
| `label_index/<label>/<issue_id>` | Empty (index) |
| `dep_forward/<source_id>/<target_id>/<type>` | Empty (dependency index) |
| `dep_reverse/<target_id>/<source_id>/<type>` | Empty (reverse dependency index) |
| `context_files/<path>` | `FileContext` (JSON) |
| `context_symbols/<symbol_name>/<path>` | Empty (symbol index) |
| `context_project/<key>` | `ProjectContextEntry` (JSON) |

The materialized view is a cache. It can be deleted and rebuilt from snapshots and the WAL at any time:

```bash
grite rebuild
```
