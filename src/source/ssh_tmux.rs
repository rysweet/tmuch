//! Remote tmux sessions over SSH using azlin-ssh.
//!
//! Reuses azlin-ssh's production SSH client and connection pool for
//! remote command execution. Background tokio task captures tmux
//! pane content at a configurable interval.

use super::{ContentSource, PaneSpec};
use anyhow::{Context, Result};
use azlin_ssh::{SshConfig, SshPool};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Handle;

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

/// Resolve SSH key path: explicit > ~/.ssh/azlin_key > ~/.ssh/id_rsa
fn resolve_key_path(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        let expanded = shellexpand::tilde(p);
        return Some(PathBuf::from(expanded.as_ref()));
    }
    let home = dirs::home_dir()?;
    let azlin_key = home.join(".ssh/azlin_key");
    if azlin_key.exists() {
        return Some(azlin_key);
    }
    let id_rsa = home.join(".ssh/id_rsa");
    if id_rsa.exists() {
        return Some(id_rsa);
    }
    None
}

/// Convert a RemoteConfig to an azlin-ssh SshConfig.
fn to_ssh_config(remote: &RemoteConfig) -> Result<SshConfig> {
    let key_path = resolve_key_path(remote.key.as_deref())
        .context("No SSH key found (tried ~/.ssh/azlin_key, ~/.ssh/id_rsa)")?;
    let mut config = SshConfig::new(&remote.host, &remote.user, key_path);
    config.port = remote.port;
    Ok(config)
}

/// List tmux sessions on a remote host via azlin-ssh.
pub async fn list_remote_sessions(pool: &SshPool, remote: &RemoteConfig) -> Result<Vec<String>> {
    let ssh_config = to_ssh_config(remote)?;
    let mut client = pool
        .get_or_connect(&ssh_config)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let result = client
        .execute("tmux list-sessions -F '#{session_name}' 2>/dev/null || true")
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    pool.release(client).await;

    Ok(result
        .stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

/// Execute a command on a remote host via the shared pool.
async fn ssh_exec(pool: &SshPool, remote: &RemoteConfig, command: &str) -> Result<String> {
    let ssh_config = to_ssh_config(remote)?;
    let mut client = pool
        .get_or_connect(&ssh_config)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let result = client
        .execute(command)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    pool.release(client).await;
    Ok(result.stdout)
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

pub struct SshTmuxSource {
    remote: RemoteConfig,
    session: String,
    #[allow(dead_code)]
    pool: Arc<SshPool>,
    latest_content: Arc<Mutex<String>>,
    error: Arc<Mutex<Option<String>>>,
    shutdown: Arc<Mutex<bool>>,
    display_name: String,
    label: String,
}

impl SshTmuxSource {
    pub fn new(remote: RemoteConfig, session: String, pool: Arc<SshPool>, rt: &Handle) -> Self {
        let latest_content = Arc::new(Mutex::new(String::new()));
        let error = Arc::new(Mutex::new(None));
        let shutdown = Arc::new(Mutex::new(false));
        let display_name = format!("{}:{}", remote.name, session);
        let label = format!("ssh:{}", remote.name);

        let content_clone = Arc::clone(&latest_content);
        let error_clone = Arc::clone(&error);
        let shutdown_clone = Arc::clone(&shutdown);
        let remote_clone = remote.clone();
        let session_clone = session.clone();
        let pool_clone = Arc::clone(&pool);
        let poll_ms = remote.poll_interval_ms;

        rt.spawn(async move {
            let interval = Duration::from_millis(poll_ms);

            loop {
                if *shutdown_clone.lock().unwrap() {
                    break;
                }

                let cmd = format!(
                    "tmux capture-pane -p -e -t {} 2>/dev/null || echo '[session not found]'",
                    shell_escape(&session_clone)
                );

                match ssh_exec(&pool_clone, &remote_clone, &cmd).await {
                    Ok(output) => {
                        *content_clone.lock().unwrap() = output;
                        *error_clone.lock().unwrap() = None;
                    }
                    Err(e) => {
                        *error_clone.lock().unwrap() = Some(format!("SSH: {}", e));
                    }
                }

                tokio::time::sleep(interval).await;
            }
        });

        Self {
            remote,
            session,
            pool,
            latest_content,
            error,
            shutdown,
            display_name,
            label,
        }
    }
}

impl ContentSource for SshTmuxSource {
    fn capture(&mut self, _width: u16, _height: u16) -> Result<String> {
        if let Some(err) = self.error.lock().unwrap().as_ref() {
            return Ok(format!("[{}]\n\n{}", self.display_name, err));
        }
        Ok(self.latest_content.lock().unwrap().clone())
    }

    fn send_keys(&mut self, keys: &str) -> Result<()> {
        let pool = Arc::clone(&self.pool);
        let remote = self.remote.clone();
        let session = self.session.clone();
        let keys = keys.to_string();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let cmd = format!(
                "tmux send-keys -t {} {}",
                shell_escape(&session),
                shell_escape(&keys)
            );
            let _ = rt.block_on(ssh_exec(&pool, &remote, &cmd));
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
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::Remote {
            remote_name: self.remote.name.clone(),
            session: self.session.clone(),
        }
    }
}
