"""Download and execute the tmuch Rust binary from GitHub Releases."""

import os
import platform
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path

GITHUB_REPO = "rysweet/tmuch"
VERSION = "0.2.0"


def _platform_suffix() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()

    if system == "linux":
        if machine in ("x86_64", "amd64"):
            return "linux-x86_64"
        if machine in ("aarch64", "arm64"):
            return "linux-aarch64"
    elif system == "darwin":
        if machine in ("x86_64", "amd64"):
            return "macos-x86_64"
        if machine in ("aarch64", "arm64"):
            return "macos-aarch64"

    raise RuntimeError(f"Unsupported platform: {system}-{machine}")


def _bin_dir() -> Path:
    """Where to cache the downloaded binary."""
    cache = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))
    return cache / "tmuch" / "bin"


def _ensure_binary() -> Path:
    """Download the tmuch binary if not already cached."""
    bin_dir = _bin_dir()
    binary = bin_dir / "tmuch"

    # Check if we already have the right version
    if binary.exists():
        try:
            result = subprocess.run(
                [str(binary), "--version"],
                capture_output=True,
                text=True,
                timeout=5,
            )
            if f"tmuch {VERSION}" in result.stdout:
                return binary
        except (subprocess.TimeoutExpired, OSError):
            pass

    # Download from GitHub Releases
    suffix = _platform_suffix()
    asset_name = f"tmuch-{suffix}.tar.gz"
    url = f"https://github.com/{GITHUB_REPO}/releases/download/v{VERSION}/{asset_name}"

    print(f"Downloading tmuch v{VERSION} for {suffix}...", file=sys.stderr)

    bin_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory() as tmp:
        archive_path = Path(tmp) / asset_name

        # Download
        try:
            urllib.request.urlretrieve(url, archive_path)
        except Exception as e:
            # Try gh CLI as fallback
            try:
                subprocess.run(
                    [
                        "gh",
                        "release",
                        "download",
                        f"v{VERSION}",
                        "--repo",
                        GITHUB_REPO,
                        "--pattern",
                        f"*{suffix}*",
                        "--dir",
                        tmp,
                    ],
                    check=True,
                    capture_output=True,
                )
            except (subprocess.CalledProcessError, FileNotFoundError):
                raise RuntimeError(
                    f"Failed to download tmuch: {e}\n"
                    f"URL: {url}\n"
                    f"Try: gh release download v{VERSION} --repo {GITHUB_REPO}"
                ) from e

        # Extract
        with tarfile.open(archive_path) as tar:
            tar.extractall(tmp)

        # Find the binary
        for name in os.listdir(tmp):
            if name.startswith("tmuch") and not name.endswith(".tar.gz"):
                src = Path(tmp) / name
                if src.is_file():
                    shutil.copy2(src, binary)
                    binary.chmod(binary.stat().st_mode | stat.S_IEXEC)
                    print(
                        f"Installed tmuch v{VERSION} to {binary}",
                        file=sys.stderr,
                    )
                    return binary

    raise RuntimeError("Binary not found in downloaded archive")


def main():
    """Entry point: ensure binary exists and exec into it."""
    binary = _ensure_binary()
    os.execv(str(binary), [str(binary)] + sys.argv[1:])
