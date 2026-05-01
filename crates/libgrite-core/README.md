# libgrite-core

Core types, CRDT projections, hashing, and sled storage for [Grite](https://github.com/neul-labs/grite).

This crate defines the event-sourced data model and persistence layer used by all other Grite crates. It is pure Rust with no async runtime dependency.

## Key types

- `Event` / `EventKind` — the event-sourced data model (issue created, label added, state changed, etc.)
- `IssueProjection` — materialized issue state with LWW and commutative-set semantics
- `GriteStore` / `LockedStore` — sled-backed materialized views with process-safe `flock` locking
- `SigningKeyPair` — Ed25519 event signing for cryptographic provenance
- `IntegrityReport` — hash and signature verification for audit trails

## Quick example

```rust
use libgrite_core::{GriteStore, IssueFilter};

let store = GriteStore::open(".git/grite/sled")?;
let issues = store.list_issues(&IssueFilter::default())?;
```

See the [full documentation](https://docs.rs/libgrite-core) and the [Grite repository](https://github.com/neul-labs/grite).
