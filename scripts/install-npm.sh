#!/usr/bin/env bash
set -euo pipefail

# Install grite via npm
# Usage: curl -fsSL https://raw.githubusercontent.com/neul-labs/grite/main/scripts/install-npm.sh | bash

REPO="neul-labs/grite"

error() { echo "ERROR: $*" >&2; exit 1; }
info()  { echo "INFO:  $*"; }

main() {
    echo "=== Grite Installer (npm) ==="
    echo ""

    if ! command -v npm &> /dev/null; then
        error "npm is not installed."
        echo "Install Node.js from https://nodejs.org/ and try again." >&2
        exit 1
    fi

    info "Installing grite via npm..."
    npm install -g "@neul-labs/grite"

    echo ""
    echo "Done. Installed: grite, grite-daemon"
    echo ""

    # Verify
    if command -v grite &> /dev/null; then
        echo "Version: $(grite --version 2>/dev/null || echo 'unknown')"
    else
        echo "Warning: grite not found in PATH."
        echo "npm global bin may not be in your PATH."
        echo "Add this to your shell config:"
        echo "  export PATH=\"$(npm bin -g):\$PATH\""
    fi
}

main
