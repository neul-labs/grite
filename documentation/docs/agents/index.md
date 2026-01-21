# For AI Agents

This section contains documentation specifically for AI coding agents using grit.

## Overview

Grit is designed with AI coding agents as a primary use case. The git-backed architecture ensures agents can:

- Work autonomously while coordinating with humans and other agents
- Maintain persistent memory across sessions
- Use non-interactive CLI with structured JSON output
- Coordinate work via distributed locks

## Documentation

### [Agent Playbook](playbook.md)

Quick reference for agent behavior:

- Non-interactive contract
- Startup routine
- Planning and checkpointing
- Lock coordination
- Finishing tasks

## Quick Start for Agents

### Startup Routine

Run at the beginning of each session:

```bash
grit sync --pull --json
grit issue list --state open --label agent:todo --json
grit issue list --state open --label priority:P0 --json
```

### Claim a Task

```bash
grit lock acquire --resource "issue:$ID" --ttl 30m --json
grit issue comment $ID --body "Starting work" --json
```

### Post Progress

```bash
grit issue comment $ID --body "Checkpoint: Completed X, starting Y" --json
```

### Finish

```bash
grit issue close $ID --json
grit lock release --resource "issue:$ID" --json
grit sync --push --json
```

## Key Principles

### Non-Interactive

- Always use `--json` for structured output
- Never use interactive commands
- Never force-push git refs

### Isolated Actors

Each agent should use its own actor:

```bash
grit actor init --label "agent-$(hostname)"
```

Or set via environment:

```bash
export GRIT_HOME=/path/to/agent/data
```

### Coordinate with Locks

Before modifying shared resources:

```bash
grit lock acquire --resource "path:src/critical.rs" --ttl 15m --json
# ... work ...
grit lock release --resource "path:src/critical.rs" --json
```

### Document Everything

Leave clear trails for other agents:

```bash
grit issue comment $ID --body "$(cat <<'EOF'
## Progress
- Completed: X
- In Progress: Y
- Blocked: Z (need clarification)

## Files Modified
- src/foo.rs
- src/bar.rs
EOF
)" --json
```

## Integration Examples

### Python Agent

```python
import subprocess
import json

def grit_cmd(args):
    result = subprocess.run(
        ["grit"] + args + ["--json"],
        capture_output=True,
        text=True
    )
    return json.loads(result.stdout)

# Get open tasks
tasks = grit_cmd(["issue", "list", "--state", "open", "--label", "agent:todo"])
for issue in tasks["data"]["issues"]:
    print(f"Task: {issue['title']}")
```

### Shell Script

```bash
#!/bin/bash

# Startup
grit sync --pull --json > /dev/null

# Get first available task
TASK=$(grit issue list --state open --label "agent:todo" --json | jq -r '.data.issues[0].issue_id')

if [ -n "$TASK" ] && [ "$TASK" != "null" ]; then
  # Claim it
  grit lock acquire --resource "issue:$TASK" --ttl 30m --json

  # Work...

  # Finish
  grit issue close "$TASK" --json
  grit lock release --resource "issue:$TASK" --json
  grit sync --push --json
fi
```

## Related Documentation

- [AI Agents Use Case](../use-cases/ai-agents.md) - Detailed workflows
- [Distributed Locks](../guides/locking.md) - Coordination patterns
- [CLI Reference](../reference/cli.md) - All commands
- [JSON Output](../reference/cli-json.md) - Output schemas
