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

### Milestone 5c: Agent Integration

- [x] AGENTS.md auto-generation on `grit init`
- [x] `--no-agents-md` flag to skip
- [x] Append to existing AGENTS.md (preserves content)
- [x] Trigger phrases mapping questions to grit commands
- [x] Demo script (`./demo.sh` and `./demo.sh --auto`)

### Milestone 5b: Production Hardening

- [x] `grit doctor` rebuild threshold check with actionable guidance
- [x] Snapshot-based fast rebuild (`grit rebuild --from-snapshot`)
- [x] Better error messages with suggestions for common errors
- [x] `rebuild_from_events()` for external event sources

### Milestone 6: Enhanced Sync

- [x] Auto-rebase on non-fast-forward push (`push_with_rebase`)
- [x] Sync conflict reporting (events rebased count)
- [x] Daemon sync handler (full sync through IPC)
- [x] Human-readable sync output with conflict details

## In Progress

### Milestone 6b: Advanced Sync

- [ ] Background sync in daemon (periodic auto-sync)
- [ ] Performance benchmarks command (`grit bench`)
- [ ] Partial sync (specific issues)

### Milestone 7: MCP Server for Claude Code

**Goal:** Deep integration with Claude Code's tool system via Model Context Protocol.

**Problem:** AGENTS.md provides guidance but Claude's planning mode still uses internal `TodoWrite`. MCP makes grit a first-class tool.

**Tools to expose:**

| Tool | Description |
|------|-------------|
| `grit_issue_create` | Create a new issue/task |
| `grit_issue_list` | List issues with filters (state, label) |
| `grit_issue_show` | Show issue details with comments |
| `grit_issue_comment` | Add comment to an issue |
| `grit_issue_close` | Close an issue |
| `grit_sync` | Sync with remote |
| `grit_memory_store` | Shortcut to create memory issue |
| `grit_memory_query` | Shortcut to query memories |

**Implementation options:**
1. TypeScript MCP server (Node.js, calls grit CLI)
2. Native Rust MCP server (`grit mcp serve`)

**Tasks:**
- [ ] MCP server scaffolding
- [ ] Implement core tools (create, list, show, comment, close)
- [ ] Memory shortcuts (store, query)
- [ ] Configuration documentation
- [ ] Combined AGENTS.md + MCP guidance

### Milestone 8: Future Directions

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
