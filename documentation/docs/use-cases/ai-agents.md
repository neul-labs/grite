# AI Coding Agents

Grite's primary design target is AI coding agents that need a canonical task and memory system. The git-backed architecture ensures agents can work autonomously while coordinating with humans and other agents.

## Why Grite for Agents?

- **Non-interactive CLI**: No prompts or interactive inputs
- **JSON output**: Structured data for reliable parsing
- **Distributed locks**: Coordinate multi-agent work
- **Persistent memory**: Issues survive across sessions
- **Git-native**: Syncs with standard git operations

## Task Decomposition & Orchestration

A coordinator agent can break down complex tasks into subtasks:

```bash
# Coordinator creates parent task
grite issue create --title "Implement user authentication" \
  --body "Full auth system with login, registration, and password reset" \
  --label "epic" --json

# Coordinator creates subtasks
grite issue create --title "Create user database schema" \
  --body "Design and implement User table with necessary fields" \
  --label "subtask" --label "database" --json

grite issue create --title "Implement login endpoint" \
  --body "POST /auth/login with JWT token response" \
  --label "subtask" --label "api" --json

grite issue create --title "Implement registration endpoint" \
  --body "POST /auth/register with email verification" \
  --label "subtask" --label "api" --json
```

### Parsing Task Output

```bash
# Get issue ID from creation
ISSUE_ID=$(grite issue create --title "..." --body "..." --json | jq -r '.data.issue_id')

# List subtasks
grite issue list --label "subtask" --json | jq '.data.issues[]'
```

## Multi-Agent Coordination

Multiple agents can work on the same repository by claiming tasks via locks:

```bash
# Agent A claims a task
grite lock acquire --resource "issue:$ISSUE_ID" --ttl 30m --json
grite issue update $ISSUE_ID --body "Claimed by Agent A" --json

# Agent A posts progress
grite issue comment $ISSUE_ID --body "Started implementation. Files: src/auth/login.rs" --json

# Agent A completes and releases
grite issue close $ISSUE_ID --json
grite lock release --resource "issue:$ISSUE_ID" --json
grite sync --push --json
```

### Checking Lock Status

Before claiming work:

```bash
# Check if issue is locked
LOCKED=$(grite lock status --json | jq -r ".data.locks[] | select(.resource == \"issue:$ISSUE_ID\")")
if [ -z "$LOCKED" ]; then
  grite lock acquire --resource "issue:$ISSUE_ID" --ttl 30m
fi
```

## Agent Memory Persistence

Agents can use issues as persistent memory that survives across sessions:

```bash
# Store discoveries about the codebase
grite issue create --title "[Memory] Authentication patterns" \
  --body "Discovered: All auth uses middleware in src/middleware/auth.rs. Token validation via jsonwebtoken crate." \
  --label "memory" --label "auth" --json

# Store lessons learned
grite issue create --title "[Memory] Testing conventions" \
  --body "Integration tests go in tests/integration/. Use test_helpers::setup_db() for database fixtures." \
  --label "memory" --label "testing" --json

# Query memory at session start
grite issue list --label "memory" --json
```

### Memory Categories

Use labels to categorize memories:

| Label | Use |
|-------|-----|
| `memory` | All memory issues |
| `memory:codebase` | Codebase structure |
| `memory:patterns` | Code patterns |
| `memory:conventions` | Project conventions |
| `memory:dependencies` | External dependencies |

## Agent Handoff Protocol

When an agent completes partial work, document state for another agent to resume:

```bash
# Agent A documents partial progress before session end
grite issue comment $ISSUE_ID --body "$(cat <<'EOF'
## Handoff Notes

**Completed:**
- Database schema in src/models/user.rs
- Basic login endpoint skeleton

**In Progress:**
- Password hashing (bcrypt integration started in Cargo.toml)

**Blocked:**
- Need clarification on session timeout policy

**Files Modified:**
- src/models/user.rs (new)
- src/routes/auth.rs (partial)
- Cargo.toml (added bcrypt)

**Next Steps:**
1. Complete bcrypt integration
2. Add password validation
3. Implement JWT generation
EOF
)" --json

# Agent B picks up later
grite sync --pull --json
grite issue show $ISSUE_ID --json
```

## Agent Startup Routine

At the start of each session:

```bash
#!/bin/bash
# agent_startup.sh

# Sync latest state
grite sync --pull --json

# Load memories
MEMORIES=$(grite issue list --label "memory" --json)
echo "$MEMORIES" | jq '.data.issues[] | {title, body}'

# Find available tasks
TASKS=$(grite issue list --label "todo" --state open --json)

# Claim first unclaimed task
for id in $(echo "$TASKS" | jq -r '.data.issues[].issue_id'); do
  if grite lock acquire --resource "issue:$id" --ttl 30m 2>/dev/null; then
    echo "Claimed task: $id"
    CURRENT_TASK=$id
    break
  fi
done
```

## Agent Finish Routine

At the end of each session:

```bash
#!/bin/bash
# agent_finish.sh

# If task incomplete, add handoff notes
if [ -n "$CURRENT_TASK" ]; then
  grite issue comment $CURRENT_TASK --body "Session ended. Work saved."
  grite lock release --resource "issue:$CURRENT_TASK"
fi

# Sync changes
grite sync --push --json
```

## Checkpointing

Periodically save progress:

```bash
# Every N minutes or after significant work
checkpoint() {
  local task_id=$1
  local status=$2

  grite issue comment $task_id --body "Checkpoint: $status"
  grite sync --push --json

  # Renew lock
  grite lock renew --resource "issue:$task_id" --ttl 30m
}
```

## Error Handling

Handle grite errors gracefully:

```bash
# Check command success
if ! result=$(grite issue create --title "..." --body "..." --json 2>&1); then
  error=$(echo "$result" | jq -r '.error.code')
  case "$error" in
    "conflict")
      echo "Lock conflict, task claimed by another agent"
      ;;
    "db_busy")
      echo "Database busy, retrying..."
      sleep 1
      grite issue create --title "..." --body "..." --json
      ;;
    *)
      echo "Unknown error: $error"
      ;;
  esac
fi
```

## Best Practices

### Use JSON Output

Always use `--json` for reliable parsing:

```bash
# Good
grite issue list --json | jq '.data.issues'

# Avoid (parsing human output is fragile)
grite issue list | grep "open"
```

### Renew Locks

For long-running tasks, renew locks periodically:

```bash
while working; do
  # Do work...
  sleep 300  # 5 minutes
  grite lock renew --resource "issue:$TASK" --ttl 30m
done
```

### Document Everything

Leave clear trails for other agents:

```bash
grite issue comment $ID --body "Starting work on X"
# ... do work ...
grite issue comment $ID --body "Completed X, found issue Y"
```

### Use Consistent Labels

```bash
# Task types
--label "epic"      # Large multi-part tasks
--label "subtask"   # Part of an epic
--label "todo"      # Available work
--label "blocked"   # Cannot proceed

# Memory types
--label "memory"    # Persistent knowledge
--label "discovery" # Found during exploration
```

### Sync Frequently

```bash
# After any significant change
grite issue create ... --json
grite sync --push --json
```

## Next Steps

- [Agent Playbook](../agents/playbook.md) - Quick reference for agents
- [Distributed Locks](../guides/locking.md) - Deep dive on coordination
- [CLI Reference](../reference/cli.md) - All commands with JSON schemas
