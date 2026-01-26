# Individual Developers

Solo developers benefit from git-native issue tracking that works offline and stays with the repository.

## Why Grite for Developers?

- **Offline-first**: Works without network connectivity
- **Portable**: Issues travel with the repo
- **Private**: Personal tracking without polluting team trackers
- **Lightweight**: No accounts, no services, just `grite init`

## Offline Issue Tracking

Work on issues without network connectivity, sync when connected:

```bash
# On a plane, create and work on issues
grite issue create --title "Refactor database connection pool" \
  --body "Current implementation doesn't handle reconnection properly"

grite issue comment $ISSUE_ID --body "Fixed in commit abc123"
grite issue close $ISSUE_ID

# Later, when back online
grite sync --push
```

## Private Technical Debt Tracking

Maintain a personal list of cleanup tasks:

```bash
# Track tech debt locally
grite issue create --title "Replace deprecated DateTime API" \
  --body "chrono 0.4 deprecated some methods we use in src/utils/time.rs" \
  --label "tech-debt" --label "low-priority"

grite issue create --title "Add error context to database errors" \
  --body "Raw sqlx errors leak to API responses" \
  --label "tech-debt" --label "error-handling"

# Review tech debt periodically
grite issue list --label "tech-debt"
```

### Tech Debt Labels

Organize your debt tracking:

| Label | Use |
|-------|-----|
| `tech-debt` | All technical debt |
| `low-priority` | Can wait |
| `high-priority` | Should fix soon |
| `quick-win` | Easy fixes |
| `refactor` | Needs restructuring |

## Pre-PR Checklists

Track checklist items before submitting a pull request:

```bash
# Create PR prep checklist
grite issue create --title "PR Prep: Add rate limiting" \
  --body "$(cat <<'EOF'
## Checklist
- [ ] Implementation complete
- [ ] Unit tests added
- [ ] Integration tests pass
- [ ] Documentation updated
- [ ] CHANGELOG entry added
- [ ] No TODO comments left
- [ ] Rebased on main
EOF
)"

# Update as you complete items
grite issue update $ISSUE_ID --body "$(cat <<'EOF'
## Checklist
- [x] Implementation complete
- [x] Unit tests added
- [ ] Integration tests pass
- [ ] Documentation updated
- [ ] CHANGELOG entry added
- [x] No TODO comments left
- [ ] Rebased on main
EOF
)"
```

## Investigation Notes

Document bug investigation steps for complex issues:

```bash
grite issue create --title "Investigation: Intermittent test failures in CI" \
  --body "$(cat <<'EOF'
## Symptoms
- test_concurrent_writes fails ~10% of CI runs
- Only on Linux, not macOS

## Investigation Log
1. Checked for race conditions - none obvious
2. Added logging, found timing issue in setup
3. Root cause: test database not fully initialized

## Solution
Added retry logic with backoff in test_helpers::wait_for_db()

## Prevention
Consider adding CI job to run flaky test detection
EOF
)"
```

### Investigation Template

```bash
# Create a standard investigation issue
grite issue create --title "Investigation: $PROBLEM" \
  --body "$(cat <<'EOF'
## Symptoms
-

## Steps to Reproduce
1.

## Investigation Log
-

## Root Cause
TBD

## Solution
TBD
EOF
)" --label "investigation"
```

## Personal Task Management

Keep a personal task list that travels with the repo:

```bash
# Morning: plan the day
grite issue create --title "Today: Review PR #42" --label "today"
grite issue create --title "Today: Fix login redirect bug" --label "today"
grite issue create --title "Today: Update API docs" --label "today"

# Track progress
grite issue list --label "today" --state open

# End of day: close completed
grite issue close $COMPLETED_ID

# Tomorrow: relabel remaining
grite issue label remove $REMAINING_ID --label "today"
grite issue label add $REMAINING_ID --label "backlog"
```

### Daily Workflow

```bash
# Start of day
alias today='grite issue list --label "today" --state open'
alias done='grite issue close'

# Quick task creation
alias task='grite issue create --label "today" --title'

# Usage
task "Review PR #42"
today
done abc123
```

## Learning Journal

Track learnings and discoveries:

```bash
grite issue create --title "[Learning] Rust async patterns" \
  --body "$(cat <<'EOF'
## Key Insights
- Use tokio::spawn for CPU-bound work
- async-trait crate needed for async in traits
- Pin is required for self-referential futures

## Resources
- https://tokio.rs/tokio/tutorial
- Rust async book

## Practice
Implemented in feature/async-refactor branch
EOF
)" --label "learning" --label "rust"
```

## Project Ideas

Track ideas for future work:

```bash
grite issue create --title "[Idea] Add GraphQL API" \
  --body "$(cat <<'EOF'
## Motivation
Current REST API requires multiple round-trips for related data.

## Approach
- async-graphql crate looks promising
- Start with read-only queries
- Add mutations later

## Effort Estimate
Medium - 1-2 weeks for basic implementation

## Dependencies
- None blocking
EOF
)" --label "idea" --label "api"

# Review ideas later
grite issue list --label "idea"
```

## Reading Notes

Track notes from documentation or articles:

```bash
grite issue create --title "[Notes] Kubernetes networking" \
  --body "$(cat <<'EOF'
## Source
https://kubernetes.io/docs/concepts/services-networking/

## Key Points
- Services provide stable IP for pods
- Ingress handles external traffic
- NetworkPolicies control pod-to-pod traffic

## Questions
- How does service mesh (Istio) fit in?
- When to use LoadBalancer vs NodePort?

## Apply To
Our deployment in cluster/k8s/
EOF
)" --label "notes" --label "kubernetes"
```

## Quick Reference Workflow

### Morning Startup

```bash
grite sync --pull
grite issue list --label "today" --state open
```

### Throughout Day

```bash
# Quick task
grite issue create --title "Fix typo in README" --label "quick"

# Start working
grite issue comment $ID --body "Started"

# Complete
grite issue close $ID
```

### End of Day

```bash
grite issue list --state open --label "today"  # See remaining
grite sync --push  # Save progress
```

## Best Practices

### Use Labels Consistently

```bash
# Types
--label "task"         # Regular work
--label "bug"          # Bug fix needed
--label "tech-debt"    # Cleanup
--label "idea"         # Future consideration
--label "learning"     # Learning notes

# Priority
--label "today"        # Do today
--label "this-week"    # Do this week
--label "backlog"      # Someday
```

### Keep Issues Focused

One issue = one task. Split large tasks:

```bash
# Instead of one big issue
grite issue create --title "Refactor entire API" ...

# Create multiple focused issues
grite issue create --title "Refactor auth endpoints" --label "refactor"
grite issue create --title "Refactor user endpoints" --label "refactor"
grite issue create --title "Refactor product endpoints" --label "refactor"
```

### Archive Don't Delete

Close issues rather than trying to delete:

```bash
grite issue close $ID
grite issue label add $ID --label "archived"
```

## Next Steps

- [Working with Issues](../guides/issues.md) - Complete issue guide
- [Syncing](../guides/syncing.md) - Sync across devices
- [CLI Reference](../reference/cli.md) - All commands
