# Architecture

## Overview

Grit is split into three layers:

1. **Git-backed WAL** (source of truth)
   - Append-only events in `refs/grit/wal`
   - No tracked files in the working tree
2. **Materialized view** (fast local query)
   - `sled` DB in `.git/grit/actors/<actor_id>/sled/`
   - Deterministic projections from the WAL, values encoded with `rkyv`
3. **Optional daemon** (performance only)
   - Background fetch/push
   - Warm cache and pub/sub notifications

Correctness never depends on the daemon; the CLI can always rebuild state from the WAL.

## Components

- `libgrit-core`: event types, hashing, projections, sled store
- `libgrit-git`: WAL commit read/write, snapshot handling, ref sync
- `libgrit-ipc`: shared IPC message schema (rkyv)
- `grit`: CLI frontend
- `gritd`: daemon (optional)

## Data flow

1. CLI creates events
2. Events are appended to the WAL ref as a new git commit
3. Local materialized view is updated from new WAL events
4. `grit sync` pushes/pulls the refs

## Storage footprint

Local state is scoped per actor. Each agent gets its own data directory to avoid multi-process writes to the same DB.

- `.git/grit/actors/<actor_id>/sled/`: local DB (per actor)
- `.git/grit/actors/<actor_id>/config.toml`: local config and actor identity
- `.git/grit/config.toml`: repo-level defaults (for example, default actor)
- `.grit/`: optional export output (gitignored)
- `refs/grit/wal`: append-only event log
- `refs/grit/snapshots/*`: optional, monotonic snapshots
- `refs/grit/locks/*`: optional lease locks
