# libgrite-core

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/libgrite-core.svg)](https://crates.io/crates/libgrite-core)
[![Documentation](https://img.shields.io/badge/docs-docs.rs-green.svg)](https://docs.rs/libgrite-core)
[![Build Status](https://img.shields.io/github/actions/workflow/status/neul-labs/grite/ci.yml?branch=main)](https://github.com/neul-labs/grite/actions)

**The event-sourced CRDT engine at the heart of Grite — deterministic conflict resolution, content-addressed events, and high-performance embedded storage.**

`libgrite-core` defines the fundamental data model, persistence layer, and conflict resolution algorithms used by the entire Grite ecosystem. It is pure Rust with no async runtime dependency, making it suitable for embedding in CLI tools, servers, and resource-constrained environments.

If you are building an application that needs **conflict-free collaboration**, **append-only event sourcing**, or **tamper-evident audit trails**, this crate provides the primitives.

---

## What This Crate Provides

### Event-Sourced Data Model

At the core of Grite is a simple but powerful idea: state is not stored directly. Instead, every change is recorded as an immutable event, and the current state is derived by projecting all events through a deterministic function.

This gives you:

- **Complete audit history** — Every change is preserved forever. You can reconstruct the state of your system at any point in time.
- **Immutable facts** — Events are facts that happened. They cannot be changed or deleted, only compensated by new events.
- **Content-addressed integrity** — Every event has a BLAKE2b hash that covers its content, actor, timestamp, and causal dependencies. Tamper with an event and its ID changes.

### CRDT Projections

The `IssueProjection` type applies commutative-set and Last-Write-Wins (LWW) semantics to merge events into materialized state:

- **Set semantics:** Labels, assignees, and attachments are sets. Adding and removing the same element multiple times converges to the correct state regardless of order.
- **LWW semantics:** Titles, bodies, and statuses use timestamp-based last-write-wins. Concurrent edits preserve the most recent one.
- **Deterministic merge:** Given the same set of events, every actor produces exactly the same projection. No consensus protocol needed.

This is not just "merge when you can" — it is a mathematically proven CRDT that guarantees strong eventual consistency.

### Embedded Storage

`GriteStore` and `LockedStore` provide sled-backed materialized views with process-safe exclusive access:

- **sled** — A modern embedded key-value store written in Rust. Provides O(1) lookups, ACID transactions, and crash safety.
- **flock locking** — Uses POSIX advisory file locks to prevent concurrent process access. Safe even when multiple agents run on the same machine.
- **Incremental updates** — The store tracks the last-processed WAL head and only replays new events, making updates near-instant.

### Cryptographic Provenance

Optional Ed25519 event signing provides non-repudiable audit trails:

- **SigningKeyPair** — Generate, persist, and load Ed25519 key pairs for actors.
- **Per-event signatures** — Sign every event at creation time. Verify signatures during projection.
- **IntegrityReport** — Batch verification of event hashes and signatures for compliance auditing.

---

## Key Types

### Event Model

```rust
use libgrite_core::types::event::{Event, EventKind};

// Events are the atomic unit of change
let event = Event::new(
    event_id,      // BLAKE2b hash of (issue_id, actor, timestamp, kind)
    issue_id,      // 128-bit random ID
    actor_id,      // 128-bit random actor ID
    ts_unix_ms,    // Monotonic timestamp
    parent_event,  // Optional causal dependency
    EventKind::IssueCreated {
        title: "Fix parser".into(),
        body: "Handle empty structs".into(),
        labels: vec!["bug".into()],
    },
);
```

### Event Kinds

| Kind | Semantics | CRDT Strategy |
|------|-----------|---------------|
| `IssueCreated` | Initializes an issue | Set baseline |
| `TitleChanged` | Updates issue title | LWW by timestamp |
| `BodyChanged` | Updates issue body | LWW by timestamp |
| `StatusChanged` | Open / close / reopen | LWW by timestamp |
| `LabelAdded` / `LabelRemoved` | Set operations | Commutative set |
| `AssigneeAdded` / `AssigneeRemoved` | Set operations | Commutative set |
| `CommentAdded` | Append-only comment | Append to ordered list |
| `LinkAdded` / `LinkRemoved` | Graph edges | Commutative set with cycle detection |
| `AttachmentAdded` | File references | Commutative set |
| `SignedEvent` | Wrapper with Ed25519 signature | Cryptographic verification |

### Storage

```rust
use libgrite_core::{GriteStore, IssueFilter};

// Open a materialized view database
let store = GriteStore::open(".git/grite/actors/default/sled")?;

// List issues with filtering
let filter = IssueFilter::default()
    .status(Status::Open)
    .labels(vec!["bug".into()]);
let issues = store.list_issues(&filter)?;

// Get a single issue by ID
let issue = store.get_issue(&issue_id)?;
```

### Hashing

```rust
use libgrite_core::hash::compute_event_id;

// Content-addressed event IDs ensure tamper-evidence
let event_id = compute_event_id(&issue_id, &actor_id, ts_unix_ms, parent.as_ref(), &kind);
```

---

## Quick Example

```rust
use libgrite_core::{
    GriteStore, IssueFilter, Event, EventKind,
    types::ids::{generate_issue_id, generate_actor_id},
    hash::compute_event_id,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a store
    let store = GriteStore::open("/tmp/grite-store")?;

    // Create an actor and issue
    let actor_id = generate_actor_id();
    let issue_id = generate_issue_id();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis() as u64;

    // Build an event
    let kind = EventKind::IssueCreated {
        title: "Refactor parser".into(),
        body: "Use nom instead of hand-rolled logic".into(),
        labels: vec!["tech-debt".into()],
    };
    let event_id = compute_event_id(&issue_id, &actor_id, ts, None, &kind);
    let event = Event::new(event_id, issue_id, actor_id, ts, None, kind);

    // In a real application, you would append this to the WAL
    // and let the store rebuild the projection. Here we just
    // demonstrate the data model.

    // Query the store (after WAL replay)
    let issues = store.list_issues(&IssueFilter::default())?;
    println!("Found {} issues", issues.len());

    Ok(())
}
```

---

## Design

### Why CRDTs?

Traditional distributed systems use consensus protocols (Paxos, Raft) or conflict resolution (last-write-wins with vector clocks) to handle concurrent edits. These approaches have drawbacks:

- **Consensus requires coordination** — You need a quorum, which means network round-trips and availability tradeoffs.
- **Conflict resolution requires user intervention** — "Pick version A or B" does not work for autonomous agents.
- **Vector clocks are complex** — They grow unboundedly and are hard to reason about.

CRDTs eliminate all of these problems. By designing the data model so that concurrent operations are inherently commutative, we get:

- **No coordination required** — Agents work entirely offline and merge later.
- **No conflicts** — There is nothing to resolve. The merge function always succeeds.
- **Deterministic convergence** — Every agent produces the same result after seeing the same events.

### Why sled?

We evaluated several embedded databases and chose sled for its unique combination of properties:

- **Rust-native** — No FFI, no C dependencies, no build system complexity.
- **Modern design** — Lock-free reads, copy-on-write B-trees, crash-safe transactions.
- **Small footprint** — ~1KB per issue in our workloads.
- **Fast startup** — Database open is milliseconds, not seconds.

### Why BLAKE2b?

Event IDs use BLAKE2b for content-addressing:

- **Fast** — Faster than SHA-256 on modern CPUs.
- **Secure** — 256-bit collision resistance.
- **Simple** — Single-pass hashing with a clean Rust API via the `blake2` crate.

---

## Use Cases Beyond Grite

While `libgrite-core` was built for issue tracking, its primitives are general-purpose:

- **Collaborative document editing** — The CRDT projection model works for any LWW + set data.
- **Audit logging** — Content-addressed events with Ed25519 signatures provide tamper-evident logs.
- **Offline-first applications** — The append-only WAL + CRDT merge pattern applies to any app that needs to work offline and sync later.
- **Event sourcing** — The `Event` / `EventKind` model and sled-backed projections are a minimal but complete event sourcing framework.

---

## See Also

- [Grite Repository](https://github.com/neul-labs/grite) — Main project and documentation
- [libgrite-git](../libgrite-git) — Git-backed WAL and sync (uses this crate's data model)
- [libgrite-cli](../libgrite-cli) — Programmatic API (builds on this crate's storage)
- [docs.rs/libgrite-core](https://docs.rs/libgrite-core) — Full Rust API documentation

---

## License

MIT License — see [LICENSE](../LICENSE) for details.
