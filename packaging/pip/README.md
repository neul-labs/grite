# grite

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)
[![PyPI](https://img.shields.io/pypi/v/grite)](https://pypi.org/project/grite/)
[![Build Status](https://img.shields.io/github/actions/workflow/status/neul-labs/grite/ci.yml?branch=main)](https://github.com/neul-labs/grite/actions)

**Git-backed issue tracking for coding agents and humans — distributed via PyPI.**

This package provides the `grite` and `grite-daemon` binaries as a pip-installable Python package. It is a thin wrapper around the native Rust binaries, automatically downloading the correct platform-specific binary during installation.

---

## Installation

```bash
# Install globally
pip install grite

# Or install in a virtual environment
python -m venv .venv
source .venv/bin/activate
pip install grite
```

## Usage

Once installed, the `grite` and `grite-daemon` commands are available in your PATH:

```bash
# Initialize grite in a git repository
grite init

# Create an issue
grite issue create --title "Fix race condition" --label bug

# List issues
grite issue list

# Add a comment
grite issue comment <issue-id> --body "Working on this"

# Close an issue
grite issue close <issue-id>

# Sync with remote
grite sync
```

## Supported Platforms

| Platform | Architecture | Status |
|----------|-------------|--------|
| macOS | x86_64, ARM64, Universal | Supported |
| Linux | x86_64, ARM64 (glibc + musl) | Supported |
| Windows | x86_64, ARM64 | Planned |

The install script automatically detects your platform and downloads the appropriate binary.

## Requirements

- Python 3.8 or later
- Git 2.38 or later

## How It Works

This Python package contains a post-install hook that runs during `pip install`. It:

1. Detects your operating system and CPU architecture
2. Downloads the matching pre-built binary from GitHub Releases
3. Places the binary in a platform-specific directory within the package
4. Creates entry-point scripts for `grite` and `grite-daemon`

No compilation is required. The binaries are pure native code with zero runtime dependencies.

## Python API

While this package primarily provides CLI binaries, you can also invoke grite programmatically from Python:

```python
import subprocess
import json

# List issues as JSON
result = subprocess.run(
    ["grite", "issue", "list", "--json"],
    capture_output=True,
    text=True,
    check=True,
)
issues = json.loads(result.stdout)
for issue in issues:
    print(f"{issue['id']}: {issue['title']}")
```

For deeper integration, consider using the Rust library [`libgrite-cli`](https://crates.io/crates/libgrite-cli) via PyO3 or calling the JSON CLI interface.

## Documentation

For full documentation, including architecture, API reference, and advanced usage, see the main project:

- [github.com/neul-labs/grite](https://github.com/neul-labs/grite)
- [docs.neullabs.com/grite](https://docs.neullabs.com/grite)

## License

MIT — see [LICENSE](../LICENSE) for details.
