# grite-cli

Git-backed issue tracking for coding agents and humans.

## Installation

```bash
pip install grite-cli
```

## Usage

```bash
# Initialize in a git repository
grite init

# Create an issue
grite issue new --title "My first issue"

# List issues
grite issue list

# Add a comment
grite issue comment <issue-id> --body "Working on this"

# Close an issue
grite issue close <issue-id>
```

## Requirements

- Git 2.38+

## Documentation

See the full documentation at [github.com/neul-labs/grite](https://github.com/neul-labs/grite).

## License

MIT
