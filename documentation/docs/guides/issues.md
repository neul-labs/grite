# Working with Issues

This guide covers everything you need to know about creating and managing issues in grite.

## Creating Issues

### Basic Creation

Create an issue with a title and body:

```bash
grite issue create --title "Fix login bug" --body "Users can't login with email addresses"
```

Output:

```
Created issue 8057324b1e03afd613d4b428fdee657a
```

### With Labels

Add labels when creating:

```bash
grite issue create \
  --title "Add dark mode" \
  --body "Implement dark theme toggle in settings" \
  --label "feature" \
  --label "ui"
```

### JSON Output

Get structured output for scripting:

```bash
grite issue create --title "..." --body "..." --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "issue_id": "8057324b1e03afd613d4b428fdee657a",
    "event_id": "a1b2c3d4...",
    "wal_head": "abc123..."
  }
}
```

## Listing Issues

### All Issues

```bash
grite issue list
```

Output:

```
8057324b  open   Fix login bug
a1b2c3d4  open   Add dark mode  [feature, ui]
c5d6e7f8  closed Refactor auth  [tech-debt]
```

### Filter by State

```bash
# Open issues only
grite issue list --state open

# Closed issues only
grite issue list --state closed
```

### Filter by Label

```bash
# Issues with a specific label
grite issue list --label bug

# Multiple labels (AND logic)
grite issue list --label bug --label priority:P1
```

### Combined Filters

```bash
grite issue list --state open --label feature
```

### JSON Output

```bash
grite issue list --json
```

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "issues": [
      {
        "issue_id": "8057324b1e03afd613d4b428fdee657a",
        "title": "Fix login bug",
        "state": "open",
        "labels": [],
        "assignees": [],
        "updated_ts": 1700000000000,
        "comment_count": 0
      }
    ],
    "total": 1
  }
}
```

## Viewing Issues

### Show Details

```bash
grite issue show 8057324b
```

You can use a short ID prefix as long as it's unique.

### With Full Events

```bash
grite issue show 8057324b --json
```

Returns the issue summary plus the complete event history.

## Updating Issues

### Change Title

```bash
grite issue update 8057324b --title "Fix login session timeout bug"
```

### Change Body

```bash
grite issue update 8057324b --body "Updated description with more details"
```

### Change Both

```bash
grite issue update 8057324b \
  --title "New title" \
  --body "New description"
```

## Adding Comments

### Basic Comment

```bash
grite issue comment 8057324b --body "Started investigating this issue"
```

### Multi-line Comments

Use heredocs for longer comments:

```bash
grite issue comment 8057324b --body "$(cat <<'EOF'
## Investigation Notes

Found the root cause:
- Session timeout is set too low
- Need to increase from 30s to 5m

## Next Steps
1. Update config
2. Add tests
EOF
)"
```

## Managing Labels

### Add Labels

```bash
grite issue label add 8057324b --label "bug"
grite issue label add 8057324b --label "priority:P1"
```

### Remove Labels

```bash
grite issue label remove 8057324b --label "bug"
```

### Label Conventions

Common label patterns:

| Pattern | Example | Use |
|---------|---------|-----|
| Type | `bug`, `feature`, `tech-debt` | Issue type |
| Priority | `priority:P0`, `priority:P1` | Urgency |
| Component | `component:auth`, `component:api` | Code area |
| Status | `blocked`, `in-progress` | Workflow state |

## Managing Assignees

### Add Assignee

```bash
grite issue assignee add 8057324b --user "alice"
```

### Remove Assignee

```bash
grite issue assignee remove 8057324b --user "alice"
```

## Adding Links

Attach URLs to issues:

```bash
grite issue link add 8057324b \
  --url "https://github.com/org/repo/pull/123" \
  --note "Related PR"
```

## Adding Attachments

Track file attachments by their content hash:

```bash
grite issue attachment add 8057324b \
  --name "screenshot.png" \
  --sha256 "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" \
  --mime "image/png"
```

!!! note
    Grite stores only metadata and the content hash. The actual file should be stored elsewhere (e.g., git LFS, cloud storage).

## Closing and Reopening

### Close Issue

```bash
grite issue close 8057324b
```

### Reopen Issue

```bash
grite issue reopen 8057324b
```

## Best Practices

### Use Descriptive Titles

Good:

- "Fix login timeout for email users"
- "Add dark mode toggle to settings page"

Avoid:

- "Bug fix"
- "Feature request"

### Use Consistent Labels

Establish a labeling convention for your project:

```bash
# Types
grite issue create --label "bug" ...
grite issue create --label "feature" ...
grite issue create --label "tech-debt" ...

# Priorities
grite issue create --label "priority:P0" ...  # Critical
grite issue create --label "priority:P1" ...  # High
grite issue create --label "priority:P2" ...  # Medium
```

### Document Progress in Comments

Keep a record of your investigation and progress:

```bash
grite issue comment <id> --body "Investigated - root cause is X"
grite issue comment <id> --body "Fix implemented in commit abc123"
grite issue comment <id> --body "Deployed to staging, testing now"
```

### Use JSON for Automation

Scripts should use `--json` for reliable parsing:

```bash
# Get issue ID from creation
ISSUE_ID=$(grite issue create --title "..." --body "..." --json | jq -r '.data.issue_id')

# Process issue list
grite issue list --json | jq '.data.issues[] | select(.state == "open")'
```

## Next Steps

- [Syncing with Remotes](syncing.md) - Share your issues
- [Distributed Locks](locking.md) - Coordinate work on issues
- [CLI Reference](../reference/cli.md) - Complete command documentation
