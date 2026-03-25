//! Non-blocking update check with 24-hour cooldown.
//!
//! Checks GitHub releases for newer versions and prints a one-line notice.
//! Failures are silently ignored — never blocks or slows normal operation.

use crate::consts::{sanitise_for_display, CURRENT_VERSION, GITHUB_REPO};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const COOLDOWN_SECS: u64 = 86400; // 24 hours

fn cache_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("tmuch").join("last_update_check"))
}

fn read_cache() -> Option<(String, u64)> {
    let path = cache_path()?;
    let content = fs::read_to_string(&path).ok()?;
    let mut lines = content.lines();
    let version = lines.next()?.to_string();
    let timestamp: u64 = lines.next()?.parse().ok()?;
    Some((version, timestamp))
}

fn write_cache(version: &str) {
    if let Some(path) = cache_path() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = fs::metadata(parent) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o700);
                    fs::set_permissions(parent, perms).ok();
                }
            }
        }
        let now = now_secs();
        fs::write(&path, format!("{}\n{}", version, now)).ok();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                fs::set_permissions(&path, perms).ok();
            }
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Run a command with a timeout. Returns None if the command fails or times out.
fn run_with_timeout(cmd: &str, args: &[&str], timeout: Duration) -> Option<Vec<u8>> {
    let mut child = std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                return child.stdout.take().and_then(|mut s| {
                    use std::io::Read;
                    let mut buf = Vec::new();
                    s.read_to_end(&mut buf).ok().map(|_| buf)
                });
            }
            Ok(Some(_)) => return None, // exited with error
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

/// Query GitHub for the latest release tag.
fn fetch_latest_version() -> Option<String> {
    let api_url = format!("repos/{}/releases/latest", GITHUB_REPO);

    // Try gh CLI first (authenticated, no rate limits)
    let output = run_with_timeout(
        "gh",
        &["api", &api_url, "--jq", ".tag_name"],
        Duration::from_secs(5),
    )
    .or_else(|| {
        // Fall back to curl
        let url = format!(
            "https://api.github.com/repos/{}/releases/latest",
            GITHUB_REPO
        );
        run_with_timeout(
            "curl",
            &[
                "-sS",
                "--proto",
                "=https",
                "--connect-timeout",
                "3",
                "--max-time",
                "5",
                "--max-filesize",
                "65536",
                "-H",
                "Accept: application/vnd.github+json",
                &url,
            ],
            Duration::from_secs(6),
        )
    })?;

    let stdout = String::from_utf8_lossy(&output);
    let trimmed = stdout.trim().trim_matches('"');

    // Direct tag from gh --jq
    if !trimmed.is_empty() && !trimmed.starts_with('{') {
        let tag = trimmed.strip_prefix('v').unwrap_or(trimmed);
        return Some(tag.to_string());
    }

    // JSON object from curl
    if let Ok(release) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(tag) = release["tag_name"].as_str() {
            return Some(tag.strip_prefix('v').unwrap_or(tag).to_string());
        }
    }

    None
}

/// Compare semver version strings. Returns true if `latest` is newer than `current`.
fn is_newer(current: &str, latest: &str) -> bool {
    let current_base = current.split('-').next().unwrap_or(current);
    let latest_base = latest.split('-').next().unwrap_or(latest);

    let parse = |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse().ok()).collect() };

    let cv = parse(current_base);
    let lv = parse(latest_base);

    for i in 0..cv.len().max(lv.len()) {
        let c = cv.get(i).copied().unwrap_or(0);
        let l = lv.get(i).copied().unwrap_or(0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }

    false
}

fn print_update_notice(latest: &str) {
    let safe = sanitise_for_display(latest);
    eprintln!(
        "\x1b[33mA newer version of tmuch is available (v{}). Run 'tmuch update' to upgrade.\x1b[0m",
        safe
    );
}

/// Non-blocking update check. Shows cached notice immediately, refreshes in background.
pub fn check_for_updates() {
    if std::env::var("TMUCH_NO_UPDATE_CHECK").unwrap_or_default() == "1" {
        return;
    }

    let now = now_secs();
    if let Some((cached_version, timestamp)) = read_cache() {
        if is_newer(CURRENT_VERSION, &cached_version) {
            print_update_notice(&cached_version);
        }
        if now.saturating_sub(timestamp) < COOLDOWN_SECS {
            return;
        }
    }

    std::thread::spawn(|| {
        if let Some(latest) = fetch_latest_version() {
            write_cache(&latest);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_major() {
        assert!(is_newer("0.1.0", "1.0.0"));
    }

    #[test]
    fn test_is_newer_minor() {
        assert!(is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn test_is_newer_patch() {
        assert!(is_newer("0.1.0", "0.1.1"));
    }

    #[test]
    fn test_not_newer_same() {
        assert!(!is_newer("0.1.0", "0.1.0"));
    }

    #[test]
    fn test_not_newer_older() {
        assert!(!is_newer("0.2.0", "0.1.0"));
    }

    #[test]
    fn test_is_newer_empty_strings() {
        assert!(!is_newer("", ""));
        assert!(is_newer("", "1.0.0"));
        assert!(!is_newer("1.0.0", ""));
    }

    #[test]
    fn test_cache_path_exists() {
        assert!(cache_path().is_some());
    }
}
