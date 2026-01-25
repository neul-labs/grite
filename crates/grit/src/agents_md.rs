/// Template content for the Grit section in AGENTS.md
pub const GRIT_AGENTS_SECTION: &str = r#"## Grit

This repository uses **Grit** as the canonical task and memory system. **Always use grit commands** (not git) for task/issue tracking.

### When to use grit

- **"What tasks are done/open?"** → `grit issue list`
- **"What did I work on?"** → `grit issue list --state closed`
- **"What do I know about X?"** → `grit issue list --label memory`
- **"What should I work on next?"** → `grit issue dep topo --state open`
- **"What does function X do?"** → `grit context query X`
- **Starting a new task** → `grit issue create`
- **Making progress** → `grit issue comment`
- **Task A depends on B** → `grit issue dep add`

### Startup routine

Run at the beginning of each session:

```bash
grit sync --pull --json
grit issue list --json
grit issue dep topo --state open --json
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

### Dependencies

Track task ordering and blockers with a dependency DAG:

```bash
# Task A depends on Task B (B must complete first)
grit issue dep add <A> --target <B> --type depends_on --json

# Task A blocks Task B (A must complete first)
grit issue dep add <A> --target <B> --type blocks --json

# Mark tasks as related (no ordering constraint)
grit issue dep add <A> --target <B> --type related_to --json

# List what an issue depends on
grit issue dep list <ID> --json

# List what depends on an issue (reverse)
grit issue dep list <ID> --reverse --json

# Get execution order (topological sort of open tasks)
grit issue dep topo --state open --json
```

**Always run `dep topo`** at session start to determine which task to work on next.

### Context store

Index and query codebase structure for fast navigation:

```bash
# Index all tracked files (skips unchanged files)
grit context index --json

# Index specific files or patterns
grit context index --path src/main.py --json
grit context index --pattern "*.rs" --json

# Query for a symbol (function, class, struct, etc.)
grit context query <symbol_name> --json

# Show all symbols in a file
grit context show <file_path> --json

# Set project-level context (conventions, architecture notes)
grit context set <key> <value> --json

# View all project context
grit context project --json
```

**Index after significant code changes** to keep the symbol database current.

### Key flags

- `--json` - Use for all commands (machine-readable output)
- `--quiet` - Suppress human output
"#;
