# Use Cases

Grite serves different audiences with distinct workflows. This section provides detailed use cases and practical examples for each.

## Audiences

| Audience | Primary Use Cases |
|----------|-------------------|
| [AI Coding Agents](ai-agents.md) | Task decomposition, multi-agent coordination, persistent memory |
| [Individual Developers](developers.md) | Offline issue tracking, personal task lists, technical debt |
| [Development Teams](teams.md) | Distributed coordination, code review workflows, knowledge base |
| [Security & Compliance](security.md) | Private vulnerability tracking, incident response, audit trails |
| [DevOps & Releases](devops.md) | CI/CD integration, release checklists, deployment tracking |

## Why Grite?

### For All Audiences

- **Git-native**: Issues travel with your repository
- **Offline-first**: Works without network connectivity
- **No external services**: Self-contained within the repo
- **Deterministic**: CRDT merging means no conflicts

### For AI Agents

- **Non-interactive**: CLI designed for automation
- **JSON output**: Structured data for parsing
- **Distributed locks**: Coordinate multi-agent work
- **Persistent memory**: Issues survive across sessions

### For Developers

- **Private tracking**: Personal task lists and tech debt
- **Offline work**: Create issues on planes, sync later
- **Lightweight**: No accounts, no setup, just `grite init`

### For Teams

- **Decentralized**: No central server required
- **Sync via git**: Works with existing git workflows
- **Knowledge base**: ADRs and documentation in issues

### For Security

- **Access control**: Issues follow git permissions
- **Audit trail**: Append-only event log
- **Signatures**: Optional Ed25519 signing

## Quick Examples

### AI Agent Task Tracking

```bash
# Decompose task into subtasks
grite issue create --title "Implement auth" --label "epic"
grite issue create --title "Create user schema" --label "subtask"
grite issue create --title "Implement login endpoint" --label "subtask"

# Claim a task
grite lock acquire --resource "issue:$ID" --ttl 30m

# Work and report progress
grite issue comment $ID --body "Started implementation"
```

### Developer Private List

```bash
# Track personal tech debt
grite issue create --title "Refactor API layer" --label "tech-debt"
grite issue create --title "Add missing tests" --label "tech-debt"

# Review your list
grite issue list --label "tech-debt"
```

### Team Coordination

```bash
# Create release checklist
grite issue create --title "Release v2.0.0" --label "release"
grite sync --push

# Team member pulls and contributes
grite sync --pull
grite issue comment $ID --body "Tests passing"
grite sync --push
```

### Security Tracking

```bash
# Private vulnerability
grite issue create --title "[SECURITY] SQL injection" --label "security"

# Track remediation
grite issue comment $ID --body "Fixed in commit abc123"
grite issue close $ID
```

### DevOps Checklist

```bash
# Deployment checklist
grite issue create --title "Deploy to prod" --label "deploy"

# CI creates status issue
grite issue create --title "Build #1234 failed" --label "ci-failure"
```

## Choose Your Path

Select the use case that matches your needs:

- **[AI Agents](ai-agents.md)** - Building autonomous coding agents
- **[Developers](developers.md)** - Personal productivity and tracking
- **[Teams](teams.md)** - Collaborative workflows
- **[Security](security.md)** - Vulnerability and compliance management
- **[DevOps](devops.md)** - CI/CD and release engineering
