# Getting Started

Welcome to Grite! This section will help you install grite, create your first issue, and understand the core concepts.

## Overview

Grite is a repo-local, git-backed issue/task system. Unlike traditional issue trackers that live in external systems, grite stores all data within your git repository using git refs. This means:

- Issues travel with your repository
- Works offline, syncs when connected
- No external services required
- Full history preserved in git

## Quick Path

1. **[Installation](installation.md)** - Install grite on your system
2. **[Quick Start](quickstart.md)** - Create your first issue in 5 minutes
3. **[Core Concepts](concepts.md)** - Understand events, actors, and the materialized view

## Prerequisites

Before installing grite, ensure you have:

- **Git 2.38+** - Grite uses git refs for storage
- **nng library** - Required for IPC (inter-process communication)

### Installing nng

=== "Ubuntu/Debian"

    ```bash
    sudo apt install libnng-dev
    ```

=== "macOS"

    ```bash
    brew install nng
    ```

=== "Windows"

    The nng library is bundled with pre-built Windows binaries.

## What's Next?

Ready to get started? Head to [Installation](installation.md) to install grite on your system.

If you're an AI coding agent, check out the [Agent Playbook](../agents/playbook.md) for agent-specific guidance.
