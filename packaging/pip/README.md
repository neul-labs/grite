# grit-cli

Git-backed issue tracking for coding agents and humans.

## Installation

```bash
pip install grit-cli
```

## Usage

```bash
# Initialize in a git repository
grit init

# Create an issue
grit issue new --title "My first issue"

# List issues
grit issue list

# Add a comment
grit issue comment <issue-id> --body "Working on this"

# Close an issue
grit issue close <issue-id>
```

## Requirements

- Git 2.38+
- nng library (for IPC)

### Installing nng

**macOS:**
```bash
brew install nng
```

**Ubuntu/Debian:**
```bash
sudo apt-get install libnng-dev
```

## Documentation

See the full documentation at [github.com/neul-labs/grit](https://github.com/neul-labs/grit).

## License

MIT
