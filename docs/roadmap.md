# Roadmap

## Completed

### Milestone 1: Local-only MVP

- [x] Event model + BLAKE2b-256 hashing
- [x] Sled projections with CRDT semantics
- [x] CLI: init, actor management, create/list/show/update/comment/close
- [x] Export to markdown/json
- [x] Tests: deterministic rebuild
- [x] `grit db stats` output

### Milestone 2: Git WAL

- [x] WAL commit writer/reader
- [x] Push/pull `refs/grit/*`
- [x] Snapshot support with GC
- [x] Portable WAL encoding (CBOR chunks)
- [x] Hash test vectors

### Milestone 3: Daemon and IPC

- [x] Daemon supervisor/worker architecture
- [x] IPC via nng (REQ/REP pattern)
- [x] CLI routes commands through daemon if present
- [x] Daemon ownership lock with lease/heartbeat
- [x] Multi-repo, multi-actor workers
- [x] `grit daemon start/status/stop`
- [x] Auto-spawn on first CLI command
- [x] Idle timeout with auto-shutdown
- [x] Filesystem-level flock for database exclusion
- [x] Concurrent command handling in daemon

### Milestone 4: Locks + Team workflows

- [x] Lease locks stored in `refs/grit/locks/*`
- [x] Lock GC
- [x] `--lock` flag on issue commands
- [x] Lock policy enforcement (`off|warn|require`)
- [x] `grit lock acquire/release/renew/status/gc`

### Milestone 5: Hardening

- [x] Stress tests (concurrent writers)
- [x] Ed25519 signing and verification
- [x] `grit db check` for integrity verification
- [x] `grit db verify` for signature verification
- [x] `grit doctor` for health checks

## In Progress

### Milestone 5b: Production Hardening

- [ ] Corruption recovery tools
- [ ] Database rebuild thresholds
- [ ] Better error messages and user guidance
- [ ] Performance benchmarks

## Planned

### Milestone 6: Enhanced Sync

- [ ] Handle remote-advanced push (fast-forward rebase)
- [ ] Background sync in daemon
- [ ] Sync conflict reporting
- [ ] Partial sync (specific issues)

### Milestone 7: Future Directions

- [ ] Remote collaboration (GitHub/GitLab integration)
- [ ] UI/TUI (Terminal UI or web dashboard)
- [ ] Advanced queries (search, filters, saved views)
- [ ] Webhooks/Automation (CI integration, triggers)
- [ ] Import/Migration (from GitHub Issues, Jira, etc.)
- [ ] Pub/sub notifications for external consumers

## Test Coverage

| Area | Status |
|------|--------|
| Core types and hashing | 68 tests |
| Git WAL and snapshots | 17 tests |
| IPC protocol | 16 tests |
| Daemon integration | 6 tests |
| Stress tests | 12 tests |
| **Total** | **119 tests** |
