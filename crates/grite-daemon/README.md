# grite-daemon

Background daemon for [Grite](https://github.com/neul-labs/grite) providing concurrent access and performance.

The daemon keeps sled databases open and handles IPC requests from the CLI, eliminating per-command startup overhead. It supports graceful shutdown, idle timeout, and pub/sub notifications.

## Key types

- `Supervisor` — top-level daemon lifecycle (bind, accept, route, shutdown)
- `Worker` — per-request async worker that executes CLI operations
- `WorkerMessage` — command envelope dispatched to workers
- `shutdown_signal` / `setup_signal_handlers` — graceful SIGINT/SIGTERM handling

## Quick example

```rust
use grite_daemon::Supervisor;
use std::time::Duration;

let supervisor = Supervisor::new(
    "/tmp/grite-daemon.sock",
    Some(Duration::from_secs(300)),
);
supervisor.run(shutdown_signal()).await?;
```

See the [full documentation](https://docs.rs/grite-daemon) and the [Grite repository](https://github.com/neul-labs/grite).
