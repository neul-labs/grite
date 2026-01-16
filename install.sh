#!/usr/bin/env bash
set -euo pipefail

# Install grit locally
# Usage: ./install.sh [--prefix PATH]

PREFIX="${HOME}/.local"

while [[ $# -gt 0 ]]; do
    case $1 in
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: ./install.sh [--prefix PATH]"
            echo ""
            echo "Options:"
            echo "  --prefix PATH   Install prefix (default: ~/.local)"
            echo "                  Binary installed to PREFIX/bin/grit"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

BIN_DIR="${PREFIX}/bin"

echo "Building grit (release)..."
cargo build --release --package grit

echo "Installing to ${BIN_DIR}/grit..."
mkdir -p "${BIN_DIR}"
cp target/release/grit "${BIN_DIR}/grit"

echo "Done."
echo ""
if [[ ":$PATH:" != *":${BIN_DIR}:"* ]]; then
    echo "Note: ${BIN_DIR} is not in your PATH."
    echo "Add this to your shell config:"
    echo "  export PATH=\"${BIN_DIR}:\$PATH\""
fi
