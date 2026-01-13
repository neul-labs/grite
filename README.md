# Gems

Gems is a repo-local, git-backed issue/task system designed for coding agents and humans. It keeps an append-only event log in git refs, builds a fast local materialized view, and never writes tracked state into the working tree.

This repository contains the design, data model, and implementation roadmap needed to build Gems.

## Why

- Keep state local, auditable, and diffable in git.
- Avoid worktree conflicts and tracked-file churn.
- Make merges deterministic and non-destructive.
- Require no daemon for correctness; daemon is only a performance accelerator.

## Core design (one screen)

- Canonical state lives in an append-only WAL stored in `refs/gems/wal`.
- Local state is a deterministic materialized view in `.git/gems/actors/<actor_id>/sled/`.
- Sync is `git fetch/push refs/gems/*` with monotonic fast-forward only.
- Conflicts are resolved by event union + deterministic projection rules.

## Repository layout (planned)

- `libgems-core`: event types, hashing, projections, sled store
- `libgems-git`: WAL commits, ref sync, snapshots
- `libgems-ipc`: rkyv schemas + async-nng IPC
- `gems`: CLI
- `gemsd`: optional daemon

## Docs

- `docs/architecture.md`
- `docs/actors.md`
- `docs/data-model.md`
- `docs/git-wal.md`
- `docs/cli.md`
- `docs/agent-playbook.md`
- `docs/locking.md`
- `docs/operations.md`
- `docs/roadmap.md`

## Build prerequisites (planned)

- Rust stable
- Git 2.38+
- `nng` (for IPC, optional for CLI-only builds)

## Development quickstart (planned)

```bash
cargo build
cargo test
```

## Status

This repo is currently design-first. The docs define the target architecture and implementation milestones.
