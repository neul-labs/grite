/// Template content for the Grit section in AGENTS.md
pub const GRIT_AGENTS_SECTION: &str = r#"## Grit

This repository uses **Grit** as the canonical task and memory system. **Always use grit commands** (not git) for task/issue tracking.

### When to use grit

- **"What tasks are done/open?"** → `grit issue list`
- **"What did I work on?"** → `grit issue list --state closed`
- **"What do I know about X?"** → `grit issue list --label memory`
- **Starting a new task** → `grit issue create`
- **Making progress** → `grit issue comment`

### Startup routine

Run at the beginning of each session:

```bash
grit sync --pull --json
grit issue list --json
```

### Creating tasks/memories

```bash
# Create a task
grit issue create --title "Task title" --body "Description" --label agent:todo --json

# Store a discovery as memory
grit issue create --title "[Memory] Topic" --body "What you learned..." --label memory --json
```

### Working on issues

```bash
# Add a comment with your plan before coding
grit issue comment <ID> --body "Plan: ..." --json

# Add checkpoint comments after milestones
grit issue comment <ID> --body "Checkpoint: what changed, why, tests run" --json

# Close when done
grit issue close <ID> --json
grit sync --push --json
```

### Querying tasks

```bash
# All issues
grit issue list --json

# Open tasks
grit issue list --state open --json

# Completed tasks
grit issue list --state closed --json

# Memories
grit issue list --label memory --json
```

### Key flags

- `--json` - Use for all commands (machine-readable output)
- `--quiet` - Suppress human output
"#;
