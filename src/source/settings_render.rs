use super::settings::{InputMode, SettingsSource, TAB_NAMES};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

impl SettingsSource {
    pub(super) fn render_tab_bar(&self, area: Rect, buf: &mut Buffer) {
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

    pub(super) fn render_bindings(&self, area: Rect, buf: &mut Buffer) {
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

    pub(super) fn render_remotes(&self, area: Rect, buf: &mut Buffer) {
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

    pub(super) fn render_azlin(&self, area: Rect, buf: &mut Buffer) {
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

    pub(super) fn render_theme(&self, area: Rect, buf: &mut Buffer) {
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

    pub(super) fn render_about(&self, area: Rect, buf: &mut Buffer) {
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
