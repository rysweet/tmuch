use anyhow::{Context, Result};
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct SessionInfo {
    pub name: String,
    pub attached: bool,
    /// None for local sessions, Some("hostname") for remote
    pub host: Option<String>,
}

fn run_tmux(args: &[&str]) -> Result<String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .context("failed to execute tmux")?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux {} failed: {}", args.join(" "), stderr.trim());
    }
}

pub fn list_sessions() -> Result<Vec<SessionInfo>> {
    let output = match run_tmux(&[
        "list-sessions",
        "-F",
        "#{session_name}\t#{session_attached}",
    ]) {
        Ok(o) => o,
        Err(_) => return Ok(Vec::new()), // no server running
    };
    let mut sessions = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            sessions.push(SessionInfo {
                name: parts[0].to_string(),
                attached: parts[1] != "0",
                host: None,
            });
        }
    }
    Ok(sessions)
}

pub fn create_session(name: &str, command: Option<&str>) -> Result<String> {
    let mut args = vec!["new-session", "-d", "-s", name, "-x", "80", "-y", "24"];
    if let Some(cmd) = command {
        args.push(cmd);
    }
    run_tmux(&args)?;
    Ok(name.to_string())
}

pub fn kill_session(name: &str) -> Result<()> {
    run_tmux(&["kill-session", "-t", name])?;
    Ok(())
}

pub fn capture_pane(session: &str, width: u16, height: u16) -> Result<String> {
    // Resize the tmux window to match our pane dimensions
    let w = width.to_string();
    let h = height.to_string();
    let _ = run_tmux(&["resize-window", "-t", session, "-x", &w, "-y", &h]);

    // Capture with ANSI escape codes preserved
    run_tmux(&["capture-pane", "-p", "-e", "-t", session])
}

pub fn send_keys(session: &str, keys: &str) -> Result<()> {
    run_tmux(&["send-keys", "-t", session, keys])?;
    Ok(())
}

pub fn session_exists(name: &str) -> bool {
    run_tmux(&["has-session", "-t", name]).is_ok()
}

/// Generate a unique session name with the tmuch- prefix
pub fn generate_session_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("tmuch-{}", ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_name_uniqueness() {
        let a = generate_session_name();
        // Small sleep to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = generate_session_name();
        assert!(a.starts_with("tmuch-"));
        assert!(b.starts_with("tmuch-"));
        assert_ne!(a, b, "Generated names should be unique");
    }
}
