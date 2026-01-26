# Environment Variables

This document describes environment variables that affect grite behavior.

## Variables

### GRIT_HOME

Override the actor data directory.

- **Type**: filesystem path
- **Default**: `.git/grite/actors/<actor_id>/`
- **Scope**: current process

When set, grite uses this directory for all actor data instead of the default location.

```bash
export GRIT_HOME=/custom/path/grite/actor1
grite issue list  # Uses data from /custom/path/grite/actor1
```

#### Use Cases

- Running multiple agents with isolated data
- Testing with a separate database
- Custom data directory location

#### Example: Multiple Agents

```bash
# Agent 1
export GRIT_HOME=/tmp/grite/agent1
grite issue list

# Agent 2
export GRIT_HOME=/tmp/grite/agent2
grite issue list
```

### RUST_LOG

Control logging verbosity for debugging.

- **Type**: log filter string
- **Default**: `info` (when running daemon directly)
- **Scope**: current process

```bash
export RUST_LOG=debug
grite issue list
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
# Debug only for grite crate
export RUST_LOG=grite=debug

# Debug for grite, warn for others
export RUST_LOG=warn,grite=debug,libgrite_core=debug
```

#### Common Debugging Scenarios

```bash
# Debug IPC issues
export RUST_LOG=debug,libgrite_ipc=trace

# Debug git operations
export RUST_LOG=debug,libgrite_git=trace

# Debug database operations
export RUST_LOG=debug,libgrite_core::store=trace
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
grite --data-dir /custom/path issue list

# Environment variable
export GRIT_HOME=/custom/path
grite issue list

# Flag
grite --actor abc123 issue list

# Default from config
grite issue list  # Uses default_actor from .git/grite/config.toml
```

## CI/CD Environments

### GitHub Actions

```yaml
env:
  GRIT_HOME: ${{ runner.temp }}/grite-${{ github.run_id }}

steps:
  - name: Initialize grite
    run: |
      grite actor init --label "ci-${{ github.run_number }}"
```

### GitLab CI

```yaml
variables:
  GRIT_HOME: "${CI_PROJECT_DIR}/.grite-ci-${CI_JOB_ID}"

script:
  - grite actor init --label "ci-${CI_JOB_ID}"
```

### Docker

```dockerfile
ENV GRIT_HOME=/app/.grite

# Or at runtime
docker run -e GRIT_HOME=/app/.grite myimage
```

## Shell Configuration

### Bash/Zsh

Add to `~/.bashrc` or `~/.zshrc`:

```bash
# Custom grite home
export GRIT_HOME="$HOME/.grite/default-actor"

# Debug logging
alias grite-debug='RUST_LOG=debug grite'
```

### Fish

Add to `~/.config/fish/config.fish`:

```fish
set -x GRIT_HOME "$HOME/.grite/default-actor"
```

## Troubleshooting

### Check Current Environment

```bash
# See what grite sees
grite actor current --json | jq

# Check environment
echo $GRIT_HOME
echo $RUST_LOG
```

### Common Issues

#### "Wrong actor being used"

Check actor selection precedence:

```bash
# What's the current actor?
grite actor current --json

# Is GRIT_HOME set?
echo $GRIT_HOME

# What's the default in config?
cat .git/grite/config.toml
```

#### "Missing log output"

Enable logging:

```bash
export RUST_LOG=debug
grite issue list 2>&1 | head -50
```

#### "Daemon not using environment"

The daemon inherits environment from the spawning process. If auto-spawned, it uses the environment at spawn time.

```bash
# Stop daemon and restart with new environment
grite daemon stop
export RUST_LOG=debug
grite daemon start
```

## Next Steps

- [Configuration](configuration.md) - File-based configuration
- [CLI Reference](cli.md) - Command-line options
- [Using the Daemon](../guides/daemon.md) - Daemon behavior
