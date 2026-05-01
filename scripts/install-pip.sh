#!/usr/bin/env bash
set -euo pipefail

# Install grite via pip
# Usage: curl -fsSL https://raw.githubusercontent.com/neul-labs/grite/main/scripts/install-pip.sh | bash

REPO="neul-labs/grite"

error() { echo "ERROR: $*" >&2; exit 1; }
info()  { echo "INFO:  $*"; }

main() {
    echo "=== Grite Installer (pip) ==="
    echo ""

    local pip_cmd=""
    if command -v pip3 &> /dev/null; then
        pip_cmd="pip3"
    elif command -v pip &> /dev/null; then
        pip_cmd="pip"
    else
        error "pip is not installed."
        echo "Install Python from https://python.org/ and try again." >&2
        exit 1
    fi

    info "Installing grite via $pip_cmd..."
    $pip_cmd install --user "grite-cli"

    echo ""
    echo "Done. Installed: grite, grite-daemon"
    echo ""

    # Verify
    if command -v grite &> /dev/null; then
        echo "Version: $(grite --version 2>/dev/null || echo 'unknown')"
    else
        echo "Warning: grite not found in PATH."
        echo "User site-packages bin may not be in your PATH."
        echo "Add this to your shell config:"
        echo "  export PATH=\"$(python3 -m site --user-base 2>/dev/null || python -m site --user-base)/bin:\$PATH\""
    fi
}

main
