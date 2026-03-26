use crate::app::App;
use crate::keys::Mode;
use crate::theme::parse_color;
use crate::ui_bars::{draw_hints_bar, draw_status_bar};
use crate::ui_overlays::{draw_app_launcher, draw_command_editor, draw_session_picker};
use ansi_to_tui::IntoText;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Layout: hints (1) + panes (N-3) + log line (1) + status bar (1)
    let hints_area = Rect::new(size.x, size.y, size.width, 1);
    let main_area = Rect::new(
        size.x,
        size.y + 1,
        size.width,
        size.height.saturating_sub(3),
    );
    let log_area = Rect::new(size.x, size.height.saturating_sub(2), size.width, 1);
    let status_area = Rect::new(size.x, size.height.saturating_sub(1), size.width, 1);

    // Draw hints bar
    draw_hints_bar(frame, app, hints_area);

    // Draw panes
    let theme = &app.theme;
    // border_type is now per-pane (Double for focused, Rounded for unfocused)

    // Get rects to render: if maximized, only the maximized pane
    let render_list: Vec<_> = if let Some(max_id) = app.pane_manager.maximized {
        if let Some(pane) = app.pane_manager.get(max_id) {
            vec![(max_id, main_area, pane)]
        } else {
            Vec::new()
        }
    } else {
        let rects = app.pane_manager.resolve_layout(main_area);
        rects
            .into_iter()
            .filter_map(|(id, rect)| app.pane_manager.get(id).map(|p| (id, rect, p)))
            .collect()
    };

    let focused_id = app.pane_manager.focused_id();

    for (id, rect, pane) in &render_list {
        let is_focused = *id == focused_id;
        let is_remote = pane.source_label() != "local";

        let border_color = if is_focused {
            match app.mode {
                Mode::PaneFocused => parse_color(&theme.border.focused_attached),
                _ => parse_color(&theme.border.focused),
            }
        } else if is_remote {
            parse_color(&theme.border.remote)
        } else {
            parse_color(&theme.border.unfocused)
        };

        // Focused pane uses Double border, unfocused uses Rounded
        let border_type = if is_focused {
            BorderType::Double
        } else {
            BorderType::Rounded
        };

        let label = pane.source_label();
        let title_spans = if is_focused && app.mode == Mode::PaneFocused {
            vec![Span::styled(
                format!(" \u{25b6} {} [ATTACHED] ", pane.name()),
                Style::default()
                    .fg(parse_color(&theme.title.attached_label))
                    .add_modifier(Modifier::BOLD),
            )]
        } else {
            let name_style = if is_focused {
                Style::default()
                    .fg(parse_color(&theme.title.focused))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(parse_color(&theme.title.unfocused))
            };
            // Focused pane title has triangle prefix
            let prefix = if is_focused { " \u{25b6} " } else { "   " };
            if label != "local" {
                vec![Span::styled(
                    format!("{}{}[{}] ", prefix, pane.name(), label),
                    name_style,
                )]
            } else {
                vec![Span::styled(
                    format!("{}{} ", prefix, pane.name()),
                    name_style,
                )]
            }
        };

        // Focused pane has a subtle blue background tint
        let block_style = if is_focused {
            Style::default().bg(Color::Rgb(25, 35, 50))
        } else {
            Style::default()
        };

        let block = Block::default()
            .title(Line::from(title_spans))
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(border_color))
            .style(block_style);

        let inner = block.inner(*rect);

        if pane.source.has_custom_render() {
            // Custom widget rendering path
            frame.render_widget(block, *rect);
            pane.source.render(inner, frame.buffer_mut());
        } else {
            // Standard text rendering path: parse ANSI content
            let text = pane.content.as_bytes().into_text().unwrap_or_default();
            let paragraph = Paragraph::new(text).block(block);
            frame.render_widget(paragraph, *rect);
        }
    }

    // Draw activity log line
    draw_log_line(frame, app, log_area);

    // Draw status bar
    draw_status_bar(frame, app, status_area);

    // Draw overlays
    if app.mode == Mode::SessionPicker {
        draw_session_picker(frame, app, main_area);
    }
    if app.mode == Mode::CommandEditor {
        draw_command_editor(frame, app, main_area);
    }
    if app.mode == Mode::AppLauncher {
        draw_app_launcher(frame, app, main_area);
    }
}

fn draw_log_line(frame: &mut Frame, app: &App, area: Rect) {
    use crate::source::debug_log;

    let last_msg = debug_log::last_message().unwrap_or_default();
    let display = if last_msg.is_empty() {
        if let Some(ref busy) = app.busy {
            const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let frame_char = SPINNER[app.spinner_tick % SPINNER.len()];
            format!(" {} {}", frame_char, busy)
        } else {
            " Ready".to_string()
        }
    } else {
        format!(" {}", last_msg)
    };

    let style = if app.busy.is_some() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Rgb(80, 80, 80))
    };

    let line = Line::from(Span::styled(display, style));
    let bar = Paragraph::new(line).style(Style::default().bg(Color::Rgb(15, 15, 15)));
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
            Ok("mock content".into())
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
    fn test_draw_empty_app() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }

    #[test]
    fn test_draw_with_panes() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.pane_manager.add(Box::new(MockSource("pane-a".into())));
        app.pane_manager.add(Box::new(MockSource("pane-b".into())));
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }

    #[test]
    fn test_draw_focused_mode() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.pane_manager.add(Box::new(MockSource("pane-a".into())));
        app.mode = Mode::PaneFocused;
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }

    #[test]
    fn test_draw_session_picker_mode() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.pane_manager.add(Box::new(MockSource("pane-a".into())));
        app.mode = Mode::SessionPicker;
        app.picker.sessions.push(crate::tmux::SessionInfo {
            name: "test-session".into(),
            attached: false,
            host: None,
        });
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }

    #[test]
    fn test_draw_command_editor_mode() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.mode = Mode::CommandEditor;
        app.command_editor = Some(crate::editor_state::CommandEditorState {
            entries: vec![('1', "top".into())],
            selected: 0,
            input_mode: crate::editor_state::EditorInputMode::Browse,
            input_buffer: String::new(),
            pending_key: None,
        });
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }

    #[test]
    fn test_draw_app_launcher_mode() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.mode = Mode::AppLauncher;
        app.app_launcher = Some(crate::editor_state::AppLauncherState {
            apps: vec![("clock", "Live clock", "clock:")],
            selected: 0,
        });
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }

    #[test]
    fn test_draw_maximized_pane() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        let id = app.pane_manager.add(Box::new(MockSource("pane-a".into())));
        app.pane_manager.add(Box::new(MockSource("pane-b".into())));
        app.pane_manager.maximized = Some(id);
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }

    #[test]
    fn test_draw_remote_label() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        struct RemoteMock;
        impl ContentSource for RemoteMock {
            fn capture(&mut self, _w: u16, _h: u16) -> anyhow::Result<String> {
                Ok("remote content".into())
            }
            fn send_keys(&mut self, _keys: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn name(&self) -> &str {
                "vm:session"
            }
            fn source_label(&self) -> &str {
                "ssh:vm"
            }
            fn is_interactive(&self) -> bool {
                true
            }
            fn to_spec(&self) -> PaneSpec {
                PaneSpec::Remote {
                    remote_name: "vm".into(),
                    session: "session".into(),
                }
            }
        }

        let mut app = App::new(Config::default());
        app.pane_manager.add(Box::new(RemoteMock));
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }

    #[test]
    fn test_draw_small_terminal() {
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.pane_manager.add(Box::new(MockSource("x".into())));
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }

    #[test]
    fn test_draw_custom_widget_pane() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.pane_manager
            .add(Box::new(crate::source::clock::ClockSource));
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }
}
