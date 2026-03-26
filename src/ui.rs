use crate::app::App;
use crate::keys::Mode;
use crate::theme::{parse_border_type, parse_color};
use crate::ui_bars::{draw_hints_bar, draw_status_bar};
use crate::ui_overlays::{draw_app_launcher, draw_command_editor, draw_session_picker};
use ansi_to_tui::IntoText;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Line 1 (TOP): key hints bar
    let hints_area = Rect::new(size.x, size.y, size.width, 1);
    // Lines 2..N-1: pane grid
    let main_area = Rect::new(
        size.x,
        size.y + 1,
        size.width,
        size.height.saturating_sub(2),
    );
    // Line N (BOTTOM): status bar
    let status_area = Rect::new(size.x, size.height.saturating_sub(1), size.width, 1);

    // Draw hints bar
    draw_hints_bar(frame, app, hints_area);

    // Draw panes
    let theme = &app.theme;
    let border_type = parse_border_type(&theme.border.style);

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
            if label != "local" {
                vec![Span::styled(
                    format!(" {} [{}] ", pane.name(), label),
                    name_style,
                )]
            } else {
                vec![Span::styled(format!(" {} ", pane.name()), name_style)]
            }
        };

        let block = Block::default()
            .title(Line::from(title_spans))
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(border_color));

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
