use std::fs;
use std::path::PathBuf;

use crate::cli::Cli;
use crate::context::GriteContext;
use crate::output::{output_success, print_human};
use libgrite_core::GriteError;
use serde::Serialize;

#[derive(Serialize)]
struct InstallSkillOutput {
    path: String,
    action: String,
}

/// Skill file content
const SKILL_CONTENT: &str = r#"---
name: grite
description: Git-backed issue tracker with CRDT merging, designed for AI coding agents
version: 0.3.0
---

# Grite Skill

This repository uses **Grite** as the canonical task and memory system. **Always use grite commands** (not git) for task/issue tracking.

## When to use grite

- "What tasks are done/open?" -> `grite issue list`
- "What did I work on?" -> `grite issue list --state closed`
- "What do I know about X?" -> `grite issue list --label memory`
- "What should I work on next?" -> `grite issue dep topo --state open`
- "What does function X do?" -> `grite context query X`
- Starting a new task -> `grite issue create`
- Making progress -> `grite issue comment`
- Task A depends on B -> `grite issue dep add`

## Startup routine

Run at the beginning of each session:

```bash
grite sync --pull --json
grite issue list --json
grite issue dep topo --state open --json
```

## Creating tasks/memories

```bash
# Create a task
grite issue create --title "Task title" --body "Description" --label agent:todo --json

# Store a discovery as memory
grite issue create --title "[Memory] Topic" --body "What you learned..." --label memory --json
```

## Working on issues

```bash
# Add a comment with your plan before coding
grite issue comment <ID> --body "Plan: ..." --json

# Add checkpoint comments after milestones
grite issue comment <ID> --body "Checkpoint: what changed, why, tests run" --json

# Close when done
grite issue close <ID> --json
grite sync --push --json
```

## Querying tasks

```bash
# All issues
grite issue list --json

# Open tasks
grite issue list --state open --json

# Completed tasks
grite issue list --state closed --json

# Memories
grite issue list --label memory --json
```

## Dependencies

Track task ordering and blockers with a dependency DAG:

```bash
# Task A depends on Task B (B must complete first)
grite issue dep add <A> --target <B> --type depends_on --json

# Task A blocks Task B (A must complete first)
grite issue dep add <A> --target <B> --type blocks --json

# Mark tasks as related (no ordering constraint)
grite issue dep add <A> --target <B> --type related_to --json

# List what an issue depends on
grite issue dep list <ID> --json

# List what depends on an issue (reverse)
grite issue dep list <ID> --reverse --json

# Get execution order (topological sort of open tasks)
grite issue dep topo --state open --json
```

**Always run `dep topo`** at session start to determine which task to work on next.

## Context store

Index and query codebase structure for fast navigation:

```bash
# Index all tracked files (skips unchanged files)
grite context index --json

# Index specific files or patterns
grite context index --path src/main.py --json
grite context index --pattern "*.rs" --json

# Query for a symbol (function, class, struct, etc.)
grite context query <symbol_name> --json

# Show all symbols in a file
grite context show <file_path> --json

# Set project-level context (conventions, architecture notes)
grite context set <key> <value> --json

# View all project context
grite context project --json
```

**Index after significant code changes** to keep the symbol database current.

## Key flags

- `--json` - Use for all commands (machine-readable output)
- `--quiet` - Suppress human output
"#;

pub fn run(cli: &Cli, global: bool, force: bool) -> Result<(), GriteError> {
    let target_dir = if global {
        let home = std::env::var("HOME")
            .map_err(|_| GriteError::Internal("HOME environment variable not set".to_string()))?;
        PathBuf::from(home).join(".claude").join("skills")
    } else {
        let git_dir = GriteContext::find_git_dir()?;
        let repo_root = git_dir.parent().ok_or_else(|| {
            GriteError::Internal("Could not determine repository root".to_string())
        })?;
        repo_root.join(".claude").join("skills")
    };

    fs::create_dir_all(&target_dir)
        .map_err(|e| GriteError::Internal(format!("Failed to create skills directory: {}", e)))?;

    let skill_path = target_dir.join("grite.md");

    if skill_path.exists() && !force {
        return Err(GriteError::Conflict(format!(
            "Skill already installed at {}. Use --force to overwrite.",
            skill_path.display()
        )));
    }

    let action = if skill_path.exists() {
        "updated"
    } else {
        "created"
    };

    fs::write(&skill_path, SKILL_CONTENT)
        .map_err(|e| GriteError::Internal(format!("Failed to write skill file: {}", e)))?;

    output_success(
        cli,
        InstallSkillOutput {
            path: skill_path.to_string_lossy().to_string(),
            action: action.to_string(),
        },
    );

    print_human(
        cli,
        &format!("{} skill at {}", action, skill_path.display()),
    );

    Ok(())
}
