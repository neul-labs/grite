# Storage Layout

This document describes grite's storage organization on disk.

## Overview

Grite stores data in two locations:

1. **Git refs**: Source of truth (`refs/grite/*`)
2. **Local files**: Materialized view and config (`.git/grite/`)

## Directory Structure

```
.git/
├── grite/
│   ├── config.toml                    # Repository configuration
│   ├── sled/                          # Shared materialized view database
│   ├── sled.lock                      # Filesystem lock for shared sled
│   ├── daemon.lock                    # Daemon ownership marker
│   └── actors/
│       └── <actor_id>/
│           ├── config.toml            # Actor identity and settings
│           └── keys/
│               └── signing.key        # Ed25519 private key (optional)
│
└── refs/
    └── grite/
        ├── wal                        # Append-only event log
        ├── snapshots/
        │   └── <timestamp>            # Periodic snapshots
        └── locks/
            └── <resource_hash>        # Distributed locks
```

## Repository Files

### .git/grite/config.toml

Repository-wide configuration.

```toml
default_actor = "64d15a2c383e2161772f9cea23e87222"
lock_policy = "warn"

[snapshot]
max_events = 10000
max_age_days = 7
```

| Field | Description |
|-------|-------------|
| `default_actor` | Default actor ID for commands |
| `lock_policy` | Lock enforcement: `off`, `warn`, `require` |
| `snapshot.max_events` | Snapshot threshold (event count) |
| `snapshot.max_age_days` | Snapshot threshold (age) |

## Repository Files

### .git/grite/sled/

The shared sled embedded database containing the materialized view for the entire repository. All actors share a single database — actor identity is recorded on each event, not used as a storage partition.

Internal structure (managed by sled):

```
sled/
├── conf
├── snap.*
└── db
```

This is expendable and can be rebuilt from the WAL at any time.

### .git/grite/sled.lock

Filesystem lock (`flock`) for exclusive access to the shared sled database.

- CLI acquires this lock before opening sled
- Daemon holds it for its lifetime
- Prevents concurrent access corruption

### .git/grite/daemon.lock

JSON file indicating daemon ownership. Located at the repository level (one daemon per repo, not per actor).

```json
{
  "pid": 12345,
  "started_ts": 1700000000000,
  "repo_root": "/path/to/repo",
  "actor_id": "64d15a2c383e2161772f9cea23e87222",
  "host_id": "my-laptop",
  "ipc_endpoint": "/tmp/grite-daemon.sock",
  "lease_ms": 30000,
  "last_heartbeat_ts": 1700000000000,
  "expires_ts": 1700000030000
}
```

| Field | Description |
|-------|-------------|
| `pid` | Daemon process ID |
| `started_ts` | Daemon start time |
| `repo_root` | Repository path |
| `actor_id` | Actor that started this daemon |
| `host_id` | Host identifier |
| `ipc_endpoint` | IPC socket path |
| `lease_ms` | Lease duration |
| `last_heartbeat_ts` | Last heartbeat time |
| `expires_ts` | Lease expiration time |

## Actor Files

Each actor has a directory under `.git/grite/actors/<actor_id>/` containing only identity and key material. Actors no longer own separate databases.

### config.toml

Actor identity and settings.

```toml
actor_id = "64d15a2c383e2161772f9cea23e87222"
label = "work-laptop"
created_ts = 1700000000000
public_key = "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"
key_scheme = "ed25519"
```

| Field | Description |
|-------|-------------|
| `actor_id` | Actor identifier (required) |
| `label` | Human-friendly name (optional) |
| `created_ts` | Creation timestamp (optional) |
| `public_key` | Ed25519 public key (optional) |
| `key_scheme` | Signature scheme (optional) |

### keys/signing.key

Ed25519 private key for event signing.

!!! warning
    This file contains sensitive cryptographic material. Never share or commit it.

## Git Refs

### refs/grite/wal

The append-only event log. See [Git WAL](git-wal.md) for format details.

### refs/grite/snapshots/<timestamp>

Point-in-time snapshots for fast rebuilds.

- Timestamp is Unix milliseconds
- Contains consolidated events up to that point
- Multiple snapshots may exist
- Old snapshots removed by `grite snapshot gc`

### refs/grite/locks/<resource_hash>

Distributed lease locks.

- Resource hash is SHA-256 of resource name
- Contains lock metadata (owner, expiry)
- Deleted on release or expiration

## Materialized View Keys

The sled database uses these key patterns:

| Key Pattern | Value | Purpose |
|-------------|-------|---------|
| `event/<event_id>` | Archived Event | Event storage |
| `issue_state/<issue_id>` | IssueProjection | Current issue state |
| `issue_events/<issue_id>/<ts>/<event_id>` | Empty | Event index per issue |
| `label_index/<label>/<issue_id>` | Empty | Label-to-issue index |
| `dep_forward/<source_id>/<target_id>/<type>` | Empty | Dependency: source → target |
| `dep_reverse/<target_id>/<source_id>/<type>` | Empty | Dependency: target → source (reverse lookup) |
| `context_files/<path>` | FileContext (JSON) | File context with symbols |
| `context_symbols/<symbol_name>/<path>` | Empty | Symbol-to-file inverted index |
| `context_project/<key>` | ProjectContextEntry (JSON) | Project key/value metadata |
| `meta/last_rebuild_ts` | u64 | Last rebuild timestamp |
| `meta/wal_head` | String | Last processed WAL commit |

## Storage Size

### Git Refs

- WAL size grows with event count
- Each event is ~100-500 bytes CBOR
- Snapshots compress old events

### Sled Database

- Size depends on issue count and event count
- Typical: 1-10 MB for thousands of issues
- Compacted on rebuild

### Estimating Size

```bash
# Check sled database size
grite db stats --json | jq '.data.size_bytes'

# Check git ref sizes
git for-each-ref --format='%(refname) %(objectsize)' refs/grite/
```

## Cleanup

### Rebuild Database

Recompute materialized view from events:

```bash
grite rebuild
```

### Delete and Rebuild

For a fresh start:

```bash
rm -rf .git/grite/sled
grite rebuild
```

### Garbage Collection

Remove old snapshots:

```bash
grite snapshot gc
```

Clean expired locks:

```bash
grite lock gc
```

## Multi-Actor Scenarios

All actors share a single sled database. Actor identity is an authorship field on each event, not a storage partition:

```
.git/grite/
├── sled/          # One shared database for all actors
├── daemon.lock    # One daemon per repository
└── actors/
    ├── actor-a/
    │   └── config.toml
    ├── actor-b/
    │   └── config.toml
    └── actor-c/
        └── config.toml
```

- All events from all actors land in the shared sled
- No rebuild needed when switching between actors
- Shared WAL in git refs

## Backup Considerations

### What to Backup

- **Always**: Git refs (`refs/grite/*`) - contains all state
- **Optional**: Actor configs - can be recreated
- **Skip**: sled databases - can be rebuilt

### Backup Commands

```bash
# Backup WAL (git refs are included in normal git backup)
git push backup refs/grite/*:refs/grite/*

# Export for external backup
grite export --format json
```

## Security Considerations

### Sensitive Files

| File | Sensitivity |
|------|-------------|
| `keys/signing.key` | High - private key |
| `daemon.lock` | Low - runtime info |
| `config.toml` files | Low - no secrets |
| `sled/` | Medium - contains issue content |

### Permissions

```bash
# Signing key should be owner-only
chmod 600 .git/grite/actors/*/keys/signing.key
```

## Migration from Per-Actor Sleds

Older versions of grite stored each actor's events in its own sled at `.git/grite/actors/<id>/sled/`. These are now legacy artifacts. To migrate:

```bash
grite doctor
# If legacy_actor_sleds shows unmerged events:
grite doctor --fix
```

`grite doctor --fix` reads all per-actor sleds, merges any events not already in the shared store, rebuilds projections, then the old sleds can be safely deleted.

## Next Steps

- [Git WAL](git-wal.md) - WAL format details
- [Configuration](../reference/configuration.md) - Config options
- [Operations](../operations/index.md) - Maintenance tasks
