# Contributing

Thank you for your interest in contributing to Grit! This document provides guidelines and information for contributors.

## Getting Started

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/grit.git
   ```
3. Create a branch:
   ```bash
   git checkout -b feature/your-feature-name
   ```
4. Make your changes
5. Run tests:
   ```bash
   cargo test
   ```
6. Commit your changes with a clear message
7. Push to your fork and submit a pull request

## Development Setup

### Prerequisites

- Rust 1.70+ (stable)
- Git 2.38+
- nng library

=== "Ubuntu/Debian"

    ```bash
    sudo apt install libnng-dev
    ```

=== "macOS"

    ```bash
    brew install nng
    ```

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Debug Logging

```bash
RUST_LOG=debug cargo run --bin grit -- issue list
```

## Code Style

- Follow Rust standard formatting: run `cargo fmt` before committing
- Run `cargo clippy` and address any warnings
- Write tests for new functionality
- Keep commits focused and atomic

## Pull Request Process

1. Ensure your code builds without errors
2. Run the full test suite with `cargo test`
3. Update documentation if you're changing behavior
4. Write a clear PR description explaining:
   - What the change does
   - Why it's needed
   - How to test it

## Reporting Issues

When reporting issues, please include:

- A clear, descriptive title
- Steps to reproduce the problem
- Expected vs actual behavior
- Your environment (OS, Rust version, Git version)
- Relevant logs or error messages

## Crate Structure

| Crate | Purpose |
|-------|---------|
| `libgrit-core` | Event types, hashing, projections, sled store, signing |
| `libgrit-git` | WAL commits, ref sync, snapshots, distributed locks |
| `libgrit-ipc` | IPC message schemas (rkyv), daemon lock, client/server |
| `grit` | CLI frontend |
| `grit-daemon` | Optional background daemon |

### libgrit-core

Core types and logic with no external dependencies on git or IPC:

| Module | Purpose |
|--------|---------|
| `types::event` | Event, EventKind, IssueState |
| `types::ids` | ActorId, IssueId, EventId |
| `hash` | BLAKE2b-256 canonical event hashing |
| `projection` | CRDT projection logic |
| `store` | Sled database operations |
| `signing` | Ed25519 key generation and verification |

### libgrit-git

Git operations using libgit2:

| Module | Purpose |
|--------|---------|
| `wal` | WAL append/read, chunk encoding |
| `snapshot` | Snapshot creation and GC |
| `sync` | Push/pull of refs |
| `lock_manager` | Distributed lease locks |

### libgrit-ipc

IPC communication using nng:

| Module | Purpose |
|--------|---------|
| `messages` | IpcRequest, IpcResponse |
| `client` | IPC client with retry |
| `lock` | DaemonLock for coordination |

## Design Principles

When contributing, keep these principles in mind:

1. **Git is the source of truth** - All state derivable from `refs/grit/*`
2. **No working tree pollution** - Never write tracked files
3. **Daemon optional** - CLI works standalone
4. **Deterministic merges** - CRDT semantics, no manual conflict resolution
5. **Per-actor isolation** - Multiple agents can work independently

## Testing Guidelines

### Unit Tests

Add unit tests for new functions:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        // ...
    }
}
```

### Integration Tests

Add integration tests in `tests/`:

```rust
#[test]
fn test_full_workflow() {
    // Test complete workflows
}
```

### Running Specific Tests

```bash
# Run specific test
cargo test test_name

# Run tests for specific crate
cargo test -p libgrit-core

# Run with output
cargo test -- --nocapture
```

## Documentation

- Update documentation for user-facing changes
- Add doc comments for public APIs
- Update CHANGELOG for notable changes

## Questions?

Feel free to open an issue for questions or discussion about potential contributions.

## License

By contributing to Grit, you agree that your contributions will be licensed under the MIT License.
