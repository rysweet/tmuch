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

fn validate_layout_name(name: &str) -> Result<()> {
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.is_empty() {
        anyhow::bail!("Invalid layout name: must not contain path separators or '..'");
    }
    Ok(())
}

pub fn load(name: &str) -> Result<LayoutSpec> {
    validate_layout_name(name)?;
    let path = layouts_dir().join(format!("{}.toml", name));
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Layout '{}' not found at {}", name, path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse layout '{}'", name))
}

pub fn save(spec: &LayoutSpec) -> Result<()> {
    validate_layout_name(&spec.name)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_layout_name_valid() {
        assert!(validate_layout_name("my-layout").is_ok());
        assert!(validate_layout_name("layout_2").is_ok());
    }

    #[test]
    fn test_validate_layout_name_empty() {
        assert!(validate_layout_name("").is_err());
    }

    #[test]
    fn test_validate_layout_name_slash() {
        assert!(validate_layout_name("../etc/passwd").is_err());
        assert!(validate_layout_name("foo/bar").is_err());
    }

    #[test]
    fn test_validate_layout_name_backslash() {
        assert!(validate_layout_name("foo\\bar").is_err());
    }

    #[test]
    fn test_validate_layout_name_dotdot() {
        assert!(validate_layout_name("..").is_err());
        assert!(validate_layout_name("foo..bar").is_err());
    }

    #[test]
    fn test_load_rejects_traversal() {
        assert!(load("../../../etc/passwd").is_err());
    }

    #[test]
    fn test_list_returns_vec() {
        // Just verify it doesn't panic
        let _ = list();
    }

    #[test]
    fn test_validate_layout_name_rejects_special_chars() {
        // Names with embedded dots are rejected
        assert!(validate_layout_name("a..b").is_err());
        // But normal names with hyphens/underscores pass
        assert!(validate_layout_name("my_layout-2").is_ok());
    }

    #[test]
    fn test_list_returns_string_names() {
        let names = list();
        // All returned names should be non-empty strings (no .toml suffix)
        for name in &names {
            assert!(!name.is_empty());
            assert!(!name.ends_with(".toml"));
        }
    }
}
