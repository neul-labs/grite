# Environment Variables

This document describes environment variables that affect grit behavior.

## Variables

### GRIT_HOME

Override the actor data directory.

- **Type**: filesystem path
- **Default**: `.git/grit/actors/<actor_id>/`
- **Scope**: current process

When set, grit uses this directory for all actor data instead of the default location.

```bash
export GRIT_HOME=/custom/path/grit/actor1
grit issue list  # Uses data from /custom/path/grit/actor1
```

#### Use Cases

- Running multiple agents with isolated data
- Testing with a separate database
- Custom data directory location

#### Example: Multiple Agents

```bash
# Agent 1
export GRIT_HOME=/tmp/grit/agent1
grit issue list

# Agent 2
export GRIT_HOME=/tmp/grit/agent2
grit issue list
```

### RUST_LOG

Control logging verbosity for debugging.

- **Type**: log filter string
- **Default**: `info` (when running daemon directly)
- **Scope**: current process

```bash
export RUST_LOG=debug
grit issue list
```

#### Log Levels

| Level | Description |
|-------|-------------|
| `trace` | Very detailed debugging |
| `debug` | Debugging information |
| `info` | General information |
| `warn` | Warnings |
| `error` | Errors only |

#### Module-Specific Logging

```bash
# Debug only for grit crate
export RUST_LOG=grit=debug

# Debug for grit, warn for others
export RUST_LOG=warn,grit=debug,libgrit_core=debug
```

#### Common Debugging Scenarios

```bash
# Debug IPC issues
export RUST_LOG=debug,libgrit_ipc=trace

# Debug git operations
export RUST_LOG=debug,libgrit_git=trace

# Debug database operations
export RUST_LOG=debug,libgrit_core::store=trace
```

## Precedence

### Actor Selection

Actor context is resolved in this order (highest precedence first):

1. **`--data-dir` flag** - Explicit path
2. **`GRIT_HOME` environment variable** - Override default
3. **`--actor` flag** - Specific actor ID
4. **`default_actor` in config** - Repository default
5. **Auto-create** - New actor if none exists

### Examples

```bash
# Highest precedence: --data-dir
grit --data-dir /custom/path issue list

# Environment variable
export GRIT_HOME=/custom/path
grit issue list

# Flag
grit --actor abc123 issue list

# Default from config
grit issue list  # Uses default_actor from .git/grit/config.toml
```

## CI/CD Environments

### GitHub Actions

```yaml
env:
  GRIT_HOME: ${{ runner.temp }}/grit-${{ github.run_id }}

steps:
  - name: Initialize grit
    run: |
      grit actor init --label "ci-${{ github.run_number }}"
```

### GitLab CI

```yaml
variables:
  GRIT_HOME: "${CI_PROJECT_DIR}/.grit-ci-${CI_JOB_ID}"

script:
  - grit actor init --label "ci-${CI_JOB_ID}"
```

### Docker

```dockerfile
ENV GRIT_HOME=/app/.grit

# Or at runtime
docker run -e GRIT_HOME=/app/.grit myimage
```

## Shell Configuration

### Bash/Zsh

Add to `~/.bashrc` or `~/.zshrc`:

```bash
# Custom grit home
export GRIT_HOME="$HOME/.grit/default-actor"

# Debug logging
alias grit-debug='RUST_LOG=debug grit'
```

### Fish

Add to `~/.config/fish/config.fish`:

```fish
set -x GRIT_HOME "$HOME/.grit/default-actor"
```

## Troubleshooting

### Check Current Environment

```bash
# See what grit sees
grit actor current --json | jq

# Check environment
echo $GRIT_HOME
echo $RUST_LOG
```

### Common Issues

#### "Wrong actor being used"

Check actor selection precedence:

```bash
# What's the current actor?
grit actor current --json

# Is GRIT_HOME set?
echo $GRIT_HOME

# What's the default in config?
cat .git/grit/config.toml
```

#### "Missing log output"

Enable logging:

```bash
export RUST_LOG=debug
grit issue list 2>&1 | head -50
```

#### "Daemon not using environment"

The daemon inherits environment from the spawning process. If auto-spawned, it uses the environment at spawn time.

```bash
# Stop daemon and restart with new environment
grit daemon stop
export RUST_LOG=debug
grit daemon start
```

## Next Steps

- [Configuration](configuration.md) - File-based configuration
- [CLI Reference](cli.md) - Command-line options
- [Using the Daemon](../guides/daemon.md) - Daemon behavior
