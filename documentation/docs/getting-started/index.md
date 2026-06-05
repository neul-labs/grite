# Getting Started

Welcome to Grite. This section walks you from installing the binary to creating, syncing, and reasoning about your first issues.

## What Grite Is

Grite is a repo-local, git-backed issue and task system designed for both AI coding agents and humans. Instead of pushing state to an external service (GitHub Issues, Jira, Linear), grite stores an append-only event log inside your git repository under `refs/grite/*` and projects it into a fast local view.

- Issues live in the repo. `git clone` brings the entire tracker with you.
- Works offline. Syncing is a normal `git fetch` / `git push`.
- No daemons required for correctness. The CLI is self-sufficient; the daemon is a performance optimization.
- Deterministic merges. State is built from CRDT-style projections so two actors that diverge converge again after sync.

## Quick Path

1. **[Installation](installation.md)** — Install the `grite` and `grite-daemon` binaries via the install script, Homebrew, Cargo, npm, pip, gem, or Chocolatey.
2. **[Quick Start](quickstart.md)** — Initialize a repo, create your first issue, sync to a remote.
3. **[Core Concepts](concepts.md)** — Understand events, actors, the materialized view, and the WAL.

## Prerequisites

Before installing grite, make sure you have:

- **Git 2.38+** — Grite relies on git refs (`refs/grite/wal`, `refs/grite/locks/*`) and modern ref transactions.
- **A git repository** — `grite init` runs inside an existing git repo; it does not initialize git for you.

Optional:

- **Rust 1.75+** if you want to build from source or install via `cargo install grite grite-daemon`.

## What You Get After Installation

After running the install script you will have two binaries on your `PATH`:

| Binary | Purpose |
|--------|---------|
| `grite` | The CLI you interact with day-to-day |
| `grite-daemon` | Optional background process, auto-spawned by the CLI for performance |

You do not need to start `grite-daemon` manually. The CLI spawns it on first use and shuts it down after an idle timeout.

## Recommended Reading Order

If you are a **developer** trying grite for the first time, read in this order:

1. [Installation](installation.md)
2. [Quick Start](quickstart.md)
3. [Working with Issues](../guides/issues.md)
4. [Syncing with Remotes](../guides/syncing.md)

If you are an **AI coding agent** (or building one), read:

1. [Installation](installation.md)
2. [Agent Playbook](../agents/playbook.md) — JSON output, actor identity, lock etiquette.
3. [JSON Output](../reference/cli-json.md) — Schemas for scripting.

If you are an **operator** running grite for a team, read:

1. [Quick Start](quickstart.md)
2. [Health Checks](../operations/doctor.md)
3. [Rebuilding](../operations/rebuild.md) and [Snapshots](../operations/snapshots.md)

## Next

Ready to install? Continue to [Installation](installation.md).
