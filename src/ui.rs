use crate::app::App;
use crate::keys::Mode;
use crate::layout;
use ansi_to_tui::IntoText;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Reserve bottom line for status bar
    let main_area = Rect::new(size.x, size.y, size.width, size.height.saturating_sub(1));
    let status_area = Rect::new(size.x, size.height.saturating_sub(1), size.width, 1);

    // Draw panes
    let rects = layout::compute(app.pane_manager.count(), main_area);

    for (i, pane) in app.pane_manager.panes().iter().enumerate() {
        if let Some(&rect) = rects.get(i) {
            let is_focused = i == app.pane_manager.focused_index();
            let border_color = if is_focused {
                match app.mode {
                    Mode::PaneFocused => Color::Green,
                    _ => Color::Yellow,
                }
            } else {
                Color::DarkGray
            };

            let title = if is_focused && app.mode == Mode::PaneFocused {
                format!(" {} [ATTACHED] ", pane.session_name)
            } else {
                format!(" {} ", pane.session_name)
            };

            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            let inner = block.inner(rect);

            // Parse ANSI content
            let text = pane
                .content
                .as_bytes()
                .into_text()
                .unwrap_or_default();

            let paragraph = Paragraph::new(text).block(block);
            frame.render_widget(paragraph, rect);
            let _ = inner; // inner used implicitly by block
        }
    }

    // Draw status bar
    draw_status_bar(frame, app, status_area);

    // Draw session picker overlay if active
    if app.mode == Mode::SessionPicker {
        draw_session_picker(frame, app, main_area);
    }
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode_str = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::PaneFocused => "FOCUSED",
        Mode::SessionPicker => "PICKER",
    };

    let pane_info = if let Some(pane) = app.pane_manager.focused() {
        format!("[{}/{}] {}", app.pane_manager.focused_index() + 1, app.pane_manager.count(), pane.session_name)
    } else {
        format!("[0/{}]", app.pane_manager.count())
    };

    let bindings_hint = if app.mode == Mode::Normal {
        " | ^A:add ^D:drop ^S:list Tab:next Enter:focus"
    } else if app.mode == Mode::PaneFocused {
        " | Esc:unfocus"
    } else {
        " | j/k:nav Enter:select Esc:cancel"
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", mode_str),
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::raw(" "),
        Span::styled(pane_info, Style::default().fg(Color::White)),
        Span::styled(bindings_hint, Style::default().fg(Color::DarkGray)),
    ]);

    let bar = Paragraph::new(line).style(Style::default().bg(Color::Black));
    frame.render_widget(bar, area);
}

fn draw_session_picker(frame: &mut Frame, app: &App, area: Rect) {
    // Center a popup
    let w = 50.min(area.width.saturating_sub(4));
    let h = 15.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);

    frame.render_widget(Clear, popup);

    let items: Vec<ListItem> = app
        .picker
        .sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let prefix = if i == app.picker.selected { "▶ " } else { "  " };
            let attached = if s.attached { " (attached)" } else { "" };
            let style = if i == app.picker.selected {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("{}{}{}", prefix, s.name, attached)).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" tmux sessions ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(list, popup);
}
