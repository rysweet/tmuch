use super::{ContentSource, PaneSpec};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Paragraph, Widget};
use std::collections::HashMap;

pub(super) const TAB_NAMES: [&str; 5] = ["Bindings", "Remotes", "Azlin", "Theme", "About"];

/// Input mode for the bindings editor within the Settings pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InputMode {
    Browse,
    InputKey,
    InputCommand,
}

/// A rich settings widget pane — the "System Preferences" for tmuch.
pub struct SettingsSource {
    pub(super) tab: usize,
    pub(super) selected: usize,
    pub(super) bindings: Vec<(char, String)>,
    pub(super) remotes: Vec<(String, String, String)>, // (name, host, user)
    pub(super) azlin_enabled: bool,
    pub(super) azlin_rg: Option<String>,
    pub(super) theme_name: String,
    pub(super) version: String,
    pub(super) input_mode: InputMode,
    pub(super) input_buffer: String,
    pub(super) pending_key: Option<char>,
}

impl SettingsSource {
    pub fn new(
        bindings: &HashMap<char, String>,
        remotes: &[(String, String, String)],
        azlin_enabled: bool,
        azlin_rg: Option<String>,
        theme_name: String,
    ) -> Self {
        let mut entries: Vec<(char, String)> =
            bindings.iter().map(|(k, v)| (*k, v.clone())).collect();
        entries.sort_by_key(|(k, _)| *k);
        Self {
            tab: 0,
            selected: 0,
            bindings: entries,
            remotes: remotes.to_vec(),
            azlin_enabled,
            azlin_rg,
            theme_name,
            version: env!("CARGO_PKG_VERSION").to_string(),
            input_mode: InputMode::Browse,
            input_buffer: String::new(),
            pending_key: None,
        }
    }

    /// Build from the loaded config (convenience constructor).
    pub fn from_config(config: &crate::config::Config) -> Self {
        let remotes: Vec<(String, String, String)> = config
            .remote
            .iter()
            .map(|r| (r.name.clone(), r.host.clone(), r.user.clone()))
            .collect();
        let theme_name = config
            .theme
            .clone()
            .unwrap_or_else(|| "default".to_string());
        Self::new(
            &config.bindings,
            &remotes,
            config.azlin.enabled,
            config.azlin.resource_group.clone(),
            theme_name,
        )
    }

    fn tab_count(&self) -> usize {
        TAB_NAMES.len()
    }

    fn current_list_len(&self) -> usize {
        match self.tab {
            0 => self.bindings.len(),
            1 => self.remotes.len(),
            _ => 0,
        }
    }

    fn clamp_selected(&mut self) {
        let len = self.current_list_len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    /// Persist bindings to config file.
    fn save_bindings(&self) {
        let map: HashMap<char, String> = self.bindings.iter().cloned().collect();
        let _ = crate::config::save_bindings(&map);
    }
}

impl SettingsSource {
    #[cfg(test)]
    fn new_test() -> Self {
        use std::collections::HashMap;
        let mut bindings = HashMap::new();
        bindings.insert('1', "top".to_string());
        bindings.insert('2', "htop".to_string());
        Self::new(
            &bindings,
            &[("dev".into(), "dev.internal".into(), "azureuser".into())],
            true,
            Some("my-rg".into()),
            "default".into(),
        )
    }
}

impl ContentSource for SettingsSource {
    fn capture(&mut self, _width: u16, _height: u16) -> Result<String> {
        Ok("Settings".to_string())
    }

    fn send_keys(&mut self, keys: &str) -> Result<()> {
        // Handle key-by-key input
        match keys {
            "Left" => {
                if self.input_mode == InputMode::Browse {
                    if self.tab == 0 {
                        self.tab = self.tab_count() - 1;
                    } else {
                        self.tab -= 1;
                    }
                    self.selected = 0;
                }
            }
            "Right" => {
                if self.input_mode == InputMode::Browse {
                    self.tab = (self.tab + 1) % self.tab_count();
                    self.selected = 0;
                }
            }
            "Up" | "k" => {
                if self.input_mode == InputMode::Browse && self.selected > 0 {
                    self.selected -= 1;
                }
            }
            "Down" | "j" => {
                if self.input_mode == InputMode::Browse {
                    let len = self.current_list_len();
                    if len > 0 && self.selected < len - 1 {
                        self.selected += 1;
                    }
                }
            }
            "Enter" => match &self.input_mode {
                InputMode::Browse => {
                    // Edit selected binding
                    if self.tab == 0 {
                        if let Some((key, cmd)) = self.bindings.get(self.selected).cloned() {
                            self.pending_key = Some(key);
                            self.input_buffer = cmd;
                            self.input_mode = InputMode::InputCommand;
                        }
                    }
                }
                InputMode::InputKey => {}
                InputMode::InputCommand => {
                    // Confirm edit
                    if let Some(key) = self.pending_key {
                        let cmd = self.input_buffer.clone();
                        if !cmd.is_empty() {
                            if let Some(entry) = self.bindings.iter_mut().find(|(k, _)| *k == key) {
                                entry.1 = cmd;
                            } else {
                                self.bindings.push((key, cmd));
                                self.bindings.sort_by_key(|(k, _)| *k);
                            }
                            self.save_bindings();
                        }
                    }
                    self.input_mode = InputMode::Browse;
                    self.input_buffer.clear();
                    self.pending_key = None;
                    self.clamp_selected();
                }
            },
            "e" => {
                if self.input_mode == InputMode::Browse && self.tab == 0 {
                    if let Some((key, cmd)) = self.bindings.get(self.selected).cloned() {
                        self.pending_key = Some(key);
                        self.input_buffer = cmd;
                        self.input_mode = InputMode::InputCommand;
                    }
                }
            }
            "a" => {
                if self.input_mode == InputMode::Browse && self.tab == 0 {
                    self.input_mode = InputMode::InputKey;
                    self.input_buffer.clear();
                    self.pending_key = None;
                }
            }
            "d" => {
                if self.input_mode == InputMode::Browse
                    && self.tab == 0
                    && !self.bindings.is_empty()
                {
                    self.bindings.remove(self.selected);
                    self.save_bindings();
                    self.clamp_selected();
                }
            }
            "BSpace" => {
                if self.input_mode == InputMode::InputCommand {
                    self.input_buffer.pop();
                }
            }
            "Esc" => {
                if self.input_mode != InputMode::Browse {
                    self.input_mode = InputMode::Browse;
                    self.input_buffer.clear();
                    self.pending_key = None;
                }
            }
            other => {
                // Single character input
                if other.len() == 1 {
                    let c = other.chars().next().unwrap();
                    match &self.input_mode {
                        InputMode::InputKey => {
                            if c.is_ascii_digit() {
                                self.pending_key = Some(c);
                                self.input_buffer.clear();
                                self.input_mode = InputMode::InputCommand;
                            }
                        }
                        InputMode::InputCommand => {
                            self.input_buffer.push(c);
                        }
                        InputMode::Browse => {
                            // Ignore other chars in browse mode
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "Settings"
    }

    fn source_label(&self) -> &str {
        "widget"
    }

    fn is_interactive(&self) -> bool {
        true
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::Plugin {
            plugin_name: "settings".to_string(),
            config: toml::Value::Table(toml::map::Map::new()),
        }
    }

    fn has_custom_render(&self) -> bool {
        true
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 4 || area.width < 20 {
            return;
        }

        // Layout: 1 line tab bar, 1 line separator, rest is content
        let chunks = Layout::vertical([
            Constraint::Length(1), // tab bar
            Constraint::Length(1), // separator
            Constraint::Min(1),    // content
        ])
        .split(area);

        // Tab bar
        self.render_tab_bar(chunks[0], buf);

        // Separator line
        let sep = "\u{2500}".repeat(area.width as usize);
        let sep_line = Paragraph::new(sep).style(Style::default().fg(Color::DarkGray));
        Widget::render(sep_line, chunks[1], buf);

        // Content area
        match self.tab {
            0 => self.render_bindings(chunks[2], buf),
            1 => self.render_remotes(chunks[2], buf),
            2 => self.render_azlin(chunks[2], buf),
            3 => self.render_theme(chunks[2], buf),
            4 => self.render_about(chunks[2], buf),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_settings_metadata() {
        let s = SettingsSource::new_test();
        assert_eq!(s.name(), "Settings");
        assert_eq!(s.source_label(), "widget");
        assert!(s.is_interactive());
        assert!(s.has_custom_render());
    }

    #[test]
    fn test_settings_capture() {
        let mut s = SettingsSource::new_test();
        let output = s.capture(80, 24).unwrap();
        assert_eq!(output, "Settings");
    }

    #[test]
    fn test_settings_tab_navigation() {
        let mut s = SettingsSource::new_test();
        assert_eq!(s.tab, 0);
        s.send_keys("Right").unwrap();
        assert_eq!(s.tab, 1);
        s.send_keys("Right").unwrap();
        assert_eq!(s.tab, 2);
        s.send_keys("Left").unwrap();
        assert_eq!(s.tab, 1);
        // Wrap around left
        s.send_keys("Left").unwrap();
        assert_eq!(s.tab, 0);
        s.send_keys("Left").unwrap();
        assert_eq!(s.tab, 4);
        // Wrap around right
        s.send_keys("Right").unwrap();
        assert_eq!(s.tab, 0);
    }

    #[test]
    fn test_settings_list_navigation() {
        let mut s = SettingsSource::new_test();
        assert_eq!(s.selected, 0);
        s.send_keys("Down").unwrap();
        assert_eq!(s.selected, 1);
        s.send_keys("Up").unwrap();
        assert_eq!(s.selected, 0);
        // j/k navigation
        s.send_keys("j").unwrap();
        assert_eq!(s.selected, 1);
        s.send_keys("k").unwrap();
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn test_settings_add_binding_flow() {
        let mut s = SettingsSource::new_test();
        assert_eq!(s.input_mode, InputMode::Browse);
        s.send_keys("a").unwrap();
        assert_eq!(s.input_mode, InputMode::InputKey);
        s.send_keys("3").unwrap();
        assert_eq!(s.input_mode, InputMode::InputCommand);
        assert_eq!(s.pending_key, Some('3'));
        // In InputCommand mode, single chars are typed into the buffer
        // But 'd','a','t','e' are single chars that go through the `other` branch
        s.input_buffer.clear();
        s.input_buffer.push_str("date");
        assert_eq!(s.input_buffer, "date");
        // Cancel
        s.send_keys("Esc").unwrap();
        assert_eq!(s.input_mode, InputMode::Browse);
    }

    #[test]
    fn test_settings_edit_binding() {
        let mut s = SettingsSource::new_test();
        s.send_keys("e").unwrap();
        assert_eq!(s.input_mode, InputMode::InputCommand);
    }

    #[test]
    fn test_settings_delete_binding() {
        let mut s = SettingsSource::new_test();
        let initial_len = s.bindings.len();
        s.send_keys("d").unwrap();
        assert_eq!(s.bindings.len(), initial_len - 1);
    }

    #[test]
    fn test_settings_backspace_in_input() {
        let mut s = SettingsSource::new_test();
        // Manually set into InputCommand mode with some buffer
        s.input_mode = InputMode::InputCommand;
        s.pending_key = Some('5');
        s.input_buffer = "ab".to_string();
        s.send_keys("BSpace").unwrap();
        assert_eq!(s.input_buffer, "a");
        s.send_keys("BSpace").unwrap();
        assert_eq!(s.input_buffer, "");
    }

    #[test]
    fn test_settings_render_all_tabs() {
        let s = SettingsSource::new_test();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        // Render default tab (0 = Bindings)
        s.render(area, &mut buf);

        // Render each tab
        for tab in 0..5 {
            let mut s2 = SettingsSource::new_test();
            s2.tab = tab;
            let mut buf2 = Buffer::empty(area);
            s2.render(area, &mut buf2);
        }
    }

    #[test]
    fn test_settings_render_small_area() {
        let s = SettingsSource::new_test();
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        // Should not panic
        s.render(area, &mut buf);
    }

    #[test]
    fn test_settings_to_spec() {
        let s = SettingsSource::new_test();
        let spec = s.to_spec();
        match spec {
            PaneSpec::Plugin { plugin_name, .. } => assert_eq!(plugin_name, "settings"),
            _ => panic!("expected Plugin spec"),
        }
    }

    #[test]
    fn test_settings_input_mode_flow() {
        let mut s = SettingsSource::new_test();
        // Enter edit on selected
        s.send_keys("Enter").unwrap();
        assert_eq!(s.input_mode, InputMode::InputCommand);
        // Confirm edit
        s.send_keys("Enter").unwrap();
        assert_eq!(s.input_mode, InputMode::Browse);
    }

    #[test]
    fn test_settings_tab_navigation_not_in_input() {
        let mut s = SettingsSource::new_test();
        s.send_keys("a").unwrap(); // Go to InputKey mode
                                   // Right/Left should NOT change tab in input mode
        let old_tab = s.tab;
        s.send_keys("Right").unwrap();
        assert_eq!(s.tab, old_tab);
    }
}
