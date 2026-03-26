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

            let label = pane.source_label();
            let title = if is_focused && app.mode == Mode::PaneFocused {
                format!(" {} [ATTACHED] ", pane.name())
            } else if label != "local" {
                format!(" {} [{}] ", pane.name(), label)
            } else {
                format!(" {} ", pane.name())
            };

            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            let inner = block.inner(rect);

            // Parse ANSI content
            let text = pane.content.as_bytes().into_text().unwrap_or_default();

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

    // Draw command editor overlay if active
    if app.mode == Mode::CommandEditor {
        draw_command_editor(frame, app, main_area);
    }
}

fn draw_hints_bar(frame: &mut Frame, app: &App, area: Rect) {
    let hints = match app.mode {
        Mode::Normal => {
            " q Quit \u{2502} ^A Add \u{2502} ^D Drop \u{2502} ^S Sessions \u{2502} ^Z Azlin \u{2502} ^E Edit Cmds \u{2502} Tab Next \u{2502} Enter Focus \u{2502} 1-9 Bindings"
        }
        Mode::PaneFocused => " Esc Unfocus \u{2502} All keys forwarded to session",
        Mode::SessionPicker => {
            " \u{2191}\u{2193}/jk Navigate \u{2502} Enter Select \u{2502} a Add All \u{2502} z Scan Azlin \u{2502} Esc Cancel"
        }
        Mode::CommandEditor => {
            " \u{2191}\u{2193} Navigate \u{2502} d Delete \u{2502} Esc Close \u{2502} Edit ~/.config/tmuch/config.toml to add bindings"
        }
    };

    let line = Line::from(Span::styled(hints, Style::default().fg(Color::DarkGray)));
    let bar = Paragraph::new(line).style(Style::default());
    frame.render_widget(bar, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode_str = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::PaneFocused => "FOCUSED",
        Mode::SessionPicker => "PICKER",
        Mode::CommandEditor => "EDITOR",
    };

    let pane_info = if let Some(pane) = app.pane_manager.focused() {
        format!(
            "[{}/{}] {}",
            app.pane_manager.focused_index() + 1,
            app.pane_manager.count(),
            pane.name()
        )
    } else {
        format!("[0/{}]", app.pane_manager.count())
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", mode_str),
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::raw(" "),
        Span::styled(pane_info, Style::default().fg(Color::White)),
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
            let prefix = if i == app.picker.selected {
                "\u{25b6} "
            } else {
                "  "
            };
            let attached = if s.attached { " (attached)" } else { "" };
            let host_label = match &s.host {
                Some(h) => format!(" [{}]", h),
                None => String::new(),
            };
            let style = if i == app.picker.selected {
                Style::default().fg(Color::Yellow)
            } else if s.host.is_some() {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("{}{}{}{}", prefix, s.name, host_label, attached)).style(style)
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

fn draw_command_editor(frame: &mut Frame, app: &App, area: Rect) {
    let editor = match &app.command_editor {
        Some(e) => e,
        None => return,
    };

    let w = 50.min(area.width.saturating_sub(4));
    let entry_count = editor.entries.len();
    // +3 for title border + bottom border + hint line
    let h = ((entry_count as u16) + 4)
        .min(area.height.saturating_sub(4))
        .max(6);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);

    frame.render_widget(Clear, popup);

    let mut items: Vec<ListItem> = editor
        .entries
        .iter()
        .enumerate()
        .map(|(i, (key, cmd))| {
            let prefix = if i == editor.selected {
                "\u{25b6} "
            } else {
                "  "
            };
            let style = if i == editor.selected {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("{}{}: {}", prefix, key, cmd)).style(style)
        })
        .collect();

    if items.is_empty() {
        items.push(ListItem::new("  (no bindings)").style(Style::default().fg(Color::DarkGray)));
    }

    let list = List::new(items).block(
        Block::default()
            .title(" Command Bindings ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(list, popup);
}
