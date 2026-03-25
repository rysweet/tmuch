//! Self-update command: downloads the latest tmuch binary from GitHub Releases.

use crate::consts::{sanitise_for_display, ALLOWED_DOWNLOAD_HOSTS, CURRENT_VERSION, GITHUB_REPO};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

fn platform_suffix() -> Option<&'static str> {
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        Some("linux-x86_64")
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        Some("linux-aarch64")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        Some("macos-x86_64")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        Some("macos-aarch64")
    } else {
        None
    }
}

/// Validate that a download URL points to an allowed GitHub host.
fn validate_download_url(url: &str) -> Result<()> {
    for prefix in ALLOWED_DOWNLOAD_HOSTS {
        if url.starts_with(prefix) {
            return Ok(());
        }
    }
    anyhow::bail!(
        "Download URL does not match allowed hosts: {}",
        url.get(..80).unwrap_or(url)
    );
}

fn find_latest_release() -> Result<(String, String)> {
    let suffix =
        platform_suffix().ok_or_else(|| anyhow::anyhow!("Unsupported platform for self-update"))?;

    let api_url = format!("repos/{}/releases/latest", GITHUB_REPO);

    let output = std::process::Command::new("gh")
        .args(["api", &api_url, "--jq", "."])
        .output()
        .or_else(|_| {
            let url = format!("https://api.github.com/{}", api_url);
            std::process::Command::new("curl")
                .args([
                    "-sS",
                    "--proto",
                    "=https",
                    "--connect-timeout",
                    "10",
                    "--max-time",
                    "30",
                    "-H",
                    "Accept: application/vnd.github+json",
                    &url,
                ])
                .output()
        })
        .context("Failed to query GitHub releases (need gh or curl installed)")?;

    if !output.status.success() {
        anyhow::bail!(
            "GitHub API request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let release: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse GitHub release JSON")?;

    let tag = release["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No tag_name in release"))?;

    let version = tag.strip_prefix('v').unwrap_or(tag).to_string();

    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No assets in release"))?;

    for asset in assets {
        let name = asset["name"].as_str().unwrap_or("");
        if name.contains(suffix) && name.ends_with(".tar.gz") {
            let dl_url = asset["browser_download_url"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing download URL"))?;
            validate_download_url(dl_url)?;
            return Ok((dl_url.to_string(), version));
        }
    }

    anyhow::bail!("No release asset found for platform '{}'", suffix)
}

fn download_and_replace(url: &str, version: &str) -> Result<()> {
    let current_exe =
        std::env::current_exe().context("Cannot determine current executable path")?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine executable directory"))?;
    let tmp_dir = std::env::temp_dir().join(format!("tmuch-update-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir).context("Failed to create temp directory")?;

    let archive_path = tmp_dir.join("tmuch.tar.gz");
    let archive_str = archive_path.to_str().context("Non-UTF-8 archive path")?;
    let tmp_dir_str = tmp_dir.to_str().context("Non-UTF-8 temp dir path")?;

    let safe_version = sanitise_for_display(version);
    eprintln!("Downloading tmuch v{}...", safe_version);

    let dl_status = std::process::Command::new("curl")
        .args([
            "-sS",
            "--proto",
            "=https",
            "-L",
            "--connect-timeout",
            "10",
            "--max-time",
            "120",
            "--max-filesize",
            "104857600", // 100MB
            "-o",
            archive_str,
            url,
        ])
        .status()
        .context("Failed to download release")?;

    if !dl_status.success() {
        anyhow::bail!("Download failed");
    }

    eprintln!("Extracting...");

    let tar_status = std::process::Command::new("tar")
        .args(["xzf", archive_str, "-C", tmp_dir_str])
        .status()
        .context("Failed to extract archive")?;

    if !tar_status.success() {
        anyhow::bail!("Extraction failed");
    }

    let new_bin = find_binary_in_dir(&tmp_dir)?;

    // Verify extracted binary is within tmp_dir (prevent path traversal)
    let canonical_bin = new_bin
        .canonicalize()
        .context("Cannot canonicalize new binary path")?;
    let canonical_tmp = tmp_dir
        .canonicalize()
        .context("Cannot canonicalize temp dir")?;
    if !canonical_bin.starts_with(&canonical_tmp) {
        anyhow::bail!("Extracted binary path is outside temp directory — possible path traversal");
    }

    // Atomic replacement: write new binary to temp file in exe_dir, then rename over current
    let staging_path = exe_dir.join(format!(".tmuch-update-staging-{}", std::process::id()));
    fs::copy(&new_bin, &staging_path).context("Failed to stage new binary")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staging_path, fs::Permissions::from_mode(0o755))?;
    }

    // Keep backup in case we need to rollback
    let backup = current_exe.with_extension("old");
    if backup.exists() {
        fs::remove_file(&backup).ok();
    }
    fs::rename(&current_exe, &backup)
        .context("Failed to backup current binary (try running with sudo)")?;
    fs::rename(&staging_path, &current_exe).context("Failed to install new binary")?;

    // Verify new binary works before cleaning up backup
    let verify = std::process::Command::new(&current_exe)
        .args(["--version"])
        .output();
    match verify {
        Ok(output) if output.status.success() => {
            // New binary works — safe to remove backup
            fs::remove_file(&backup).ok();
        }
        _ => {
            // New binary is broken — rollback
            eprintln!("Warning: new binary failed verification, rolling back");
            fs::rename(&backup, &current_exe).ok();
            anyhow::bail!("New binary failed --version check, rolled back to previous version");
        }
    }

    // Clean up temp dir
    fs::remove_dir_all(&tmp_dir).ok();

    eprintln!("Updated tmuch: v{} → v{}", CURRENT_VERSION, safe_version);
    Ok(())
}

fn find_binary_in_dir(dir: &Path) -> Result<PathBuf> {
    fn search(dir: &Path, depth: u32) -> Option<PathBuf> {
        if depth > 3 {
            return None;
        }
        let entries = fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_file() && (name == "tmuch" || name.starts_with("tmuch-")) {
                return Some(path);
            }
            if path.is_dir() {
                if let Some(found) = search(&path, depth + 1) {
                    return Some(found);
                }
            }
        }
        None
    }
    search(dir, 0).ok_or_else(|| anyhow::anyhow!("Binary 'tmuch' not found in downloaded archive"))
}

pub fn handle_self_update() -> Result<()> {
    let safe_version = sanitise_for_display(CURRENT_VERSION);
    eprintln!("tmuch self-update (current: v{})", safe_version);

    let (url, version) = find_latest_release()?;

    if version == CURRENT_VERSION {
        eprintln!("Already at the latest version (v{}).", safe_version);
        return Ok(());
    }

    let safe_new = sanitise_for_display(&version);
    eprintln!("New version available: v{} → v{}", safe_version, safe_new);
    download_and_replace(&url, &version)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_suffix_not_none() {
        assert!(platform_suffix().is_some());
    }

    #[test]
    fn test_current_version_format() {
        assert!(CURRENT_VERSION.contains('.'));
        assert!(!CURRENT_VERSION.is_empty());
    }

    #[test]
    fn test_validate_url_github() {
        assert!(validate_download_url(
            "https://github.com/rysweet/tmuch/releases/download/v0.1.0/tmuch-linux-x86_64.tar.gz"
        )
        .is_ok());
    }

    #[test]
    fn test_validate_url_objects() {
        assert!(validate_download_url("https://objects.githubusercontent.com/foo/bar").is_ok());
    }

    #[test]
    fn test_validate_url_rejects_evil() {
        assert!(validate_download_url("https://evil.com/malware.tar.gz").is_err());
    }

    #[test]
    fn test_validate_url_rejects_http() {
        assert!(validate_download_url(
            "http://github.com/rysweet/tmuch/releases/download/v0.1.0/tmuch.tar.gz"
        )
        .is_err());
    }
}
