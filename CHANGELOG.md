# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- New `libgrite-cli` crate: programmatic API for all CLI operations, with optional async wrappers via `tokio`
- `grite install-skill` subcommand to install a Claude Code skill file per-repo or globally
- Expanded public API surface for `libgrite-core` (`IssueFilter`, `FileContext`, `ProjectContextEntry`, etc.)
- Expanded public API surface for `grite-daemon` (`Supervisor`, `Worker`, `WorkerMessage`, signal helpers)

### Changed
- `libgrite-core/src/lib.rs` now exports `actor_dir`, `list_actors`, `export_json`, `check_store_integrity`
- `grite-daemon/src/lib.rs` includes a runnable doctest example

## [0.3.0] - 2025-04-15

### Added
- Daemon mode with Unix-socket IPC for faster repeated commands
- `grite-daemon` binary and `libgrite-ipc` crate
- `grite actor` subcommand for multi-actor identity management
- `grite sync` subcommand for distributed merge and snapshot operations
- Ed25519 event signing and signature verification
- Store integrity checker (`grite db check`)
- Colored table output for `grite issue list`
- Sort-by-created-timestamp support in issue list
- Idempotent actor initialization and shared sled per repo/worker
- Git worktree support

### Changed
- Switched from libgit2 to git2 crate with vendored libgit2
- Replaced custom hash with BLAKE2b for event IDs

## [0.2.0] - 2025-03-01

### Added
- Project context extraction (`grite context`)
- File-level symbol indexing with tree-sitter
- Label and assignee management
- Issue dependencies and links
- Attachment support with SHA-256 verification
- Export to JSON and Markdown

### Changed
- Refactored storage layer to use sled for projections

## [0.1.0] - 2025-01-15

### Added
- Initial release: git-backed issue tracker
- Event-sourced data model with CRDT semantics
- Basic issue CRUD operations
- Git WAL for event persistence
