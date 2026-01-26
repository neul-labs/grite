# Security & Compliance

Grite's git-backed storage keeps sensitive data within repository access controls, making it suitable for security-sensitive tracking.

## Why Grite for Security?

- **Access control**: Issues follow git permissions
- **Audit trail**: Append-only event log
- **No external exposure**: Data stays in the repo
- **Signatures**: Optional Ed25519 signing
- **Offline capable**: Incident response without network

## Private Vulnerability Tracking

Track security issues privately before public disclosure:

```bash
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

### Vulnerability Labels

| Label | Use |
|-------|-----|
| `security` | All security issues |
| `severity:critical` | Immediate action needed |
| `severity:high` | Fix within days |
| `severity:medium` | Fix within weeks |
| `severity:low` | Fix when convenient |
| `cve:pending` | CVE requested |
| `cve:CVE-2024-XXXX` | CVE assigned |

## Incident Timeline & Response

Document incident response with timestamps for post-mortems:

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

### Incident Workflow

1. Create incident issue immediately
2. Update timeline as events unfold
3. Document root cause after resolution
4. Track action items to completion
5. Close after all items done

## Security Audit Checklists

Track security review items for each component:

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

### Audit Templates

Create reusable audit templates:

```bash
# Web application audit
grite issue create --title "Security audit: [Component]" \
  --body "$(cat audit-template-web.md)" \
  --label "security-audit"

# API audit
grite issue create --title "API Security audit: [Service]" \
  --body "$(cat audit-template-api.md)" \
  --label "security-audit" --label "api"
```

## Audit Trail Preservation

The append-only event log creates an immutable audit trail:

```bash
# Verify audit trail integrity
grite doctor --json | jq '.data.checks[] | select(.id == "store_integrity")'

# Export audit log for compliance
grite export --format json
cp .grite/export.json "audit-$(date +%Y%m%d).json"
```

### Event Signing

Enable signatures for authenticity:

```bash
# Create actor with signing key
grite actor init --label "security-team" --generate-key

# Events are now signed automatically
grite issue create --title "[SECURITY] ..." --label "security"

# Verify signatures
grite db verify --verbose --json
```

## Compliance Tracking

Track compliance requirements:

```bash
grite issue create --title "SOC 2 Type II - 2024" \
  --body "$(cat <<'EOF'
## Requirements

### Security
- [x] Access control policies documented
- [x] Encryption at rest implemented
- [ ] Annual penetration test scheduled

### Availability
- [x] Uptime SLA defined (99.9%)
- [x] Disaster recovery plan documented
- [ ] DR test scheduled

### Confidentiality
- [x] Data classification policy
- [x] PII handling procedures
- [x] Vendor security assessments

## Evidence
- Access control: docs/security/access-control.md
- Encryption: infra/terraform/rds.tf (storage_encrypted = true)
- DR plan: docs/disaster-recovery.md

## Audit Date
Scheduled: 2024-03-15
EOF
)" --label "compliance" --label "soc2"
```

## Penetration Testing

Track pen test findings:

```bash
grite issue create --title "Pentest 2024-Q1: Findings" \
  --body "$(cat <<'EOF'
## Summary
External penetration test by SecureCo, January 2024.

## Critical Findings
None

## High Findings
1. [FIXED] Session tokens not rotated after password change
   - Issue: security-001
   - Fixed: commit abc123

## Medium Findings
1. [IN PROGRESS] Missing rate limiting on login
   - Issue: security-002

2. [TODO] CORS too permissive
   - Issue: security-003

## Low Findings
1. [ACCEPTED] Server version disclosed in headers
   - Risk accepted per security policy

## Report
Attached: pentest-2024-q1.pdf (encrypted)
EOF
)" --label "pentest" --label "2024-Q1"
```

## Security Runbooks

Document incident response procedures:

```bash
grite issue create --title "[Runbook] Suspected data breach" \
  --body "$(cat <<'EOF'
## Immediate Actions (First 15 minutes)
1. [ ] Confirm breach via logs
2. [ ] Page security team lead
3. [ ] Preserve evidence (don't restart services)
4. [ ] Begin incident timeline

## Investigation (First hour)
1. [ ] Identify affected systems
2. [ ] Determine data accessed
3. [ ] Identify attack vector
4. [ ] Check for persistence mechanisms

## Containment
1. [ ] Isolate affected systems
2. [ ] Rotate compromised credentials
3. [ ] Block attacker IPs
4. [ ] Enable enhanced logging

## Communication
1. [ ] Internal notification (security@company.com)
2. [ ] Legal notification (if PII involved)
3. [ ] Customer notification (if required)

## Recovery
1. [ ] Patch vulnerability
2. [ ] Restore from clean backup if needed
3. [ ] Gradual service restoration
4. [ ] Enhanced monitoring period

## Post-Incident
1. [ ] Complete incident report
2. [ ] Update runbooks
3. [ ] Schedule lessons learned
EOF
)" --label "runbook" --label "security"
```

## Best Practices

### Restrict Access

Security issues should be in a repo with restricted access:

- Use a private repository
- Limit team access to security personnel
- Sync only with secure remotes

### Use Signatures

Enable event signing for security-critical tracking:

```bash
grite actor init --label "security-auditor" --generate-key
```

### Regular Exports

Create periodic backups of security issues:

```bash
# Weekly backup
grite export --format json
gpg --encrypt --recipient security@company.com .grite/export.json
mv .grite/export.json.gpg "backups/security-$(date +%Y%m%d).json.gpg"
```

### Timely Updates

Update issues as situations evolve:

```bash
grite issue comment $INCIDENT_ID --body "$(date -u +%H:%M) - Update: ..."
grite sync --push
```

## Next Steps

- [Actor Identity](../guides/actors.md) - Signing key setup
- [Exporting Data](../guides/export.md) - Audit export
- [Configuration](../reference/configuration.md) - Security settings
