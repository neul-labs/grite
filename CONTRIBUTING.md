# Contributing to Grit

Thank you for your interest in contributing to Grit! This document provides guidelines and information for contributors.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/grit.git`
3. Create a branch: `git checkout -b feature/your-feature-name`
4. Make your changes
5. Run tests: `cargo test`
6. Commit your changes with a clear message
7. Push to your fork and submit a pull request

## Development Setup

### Prerequisites

- Rust 1.70+ (stable)
- Git 2.38+
- nng library

On Ubuntu/Debian:
```bash
sudo apt install libnng-dev
```

On macOS:
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
| `grited` | Optional background daemon |

## Design Principles

When contributing, keep these principles in mind:

1. **Git is the source of truth** - All state derivable from `refs/grit/*`
2. **No working tree pollution** - Never write tracked files
3. **Daemon optional** - CLI works standalone
4. **Deterministic merges** - CRDT semantics, no manual conflict resolution
5. **Per-actor isolation** - Multiple agents can work independently

## Questions?

Feel free to open an issue for questions or discussion about potential contributions.

## License

By contributing to Grit, you agree that your contributions will be licensed under the MIT License.
