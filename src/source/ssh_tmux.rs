//! Remote tmux sessions over SSH using russh.
//!
//! Captures remote tmux pane content on a background tokio task at a configurable
//! interval (default 500ms). The main render loop reads from a shared buffer
//! without blocking.

use super::{ContentSource, PaneSpec};
use anyhow::{Context, Result};
use russh::client;
use russh_keys::key::PrivateKeyWithHashAlg;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Handle;

/// Configuration for a remote SSH host.
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
fn resolve_key_path(explicit: Option<&str>) -> Option<std::path::PathBuf> {
    if let Some(p) = explicit {
        let expanded = shellexpand::tilde(p);
        return Some(std::path::PathBuf::from(expanded.as_ref()));
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

/// SSH client handler for russh (same pattern as azlin-ssh).
struct SshHandler;

#[async_trait::async_trait]
impl client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Execute a command over SSH and return stdout.
async fn ssh_exec(config: &RemoteConfig, command: &str) -> Result<String> {
    let key_path = resolve_key_path(config.key.as_deref())
        .context("No SSH key found (tried ~/.ssh/azlin_key, ~/.ssh/id_rsa)")?;

    let key_pair =
        russh_keys::load_secret_key(&key_path, None).context("Failed to load SSH key")?;

    let ssh_config = client::Config {
        ..Default::default()
    };

    let mut session = client::connect(
        Arc::new(ssh_config),
        (config.host.as_str(), config.port),
        SshHandler,
    )
    .await
    .context(format!(
        "SSH connect to {}:{} failed",
        config.host, config.port
    ))?;

    let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key_pair), None)
        .context("Failed to create key with hash algorithm")?;

    let auth_ok = session
        .authenticate_publickey(&config.user, key_with_alg)
        .await
        .context("SSH auth failed")?;

    if !auth_ok {
        anyhow::bail!(
            "SSH authentication rejected by {}@{}",
            config.user,
            config.host
        );
    }

    let mut channel = session.channel_open_session().await?;
    channel.exec(true, command).await?;

    let mut stdout = Vec::new();
    while let Some(msg) = channel.wait().await {
        match msg {
            russh::ChannelMsg::Data { data } => {
                stdout.extend_from_slice(&data);
            }
            russh::ChannelMsg::Eof => break,
            _ => {}
        }
    }

    Ok(String::from_utf8_lossy(&stdout).to_string())
}

/// List tmux sessions on a remote host.
pub async fn list_remote_sessions(config: &RemoteConfig) -> Result<Vec<String>> {
    let output = ssh_exec(
        config,
        "tmux list-sessions -F '#{session_name}' 2>/dev/null || true",
    )
    .await?;

    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

pub struct SshTmuxSource {
    remote: RemoteConfig,
    session: String,
    latest_content: Arc<Mutex<String>>,
    error: Arc<Mutex<Option<String>>>,
    shutdown: Arc<Mutex<bool>>,
    display_name: String,
    label: String,
}

impl SshTmuxSource {
    pub fn new(remote: RemoteConfig, session: String, rt: &Handle) -> Self {
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
        let poll_ms = remote.poll_interval_ms;

        // Spawn background capture task
        rt.spawn(async move {
            let interval = Duration::from_millis(poll_ms);
            // Store dimensions from last capture request
            let width = Arc::new(Mutex::new(80u16));
            let height = Arc::new(Mutex::new(24u16));

            loop {
                if *shutdown_clone.lock().unwrap() {
                    break;
                }

                let w = *width.lock().unwrap();
                let h = *height.lock().unwrap();

                let cmd = format!(
                    "tmux resize-window -t {} -x {} -y {} 2>/dev/null; tmux capture-pane -p -e -t {}",
                    shell_escape(&session_clone), w, h, shell_escape(&session_clone)
                );

                match ssh_exec(&remote_clone, &cmd).await {
                    Ok(output) => {
                        *content_clone.lock().unwrap() = output;
                        *error_clone.lock().unwrap() = None;
                    }
                    Err(e) => {
                        *error_clone.lock().unwrap() = Some(format!("SSH error: {}", e));
                    }
                }

                tokio::time::sleep(interval).await;
            }
        });

        Self {
            remote,
            session,
            latest_content,
            error,
            shutdown,
            display_name,
            label,
        }
    }
}

fn shell_escape(s: &str) -> String {
    // Simple shell escape for tmux session names (alphanumeric + dash + underscore)
    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

impl ContentSource for SshTmuxSource {
    fn capture(&mut self, _width: u16, _height: u16) -> Result<String> {
        // Check for errors first
        if let Some(err) = self.error.lock().unwrap().as_ref() {
            return Ok(format!("[{}]\n\n{}", self.display_name, err));
        }
        Ok(self.latest_content.lock().unwrap().clone())
    }

    fn send_keys(&mut self, keys: &str) -> Result<()> {
        // Fire-and-forget: send keys to remote session
        let remote = self.remote.clone();
        let session = self.session.clone();
        let keys = keys.to_string();

        // Use a blocking spawn since we may not be in an async context
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
            let _ = rt.block_on(ssh_exec(&remote, &cmd));
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
