"""Download and execute the tmuch Rust binary from GitHub Releases."""

import json
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
# Fallback version if API discovery fails
FALLBACK_VERSION = "0.3.8"


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


def _discover_latest_version() -> str:
    """Query GitHub API for the latest release tag."""
    # Try gh CLI first (fast, authenticated)
    try:
        result = subprocess.run(
            ["gh", "api", f"repos/{GITHUB_REPO}/releases/latest", "--jq", ".tag_name"],
            capture_output=True, text=True, timeout=5,
        )
        if result.returncode == 0 and result.stdout.strip():
            tag = result.stdout.strip().lstrip("v")
            return tag
    except (FileNotFoundError, subprocess.TimeoutExpired):
        pass

    # Fallback: GitHub API via urllib
    try:
        url = f"https://api.github.com/repos/{GITHUB_REPO}/releases/latest"
        req = urllib.request.Request(url, headers={"Accept": "application/vnd.github+json"})
        with urllib.request.urlopen(req, timeout=5) as resp:
            data = json.loads(resp.read())
            tag = data.get("tag_name", "").lstrip("v")
            if tag:
                return tag
    except Exception:
        pass

    return FALLBACK_VERSION


def _ensure_binary() -> Path:
    """Download the tmuch binary if not already cached."""
    bin_dir = _bin_dir()
    binary = bin_dir / "tmuch"

    # Discover latest version
    version = _discover_latest_version()

    # Check if we already have a working binary (any version)
    if binary.exists():
        try:
            result = subprocess.run(
                [str(binary), "--version"],
                capture_output=True, text=True, timeout=5,
            )
            if result.returncode == 0:
                cached_ver = result.stdout.strip().split()[-1] if result.stdout.strip() else ""
                if cached_ver == version:
                    return binary
                # Different version — re-download
                print(f"Updating tmuch {cached_ver} → {version}...", file=sys.stderr)
        except (subprocess.TimeoutExpired, OSError):
            pass

    # Download from GitHub Releases
    suffix = _platform_suffix()
    asset_name = f"tmuch-{suffix}.tar.gz"
    url = f"https://github.com/{GITHUB_REPO}/releases/download/v{version}/{asset_name}"

    print(f"Downloading tmuch v{version} for {suffix}...", file=sys.stderr)

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
                        "gh", "release", "download", f"v{version}",
                        "--repo", GITHUB_REPO,
                        "--pattern", f"*{suffix}*",
                        "--dir", tmp,
                    ],
                    check=True, capture_output=True,
                )
            except (subprocess.CalledProcessError, FileNotFoundError):
                raise RuntimeError(
                    f"Failed to download tmuch: {e}\n"
                    f"URL: {url}\n"
                    f"Try: gh release download v{version} --repo {GITHUB_REPO}"
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
                        f"Installed tmuch v{version} to {binary}",
                        file=sys.stderr,
                    )
                    return binary

    raise RuntimeError("Binary not found in downloaded archive")


def main():
    """Entry point: ensure binary exists and exec into it."""
    binary = _ensure_binary()
    os.execv(str(binary), [str(binary)] + sys.argv[1:])
