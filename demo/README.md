# Grit + Claude Code Demo

This demo shows how Grit provides persistent memory for AI coding agents like Claude Code.

## Quick Start

```bash
# Build grit first (if not already built)
cargo build

# Run the interactive demo
./demo.sh

# Or run the automated demo (no pauses)
./demo.sh --auto
```

## What the Demo Shows

### 1. Project Setup
- Creates a sample Python CLI project in `/tmp/grit-demo`
- Initializes git and grit
- Shows how `grit init` automatically creates `AGENTS.md`

### 2. Agent Discovery via AGENTS.md
- `AGENTS.md` is the convention that AI coding agents read
- Contains instructions for using grit as the task/memory system
- Claude Code reads this automatically when entering the repo

### 3. Task Creation
- Shows creating issues with `grit issue create`
- Uses `--label agent:todo` for agent-specific tasks
- JSON output for machine consumption

### 4. Working with Checkpoints
- Post plans before starting work
- Add checkpoint comments after milestones
- Track progress within issues

### 5. Memory Persistence
- Store learnings with `--label memory`
- Memories survive across sessions
- Query memories with `grit issue list --label memory`

### 6. Session Resume
- New sessions run startup routine from AGENTS.md
- `grit sync --pull` retrieves latest state
- `grit issue list` shows open tasks and memories

## Demo Modes

| Mode | Command | Description |
|------|---------|-------------|
| Interactive | `./demo.sh` | Step-by-step with pauses |
| Automated | `./demo.sh --auto` | Runs all steps without pauses |

## After the Demo

Try using Claude Code with the demo project:

```bash
cd /tmp/grit-demo
claude
```

Claude Code will:
1. Read `AGENTS.md` automatically
2. Run the startup routine (sync, list tasks)
3. Use grit for any new tasks you assign
4. Store memories about the codebase

## Requirements

- `jq` - JSON processor (for parsing grit output)
- `grit` - Built from this repository

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `GRIT_BIN` | `./target/debug/grit` | Path to grit binary |

## Troubleshooting

**"grit binary not found"**
```bash
cargo build
```

**"jq is required"**
```bash
# Ubuntu/Debian
sudo apt install jq

# macOS
brew install jq
```
