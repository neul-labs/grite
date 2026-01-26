# Actor Identity

This guide explains actors in grite and how to manage them for multi-agent scenarios.

## What Are Actors?

An actor represents a device or agent using grite. Each actor has:

- **actor_id**: 128-bit random identifier
- **label**: Human-friendly name (e.g., "work-laptop", "ci-agent")
- **data directory**: Isolated local database

## Why Actors Matter

### Attribution

Every event records which actor created it:

```json
{
  "event_id": "...",
  "actor": "64d15a2c383e2161772f9cea23e87222",
  "kind": { "IssueCreated": { ... } }
}
```

### Isolation

Each actor has its own materialized view database. This allows:

- Multiple agents on the same machine
- No conflicts between concurrent processes
- Independent state rebuilds

### CRDT Tie-Breaking

When two actors update the same field simultaneously, the actor ID is used for deterministic tie-breaking (after timestamp comparison).

## Creating Actors

### First Actor

When you run `grite init`, a default actor is created:

```bash
grite init
# Creates actor and sets as default
```

### Additional Actors

Create more actors for different purposes:

```bash
grite actor init --label "ci-agent"
```

Output:

```
Created actor e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0
  Label: ci-agent
  Data dir: .git/grite/actors/e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0/
```

## Listing Actors

See all actors in the repository:

```bash
grite actor list
```

Output:

```
64d15a2c...  work-laptop    (default)
e5f6a7b8...  ci-agent
```

### JSON Output

```bash
grite actor list --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "actors": [
      {
        "actor_id": "64d15a2c383e2161772f9cea23e87222",
        "label": "work-laptop",
        "data_dir": ".git/grite/actors/64d15a2c383e2161772f9cea23e87222/"
      }
    ]
  }
}
```

## Viewing Actor Details

Show details for a specific actor:

```bash
grite actor show 64d15a2c
```

Or for the current actor:

```bash
grite actor show
```

## Switching Actors

### Set Default Actor

Set the repository's default actor:

```bash
grite actor use e5f6a7b8
```

This updates `.git/grite/config.toml`:

```toml
default_actor = "e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0"
```

### Per-Command Actor

Use a specific actor for one command:

```bash
grite --actor e5f6a7b8 issue create --title "..."
```

### Environment Variable

Set actor via environment:

```bash
export GRIT_HOME=.git/grite/actors/e5f6a7b8/
grite issue list
```

## Actor Selection Order

Grite resolves actor context in this order:

1. `--data-dir` or `GRIT_HOME` environment variable
2. `--actor <id>` flag
3. `default_actor` in `.git/grite/config.toml`
4. Auto-create new actor if none exists

## Current Actor

Check which actor is currently active:

```bash
grite actor current
```

Output:

```
Actor: 64d15a2c383e2161772f9cea23e87222
Label: work-laptop
Source: repo_default
```

With JSON:

```bash
grite actor current --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "actor_id": "64d15a2c383e2161772f9cea23e87222",
    "data_dir": ".git/grite/actors/64d15a2c383e2161772f9cea23e87222/",
    "source": "repo_default"
  }
}
```

## Multi-Agent Scenarios

### CI/CD Pipeline

Create a dedicated CI actor:

```bash
# In CI setup
grite actor init --label "ci-$(CI_JOB_ID)"
```

### Multiple AI Agents

Each agent should use its own actor:

```bash
# Agent A
grite actor init --label "agent-planner"
PLANNER_ID=$(grite actor current --json | jq -r '.data.actor_id')

# Agent B
grite actor init --label "agent-implementer"
IMPL_ID=$(grite actor current --json | jq -r '.data.actor_id')
```

### Team Members

Each team member can use their own actor:

```bash
grite actor init --label "alice-laptop"
grite actor init --label "bob-desktop"
```

## Actor Configuration

Actor config is stored in `.git/grite/actors/<id>/config.toml`:

```toml
actor_id = "64d15a2c383e2161772f9cea23e87222"
label = "work-laptop"
created_ts = 1700000000000
public_key = "..."  # Optional, if signing enabled
key_scheme = "ed25519"  # Optional
```

## Signing Keys

Actors can have Ed25519 signing keys for event authentication:

```bash
grite actor init --label "signed-actor" --generate-key
```

This creates:

- Private key: `.git/grite/actors/<id>/keys/signing.key`
- Public key: stored in `config.toml`

Events from this actor will be signed automatically.

## Best Practices

### Use Descriptive Labels

```bash
# Good
grite actor init --label "ci-main-branch"
grite actor init --label "alice-macbook"

# Avoid
grite actor init --label "actor1"
```

### One Actor Per Purpose

- One actor per developer workstation
- One actor per CI job
- One actor per AI agent instance

### Don't Share Actors

Each process/agent should use its own actor to avoid database conflicts.

### Clean Up Old Actors

Remove actors you no longer use:

```bash
rm -rf .git/grite/actors/<old-actor-id>/
```

## Next Steps

- [Distributed Locks](locking.md) - Coordinate between actors
- [Using the Daemon](daemon.md) - Actor and daemon interaction
- [Configuration Reference](../reference/configuration.md) - Actor config details
