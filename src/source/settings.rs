use super::{ContentSource, PaneSpec};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::collections::HashMap;

const TAB_NAMES: [&str; 5] = ["Bindings", "Remotes", "Azlin", "Theme", "About"];

/// Input mode for the bindings editor within the Settings pane.
#[derive(Debug, Clone, PartialEq, Eq)]
enum InputMode {
    Browse,
    InputKey,
    InputCommand,
}

/// A rich settings widget pane — the "System Preferences" for tmuch.
pub struct SettingsSource {
    tab: usize,
    selected: usize,
    bindings: Vec<(char, String)>,
    remotes: Vec<(String, String, String)>, // (name, host, user)
    azlin_enabled: bool,
    azlin_rg: Option<String>,
    theme_name: String,
    version: String,
    input_mode: InputMode,
    input_buffer: String,
    pending_key: Option<char>,
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

    // ---- rendering helpers ----

    fn render_tab_bar(&self, area: Rect, buf: &mut Buffer) {
        let mut spans = Vec::new();
        for (i, name) in TAB_NAMES.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  ", Style::default()));
            }
            let style = if i == self.tab {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(format!(" {} ", name), style));
        }
        let line = Line::from(spans);
        let para = Paragraph::new(line);
        Widget::render(para, area, buf);
    }

    fn render_bindings(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = Vec::new();

        // Header
        lines.push(Line::from(vec![
            Span::styled(
                "  Key  ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Command",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));

        if self.bindings.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No bindings configured. Press [a] to add.",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for (i, (key, cmd)) in self.bindings.iter().enumerate() {
                let cursor = if i == self.selected && self.input_mode == InputMode::Browse {
                    "\u{25b6} "
                } else {
                    "  "
                };
                let style = if i == self.selected && self.input_mode == InputMode::Browse {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                lines.push(Line::from(vec![
                    Span::styled(cursor.to_string(), style),
                    Span::styled(
                        format!("{}  ", key),
                        style.fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(cmd.clone(), style),
                ]));
            }
        }

        // Input area
        lines.push(Line::from(""));
        match &self.input_mode {
            InputMode::Browse => {
                lines.push(Line::from(vec![
                    Span::styled("  [a]", Style::default().fg(Color::Cyan)),
                    Span::styled("dd  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("[e]", Style::default().fg(Color::Cyan)),
                    Span::styled("dit  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("[d]", Style::default().fg(Color::Cyan)),
                    Span::styled("elete", Style::default().fg(Color::DarkGray)),
                ]));
            }
            InputMode::InputKey => {
                lines.push(Line::from(Span::styled(
                    "  Press a digit key (0-9) for the binding:",
                    Style::default().fg(Color::Yellow),
                )));
            }
            InputMode::InputCommand => {
                let key_label = self
                    .pending_key
                    .map(|k| format!("  Key [{}] > ", k))
                    .unwrap_or_else(|| "  > ".to_string());
                lines.push(Line::from(vec![
                    Span::styled(key_label, Style::default().fg(Color::Yellow)),
                    Span::styled(self.input_buffer.clone(), Style::default().fg(Color::White)),
                    Span::styled("\u{2588}", Style::default().fg(Color::Cyan)),
                ]));
                lines.push(Line::from(Span::styled(
                    "  Enter to confirm, Esc to cancel",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        let para = Paragraph::new(lines);
        Widget::render(para, area, buf);
    }

    fn render_remotes(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled(
                "  Name       ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Host             ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "User",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));

        if self.remotes.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No remotes configured. Add [[remote]] to config.toml.",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for (i, (name, host, user)) in self.remotes.iter().enumerate() {
                let cursor = if i == self.selected {
                    "\u{25b6} "
                } else {
                    "  "
                };
                let style = if i == self.selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                lines.push(Line::from(vec![
                    Span::styled(cursor.to_string(), style),
                    Span::styled(format!("{:<11}", name), style.fg(Color::Cyan)),
                    Span::styled(format!("{:<17}", host), style),
                    Span::styled(user.clone(), style),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Read-only. Edit ~/.config/tmuch/config.toml to manage remotes.",
            Style::default().fg(Color::DarkGray),
        )));

        let para = Paragraph::new(lines);
        Widget::render(para, area, buf);
    }

    fn render_azlin(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = Vec::new();

        let status = if self.azlin_enabled {
            Span::styled(
                "  Enabled",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "  Disabled",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        };

        lines.push(Line::from(vec![
            Span::styled(
                "  Status:  ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            status,
        ]));
        lines.push(Line::from(""));

        let rg_display = self.azlin_rg.as_deref().unwrap_or("(not set)");
        lines.push(Line::from(vec![
            Span::styled(
                "  Resource Group:  ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(rg_display.to_string(), Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Azlin integrates with Azure VM discovery.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  When enabled, Ctrl-G discovers VMs and their tmux sessions.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Edit ~/.config/tmuch/config.toml [azlin] section to configure.",
            Style::default().fg(Color::DarkGray),
        )));

        let para = Paragraph::new(lines);
        Widget::render(para, area, buf);
    }

    fn render_theme(&self, area: Rect, buf: &mut Buffer) {
        let theme = crate::theme::Theme::load();
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled(
                "  Theme: ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                self.theme_name.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));

        // Color swatches for borders
        lines.push(Line::from(Span::styled(
            "  Borders",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        let swatch = |label: &str, color_str: &str| -> Line {
            let c = crate::theme::parse_color(color_str);
            Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled("\u{2588}\u{2588}\u{2588}", Style::default().fg(c)),
                Span::styled(
                    format!("  {} ({})", label, color_str),
                    Style::default().fg(Color::Gray),
                ),
            ])
        };
        lines.push(swatch("Focused", &theme.border.focused));
        lines.push(swatch("Focused+Attached", &theme.border.focused_attached));
        lines.push(swatch("Unfocused", &theme.border.unfocused));
        lines.push(swatch("Remote", &theme.border.remote));
        lines.push(Line::from(""));

        // Title colors
        lines.push(Line::from(Span::styled(
            "  Titles",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(swatch("Focused", &theme.title.focused));
        lines.push(swatch("Unfocused", &theme.title.unfocused));
        lines.push(swatch("Attached", &theme.title.attached_label));
        lines.push(Line::from(""));

        // Status bar
        lines.push(Line::from(Span::styled(
            "  Status Bar",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(swatch("Background", &theme.status_bar.bg));
        lines.push(swatch("Mode FG", &theme.status_bar.mode_fg));
        lines.push(swatch("Mode BG", &theme.status_bar.mode_bg));
        lines.push(swatch("Text", &theme.status_bar.text));
        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            "  Edit ~/.config/tmuch/theme.toml to customize.",
            Style::default().fg(Color::DarkGray),
        )));

        let para = Paragraph::new(lines);
        Widget::render(para, area, buf);
    }

    fn render_about(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled(
                "  tmuch ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("v{}", self.version),
                Style::default().fg(Color::White),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            "  TUI tmux multiplexer",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            "  Keybindings",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        let kb = |key: &str, desc: &str| -> Line {
            Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(format!("{:<16}", key), Style::default().fg(Color::Cyan)),
                Span::styled(desc.to_string(), Style::default().fg(Color::Gray)),
            ])
        };
        lines.push(kb("Tab / Arrows", "Navigate panes"));
        lines.push(kb("Enter", "Focus pane (attach)"));
        lines.push(kb("Esc", "Exit pane focus"));
        lines.push(kb("Ctrl-A", "Add new pane"));
        lines.push(kb("Ctrl-D", "Drop focused pane"));
        lines.push(kb("Ctrl-S", "Session picker"));
        lines.push(kb("Ctrl-N", "App launcher"));
        lines.push(kb("Ctrl-E", "Settings"));
        lines.push(kb("Ctrl-V", "Split vertical"));
        lines.push(kb("Ctrl-H", "Split horizontal"));
        lines.push(kb("Ctrl-F / F11", "Toggle maximize"));
        lines.push(kb("Ctrl-X", "Swap pane"));
        lines.push(kb("Ctrl-G", "Discover azlin VMs"));
        lines.push(kb("Ctrl-Q / q", "Quit"));
        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            "  https://github.com/pact-im/tmuch",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
        )));

        let para = Paragraph::new(lines);
        Widget::render(para, area, buf);
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
