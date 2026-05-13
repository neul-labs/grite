# Installation

This guide covers all methods to install grite on your system.

## Quick Install (Recommended)

The fastest way to install grite:

```bash
curl -fsSL https://raw.githubusercontent.com/neul-labs/grite/main/install.sh | bash
```

This downloads the pre-built binary for your platform and installs to `~/.local/bin/`.

## Package Managers

=== "Homebrew (macOS/Linux)"

    ```bash
    brew install neul-labs/tap/grite
    ```

=== "Cargo (Rust)"

    ```bash
    cargo install grite grite-daemon
    ```

    Requires Rust 1.75+.

=== "npm"

    ```bash
    npm install -g grite-cli
    ```

=== "pip"

    ```bash
    pip install grite-cli
    ```

=== "gem"

    ```bash
    gem install grite-cli
    ```

=== "Chocolatey (Windows)"

    ```powershell
    choco install grite
    ```

## From Source

Build from source for the latest development version:

```bash
git clone https://github.com/neul-labs/grite.git
cd grite
./install.sh --source
```

This requires:

- Rust 1.75+

## Prerequisites

### Git

Grite requires Git 2.38 or later. Check your version:

```bash
git --version
```

## Verifying Installation

After installation, verify grite is working:

```bash
grite --version
```

You should see output like:

```
grite 0.1.0
```

## Updating

To update grite to the latest version:

=== "Quick Install"

    Run the install script again:
    ```bash
    curl -fsSL https://raw.githubusercontent.com/neul-labs/grite/main/install.sh | bash
    ```

=== "Homebrew"

    ```bash
    brew upgrade grite
    ```

=== "Cargo"

    ```bash
    cargo install grite grite-daemon --force
    ```

=== "npm"

    ```bash
    npm update -g grite
    ```

## Uninstalling

=== "Quick Install"

    Remove the binaries:
    ```bash
    rm ~/.local/bin/grite ~/.local/bin/grite-daemon
    ```

=== "Homebrew"

    ```bash
    brew uninstall grite
    ```

=== "Cargo"

    ```bash
    cargo uninstall grite grite-daemon
    ```

=== "npm"

    ```bash
    npm uninstall -g grite
    ```

## Next Steps

Now that grite is installed, head to [Quick Start](quickstart.md) to create your first issue.
