//! Remote tmux sessions via SSH subprocess.
//!
//! Uses the system `ssh` command instead of russh, which supports
//! all auth methods (SSH agent, AAD, keys) without requiring an
//! explicit key file. Background thread captures tmux pane content.

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
    "azureuser".into()
}
fn default_port() -> u16 {
    22
}
fn default_poll() -> u64 {
    500
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

        let content_clone = Arc::clone(&latest_content);
        let error_clone = Arc::clone(&error);
        let shutdown_clone = Arc::clone(&shutdown);
        let host_clone = host.clone();
        let user_clone = user.clone();
        let session_clone = session.clone();

        std::thread::spawn(move || {
            let interval = Duration::from_millis(poll_interval_ms);

            loop {
                if *shutdown_clone.lock().unwrap() {
                    break;
                }

                // Capture without resizing — resizing remote windows causes
                // them to stay tiny after tmuch exits (issue #14)
                let cmd = format!(
                    "tmux capture-pane -p -e -t {} 2>/dev/null || echo '[session not found]'",
                    shell_escape(&session_clone)
                );

                let result = run_ssh_command(&host_clone, &user_clone, port, &cmd);

                match result {
                    Ok(output) => {
                        *content_clone.lock().unwrap() = output;
                        *error_clone.lock().unwrap() = None;
                    }
                    Err(e) => {
                        *error_clone.lock().unwrap() = Some(format!("{}", e));
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
    let mut args = vec![
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

fn shell_escape(s: &str) -> String {
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
        if let Some(err) = self.error.lock().unwrap().as_ref() {
            return Ok(format!("[{}]\n\n{}\n\nRetrying...", self.display_name, err));
        }
        Ok(self.latest_content.lock().unwrap().clone())
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
        *self.shutdown.lock().unwrap() = true;

        // Restore remote window to automatic size
        let cmd = format!(
            "tmux resize-window -t {} -A 2>/dev/null; true",
            shell_escape(&self.session)
        );
        let _ = run_ssh_command(&self.host, &self.user, self.port, &cmd);
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::Remote {
            remote_name: self.display_name.clone(),
            session: self.session.clone(),
        }
    }
}
