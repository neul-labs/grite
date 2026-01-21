# DevOps & Release Engineering

Grit integrates with CI/CD workflows and release processes.

## Why Grit for DevOps?

- **CI-friendly**: Non-interactive CLI with JSON output
- **Git-native**: Fits existing git workflows
- **Traceable**: All changes in append-only log
- **Coordinated**: Distributed locks for deployments

## CI Failure Tracking

Track CI failures and their resolution:

```bash
# CI system creates issue on failure
grit issue create --title "CI Failure: test_integration_auth [build #1234]" \
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
grit issue comment $ISSUE_ID --body "Flaky test - added retry logic"
grit issue close $ISSUE_ID
```

### CI Integration Script

```bash
#!/bin/bash
# ci-failure-reporter.sh

if [ "$CI_STATUS" = "failed" ]; then
  grit issue create \
    --title "CI Failure: $CI_JOB_NAME [build #$CI_BUILD_NUMBER]" \
    --body "Build: #$CI_BUILD_NUMBER
Branch: $CI_BRANCH
Commit: $CI_COMMIT

Error: $CI_ERROR_MESSAGE" \
    --label "ci-failure" \
    --json

  grit sync --push
fi
```

## Release Checklist Management

Track release checklist items that sync across the team:

```bash
grit issue create --title "Release v2.0.0 checklist" \
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

### Release Workflow

```bash
# Start release
grit issue create --title "Release v2.0.0" --label "release"
grit lock acquire --resource "repo:release" --ttl 2h

# Update as you go
grit issue comment $RELEASE_ID --body "Tag created"
grit issue comment $RELEASE_ID --body "Binaries uploaded"

# Complete release
grit issue close $RELEASE_ID
grit lock release --resource "repo:release"
grit sync --push
```

## Deployment Coordination

Track deployment readiness across environments:

```bash
grit issue create --title "Deploy v2.0.0 to production" \
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

### Deployment Lock

Prevent concurrent deployments:

```bash
# Acquire deployment lock
if ! grit lock acquire --resource "deploy:production" --ttl 1h; then
  echo "Deployment already in progress"
  exit 1
fi

# Deploy
./deploy.sh

# Release lock
grit lock release --resource "deploy:production"
```

## Breaking Change Tracking

Track breaking changes and migration status:

```bash
grit issue create --title "Breaking change: API v1 deprecation" \
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

## Feature Flag Coordination

Track feature flag states and ownership:

```bash
grit issue create --title "[Feature Flags] Active flags inventory" \
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

## Infrastructure Tracking

Track infrastructure changes:

```bash
grit issue create --title "Infra: Upgrade Kubernetes to 1.28" \
  --body "$(cat <<'EOF'
## Reason
Security patches and new features needed.

## Plan
1. [ ] Update staging cluster
2. [ ] Run integration tests
3. [ ] Update production cluster (maintenance window)

## Rollback
kubectl can downgrade within 24h window.

## Dependencies
- [ ] Update kubectl locally
- [ ] Update CI runners
- [ ] Update helm charts
EOF
)" --label "infrastructure" --label "kubernetes"
```

## On-Call Handoff

Document on-call status:

```bash
grit issue create --title "On-call handoff: Week of 2024-01-15" \
  --body "$(cat <<'EOF'
## Outgoing: @alice
## Incoming: @bob

## Active Issues
1. Elevated error rate on /api/search - monitoring
2. Database slow queries - investigating

## Recent Incidents
- 2024-01-14: Brief API outage, resolved (see incident-123)

## Upcoming Maintenance
- 2024-01-17 02:00 UTC: Database maintenance window

## Notes
- Deploy freeze until 2024-01-18
- New runbook for auth issues: docs/runbooks/auth.md
EOF
)" --label "oncall" --label "handoff"
```

## Monitoring Alerts

Track alert status:

```bash
grit issue create --title "Alert: High CPU on prod-api-3" \
  --body "$(cat <<'EOF'
## Alert Details
- Host: prod-api-3
- Metric: CPU > 90% for 5m
- Started: 2024-01-15 14:30 UTC

## Investigation
- Checked recent deploys: none
- Checked traffic: normal
- Found: Runaway process in container

## Resolution
Restarted container, CPU normalized.

## Follow-up
- [ ] Add memory limits to container spec
- [ ] Create alert for container restarts
EOF
)" --label "alert" --label "resolved"
```

## CI/CD Pipeline Integration

### GitHub Actions Example

```yaml
- name: Report CI Failure
  if: failure()
  run: |
    grit issue create \
      --title "CI Failure: ${{ github.job }} [#${{ github.run_number }}]" \
      --body "Workflow: ${{ github.workflow }}
    Branch: ${{ github.ref_name }}
    Commit: ${{ github.sha }}
    Run: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}" \
      --label "ci-failure" \
      --json
    grit sync --push
```

### GitLab CI Example

```yaml
report_failure:
  stage: .post
  when: on_failure
  script:
    - grit issue create --title "CI Failure: $CI_JOB_NAME" --label "ci-failure"
    - grit sync --push
```

## Best Practices

### Use Locks for Critical Operations

```bash
# Deployment
grit lock acquire --resource "deploy:$ENV" --ttl 1h

# Database migrations
grit lock acquire --resource "migration:$DB" --ttl 30m

# Release creation
grit lock acquire --resource "repo:release" --ttl 2h
```

### Automate Issue Creation

Integrate with your CI/CD to create issues automatically:

- On build failure
- On deployment start/complete
- On alert firing

### Keep Issues Updated

```bash
# Deployment progress
grit issue comment $DEPLOY_ID --body "Canary at 5%"
grit issue comment $DEPLOY_ID --body "Canary healthy, proceeding to 100%"
grit issue comment $DEPLOY_ID --body "Deployment complete"
grit issue close $DEPLOY_ID
```

### Archive Old Issues

```bash
# Monthly cleanup
grit issue list --state closed --label "ci-failure" | \
  xargs -I {} grit issue label add {} --label "archived"
```

## Next Steps

- [Distributed Locks](../guides/locking.md) - Deployment coordination
- [CLI Reference](../reference/cli.md) - CI automation
- [JSON Output](../reference/cli-json.md) - Parsing in scripts
