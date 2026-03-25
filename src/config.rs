use crate::azlin_integration::AzlinConfig;
use crate::source::ssh_tmux::RemoteConfig;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub keys: KeyConfig,
    pub bindings: HashMap<char, String>,
    pub display: DisplayConfig,
    #[serde(default)]
    pub remote: Vec<RemoteConfig>,
    #[serde(default)]
    pub azlin: AzlinConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct KeyConfig {
    pub quit: String,
    pub add_pane: String,
    pub drop_pane: String,
    pub next_pane: String,
    pub prev_pane: String,
    pub select_session: String,
    pub enter_pane: String,
    pub exit_pane: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub poll_interval_ms: u64,
    pub border_style: String,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            quit: "Ctrl-q".into(),
            add_pane: "Ctrl-a".into(),
            drop_pane: "Ctrl-d".into(),
            next_pane: "Tab".into(),
            prev_pane: "Shift-Tab".into(),
            select_session: "Ctrl-s".into(),
            enter_pane: "Enter".into(),
            exit_pane: "Esc".into(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 150,
            border_style: "rounded".into(),
        }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tmuch")
        .join("config.toml")
}

pub fn load() -> Result<Config> {
    let path = config_path();
    if path.exists() {
        let contents = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}

/// Print warnings for common configuration issues.
pub fn validate_warnings(config: &Config) {
    // Warn if azlin enabled but az CLI not found
    if config.azlin.enabled
        && std::process::Command::new("az")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_err()
    {
        eprintln!("\x1b[33mWarning: azlin.enabled=true but `az` CLI not found in PATH.\x1b[0m");
    }

    // Warn if remote hosts configured but no SSH key found
    if !config.remote.is_empty() {
        let has_key = config.remote.iter().any(|r| r.key.is_some())
            || dirs::home_dir()
                .map(|h| {
                    h.join(".ssh/azlin_key").exists()
                        || h.join(".ssh/id_rsa").exists()
                        || h.join(".ssh/id_ed25519").exists()
                })
                .unwrap_or(false);
        if !has_key {
            eprintln!(
                "\x1b[33mWarning: remote hosts configured but no SSH key found \
                 (~/.ssh/azlin_key, ~/.ssh/id_rsa, ~/.ssh/id_ed25519).\x1b[0m"
            );
        }
    }
}
