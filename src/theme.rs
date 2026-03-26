use ratatui::style::Color;
use ratatui::widgets::BorderType;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub border: BorderTheme,
    pub title: TitleTheme,
    pub status_bar: StatusBarTheme,
    pub hints_bar: HintsBarTheme,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct BorderTheme {
    pub focused: String,
    pub focused_attached: String,
    pub unfocused: String,
    pub remote: String,
    pub style: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TitleTheme {
    pub focused: String,
    pub unfocused: String,
    pub attached_label: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct StatusBarTheme {
    pub bg: String,
    pub mode_fg: String,
    pub mode_bg: String,
    pub text: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct HintsBarTheme {
    pub bg: String,
    pub key: String,
    pub label: String,
    pub separator: String,
}

// Theme derives Default from all fields having Default impls

impl Default for BorderTheme {
    fn default() -> Self {
        Self {
            focused: "yellow".into(),
            focused_attached: "green".into(),
            unfocused: "#3c3c3c".into(),
            remote: "#283c50".into(),
            style: "rounded".into(),
        }
    }
}

impl Default for TitleTheme {
    fn default() -> Self {
        Self {
            focused: "white".into(),
            unfocused: "#787878".into(),
            attached_label: "green".into(),
        }
    }
}

impl Default for StatusBarTheme {
    fn default() -> Self {
        Self {
            bg: "black".into(),
            mode_fg: "black".into(),
            mode_bg: "cyan".into(),
            text: "white".into(),
            version: "darkgray".into(),
        }
    }
}

impl Default for HintsBarTheme {
    fn default() -> Self {
        Self {
            bg: "#1e1e1e".into(),
            key: "cyan".into(),
            label: "darkgray".into(),
            separator: "#3c3c3c".into(),
        }
    }
}

impl Theme {
    /// Load theme from ~/.config/tmuch/theme.toml, falling back to defaults.
    pub fn load() -> Self {
        let path = theme_path();
        if path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(theme) = toml::from_str(&contents) {
                    return theme;
                }
            }
        }
        Self::default()
    }
}

fn theme_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tmuch")
        .join("theme.toml")
}

/// Parse a color string into a ratatui Color.
/// Supports "#rrggbb" hex format and named colors.
pub fn parse_color(s: &str) -> Color {
    let s = s.trim();

    // Hex color
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Color::Rgb(r, g, b);
            }
        }
    }

    // Named colors
    match s.to_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" | "dark_gray" | "dark_grey" => Color::DarkGray,
        "lightred" | "light_red" => Color::LightRed,
        "lightgreen" | "light_green" => Color::LightGreen,
        "lightyellow" | "light_yellow" => Color::LightYellow,
        "lightblue" | "light_blue" => Color::LightBlue,
        "lightmagenta" | "light_magenta" => Color::LightMagenta,
        "lightcyan" | "light_cyan" => Color::LightCyan,
        _ => Color::White, // fallback
    }
}

/// Parse a border type string.
pub fn parse_border_type(s: &str) -> BorderType {
    match s.to_lowercase().as_str() {
        "plain" => BorderType::Plain,
        "double" => BorderType::Double,
        "thick" => BorderType::Thick,
        _ => BorderType::Rounded,
    }
}
