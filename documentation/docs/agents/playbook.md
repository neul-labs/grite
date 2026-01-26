# Agent Playbook

This playbook provides guidelines for AI coding agents using grite as their task and memory system.

## Non-Interactive Contract

When using grite as an agent:

- **Use `--json`** for all reads and writes
- **Do not** run interactive commands (no editor prompts)
- **Do not** force-push `refs/grite/*`
- **On inconsistencies**, run `grite doctor --json` and follow the plan

## Startup Routine

Run at the beginning of each session:

```bash
# Sync latest state
grite sync --pull --json

# Get high-priority tasks
grite issue list --state open --label priority:P0 --json

# Get agent-assigned tasks
grite issue list --state open --label agent:todo --json
```

**Select exactly one issue at a time.**

## Actor Isolation

If multiple agents share the same `.git` directory, each agent must use a separate data directory.

Options:

```bash
# Option 1: Environment variable
export GRIT_HOME=/path/to/agent/data

# Option 2: Flag
grite --data-dir /path/to/agent/data issue list --json

# Option 3: Actor ID
grite --actor <actor_id> issue list --json
```

**Never share the local sled database between processes.**

## Claiming Work

Before starting work on an issue:

```bash
# Acquire lock on the issue
grite lock acquire --resource "issue:$ISSUE_ID" --ttl 30m --json

# Update issue to indicate work started
grite issue comment $ISSUE_ID --body "Agent starting work" --json
```

If the lock is unavailable:

1. Pick another issue
2. Or coordinate via comments
3. Or wait for lock to expire

## Plan Format

Before coding, post a plan comment:

```bash
grite issue comment $ISSUE_ID --body "$(cat <<'EOF'
## Plan

**Intended changes:**
- src/auth/login.rs
- src/routes/api.rs

**Tests:**
- cargo test auth::
- cargo test integration::login

**Rollback:**
- Revert commit if tests fail
EOF
)" --json
```

## Checkpoints

After each milestone, post a checkpoint:

```bash
grite issue comment $ISSUE_ID --body "$(cat <<'EOF'
## Checkpoint

**What changed:**
- Added password validation in src/auth/validate.rs
- Updated login endpoint to use new validation

**Why:**
- Fix security vulnerability in password handling

**Tests run:**
- cargo test auth::validate - PASSED
- cargo test integration::login - PASSED
EOF
)" --json
```

## Lock Management

### For File Editing

When editing shared or risky areas:

```bash
# Acquire lock
grite lock acquire --resource "path:src/critical_file.rs" --ttl 15m --json

# Work on file...

# Renew if needed
grite lock renew --resource "path:src/critical_file.rs" --ttl 15m --json

# Release when done
grite lock release --resource "path:src/critical_file.rs" --json
```

### Lock Namespaces

| Namespace | Use |
|-----------|-----|
| `issue:$ID` | Claiming an issue |
| `path:$PATH` | File or directory |
| `repo:$NAME` | Repository-wide operations |

### If Lock Unavailable

```bash
# Check who has the lock
grite lock status --json

# Coordinate via comments
grite issue comment $OTHER_ISSUE --body "Need access to src/auth.rs - please release when done" --json
```

## Handoff Notes

Before ending a session with incomplete work:

```bash
grite issue comment $ISSUE_ID --body "$(cat <<'EOF'
## Handoff Notes

**Completed:**
- Database schema changes
- API endpoint skeleton

**In Progress:**
- Password hashing (50% done)

**Blocked:**
- Need clarification on session timeout policy

**Files Modified:**
- src/models/user.rs (new)
- src/routes/auth.rs (partial)
- Cargo.toml (added bcrypt)

**Next Steps:**
1. Complete bcrypt integration
2. Add input validation
3. Write integration tests
EOF
)" --json

# Release lock so another agent can continue
grite lock release --resource "issue:$ISSUE_ID" --json

# Sync
grite sync --push --json
```

## Finishing a Task

Before closing an issue:

```bash
# Post verification notes
grite issue comment $ISSUE_ID --body "$(cat <<'EOF'
## Verification

**Commands run:**
- cargo test - all 42 tests passed
- cargo clippy - no warnings

**Manual verification:**
- Tested login flow locally
- Confirmed password validation works

**Commit:**
- abc123: Implement password validation
EOF
)" --json

# Close the issue
grite issue close $ISSUE_ID --json

# Release any locks
grite lock release --resource "issue:$ISSUE_ID" --json

# Sync to remote
grite sync --push --json
```

## Memory Persistence

Store discoveries for future sessions:

```bash
# Save codebase knowledge
grite issue create \
  --title "[Memory] Authentication patterns" \
  --body "All auth uses middleware in src/middleware/auth.rs" \
  --label "memory" --label "auth" \
  --json

# Query memories at startup
grite issue list --label "memory" --json
```

## Error Handling

### On Doctor Warnings

```bash
result=$(grite doctor --json)
if echo "$result" | jq -e '.data.checks[] | select(.status == "error")' > /dev/null; then
  # Follow the remediation plan
  plan=$(echo "$result" | jq -r '.data.checks[] | select(.status == "error") | .plan[]')
  for cmd in $plan; do
    eval "$cmd"
  done
fi
```

### On Lock Conflicts

```bash
if ! grite lock acquire --resource "issue:$ID" --ttl 30m --json 2>/dev/null; then
  echo "Lock conflict - selecting different task"
  # Try next task
fi
```

### On Sync Failures

```bash
if ! grite sync --push --json; then
  # Full sync handles conflicts
  grite sync --json
fi
```

## Command Reference

| Task | Command |
|------|---------|
| Sync | `grite sync --json` |
| List tasks | `grite issue list --state open --json` |
| Show task | `grite issue show $ID --json` |
| Comment | `grite issue comment $ID --body "..." --json` |
| Close | `grite issue close $ID --json` |
| Acquire lock | `grite lock acquire --resource "..." --ttl 15m --json` |
| Renew lock | `grite lock renew --resource "..." --ttl 15m --json` |
| Release lock | `grite lock release --resource "..." --json` |
| Task order | `grite issue dep topo --state open --json` |
| Add dependency | `grite issue dep add $ID --target $TARGET --type blocks --json` |
| Index files | `grite context index --json` |
| Query symbols | `grite context query "Name" --json` |
| Project context | `grite context project --json` |
| Health check | `grite doctor --json` |

## Using Dependencies

Track task ordering with typed dependencies:

```bash
# Check what blocks your current task
grite issue dep list $ISSUE_ID --json

# Check what this task blocks (dependents waiting on you)
grite issue dep list $ISSUE_ID --reverse --json

# Find the next workable task (respects dependency order)
grite issue dep topo --state open --label agent:todo --json
```

## Using the Context Store

Orient yourself in a new codebase:

```bash
# Index project files (incremental, skips unchanged)
grite context index --json

# Find relevant symbols for your task
grite context query "Authentication" --json

# Understand a specific file
grite context show src/auth/mod.rs --json

# Record project knowledge for other agents
grite context set "auth_library" "argon2"
grite context set "test_command" "cargo test"

# Read project knowledge
grite context project --json
```

## Quick Reference Card

```
STARTUP:
  grite sync --pull --json
  grite issue dep topo --state open --label agent:todo --json

CLAIM:
  grite lock acquire --resource "issue:$ID" --ttl 30m --json
  grite issue comment $ID --body "Starting" --json

CHECKPOINT:
  grite issue comment $ID --body "Progress..." --json

FINISH:
  grite issue close $ID --json
  grite lock release --resource "issue:$ID" --json
  grite sync --push --json

ON ERROR:
  grite doctor --json
```
