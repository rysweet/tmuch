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

const TAB_FG: Color = Color::Rgb(220, 220, 220);
const TAB_DARK_RED: Color = Color::Rgb(80, 30, 30);
const TAB_DARK_GREEN: Color = Color::Rgb(30, 80, 30);
const TAB_DARK_CYAN: Color = Color::Rgb(30, 80, 80);
const TAB_DARK_BLUE: Color = Color::Rgb(30, 30, 80);
const TAB_DARK_MAGENTA: Color = Color::Rgb(80, 30, 80);
const TAB_DARK_YELLOW: Color = Color::Rgb(80, 80, 30);

const TAB_SELECTED_BG: Color = Color::Rgb(255, 255, 255);
const TAB_SELECTED_FG: Color = Color::Rgb(0, 0, 0);

/// Render a hint tab. If `selected`, render with bright inverted colors.
fn hint_tab(key: &str, label: &str, bg: Color, selected: bool) -> Vec<Span<'static>> {
    let style = if selected {
        Style::default()
            .fg(TAB_SELECTED_FG)
            .bg(TAB_SELECTED_BG)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(TAB_FG)
            .bg(bg)
            .add_modifier(Modifier::BOLD)
    };
    vec![
        Span::styled(format!(" {} {} ", key, label), style),
        Span::raw(" "),
    ]
}

/// Number of hint tabs in Normal mode.
pub const NORMAL_HINT_COUNT: usize = 12;

pub fn draw_hints_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    let sel = app.selected_hint;

    match app.mode {
        Mode::Normal => {
            spans.extend(hint_tab("q", "Quit", TAB_DARK_RED, sel == 0));
            spans.extend(hint_tab("^A", "Add", TAB_DARK_GREEN, sel == 1));
            spans.extend(hint_tab("^D", "Drop", TAB_DARK_RED, sel == 2));
            spans.extend(hint_tab("^S", "Sessions", TAB_DARK_CYAN, sel == 3));
            spans.extend(hint_tab("^G", "Azlin", TAB_DARK_BLUE, sel == 4));
            spans.extend(hint_tab("^E", "Settings", TAB_DARK_MAGENTA, sel == 5));
            spans.extend(hint_tab("^N", "Apps", TAB_DARK_CYAN, sel == 6));
            spans.extend(hint_tab("Tab", "Next", TAB_DARK_YELLOW, sel == 7));
            spans.extend(hint_tab("Enter", "Focus", TAB_DARK_GREEN, sel == 8));
            spans.extend(hint_tab("^V/^H", "Split", TAB_DARK_CYAN, sel == 9));
            spans.extend(hint_tab("F11", "Max", TAB_DARK_YELLOW, sel == 10));
            spans.extend(hint_tab("1-9", "Bindings", TAB_DARK_MAGENTA, sel == 11));
        }
        Mode::PaneFocused => {
            spans.extend(hint_tab("Esc", "Unfocus", TAB_DARK_RED, false));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                "All keys forwarded to session",
                Style::default().fg(Color::Rgb(100, 100, 100)),
            ));
        }
        Mode::SessionPicker => {
            spans.extend(hint_tab(
                "\u{2191}\u{2193}/jk",
                "Nav",
                TAB_DARK_YELLOW,
                false,
            ));
            spans.extend(hint_tab("Enter", "Select", TAB_DARK_GREEN, false));
            spans.extend(hint_tab("a", "Add All", TAB_DARK_CYAN, false));
            spans.extend(hint_tab("z", "Scan Azlin", TAB_DARK_BLUE, false));
            spans.extend(hint_tab("Esc", "Cancel", TAB_DARK_RED, false));
        }
        Mode::CommandEditor => {
            let input_mode = app.editor_input_mode();
            match input_mode {
                EditorInputMode::Browse => {
                    spans.extend(hint_tab("\u{2191}\u{2193}", "Nav", TAB_DARK_YELLOW, false));
                    spans.extend(hint_tab("a", "Add", TAB_DARK_GREEN, false));
                    spans.extend(hint_tab("e/Enter", "Edit", TAB_DARK_CYAN, false));
                    spans.extend(hint_tab("d", "Delete", TAB_DARK_RED, false));
                    spans.extend(hint_tab("Esc", "Close", TAB_DARK_RED, false));
                }
                EditorInputMode::InputKey => {
                    spans.extend(hint_tab("0-9", "Press a key", TAB_DARK_GREEN, false));
                    spans.extend(hint_tab("Esc", "Cancel", TAB_DARK_RED, false));
                }
                EditorInputMode::InputCommand => {
                    spans.extend(hint_tab("Enter", "Save", TAB_DARK_GREEN, false));
                    spans.extend(hint_tab("Esc", "Cancel", TAB_DARK_RED, false));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        "Type command...",
                        Style::default().fg(Color::Rgb(80, 80, 80)),
                    ));
                }
            }
        }
        Mode::AppLauncher => {
            spans.extend(hint_tab(
                "\u{2191}\u{2193}/jk",
                "Nav",
                TAB_DARK_YELLOW,
                false,
            ));
            spans.extend(hint_tab("Enter", "Launch", TAB_DARK_GREEN, false));
            spans.extend(hint_tab("Esc", "Cancel", TAB_DARK_RED, false));
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
        format!(
            "\u{25b6} [{}/{}] {}",
            pos + 1,
            app.pane_manager.count(),
            pane.name()
        )
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
            Style::default()
                .fg(parse_color(&theme.status_bar.text))
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let left_len = (mode_str.len() + 2) + 1 + pane_info.len() + maximize_indicator.len();

    // Busy indicator with spinner
    let busy_span = if let Some(ref msg) = app.busy {
        const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let frame = SPINNER[app.spinner_tick % SPINNER.len()];
        format!(" {} {} ", frame, msg)
    } else {
        String::new()
    };
    let busy_len = busy_span.len();

    let mut spans = left_spans;

    if !busy_span.is_empty() {
        spans.push(Span::styled(
            busy_span,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let total_left = left_len + busy_len;
    let padding = if area.width as usize > total_left + version_len as usize {
        area.width as usize - total_left - version_len as usize
    } else {
        1
    };
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
