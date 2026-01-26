# Use Cases

Grite is designed for different audiences with distinct workflows. This document provides detailed use cases and practical examples for each.

## AI Coding Agents

Grite's primary design target is AI coding agents that need a canonical task and memory system. The git-backed architecture ensures agents can work autonomously while coordinating with humans and other agents.

### Task Decomposition & Orchestration

A coordinator agent can break down complex tasks into subtasks, each tracked as a separate issue.

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

### Multi-Agent Coordination

Multiple agents can work on the same repository by claiming tasks via locks and coordinating through comments.

```bash
# Agent A claims a task
grite lock acquire --resource "issue:<ISSUE_ID>" --ttl 30m --json
grite issue update <ISSUE_ID> --body "Claimed by Agent A" --json

# Agent A posts progress
grite issue comment <ISSUE_ID> --body "Started implementation. Files: src/auth/login.rs" --json

# Agent A completes and releases
grite issue close <ISSUE_ID> --json
grite lock release --resource "issue:<ISSUE_ID>" --json
grite sync --push --json
```

### Agent Memory Persistence

Agents can use issues as persistent memory that survives across sessions.

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

### Agent Handoff Protocol

When an agent completes partial work, it can document state for another agent to resume.

```bash
# Agent A documents partial progress before session end
grite issue comment <ISSUE_ID> --body "$(cat <<'EOF'
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
grite issue show <ISSUE_ID> --json
```

---

## Individual Developers

Solo developers benefit from git-native issue tracking that works offline and stays with the repository.

### Offline Issue Tracking

Work on issues without network connectivity, syncing when connected.

```bash
# On a plane, create and work on issues
grite issue create --title "Refactor database connection pool" \
  --body "Current implementation doesn't handle reconnection properly"

grite issue comment <ISSUE_ID> --body "Fixed in commit abc123"
grite issue close <ISSUE_ID>

# Later, when back online
grite sync --push
```

### Private Technical Debt Tracking

Maintain a personal list of cleanup tasks without polluting the team's issue tracker.

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

### Pre-PR Checklists

Track checklist items before submitting a pull request.

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
grite issue update <ISSUE_ID> --body "$(cat <<'EOF'
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

### Investigation Notes

Document bug investigation steps for complex issues that may recur.

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

### Personal Task Management

Keep a personal task list that travels with the repo.

```bash
# Morning: plan the day
grite issue create --title "Today: Review PR #42" --label "today"
grite issue create --title "Today: Fix login redirect bug" --label "today"
grite issue create --title "Today: Update API docs" --label "today"

# Track progress
grite issue list --label "today" --state open

# End of day: close completed, carry over rest
grite issue close <COMPLETED_ID>
```

---

## Development Teams

Teams benefit from distributed coordination that syncs via git without requiring external services.

### Distributed Coordination

Multiple team members sync issues through standard git operations.

```bash
# Developer A creates an issue
grite issue create --title "API response time degradation" \
  --body "P95 latency increased 40% after last deploy" \
  --label "bug" --label "priority:P1"
grite sync --push

# Developer B pulls and claims
grite sync --pull
grite issue list --label "priority:P1"
grite issue comment <ISSUE_ID> --body "I'll investigate this"
grite sync --push
```

### Code Review Workflows

Track code review feedback offline, sync when ready.

```bash
# Reviewer creates review issues
grite issue create --title "Review: PR #87 - Add caching layer" \
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
grite issue comment <ISSUE_ID> --body "Addressed TTL and cache miss handling. Will look into dashmap."
```

### Onboarding Task Lists

Create templated onboarding checklists for new team members.

```bash
# Team lead creates onboarding issue for new developer
grite issue create --title "Onboarding: Alice" \
  --body "$(cat <<'EOF'
## Week 1
- [ ] Set up development environment (see docs/setup.md)
- [ ] Complete codebase walkthrough with mentor
- [ ] Fix a "good first issue" bug
- [ ] Set up grite actor: `grite actor init --label "alice-laptop"`

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

### Knowledge Base / Architectural Decision Records

Record architectural decisions with rationale that persists with the codebase.

```bash
grite issue create --title "ADR-001: Use PostgreSQL for primary database" \
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

### Large Refactoring Coordination

Track progress on refactoring efforts that span multiple files and team members.

```bash
# Create tracking issue for refactoring
grite issue create --title "Refactor: Migrate from callbacks to async/await" \
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
- Use `grite lock acquire --resource "path:src/<module>"` before starting
EOF
)" --label "refactor" --label "epic"
```

### Code Ownership Documentation

Track who owns/maintains which areas of the codebase.

```bash
grite issue create --title "[Ownership] Code ownership map" \
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

---

## Security & Compliance

Grite's git-backed storage keeps sensitive data within repository access controls.

### Private Vulnerability Tracking

Track security issues privately before public disclosure.

```bash
# Security researcher finds vulnerability
grite issue create --title "[SECURITY] SQL injection in search endpoint" \
  --body "$(cat <<'EOF'
## Severity
HIGH

## Description
User input in /api/search is not properly sanitized, allowing SQL injection.

## Reproduction
1. Navigate to search
2. Enter: `'; DROP TABLE users; --`
3. Observe error revealing SQL structure

## Affected Versions
- v1.2.0 through v1.4.2

## Mitigation
Use parameterized queries in src/api/search.rs

## Timeline
- Discovered: 2024-01-15
- Fix target: 2024-01-17
- Disclosure: 2024-01-22 (after patch release)
EOF
)" --label "security" --label "severity:high"
```

### Incident Timeline & Response

Document incident response with timestamps for post-mortems.

```bash
grite issue create --title "Incident: Database outage 2024-01-20" \
  --body "$(cat <<'EOF'
## Summary
Production database unavailable for 47 minutes.

## Timeline (UTC)
- 14:23 - First alerts triggered
- 14:25 - On-call acknowledged
- 14:32 - Root cause identified: disk full
- 14:45 - Emergency disk expansion initiated
- 14:58 - Database recovered
- 15:10 - All services healthy

## Root Cause
Log files not rotating properly, filled /var partition.

## Action Items
- [ ] Fix log rotation config
- [ ] Add disk space monitoring
- [ ] Create runbook for disk issues

## Impact
- ~500 failed API requests
- No data loss
EOF
)" --label "incident" --label "postmortem"
```

### Security Audit Checklists

Track security review items for each component.

```bash
grite issue create --title "Security audit: Authentication module" \
  --body "$(cat <<'EOF'
## OWASP Top 10 Checklist

### A01 Broken Access Control
- [x] Verify authorization on all endpoints
- [x] Test for IDOR vulnerabilities
- [ ] Review role-based access control

### A02 Cryptographic Failures
- [x] Verify TLS configuration
- [x] Check password hashing (using bcrypt)
- [x] Review secret management

### A03 Injection
- [x] SQL injection testing
- [x] Command injection testing
- [x] LDAP injection (N/A)

### A07 Authentication Failures
- [x] Brute force protection
- [x] Session management
- [ ] MFA implementation review
EOF
)" --label "security-audit" --label "auth"
```

### Audit Trail Preservation

The append-only event log creates an immutable audit trail.

```bash
# Verify audit trail integrity
grite db check --verify-parents --json

# Export audit log for compliance
grite export --format json > audit-$(date +%Y%m%d).json

# Verify signatures if enabled
grite db verify --verbose --json
```

---

## DevOps & Release Engineering

Grite integrates with CI/CD workflows and release processes.

### CI Failure Tracking

Track CI failures and their resolution status.

```bash
# CI system creates issue on failure
grite issue create --title "CI Failure: test_integration_auth [build #1234]" \
  --body "$(cat <<'EOF'
## Build Info
- Build: #1234
- Branch: feature/oauth
- Commit: abc123

## Failure
```
test_integration_auth::test_login_success FAILED
thread panicked at 'assertion failed: response.status == 200'
```

## Logs
[Link to full logs]
EOF
)" --label "ci-failure" --label "test"

# Developer investigates and resolves
grite issue comment <ISSUE_ID> --body "Flaky test - added retry logic"
grite issue close <ISSUE_ID>
```

### Release Checklist Management

Track release checklist items that sync across the team.

```bash
grite issue create --title "Release v2.0.0 checklist" \
  --body "$(cat <<'EOF'
## Pre-release
- [ ] All PRs merged
- [ ] Version bumped in Cargo.toml
- [ ] CHANGELOG.md updated
- [ ] Release notes drafted
- [ ] Security scan passed

## Release
- [ ] Tag created: v2.0.0
- [ ] Binaries built
- [ ] Docker images pushed
- [ ] GitHub release published

## Post-release
- [ ] Documentation site updated
- [ ] Announcement posted
- [ ] Monitor error rates for 24h
- [ ] Close release milestone
EOF
)" --label "release" --label "v2.0.0"
```

### Deployment Coordination

Track deployment readiness across environments.

```bash
grite issue create --title "Deploy v2.0.0 to production" \
  --body "$(cat <<'EOF'
## Deployment Plan

### Stage 1: Staging
- [x] Deploy to staging
- [x] Run smoke tests
- [x] QA sign-off

### Stage 2: Canary
- [ ] Deploy to 5% of production
- [ ] Monitor metrics for 1h
- [ ] Verify no error spike

### Stage 3: Full Rollout
- [ ] Deploy to remaining 95%
- [ ] Monitor metrics for 4h
- [ ] Update status page

## Rollback Plan
If error rate >1%: `kubectl rollout undo deployment/api`

## Contacts
- Release lead: @alice
- On-call: @bob
EOF
)" --label "deployment" --label "production"
```

### Breaking Change Tracking

Track breaking changes and migration status across consumers.

```bash
grite issue create --title "Breaking change: API v1 deprecation" \
  --body "$(cat <<'EOF'
## Change
API v1 endpoints will be removed in v3.0.0.

## Migration Guide
Replace `/api/v1/*` with `/api/v2/*`. See docs/migration-v2.md.

## Consumer Status
| Consumer | Status | Contact |
|----------|--------|---------|
| Mobile app | Migrated | @mobile-team |
| Partner A | In progress | partner-a@example.com |
| Partner B | Not started | partner-b@example.com |
| Internal dashboard | Migrated | @frontend |

## Timeline
- Deprecation notice: 2024-01-01
- Final warning: 2024-03-01
- Removal: 2024-04-01
EOF
)" --label "breaking-change" --label "api"
```

### Feature Flag Coordination

Track feature flag states and ownership.

```bash
grite issue create --title "[Feature Flags] Active flags inventory" \
  --body "$(cat <<'EOF'
## Active Feature Flags

| Flag | Purpose | Owner | Status |
|------|---------|-------|--------|
| `new_checkout_flow` | Redesigned checkout | @alice | 50% rollout |
| `dark_mode` | UI dark theme | @bob | 100% (ready to remove) |
| `experimental_search` | New search algorithm | @carol | Internal only |
| `rate_limit_v2` | New rate limiting | @dave | 10% rollout |

## Cleanup Queue
Flags at 100% for >30 days should be removed:
- [ ] `dark_mode` - remove flag, keep feature
- [ ] `new_login_page` - remove flag, keep feature
EOF
)" --label "feature-flags" --label "inventory"
```
