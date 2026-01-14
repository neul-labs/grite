# Roadmap

## Milestone 1: Local-only MVP

- Event model + hashing
- sled projections
- CLI: init, actor management, create/list/show/update/comment/close
- Export to markdown/json (with schema)
- Tests: deterministic rebuild
- `grit db stats` output schema

## Milestone 2: Git WAL

- WAL commit writer/reader
- Push/pull `refs/grit/*`
- Handle remote-advanced push (fast-forward rebase)
- Snapshot support
- Portable WAL encoding (CBOR)
- Hash test vectors

## Milestone 3: Daemon and IPC

- Daemon discovery
- `grit` routes all commands through the daemon if present for the selected `(repo, actor)`
- Pub/sub notifications
- Daemon ownership lock with lease/heartbeat
- Multi-repo, multi-actor workers
- `grit daemon status`/`grit daemon stop` behavior and output schema

## Milestone 4: Locks + Team workflows

- Lease locks stored in refs
- Lock GC
- `grit issue edit --lock` (optional)
- Lock policy enforcement (`off|warn|require`)
- `grit lock status`

## Milestone 5: Hardening

- Stress tests (concurrent writers)
- Corruption recovery
- Security (signing and verification)
- DB maintenance (`grit db stats`, rebuild thresholds)
