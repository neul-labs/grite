# Installation

This guide covers all methods to install grit on your system.

## Quick Install (Recommended)

The fastest way to install grit:

```bash
curl -fsSL https://raw.githubusercontent.com/neul-labs/grit/main/install.sh | bash
```

This downloads the pre-built binary for your platform and installs to `~/.local/bin/`.

## Package Managers

=== "Homebrew (macOS/Linux)"

    ```bash
    brew install neul-labs/tap/grit
    ```

=== "Cargo (Rust)"

    ```bash
    cargo install grit grit-daemon
    ```

    Requires Rust 1.75+.

=== "npm"

    ```bash
    npm install -g @neul-labs/grit
    ```

=== "pip"

    ```bash
    pip install grit-cli
    ```

=== "gem"

    ```bash
    gem install grit-cli
    ```

=== "Chocolatey (Windows)"

    ```powershell
    choco install grit
    ```

## From Source

Build from source for the latest development version:

```bash
git clone https://github.com/neul-labs/grit.git
cd grit
./install.sh --source
```

This requires:

- Rust 1.75+
- nng library (see prerequisites below)

## Prerequisites

### Git

Grit requires Git 2.38 or later. Check your version:

```bash
git --version
```

### nng Library

The nng (nanomsg-next-gen) library is required for inter-process communication between the CLI and daemon.

=== "Ubuntu/Debian"

    ```bash
    sudo apt install libnng-dev
    ```

=== "macOS"

    ```bash
    brew install nng
    ```

=== "Windows"

    The nng library is bundled with pre-built Windows binaries. No separate installation needed.

=== "From Source"

    ```bash
    git clone https://github.com/nanomsg/nng.git
    cd nng
    mkdir build && cd build
    cmake ..
    make
    sudo make install
    ```

## Verifying Installation

After installation, verify grit is working:

```bash
grit --version
```

You should see output like:

```
grit 0.1.0
```

## Updating

To update grit to the latest version:

=== "Quick Install"

    Run the install script again:
    ```bash
    curl -fsSL https://raw.githubusercontent.com/neul-labs/grit/main/install.sh | bash
    ```

=== "Homebrew"

    ```bash
    brew upgrade grit
    ```

=== "Cargo"

    ```bash
    cargo install grit grit-daemon --force
    ```

=== "npm"

    ```bash
    npm update -g @neul-labs/grit
    ```

## Uninstalling

=== "Quick Install"

    Remove the binaries:
    ```bash
    rm ~/.local/bin/grit ~/.local/bin/grit-daemon
    ```

=== "Homebrew"

    ```bash
    brew uninstall grit
    ```

=== "Cargo"

    ```bash
    cargo uninstall grit grit-daemon
    ```

=== "npm"

    ```bash
    npm uninstall -g @neul-labs/grit
    ```

## Next Steps

Now that grit is installed, head to [Quick Start](quickstart.md) to create your first issue.
