//! Non-blocking update check with 24-hour cooldown.
//!
//! Checks GitHub releases for newer versions and prints a one-line notice.
//! Failures are silently ignored — never blocks or slows normal operation.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const COOLDOWN_SECS: u64 = 86400; // 24 hours
const GITHUB_REPO: &str = "rysweet/tmuch";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

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

/// Query GitHub for the latest release tag.
/// Uses `gh` CLI first (authenticated), falls back to `curl`.
fn fetch_latest_version() -> Option<String> {
    let output = std::process::Command::new("timeout")
        .args([
            "5",
            "gh",
            "api",
            &format!("repos/{}/releases/latest", GITHUB_REPO),
            "--jq",
            ".tag_name",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .or_else(|| {
            std::process::Command::new("curl")
                .args([
                    "-sS",
                    "--connect-timeout",
                    "3",
                    "--max-time",
                    "5",
                    "--max-filesize",
                    "65536",
                    "-H",
                    "Accept: application/vnd.github+json",
                    &format!(
                        "https://api.github.com/repos/{}/releases/latest",
                        GITHUB_REPO
                    ),
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim().trim_matches('"');

    // Direct tag from gh --jq
    if !trimmed.is_empty() && !trimmed.starts_with('{') {
        let tag = trimmed.strip_prefix('v').unwrap_or(trimmed);
        return Some(tag.to_string());
    }

    // JSON object from curl (releases/latest returns a single object)
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

fn sanitise_for_display(s: &str) -> String {
    s.chars().filter(|c| !c.is_ascii_control()).collect()
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

/// Interactive update check. Returns newer version string or None.
#[allow(dead_code)] // available for future interactive prompt integration
pub fn check_for_updates_interactive() -> Option<String> {
    if std::env::var("TMUCH_NO_UPDATE_CHECK").unwrap_or_default() == "1" {
        return None;
    }

    let now = now_secs();

    if let Some((cached_version, timestamp)) = read_cache() {
        if now.saturating_sub(timestamp) < COOLDOWN_SECS {
            if is_newer(CURRENT_VERSION, &cached_version) {
                return Some(cached_version);
            }
            return None;
        }
    }

    if let Some(latest) = fetch_latest_version() {
        write_cache(&latest);
        if is_newer(CURRENT_VERSION, &latest) {
            return Some(latest);
        }
    }

    None
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
    fn test_sanitise_strips_escape_sequences() {
        assert_eq!(sanitise_for_display("1.0.0\x1b[2J"), "1.0.0[2J");
    }

    #[test]
    fn test_sanitise_passes_normal_version() {
        assert_eq!(sanitise_for_display("0.2.0"), "0.2.0");
    }

    #[test]
    fn test_cache_path_exists() {
        assert!(cache_path().is_some());
    }

    #[test]
    fn test_current_version_valid() {
        assert!(!CURRENT_VERSION.is_empty());
        assert!(CURRENT_VERSION.contains('.'));
    }
}
