# grite-cli

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)
[![RubyGems](https://img.shields.io/gem/v/grite-cli)](https://rubygems.org/gems/grite-cli)
[![Build Status](https://img.shields.io/github/actions/workflow/status/neul-labs/grite/ci.yml?branch=main)](https://github.com/neul-labs/grite/actions)

**Git-backed issue tracking for coding agents and humans — distributed via RubyGems.**

This gem provides the `grite` and `grite-daemon` binaries as a RubyGems-installable package. It is a thin wrapper around the native Rust binaries, automatically downloading the correct platform-specific binary during installation.

---

## Installation

```bash
# Install globally
gem install grite-cli

# Or add to your Gemfile
gem 'grite-cli'
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

- Ruby 2.7 or later
- Git 2.38 or later

## How It Works

This gem contains a post-install hook that runs during `gem install`. It:

1. Detects your operating system and CPU architecture
2. Downloads the matching pre-built binary from GitHub Releases
3. Places the binary in a platform-specific directory within the gem
4. Creates wrapper executables in the gem's `bin/` directory

No compilation is required. The binaries are pure native code with zero runtime dependencies.

## Documentation

For full documentation, including architecture, API reference, and advanced usage, see the main project:

- [github.com/neul-labs/grite](https://github.com/neul-labs/grite)
- [docs.neullabs.com/grite](https://docs.neullabs.com/grite)

## License

MIT — see [LICENSE](../LICENSE) for details.
