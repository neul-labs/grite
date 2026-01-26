"""
grit-cli: Git-backed issue tracking for coding agents and humans

This package provides a wrapper around the grit binary.
"""

__version__ = "0.1.0"

import os
import sys
import platform
import stat
import urllib.request
import tarfile
import zipfile
import tempfile
import shutil
from pathlib import Path

REPO = "neul-labs/grit"


def get_cache_dir() -> Path:
    """Get the cache directory for storing the binary."""
    if platform.system() == "Windows":
        base = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData" / "Local"))
    elif platform.system() == "Darwin":
        base = Path.home() / "Library" / "Caches"
    else:
        base = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))

    cache_dir = base / "grit-cli"
    cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir


def get_platform() -> str:
    """Get the platform identifier for downloading binaries."""
    system = platform.system()
    machine = platform.machine().lower()

    system_map = {
        "Darwin": "apple-darwin",
        "Linux": "unknown-linux-gnu",
        "Windows": "pc-windows-msvc",
    }

    arch_map = {
        "x86_64": "x86_64",
        "amd64": "x86_64",
        "aarch64": "aarch64",
        "arm64": "aarch64",
    }

    mapped_system = system_map.get(system)
    mapped_arch = arch_map.get(machine)

    if not mapped_system or not mapped_arch:
        raise RuntimeError(f"Unsupported platform: {system}-{machine}")

    # Use universal binary for macOS
    if system == "Darwin":
        return "universal-apple-darwin"

    return f"{mapped_arch}-{mapped_system}"


def get_archive_ext() -> str:
    """Get the archive extension for the current platform."""
    return ".zip" if platform.system() == "Windows" else ".tar.gz"


def get_binary_name(name: str) -> str:
    """Get the binary name with extension for current platform."""
    return f"{name}.exe" if platform.system() == "Windows" else name


def get_binary_path(name: str = "grit") -> Path:
    """Get the path to the binary, downloading if necessary."""
    cache_dir = get_cache_dir()
    version_dir = cache_dir / __version__
    binary_path = version_dir / get_binary_name(name)

    if binary_path.exists():
        return binary_path

    # Download binary
    download_binary(version_dir)

    if not binary_path.exists():
        raise RuntimeError(f"Binary not found after download: {binary_path}")

    return binary_path


def download_binary(dest_dir: Path) -> None:
    """Download and extract the binary for the current platform."""
    plat = get_platform()
    ext = get_archive_ext()
    archive_name = f"grit-{__version__}-{plat}{ext}"
    url = f"https://github.com/{REPO}/releases/download/v{__version__}/{archive_name}"

    print(f"Downloading grit v{__version__} for {plat}...")

    with tempfile.TemporaryDirectory() as temp_dir:
        temp_path = Path(temp_dir)
        archive_path = temp_path / archive_name

        # Download
        urllib.request.urlretrieve(url, archive_path)

        # Extract
        if ext == ".tar.gz":
            with tarfile.open(archive_path, "r:gz") as tar:
                tar.extractall(temp_path)
        else:
            with zipfile.ZipFile(archive_path, "r") as zip_ref:
                zip_ref.extractall(temp_path)

        # Find extracted directory
        extracted_dirs = [d for d in temp_path.iterdir() if d.is_dir() and d.name.startswith("grit-")]
        if not extracted_dirs:
            raise RuntimeError("Could not find extracted directory")

        src_dir = extracted_dirs[0]

        # Create destination directory
        dest_dir.mkdir(parents=True, exist_ok=True)

        # Copy binaries
        for binary in ["grit", "grit-daemon"]:
            src = src_dir / get_binary_name(binary)
            dst = dest_dir / get_binary_name(binary)
            shutil.copy2(src, dst)

            # Make executable on Unix
            if platform.system() != "Windows":
                dst.chmod(dst.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)

    print(f"Successfully installed grit to {dest_dir}")


def main() -> None:
    """Entry point for grit command."""
    binary = get_binary_path("grit")
    os.execv(str(binary), [str(binary)] + sys.argv[1:])


def main_daemon() -> None:
    """Entry point for grit-daemon command."""
    binary = get_binary_path("grit-daemon")
    os.execv(str(binary), [str(binary)] + sys.argv[1:])
