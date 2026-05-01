# libgrite-cli

Programmatic API for all [Grite](https://github.com/neul-labs/grite) CLI operations.

This crate exposes every command the `grite` binary supports as a library function, making it easy to embed issue tracking into other Rust programs, multi-agent harnesses, or custom workflows. An optional `async` feature provides `tokio`-based wrappers.

## Key modules

- `context` — `GriteContext` resolves actors, repos, and execution modes
- `issue` — create, list, show, update, comment, close, label, assign, link, attach
- `actor` — init, list, show, set-default
- `sync` — pull, push, merge, snapshot, rebuild
- `async_wrappers` — `tokio::spawn_blocking` wrappers for every sync function (`async` feature)

## Quick example

```rust
use libgrite_cli::{GriteContext, IssueCreateOptions, issue_create};

let ctx = GriteContext::resolve(&ResolveOptions::default())?;
let result = issue_create(&ctx, &IssueCreateOptions {
    title: "Refactor parser".into(),
    body: "Use nom instead of hand-rolled logic".into(),
    labels: vec!["tech-debt".into()],
})?;
```

See the [full documentation](https://docs.rs/libgrite-cli) and the [Grite repository](https://github.com/neul-labs/grite).
