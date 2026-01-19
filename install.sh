#!/usr/bin/env bash
set -euo pipefail

# Install grit - git-backed issue tracking for coding agents and humans
# Usage: curl -fsSL https://raw.githubusercontent.com/neul-labs/grit/main/install.sh | bash
#    or: ./install.sh [OPTIONS]

REPO="neul-labs/grit"
PREFIX="${HOME}/.local"
FORCE_SOURCE=false

print_help() {
    cat << EOF
Usage: ./install.sh [OPTIONS]

Install grit - git-backed issue tracking for coding agents and humans

Options:
  --prefix PATH   Install prefix (default: ~/.local)
                  Binaries installed to PREFIX/bin/
  --source        Force build from source (requires Rust toolchain)
  -h, --help      Show this help message

Examples:
  # Quick install (downloads pre-built binary)
  curl -fsSL https://raw.githubusercontent.com/neul-labs/grit/main/install.sh | bash

  # Install to custom location
  ./install.sh --prefix /usr/local

  # Build from source
  ./install.sh --source
EOF
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        --source)
            FORCE_SOURCE=true
            shift
            ;;
        -h|--help)
            print_help
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            print_help
            exit 1
            ;;
    esac
done

BIN_DIR="${PREFIX}/bin"

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        MINGW*|MSYS*|CYGWIN*) os="pc-windows-msvc" ;;
        *)
            echo "Unsupported OS: $(uname -s)"
            return 1
            ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *)
            echo "Unsupported architecture: $(uname -m)"
            return 1
            ;;
    esac

    # Use universal binary for macOS
    if [ "$os" = "apple-darwin" ]; then
        echo "universal-apple-darwin"
    else
        echo "${arch}-${os}"
    fi
}

# Get latest release version from GitHub
get_latest_version() {
    local version
    if command -v curl &> /dev/null; then
        version=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
    elif command -v wget &> /dev/null; then
        version=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
    else
        echo "Error: curl or wget required" >&2
        return 1
    fi
    echo "$version"
}

# Download and install binary
install_binary() {
    local platform version url archive_name temp_dir

    platform=$(detect_platform) || return 1
    version=$(get_latest_version) || return 1

    if [ -z "$version" ]; then
        echo "Could not determine latest version"
        return 1
    fi

    echo "Installing grit v${version} for ${platform}..."

    # Determine archive extension
    local ext="tar.gz"
    if [[ "$platform" == *"windows"* ]]; then
        ext="zip"
    fi

    archive_name="grit-${version}-${platform}.${ext}"
    url="https://github.com/${REPO}/releases/download/v${version}/${archive_name}"

    # Create temp directory
    temp_dir=$(mktemp -d)
    trap "rm -rf $temp_dir" EXIT

    echo "Downloading ${url}..."

    # Download archive
    if command -v curl &> /dev/null; then
        if ! curl -fsSL "$url" -o "${temp_dir}/${archive_name}"; then
            echo "Download failed. Binary may not be available for your platform."
            return 1
        fi
    elif command -v wget &> /dev/null; then
        if ! wget -q "$url" -O "${temp_dir}/${archive_name}"; then
            echo "Download failed. Binary may not be available for your platform."
            return 1
        fi
    fi

    # Extract archive
    cd "$temp_dir"
    if [[ "$ext" == "tar.gz" ]]; then
        tar -xzf "$archive_name"
    else
        unzip -q "$archive_name"
    fi

    # Find extracted directory
    local extracted_dir
    extracted_dir=$(find . -maxdepth 1 -type d -name "grit-*" | head -1)

    if [ -z "$extracted_dir" ]; then
        echo "Error: Could not find extracted files"
        return 1
    fi

    # Install binaries
    echo "Installing to ${BIN_DIR}..."
    mkdir -p "${BIN_DIR}"

    if [[ "$platform" == *"windows"* ]]; then
        cp "${extracted_dir}/grit.exe" "${BIN_DIR}/"
        cp "${extracted_dir}/gritd.exe" "${BIN_DIR}/"
    else
        cp "${extracted_dir}/grit" "${BIN_DIR}/"
        cp "${extracted_dir}/gritd" "${BIN_DIR}/"
        chmod +x "${BIN_DIR}/grit" "${BIN_DIR}/gritd"
    fi

    echo "Successfully installed grit v${version}"
    return 0
}

# Build from source
build_from_source() {
    echo "Building from source..."

    # Check for Rust
    if ! command -v cargo &> /dev/null; then
        echo "Error: Rust toolchain not found."
        echo "Install Rust from https://rustup.rs/ and try again."
        exit 1
    fi

    # Check if we're in the grit repo
    if [ -f "Cargo.toml" ] && grep -q 'name = "grit"' crates/grit/Cargo.toml 2>/dev/null; then
        echo "Building grit and gritd (release)..."
        cargo build --release --package grit --package gritd

        echo "Installing to ${BIN_DIR}..."
        mkdir -p "${BIN_DIR}"
        cp target/release/grit "${BIN_DIR}/grit"
        cp target/release/gritd "${BIN_DIR}/gritd"
    else
        # Install from crates.io or git
        echo "Installing from crates.io..."
        cargo install grit gritd --root "${PREFIX}"
    fi

    echo "Successfully built and installed grit"
}

# Main installation logic
main() {
    echo "=== Grit Installer ==="
    echo ""

    if [ "$FORCE_SOURCE" = true ]; then
        build_from_source
    else
        # Try binary first, fall back to source
        if ! install_binary; then
            echo ""
            echo "Binary installation failed, falling back to source build..."
            echo ""
            build_from_source
        fi
    fi

    echo ""
    echo "Done. Installed: grit, gritd"
    echo ""

    # Verify installation
    if [ -x "${BIN_DIR}/grit" ]; then
        echo "Version: $(${BIN_DIR}/grit --version 2>/dev/null || echo 'unknown')"
    fi

    # Check PATH
    if [[ ":$PATH:" != *":${BIN_DIR}:"* ]]; then
        echo ""
        echo "Note: ${BIN_DIR} is not in your PATH."
        echo "Add this to your shell config (~/.bashrc, ~/.zshrc, etc.):"
        echo ""
        echo "  export PATH=\"${BIN_DIR}:\$PATH\""
    fi
}

main
