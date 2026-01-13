# Roadmap

## Milestone 1: Local-only MVP

- Event model + hashing
- sled projections
- CLI: create/list/show/comment/close
- Export to markdown/json
- Tests: deterministic rebuild

## Milestone 2: Git WAL

- WAL commit writer/reader
- Push/pull `refs/gems/*`
- Handle remote-advanced push (fast-forward rebase)
- Snapshot support

## Milestone 3: Daemon and IPC

- Daemon discovery
- `gems sync` uses daemon if present
- Pub/sub notifications

## Milestone 4: Locks + Team workflows

- Lease locks stored in refs
- Lock GC
- `gems issue edit --lock` (optional)

## Milestone 5: Hardening

- Stress tests (concurrent writers)
- Corruption recovery
- Security (signing and verification)
