//! Self-update command: downloads the latest tmuch binary from GitHub Releases.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

const GITHUB_REPO: &str = "rysweet/tmuch";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn platform_suffix() -> Option<&'static str> {
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        Some("linux-x86_64")
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        Some("linux-aarch64")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        Some("macos-x86_64")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        Some("macos-aarch64")
    } else if cfg!(target_os = "windows") {
        Some("windows-x86_64")
    } else {
        None
    }
}

fn find_latest_release() -> Result<(String, String)> {
    let suffix =
        platform_suffix().ok_or_else(|| anyhow::anyhow!("Unsupported platform for self-update"))?;

    let output = std::process::Command::new("gh")
        .args([
            "api",
            &format!("repos/{}/releases/latest", GITHUB_REPO),
            "--jq",
            ".",
        ])
        .output()
        .or_else(|_| {
            std::process::Command::new("curl")
                .args([
                    "-sS",
                    "-H",
                    "Accept: application/vnd.github+json",
                    &format!(
                        "https://api.github.com/repos/{}/releases/latest",
                        GITHUB_REPO
                    ),
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
            return Ok((dl_url.to_string(), version));
        }
    }

    anyhow::bail!("No release asset found for platform '{}'", suffix)
}

fn download_and_replace(url: &str, version: &str) -> Result<()> {
    let current_exe =
        std::env::current_exe().context("Cannot determine current executable path")?;
    let tmp_dir = std::env::temp_dir().join(format!("tmuch-update-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir).context("Failed to create temp directory")?;
    let archive_path = tmp_dir.join("tmuch.tar.gz");

    eprintln!("Downloading tmuch v{}...", version);

    let dl_status = std::process::Command::new("curl")
        .args(["-sS", "-L", "-o", archive_path.to_str().unwrap(), url])
        .status()
        .context("Failed to download release")?;

    if !dl_status.success() {
        anyhow::bail!("Download failed");
    }

    eprintln!("Extracting...");

    let tar_status = std::process::Command::new("tar")
        .args([
            "xzf",
            archive_path.to_str().unwrap(),
            "-C",
            tmp_dir.to_str().unwrap(),
        ])
        .status()
        .context("Failed to extract archive")?;

    if !tar_status.success() {
        anyhow::bail!("Extraction failed");
    }

    let new_bin = find_binary_in_dir(&tmp_dir)?;

    // Replace current binary
    let backup = current_exe.with_extension("old");
    if backup.exists() {
        if let Err(e) = fs::remove_file(&backup) {
            eprintln!("Warning: failed to remove old backup: {e}");
        }
    }
    fs::rename(&current_exe, &backup)
        .context("Failed to backup current binary (try running with sudo)")?;

    fs::copy(&new_bin, &current_exe).context("Failed to install new binary")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&current_exe, fs::Permissions::from_mode(0o755))?;
    }

    // Clean up
    if let Err(e) = fs::remove_file(&backup) {
        eprintln!("Warning: failed to clean up backup: {e}");
    }
    if let Err(e) = fs::remove_dir_all(&tmp_dir) {
        eprintln!("Warning: failed to clean up temp directory: {e}");
    }

    eprintln!("Updated tmuch: v{} → v{}", CURRENT_VERSION, version);
    Ok(())
}

fn find_binary_in_dir(dir: &std::path::Path) -> Result<PathBuf> {
    fn search(dir: &std::path::Path, depth: u32) -> Option<PathBuf> {
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
    eprintln!("tmuch self-update (current: v{})", CURRENT_VERSION);

    let (url, version) = find_latest_release()?;

    if version == CURRENT_VERSION {
        eprintln!("Already at the latest version (v{}).", CURRENT_VERSION);
        return Ok(());
    }

    eprintln!("New version available: v{} → v{}", CURRENT_VERSION, version);
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
}
