use crate::source::PaneSpec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct LayoutSpec {
    pub name: String,
    #[serde(rename = "pane")]
    pub panes: Vec<PaneSpec>,
}

fn layouts_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tmuch")
        .join("layouts")
}

pub fn load(name: &str) -> Result<LayoutSpec> {
    let path = layouts_dir().join(format!("{}.toml", name));
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Layout '{}' not found at {}", name, path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse layout '{}'", name))
}

pub fn save(spec: &LayoutSpec) -> Result<()> {
    let dir = layouts_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.toml", spec.name));
    let content = toml::to_string_pretty(spec)?;
    std::fs::write(&path, content)?;
    eprintln!("Layout saved to {}", path.display());
    Ok(())
}

pub fn list() -> Vec<String> {
    let dir = layouts_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".toml").map(|s| s.to_string())
        })
        .collect()
}
