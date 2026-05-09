# libgrite-cli

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/libgrite-cli.svg)](https://crates.io/crates/libgrite-cli)
[![Documentation](https://img.shields.io/badge/docs-docs.rs-green.svg)](https://docs.rs/libgrite-cli)
[![Build Status](https://img.shields.io/github/actions/workflow/status/neul-labs/grite/ci.yml?branch=main)](https://github.com/neul-labs/grite/actions)

**Programmatic API for all Grite operations — embed issue tracking, task coordination, and distributed sync directly into your Rust applications.**

`libgrite-cli` exposes every command the `grite` binary supports as a clean, composable library function. This makes it easy to build custom workflows, integrate issue tracking into existing applications, create multi-agent harnesses, or write automation scripts that interact with Grite repositories.

If you are building a Rust application that needs **programmatic issue tracking**, **agent orchestration**, or **git-backed task management**, this crate is your entry point.

---

## What This Crate Provides

### Complete Command Coverage

Every CLI command has a corresponding library function:

| Module | Operations |
|--------|------------|
| `issue` | `create`, `list`, `show`, `update`, `close`, `reopen`, `comment`, `label`, `assign`, `link`, `attach` |
| `actor` | `init`, `list`, `show`, `set_default` |
| `sync` | `pull`, `push`, `merge`, `snapshot`, `rebuild` |
| `context` | `extract`, `query` (tree-sitter symbol extraction) |
| `lock` | `acquire`, `release`, `status`, `list` |
| `daemon` | `start`, `stop`, `status` |

### Context Resolution

The `GriteContext` type handles all the complexity of finding repositories, resolving actors, and choosing execution modes (daemon vs. standalone):

```rust
use libgrite_cli::{GriteContext, ResolveOptions};

// Automatically resolves the git repo, actor, and execution mode
let ctx = GriteContext::resolve(&ResolveOptions::default())?;
```

`GriteContext` automatically:

- Discovers the git repository from the current directory or ancestors
- Loads the default actor configuration
- Detects whether the daemon is running and prefers IPC when available
- Falls back to standalone mode if the daemon is unavailable

### Async Support

Enable the `async` feature for `tokio`-based wrappers around all sync operations:

```toml
[dependencies]
libgrite-cli = { version = "0.5", features = ["async"] }
```

```rust
use libgrite_cli::async_wrappers::issue_create_async;

// Run issue creation in a blocking task pool
let result = issue_create_async(ctx, options).await?;
```

Every sync function has an `*_async` counterpart that runs the operation in `tokio::spawn_blocking`, preventing event loop blocking during database operations.

### Type-Safe Options

Every operation takes a dedicated options struct with strongly typed fields:

```rust
use libgrite_cli::IssueCreateOptions;

let options = IssueCreateOptions {
    title: "Refactor parser".into(),
    body: "Use nom instead of hand-rolled logic".into(),
    labels: vec!["tech-debt".into()],
    assignees: vec![],
    parent_issue: None,
};
```

This eliminates stringly-typed APIs and provides compile-time guarantees about which options are valid for which operations.

---

## Key Modules

### `context`

Repository and actor resolution:

```rust
use libgrite_cli::{GriteContext, ResolveOptions};

let ctx = GriteContext::resolve(&ResolveOptions {
    git_dir: Some("/path/to/.git".into()),
    actor_id: None, // Use default actor
})?;
```

### `issue`

Full issue lifecycle management:

```rust
use libgrite_cli::{issue_create, issue_list, issue_show, IssueCreateOptions, IssueFilter};

// Create an issue
let result = issue_create(&ctx, &IssueCreateOptions {
    title: "Fix race condition".into(),
    body: "...".into(),
    labels: vec!["bug".into()],
    ..Default::default()
})?;

// List open issues
let issues = issue_list(&ctx, &IssueFilter::default().status(Status::Open))?;

// Show details
let issue = issue_show(&ctx, &result.issue_id)?;
```

### `actor`

Multi-actor identity management:

```rust
use libgrite_cli::{actor_init, actor_list, actor_set_default};

// Initialize a new actor
let actor = actor_init(&ctx, "CI Runner")?;

// List all actors
let actors = actor_list(&ctx)?;

// Set default
actor_set_default(&ctx, &actor.id)?;
```

### `sync`

Distributed synchronization:

```rust
use libgrite_cli::{sync_pull, sync_push, sync_rebuild};

// Pull remote changes
sync_pull(&ctx, "origin")?;

// Push local changes
sync_push(&ctx, "origin")?;

// Rebuild materialized view from WAL
sync_rebuild(&ctx, true)?; // true = from snapshot
```

### `lock`

Distributed resource locking:

```rust
use libgrite_cli::{lock_acquire, lock_release};

// Claim exclusive access to a resource
lock_acquire(&ctx, "src/parser.rs", 3600)?; // TTL: 1 hour

// Release when done
lock_release(&ctx, "src/parser.rs")?;
```

### `async_wrappers`

Tokio-compatible async variants:

```rust
use libgrite_cli::async_wrappers::{
    issue_create_async, issue_list_async, sync_pull_async,
};

let issues = issue_list_async(ctx.clone(), filter).await?;
```

---

## Quick Example

```rust
use libgrite_cli::{
    GriteContext, ResolveOptions,
    IssueCreateOptions, IssueFilter,
    issue_create, issue_list,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Resolve context from the current directory
    let ctx = GriteContext::resolve(&ResolveOptions::default())?;

    // Create an issue
    let result = issue_create(&ctx, &IssueCreateOptions {
        title: "Refactor parser".into(),
        body: "Use nom instead of hand-rolled logic".into(),
        labels: vec!["tech-debt".into()],
        assignees: vec![],
        parent_issue: None,
    })?;

    println!("Created issue: {}", result.issue_id);

    // List all open issues
    let filter = IssueFilter::default()
        .status(Status::Open);
    let issues = issue_list(&ctx, &filter)?;

    println!("Open issues: {}", issues.len());
    for issue in &issues {
        println!("  - {}: {}", issue.id, issue.title);
    }

    Ok(())
}
```

---

## Architecture

```
libgrite-cli (programmatic API)
  |
  +-- libgrite-core (data model, storage, CRDTs)
  +-- libgrite-git (WAL, sync, snapshots)
  +-- libgrite-ipc (daemon communication, optional)
```

The library abstracts over the underlying crates, providing a unified interface regardless of whether the daemon is running. When the daemon is available, operations are dispatched via IPC. When it is not, the library opens the database directly.

---

## Why Use the Library Instead of the CLI?

### For Application Developers

If you are building an application that needs issue tracking (a CI system, a project management tool, an agent harness), shelling out to the CLI is fragile:

- **Parsing output** — You must parse human-readable output or JSON from stdout.
- **Error handling** — Exit codes are coarse. You lose structured error information.
- **Performance** — Every invocation pays process startup cost.
- **State management** — You must manage working directory and environment variables.

The library gives you:

- **Native Rust types** — Results are structs, not strings.
- **Rich errors** — Every error variant is typed and carry context.
- **In-process execution** — No process overhead. Direct function calls.
- **Composable** — Build higher-level abstractions by combining library functions.

### For Agent Harnesses

Multi-agent systems need to query and update issue state at high frequency:

```rust
// An agent harness can query issues, claim locks, and post updates
// without ever shelling out to a subprocess
loop {
    let tasks = issue_list_async(&ctx, todo_filter).await?;
    for task in tasks {
        if lock_acquire_async(ctx.clone(), &task.id, 3600).await.is_ok() {
            spawn_agent(task).await;
        }
    }
}
```

---

## Configuration

Library behavior is controlled through `ResolveOptions` and environment variables:

| Variable | Description |
|----------|-------------|
| `GRITE_GIT_DIR` | Override the detected `.git` directory |
| `GRITE_ACTOR_ID` | Override the default actor |
| `GRITE_NO_DAEMON` | Force standalone mode (no IPC) |

---

## See Also

- [Grite Repository](https://github.com/neul-labs/grite) — Main project and documentation
- [grite](../grite) — The CLI binary that wraps this library
- [libgrite-core](../libgrite-core) — Data model and storage engine
- [libgrite-git](../libgrite-git) — Git-backed WAL and sync
- [docs.rs/libgrite-cli](https://docs.rs/libgrite-cli) — Full Rust API documentation

---

## License

MIT License — see [LICENSE](../LICENSE) for details.
