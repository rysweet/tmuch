//! Hints bar and status bar rendering for the TUI.

use crate::app::App;
use crate::editor_state::EditorInputMode;
use crate::keys::Mode;
use crate::theme::parse_color;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

fn sep() -> Span<'static> {
    Span::styled(" \u{2502} ", Style::default().fg(Color::Rgb(50, 50, 50)))
}

/// Each hotkey group gets a distinct color so they're easy to scan visually.
fn hint(key: &'static str, label: &'static str, color: Color) -> Vec<Span<'static>> {
    vec![
        Span::styled(key, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled(label, Style::default().fg(Color::Rgb(120, 120, 120))),
    ]
}

pub fn draw_hints_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![Span::raw(" ")];

    match app.mode {
        Mode::Normal => {
            spans.extend(hint("q", " Quit", Color::Red));
            spans.push(sep());
            spans.extend(hint("^A", " Add", Color::Green));
            spans.push(sep());
            spans.extend(hint("^D", " Drop", Color::Red));
            spans.push(sep());
            spans.extend(hint("^S", " Sessions", Color::Cyan));
            spans.push(sep());
            spans.extend(hint("^G", " Azlin", Color::Blue));
            spans.push(sep());
            spans.extend(hint("^E", " Edit Cmds", Color::Magenta));
            spans.push(sep());
            spans.extend(hint("Tab", " Next", Color::Yellow));
            spans.push(sep());
            spans.extend(hint("Enter", " Focus", Color::Green));
            spans.push(sep());
            spans.extend(hint("^V/^H", " Split", Color::Cyan));
            spans.push(sep());
            spans.extend(hint("F11", " Max", Color::Yellow));
            spans.push(sep());
            spans.extend(hint("^N", " Apps", Color::LightCyan));
            spans.push(sep());
            spans.extend(hint("1-9", " Bindings", Color::Magenta));
        }
        Mode::PaneFocused => {
            spans.extend(hint("Esc", " Unfocus", Color::Red));
            spans.push(sep());
            spans.push(Span::styled(
                "All keys forwarded to session",
                Style::default().fg(Color::Rgb(100, 100, 100)),
            ));
        }
        Mode::SessionPicker => {
            spans.extend(hint("\u{2191}\u{2193}/jk", " Nav", Color::Yellow));
            spans.push(sep());
            spans.extend(hint("Enter", " Select", Color::Green));
            spans.push(sep());
            spans.extend(hint("a", " Add All", Color::Cyan));
            spans.push(sep());
            spans.extend(hint("z", " Scan Azlin", Color::Blue));
            spans.push(sep());
            spans.extend(hint("Esc", " Cancel", Color::Red));
        }
        Mode::CommandEditor => {
            let input_mode = app.editor_input_mode();
            match input_mode {
                EditorInputMode::Browse => {
                    spans.extend(hint("\u{2191}\u{2193}", " Nav", Color::Yellow));
                    spans.push(sep());
                    spans.extend(hint("a", " Add", Color::Green));
                    spans.push(sep());
                    spans.extend(hint("e/Enter", " Edit", Color::Cyan));
                    spans.push(sep());
                    spans.extend(hint("d", " Delete", Color::Red));
                    spans.push(sep());
                    spans.extend(hint("Esc", " Close", Color::Red));
                }
                EditorInputMode::InputKey => {
                    spans.extend(hint("0-9", " Press a key", Color::Green));
                    spans.push(sep());
                    spans.extend(hint("Esc", " Cancel", Color::Red));
                }
                EditorInputMode::InputCommand => {
                    spans.extend(hint("Enter", " Save", Color::Green));
                    spans.push(sep());
                    spans.extend(hint("Esc", " Cancel", Color::Red));
                    spans.push(sep());
                    spans.push(Span::styled(
                        "Type command...",
                        Style::default().fg(Color::Rgb(80, 80, 80)),
                    ));
                }
            }
        }
        Mode::AppLauncher => {
            spans.extend(hint("\u{2191}\u{2193}/jk", " Nav", Color::Yellow));
            spans.push(sep());
            spans.extend(hint("Enter", " Launch", Color::Green));
            spans.push(sep());
            spans.extend(hint("Esc", " Cancel", Color::Red));
        }
    }

    let line = Line::from(spans);
    let bar = Paragraph::new(line).style(Style::default().bg(parse_color(&app.theme.hints_bar.bg)));
    frame.render_widget(bar, area);
}

pub fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode_str = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::PaneFocused => "FOCUSED",
        Mode::SessionPicker => "PICKER",
        Mode::CommandEditor => "EDITOR",
        Mode::AppLauncher => "APPS",
    };

    let theme = &app.theme;
    let ids = app.pane_manager.pane_ids_in_order();
    let focused_id = app.pane_manager.focused_id();
    let focused_pos = ids.iter().position(|&id| id == focused_id);

    let pane_info = if let (Some(pos), Some(pane)) = (focused_pos, app.pane_manager.focused()) {
        format!("[{}/{}] {}", pos + 1, app.pane_manager.count(), pane.name())
    } else {
        format!("[0/{}]", app.pane_manager.count())
    };

    let maximize_indicator = if app.pane_manager.maximized.is_some() {
        " [MAX]"
    } else {
        ""
    };

    let version_tag = concat!("tmuch v", env!("CARGO_PKG_VERSION"), " ");
    let version_len = version_tag.len() as u16;

    let left_spans = vec![
        Span::styled(
            format!(" {} ", mode_str),
            Style::default()
                .fg(parse_color(&theme.status_bar.mode_fg))
                .bg(parse_color(&theme.status_bar.mode_bg)),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{}{}", pane_info, maximize_indicator),
            Style::default().fg(parse_color(&theme.status_bar.text)),
        ),
    ];

    let left_len = (mode_str.len() + 2) + 1 + pane_info.len() + maximize_indicator.len();
    let padding = if area.width as usize > left_len + version_len as usize {
        area.width as usize - left_len - version_len as usize
    } else {
        1
    };

    let mut spans = left_spans;
    spans.push(Span::raw(" ".repeat(padding)));
    spans.push(Span::styled(
        version_tag,
        Style::default().fg(parse_color(&theme.status_bar.version)),
    ));

    let line = Line::from(spans);
    let bar = Paragraph::new(line).style(Style::default().bg(parse_color(&theme.status_bar.bg)));
    frame.render_widget(bar, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::source::{ContentSource, PaneSpec};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    struct MockSource(String);

    impl ContentSource for MockSource {
        fn capture(&mut self, _w: u16, _h: u16) -> anyhow::Result<String> {
            Ok("mock".into())
        }
        fn send_keys(&mut self, _keys: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn name(&self) -> &str {
            &self.0
        }
        fn source_label(&self) -> &str {
            "mock"
        }
        fn is_interactive(&self) -> bool {
            false
        }
        fn to_spec(&self) -> PaneSpec {
            PaneSpec::Command {
                command: "mock".into(),
                interval_ms: 1000,
            }
        }
    }

    #[test]
    fn test_hints_bar_normal_mode() {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());
        terminal
            .draw(|frame| {
                draw_hints_bar(frame, &app, Rect::new(0, 0, 120, 1));
            })
            .unwrap();
    }

    #[test]
    fn test_hints_bar_focused_mode() {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.mode = Mode::PaneFocused;
        terminal
            .draw(|frame| {
                draw_hints_bar(frame, &app, Rect::new(0, 0, 120, 1));
            })
            .unwrap();
    }

    #[test]
    fn test_hints_bar_picker_mode() {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.mode = Mode::SessionPicker;
        terminal
            .draw(|frame| {
                draw_hints_bar(frame, &app, Rect::new(0, 0, 120, 1));
            })
            .unwrap();
    }

    #[test]
    fn test_hints_bar_editor_browse() {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.mode = Mode::CommandEditor;
        app.command_editor = Some(crate::editor_state::CommandEditorState {
            entries: vec![],
            selected: 0,
            input_mode: EditorInputMode::Browse,
            input_buffer: String::new(),
            pending_key: None,
        });
        terminal
            .draw(|frame| {
                draw_hints_bar(frame, &app, Rect::new(0, 0, 120, 1));
            })
            .unwrap();
    }

    #[test]
    fn test_hints_bar_editor_input_key() {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.mode = Mode::CommandEditor;
        app.command_editor = Some(crate::editor_state::CommandEditorState {
            entries: vec![],
            selected: 0,
            input_mode: EditorInputMode::InputKey,
            input_buffer: String::new(),
            pending_key: None,
        });
        terminal
            .draw(|frame| {
                draw_hints_bar(frame, &app, Rect::new(0, 0, 120, 1));
            })
            .unwrap();
    }

    #[test]
    fn test_hints_bar_editor_input_command() {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.mode = Mode::CommandEditor;
        app.command_editor = Some(crate::editor_state::CommandEditorState {
            entries: vec![],
            selected: 0,
            input_mode: EditorInputMode::InputCommand,
            input_buffer: String::new(),
            pending_key: None,
        });
        terminal
            .draw(|frame| {
                draw_hints_bar(frame, &app, Rect::new(0, 0, 120, 1));
            })
            .unwrap();
    }

    #[test]
    fn test_hints_bar_app_launcher() {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.mode = Mode::AppLauncher;
        terminal
            .draw(|frame| {
                draw_hints_bar(frame, &app, Rect::new(0, 0, 120, 1));
            })
            .unwrap();
    }

    #[test]
    fn test_status_bar_empty() {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());
        terminal
            .draw(|frame| {
                draw_status_bar(frame, &app, Rect::new(0, 0, 120, 1));
            })
            .unwrap();
    }

    #[test]
    fn test_status_bar_with_panes() {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.pane_manager.add(Box::new(MockSource("test".into())));
        terminal
            .draw(|frame| {
                draw_status_bar(frame, &app, Rect::new(0, 0, 120, 1));
            })
            .unwrap();
    }

    #[test]
    fn test_status_bar_maximized() {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        let id = app.pane_manager.add(Box::new(MockSource("test".into())));
        app.pane_manager.maximized = Some(id);
        terminal
            .draw(|frame| {
                draw_status_bar(frame, &app, Rect::new(0, 0, 120, 1));
            })
            .unwrap();
    }

    #[test]
    fn test_status_bar_narrow() {
        let backend = TestBackend::new(10, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());
        terminal
            .draw(|frame| {
                draw_status_bar(frame, &app, Rect::new(0, 0, 10, 1));
            })
            .unwrap();
    }
}
