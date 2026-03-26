use crate::azlin_integration::AzlinConfig;
use crate::source::ssh_subprocess::RemoteConfig;
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
    let mut config = if path.exists() {
        let contents = std::fs::read_to_string(&path)?;
        toml::from_str(&contents)?
    } else {
        Config::default()
    };

    // If azlin is enabled but no resource_group set, read from ~/.azlin/config.toml
    if config.azlin.enabled && config.azlin.resource_group.is_none() {
        if let Some(azlin_rg) = read_azlin_default_resource_group() {
            config.azlin.resource_group = Some(azlin_rg);
        }
    }
    // Also: if user hasn't configured azlin but has ~/.azlin/config.toml,
    // populate the resource_group so `tmuch azlin` and Ctrl-Z work without
    // requiring explicit config. But don't auto-enable the session picker scan.
    if !config.azlin.enabled && config.azlin.resource_group.is_none() {
        if let Some(azlin_rg) = read_azlin_default_resource_group() {
            config.azlin.resource_group = Some(azlin_rg);
        }
    }

    Ok(config)
}

/// Read default_resource_group from ~/.azlin/config.toml (azlin's native config).
fn read_azlin_default_resource_group() -> Option<String> {
    let path = dirs::home_dir()?.join(".azlin").join("config.toml");
    let contents = std::fs::read_to_string(&path).ok()?;

    // Parse just the fields we need — azlin config has its own schema
    #[derive(Deserialize)]
    struct AzlinNativeConfig {
        default_resource_group: Option<String>,
    }

    let parsed: AzlinNativeConfig = toml::from_str(&contents).ok()?;
    parsed.default_resource_group
}

/// Find the `az` CLI, checking PATH and common install locations.
pub fn find_az_cli() -> Option<String> {
    // Try PATH first
    if std::process::Command::new("az")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
    {
        return Some("az".to_string());
    }
    // Try common locations
    for path in ["/usr/bin/az", "/usr/local/bin/az", "/opt/az/bin/az"] {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

/// Print warnings for common configuration issues.
pub fn validate_warnings(config: &Config) {
    if config.azlin.enabled && find_az_cli().is_none() {
        eprintln!(
            "\x1b[33mWarning: azlin.enabled=true but `az` CLI not found in PATH \
             or /usr/bin/az. Install Azure CLI or disable azlin.\x1b[0m"
        );
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
