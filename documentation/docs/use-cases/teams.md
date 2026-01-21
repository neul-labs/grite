# Development Teams

Teams benefit from distributed coordination that syncs via git without requiring external services.

## Why Grit for Teams?

- **Decentralized**: No central server needed
- **Git-native**: Syncs with existing git workflows
- **Offline-capable**: Team members work offline
- **Conflict-free**: CRDT merging, no manual conflicts

## Distributed Coordination

Multiple team members sync issues through standard git operations:

```bash
# Developer A creates an issue
grit issue create --title "API response time degradation" \
  --body "P95 latency increased 40% after last deploy" \
  --label "bug" --label "priority:P1"
grit sync --push

# Developer B pulls and claims
grit sync --pull
grit issue list --label "priority:P1"
grit issue comment $ISSUE_ID --body "I'll investigate this"
grit sync --push

# Developer A sees the update
grit sync --pull
grit issue show $ISSUE_ID
```

## Code Review Workflows

Track code review feedback offline, sync when ready:

```bash
# Reviewer creates review issues
grit issue create --title "Review: PR #87 - Add caching layer" \
  --body "$(cat <<'EOF'
## Overall
Good approach, some concerns about cache invalidation.

## Required Changes
- src/cache/mod.rs:45 - Need TTL on cache entries
- src/cache/mod.rs:78 - Handle cache miss more gracefully

## Suggestions
- Consider using dashmap instead of RwLock<HashMap>
- Add metrics for cache hit rate
EOF
)" --label "review" --label "pr:87"

# Author addresses feedback
grit issue comment $ISSUE_ID --body "Addressed TTL and cache miss handling. Will look into dashmap."
grit sync --push
```

### Review Workflow

1. Create review issue when starting review
2. Document findings in the issue body
3. Author responds via comments
4. Close when PR is merged

## Onboarding Task Lists

Create templated onboarding checklists for new team members:

```bash
# Team lead creates onboarding issue for new developer
grit issue create --title "Onboarding: Alice" \
  --body "$(cat <<'EOF'
## Week 1
- [ ] Set up development environment (see docs/setup.md)
- [ ] Complete codebase walkthrough with mentor
- [ ] Fix a "good first issue" bug
- [ ] Set up grit actor: `grit actor init --label "alice-laptop"`

## Week 2
- [ ] Pair on a medium complexity feature
- [ ] Review and understand CI/CD pipeline
- [ ] Shadow on-call rotation

## Resources
- Architecture docs: docs/architecture.md
- Team conventions: docs/conventions.md
- Contact: @bob for backend, @carol for frontend
EOF
)" --label "onboarding" --assignee "alice"
```

## Knowledge Base / ADRs

Record architectural decisions with rationale:

```bash
grit issue create --title "ADR-001: Use PostgreSQL for primary database" \
  --body "$(cat <<'EOF'
## Status
Accepted

## Context
Need to choose a primary database for the application. Options considered:
- PostgreSQL
- MySQL
- MongoDB

## Decision
Use PostgreSQL.

## Rationale
- Strong ACID compliance needed for financial data
- Team has PostgreSQL expertise
- Excellent tooling (pgAdmin, pg_dump)
- JSONB for semi-structured data where needed

## Consequences
- Must manage PostgreSQL in production
- Schema migrations required for changes
- Some team members need PostgreSQL training
EOF
)" --label "adr" --label "database"
```

### ADR Conventions

```bash
# List all ADRs
grit issue list --label "adr"

# ADR naming
"ADR-001: Short title"
"ADR-002: Another decision"

# ADR labels
--label "adr"
--label "status:proposed"    # Under discussion
--label "status:accepted"    # Approved
--label "status:deprecated"  # No longer applies
```

## Large Refactoring Coordination

Track progress on refactoring efforts spanning multiple team members:

```bash
# Create tracking issue for refactoring
grit issue create --title "Refactor: Migrate from callbacks to async/await" \
  --body "$(cat <<'EOF'
## Scope
Convert all callback-based async code to async/await syntax.

## Progress Tracker
| Module | Status | Owner |
|--------|--------|-------|
| src/api/ | Not started | - |
| src/db/ | In progress | Bob |
| src/services/ | Not started | - |
| src/utils/ | Complete | Alice |

## Guidelines
- One module at a time to minimize merge conflicts
- Update tests alongside implementation
- Use `grit lock acquire --resource "path:src/<module>"` before starting
EOF
)" --label "refactor" --label "epic"

# Team members update progress
grit issue comment $ISSUE_ID --body "src/db/ complete, moving to src/api/"
```

## Code Ownership Documentation

Track who owns/maintains which areas:

```bash
grit issue create --title "[Ownership] Code ownership map" \
  --body "$(cat <<'EOF'
## Module Owners

| Path | Primary | Secondary |
|------|---------|-----------|
| src/api/ | @alice | @bob |
| src/auth/ | @carol | @alice |
| src/db/ | @bob | @dave |
| src/frontend/ | @eve | @carol |
| infra/ | @dave | @bob |

## Review Policy
- Changes require approval from primary owner
- Secondary can approve if primary unavailable >24h
EOF
)" --label "meta" --label "ownership"
```

## Sprint Planning

Track sprint tasks:

```bash
# Create sprint issue
grit issue create --title "Sprint 2024-W03" \
  --body "$(cat <<'EOF'
## Goals
- Complete authentication refactor
- Deploy new monitoring

## Committed Work
- [ ] AUTH-123: Add MFA support (@alice)
- [ ] AUTH-124: Session management (@bob)
- [ ] OPS-456: Prometheus setup (@carol)

## Stretch Goals
- [ ] UI-789: Dark mode toggle
EOF
)" --label "sprint" --label "sprint:2024-W03"

# Close at sprint end
grit issue close $SPRINT_ID
```

## Standup Notes

Quick daily standup documentation:

```bash
grit issue create --title "Standup: 2024-01-15" \
  --body "$(cat <<'EOF'
## Alice
- Yesterday: Completed MFA backend
- Today: Start MFA frontend
- Blockers: None

## Bob
- Yesterday: Investigated session bug
- Today: Fix session bug
- Blockers: Need access to prod logs

## Carol
- Yesterday: Prometheus config
- Today: Grafana dashboards
- Blockers: None
EOF
)" --label "standup"
```

## Team Conventions

### Label Standards

Agree on team-wide labels:

```bash
# Types
bug, feature, tech-debt, docs, refactor

# Priority
priority:P0, priority:P1, priority:P2

# Status
status:todo, status:in-progress, status:blocked, status:review

# Components
component:api, component:frontend, component:infra

# Sprint
sprint:2024-W01, sprint:2024-W02
```

### Actor Naming

Each team member uses consistent actor labels:

```bash
grit actor init --label "alice-work-laptop"
grit actor init --label "bob-desktop"
grit actor init --label "ci-main"
```

### Sync Frequency

Establish team sync practices:

- Sync at start and end of work sessions
- Sync before and after major changes
- Push before going offline

## Best Practices

### Use Locks for Coordination

When working on shared resources:

```bash
# Before editing shared config
grit lock acquire --resource "path:config/settings.json" --ttl 30m
# Make changes
grit lock release --resource "path:config/settings.json"
```

### Keep Issues Updated

```bash
# Regular progress updates
grit issue comment $ID --body "50% complete, tests passing"
grit sync --push
```

### Archive Completed Work

```bash
# Close and label for archiving
grit issue close $ID
grit issue label add $ID --label "archived"
```

## Next Steps

- [Syncing](../guides/syncing.md) - Team sync patterns
- [Distributed Locks](../guides/locking.md) - Coordination details
- [Actor Identity](../guides/actors.md) - Team actor management
