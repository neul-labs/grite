# libgrite-ipc

IPC types and client for [Grite](https://github.com/neul-labs/grite) daemon communication.

This crate defines the wire protocol between the `grite` CLI and the `grite-daemon` background process. It uses Unix domain sockets (or named pipes on Windows) and `rkyv` for zero-copy serialization.

## Key types

- `IpcClient` / `IpcServer` — socket-based request/response channels
- `IpcCommand` / `IpcResponse` — typed messages for all CLI operations
- `DaemonLock` — on-disk lock file used to discover the running daemon

## Quick example

```rust
use libgrite_ipc::IpcClient;

let client = IpcClient::connect("/tmp/grite-daemon.sock")?;
```

See the [full documentation](https://docs.rs/libgrite-ipc) and the [Grite repository](https://github.com/neul-labs/grite).
