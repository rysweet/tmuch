//! Remote tmux sessions via SSH subprocess.
//!
//! Uses the system `ssh` command instead of russh, which supports
//! all auth methods (SSH agent, AAD, keys) without requiring an
//! explicit key file. Background thread captures tmux pane content.
//!
//! An SSH ControlMaster connection is established once per remote host
//! and reused for all subsequent commands (capture, send-keys, cleanup).

use super::{ContentSource, PaneSpec};
use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Configuration for a remote SSH host (from config.toml).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteConfig {
    pub name: String,
    pub host: String,
    #[serde(default = "default_user")]
    pub user: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_poll")]
    pub poll_interval_ms: u64,
}

fn default_user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "azureuser".into())
}
fn default_port() -> u16 {
    22
}
fn default_poll() -> u64 {
    500
}

/// Build the ControlPath string for a given user@host:port.
fn control_path(user: &str, host: &str, port: u16) -> String {
    format!("/tmp/tmuch-ssh-{}@{}:{}", user, host, port)
}

/// Establish (or reuse) an SSH ControlMaster connection.
/// The `-f` flag backgrounds it after authentication so the call returns quickly.
fn establish_control_master(host: &str, user: &str, port: u16) {
    let cp = control_path(user, host, port);

    // Check if the master is already alive
    let check = std::process::Command::new("ssh")
        .args([
            "-o",
            &format!("ControlPath={}", cp),
            "-O",
            "check",
            &format!("{}@{}", user, host),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    if let Ok(status) = check {
        if status.success() {
            return; // master already running
        }
    }

    let mut args = vec![
        "-o".to_string(),
        "ControlMaster=auto".to_string(),
        "-o".to_string(),
        format!("ControlPath={}", cp),
        "-o".to_string(),
        "ControlPersist=600".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "ConnectTimeout=10".to_string(),
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-N".to_string(), // no remote command
        "-f".to_string(), // background after auth
        "-p".to_string(),
        port.to_string(),
    ];

    if let Some(key) = find_ssh_key() {
        args.push("-i".to_string());
        args.push(key);
    }

    args.push(format!("{}@{}", user, host));

    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let _ = std::process::Command::new("ssh")
        .args(&str_args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Tear down an SSH ControlMaster connection.
fn teardown_control_master(host: &str, user: &str, port: u16) {
    let cp = control_path(user, host, port);
    let _ = std::process::Command::new("ssh")
        .args([
            "-o",
            &format!("ControlPath={}", cp),
            "-O",
            "exit",
            &format!("{}@{}", user, host),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

pub struct SshSubprocessSource {
    host: String,
    user: String,
    port: u16,
    session: String,
    latest_content: Arc<Mutex<String>>,
    error: Arc<Mutex<Option<String>>>,
    shutdown: Arc<Mutex<bool>>,
    display_name: String,
    label: String,
}

impl SshSubprocessSource {
    pub fn new(
        name: String,
        host: String,
        user: String,
        port: u16,
        session: String,
        poll_interval_ms: u64,
    ) -> Self {
        let latest_content = Arc::new(Mutex::new(String::new()));
        let error = Arc::new(Mutex::new(None));
        let shutdown = Arc::new(Mutex::new(false));
        let display_name = format!("{}:{}", name, session);
        let label = format!("ssh:{}", name);

        // Establish persistent ControlMaster connection
        establish_control_master(&host, &user, port);

        let content_clone = Arc::clone(&latest_content);
        let error_clone = Arc::clone(&error);
        let shutdown_clone = Arc::clone(&shutdown);
        let host_clone = host.clone();
        let user_clone = user.clone();
        let session_clone = session.clone();

        std::thread::spawn(move || {
            let interval = Duration::from_millis(poll_interval_ms);

            loop {
                if *shutdown_clone.lock().unwrap_or_else(|e| e.into_inner()) {
                    break;
                }

                // Capture without resizing -- resizing remote windows causes
                // them to stay tiny after tmuch exits (issue #14)
                let cmd = format!(
                    "tmux capture-pane -p -e -t {} 2>/dev/null || echo '[session not found]'",
                    shell_escape(&session_clone)
                );

                let result = run_ssh_command(&host_clone, &user_clone, port, &cmd);

                match result {
                    Ok(output) => {
                        *content_clone.lock().unwrap_or_else(|e| e.into_inner()) = output;
                        *error_clone.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    }
                    Err(e) => {
                        *error_clone.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(format!("{}", e));
                    }
                }

                std::thread::sleep(interval);
            }
        });

        Self {
            host,
            user,
            port,
            session,
            latest_content,
            error,
            shutdown,
            display_name,
            label,
        }
    }
}

fn find_ssh_key() -> Option<String> {
    let home = dirs::home_dir()?;
    for name in ["azlin_key", "id_ed25519", "id_rsa"] {
        let path = home.join(".ssh").join(name);
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

fn run_ssh_command(host: &str, user: &str, port: u16, command: &str) -> Result<String> {
    let cp = control_path(user, host, port);

    let mut args = vec![
        "-o".to_string(),
        format!("ControlPath={}", cp),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "ConnectTimeout=10".to_string(),
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-p".to_string(),
        port.to_string(),
    ];

    if let Some(key) = find_ssh_key() {
        args.push("-i".to_string());
        args.push(key);
    }

    args.push(format!("{}@{}", user, host));
    args.push(command.to_string());

    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = std::process::Command::new("ssh").args(&str_args).output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("SSH failed: {}", stderr.trim())
    }
}

pub(crate) fn shell_escape(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// List tmux sessions on a remote host via SSH subprocess.
pub fn list_remote_sessions(remote: &RemoteConfig) -> Result<Vec<String>> {
    let cmd = "tmux list-sessions -F '#{session_name}' 2>/dev/null || true";
    let output = run_ssh_command(&remote.host, &remote.user, remote.port, cmd)?;
    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

impl ContentSource for SshSubprocessSource {
    fn capture(&mut self, _width: u16, _height: u16) -> Result<String> {
        if let Some(err) = self
            .error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            return Ok(format!("[{}]\n\n{}\n\nRetrying...", self.display_name, err));
        }
        Ok(self
            .latest_content
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone())
    }

    fn send_keys(&mut self, keys: &str) -> Result<()> {
        let host = self.host.clone();
        let user = self.user.clone();
        let port = self.port;
        let session = self.session.clone();
        let keys = keys.to_string();

        std::thread::spawn(move || {
            let cmd = format!(
                "tmux send-keys -t {} {}",
                shell_escape(&session),
                shell_escape(&keys)
            );
            let _ = run_ssh_command(&host, &user, port, &cmd);
        });
        Ok(())
    }

    fn name(&self) -> &str {
        &self.display_name
    }

    fn source_label(&self) -> &str {
        &self.label
    }

    fn is_interactive(&self) -> bool {
        true
    }

    fn cleanup(&mut self) {
        *self.shutdown.lock().unwrap_or_else(|e| e.into_inner()) = true;

        // Restore remote window to automatic size
        let cmd = format!(
            "tmux resize-window -t {} -A 2>/dev/null; true",
            shell_escape(&self.session)
        );
        let _ = run_ssh_command(&self.host, &self.user, self.port, &cmd);

        // Tear down the ControlMaster connection
        teardown_control_master(&self.host, &self.user, self.port);
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::Remote {
            remote_name: self.display_name.clone(),
            session: self.session.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_escape_simple() {
        assert_eq!(shell_escape("hello"), "hello");
        assert_eq!(shell_escape("my-session"), "my-session");
        assert_eq!(shell_escape("test_name"), "test_name");
    }

    #[test]
    fn test_shell_escape_special() {
        assert_eq!(shell_escape("hello world"), "'hello world'");
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
        assert_eq!(shell_escape("a;b"), "'a;b'");
    }

    #[test]
    fn test_control_path() {
        let cp = control_path("user", "host.example.com", 22);
        assert_eq!(cp, "/tmp/tmuch-ssh-user@host.example.com:22");
    }

    #[test]
    fn test_control_path_custom_port() {
        let cp = control_path("admin", "10.0.0.1", 2222);
        assert_eq!(cp, "/tmp/tmuch-ssh-admin@10.0.0.1:2222");
    }

    #[test]
    fn test_default_user() {
        let u = default_user();
        assert!(!u.is_empty());
    }

    #[test]
    fn test_default_port() {
        assert_eq!(default_port(), 22);
    }

    #[test]
    fn test_default_poll() {
        assert_eq!(default_poll(), 500);
    }

    #[test]
    fn test_remote_config_defaults() {
        let config = RemoteConfig {
            name: "test".into(),
            host: "example.com".into(),
            user: default_user(),
            key: None,
            port: default_port(),
            poll_interval_ms: default_poll(),
        };
        assert_eq!(config.port, 22);
        assert_eq!(config.poll_interval_ms, 500);
        assert!(config.key.is_none());
    }
}
