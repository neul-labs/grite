# Quick Start

This guide walks you through using grite for the first time. In about 5 minutes, you'll create issues, add comments, and sync with a remote.

## Initialize Grite

Navigate to any git repository and initialize grite:

```bash
cd your-repo
grite init
```

This creates:

- `.git/grite/` directory for local state
- An actor identity for your device
- `AGENTS.md` file with instructions for AI coding agents

!!! note "AGENTS.md"
    The `AGENTS.md` file helps AI coding agents discover and use grite as the canonical task system. Use `--no-agents-md` to skip creating this file.

## Create an Issue

Create your first issue:

```bash
grite issue create --title "Fix login bug" --body "Users can't login with email"
```

You'll see output like:

```
Created issue 8057324b1e03afd613d4b428fdee657a
```

### Add Labels

Add labels when creating:

```bash
grite issue create --title "Add dark mode" \
  --body "Implement dark theme toggle" \
  --label "feature" --label "ui"
```

## List Issues

View all open issues:

```bash
grite issue list
```

Output:

```
8057324b  open   Fix login bug
a1b2c3d4  open   Add dark mode  [feature, ui]
```

### Filter Issues

Filter by state or label:

```bash
# Only open issues
grite issue list --state open

# Issues with a specific label
grite issue list --label bug

# Combine filters
grite issue list --state open --label feature
```

## View Issue Details

Show full details for an issue:

```bash
grite issue show 8057324b
```

You can use the short ID prefix as long as it's unique.

## Add Comments

Add a comment to track progress:

```bash
grite issue comment 8057324b --body "Investigating - looks like a session timeout issue"
```

## Update an Issue

Change the title or body:

```bash
grite issue update 8057324b --title "Fix login session timeout bug"
```

## Manage Labels

Add or remove labels:

```bash
# Add a label
grite issue label add 8057324b --label "bug"

# Remove a label
grite issue label remove 8057324b --label "bug"
```

## Close an Issue

Mark an issue as closed:

```bash
grite issue close 8057324b
```

Reopen if needed:

```bash
grite issue reopen 8057324b
```

## Sync with Remote

Push your issues to the remote repository:

```bash
grite sync
```

This:

1. Pulls any new events from the remote
2. Pushes your local events to the remote
3. Auto-rebases if there are conflicts

### Pull or Push Only

```bash
# Only pull from remote
grite sync --pull

# Only push to remote
grite sync --push
```

## Run Health Checks

Verify your grite installation is healthy:

```bash
grite doctor
```

If issues are found, auto-fix them:

```bash
grite doctor --fix
```

## JSON Output

All commands support `--json` for scripting:

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
        "comment_count": 1
      }
    ],
    "total": 1
  }
}
```

## Command Summary

| Task | Command |
|------|---------|
| Initialize | `grite init` |
| Create issue | `grite issue create --title "..." --body "..."` |
| List issues | `grite issue list` |
| Show issue | `grite issue show <id>` |
| Add comment | `grite issue comment <id> --body "..."` |
| Close issue | `grite issue close <id>` |
| Sync | `grite sync` |
| Health check | `grite doctor` |

## Next Steps

- [Core Concepts](concepts.md) - Understand how grite works under the hood
- [Working with Issues](../guides/issues.md) - Complete guide to issue management
- [CLI Reference](../reference/cli.md) - Full command documentation
