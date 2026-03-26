use crate::app::App;
use crate::keys::Mode;
use crate::layout;
use ansi_to_tui::IntoText;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph};
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
            let is_remote = pane.source_label() != "local";

            let border_color = if is_focused {
                match app.mode {
                    Mode::PaneFocused => Color::Green,
                    _ => Color::Yellow,
                }
            } else if is_remote {
                Color::Rgb(40, 60, 80)
            } else {
                Color::Rgb(60, 60, 60)
            };

            let label = pane.source_label();
            let title_spans = if is_focused && app.mode == Mode::PaneFocused {
                vec![Span::styled(
                    format!(" \u{25b6} {} [ATTACHED] ", pane.name()),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]
            } else {
                let name_style = if is_focused {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Rgb(120, 120, 120))
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
                .border_type(BorderType::Rounded)
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

fn hint_separator() -> Span<'static> {
    Span::styled(" \u{2502} ", Style::default().fg(Color::Rgb(60, 60, 60)))
}

fn hint_key(key: &'static str) -> Span<'static> {
    Span::styled(
        key,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

fn hint_label(label: &'static str) -> Span<'static> {
    Span::styled(label, Style::default().fg(Color::DarkGray))
}

fn draw_hints_bar(frame: &mut Frame, app: &App, area: Rect) {
    let spans = match app.mode {
        Mode::Normal => vec![
            Span::raw(" "),
            hint_key("q"),
            hint_label(" Quit"),
            hint_separator(),
            hint_key("^A"),
            hint_label(" Add"),
            hint_separator(),
            hint_key("^D"),
            hint_label(" Drop"),
            hint_separator(),
            hint_key("^S"),
            hint_label(" Sessions"),
            hint_separator(),
            hint_key("^Z"),
            hint_label(" Azlin"),
            hint_separator(),
            hint_key("^E"),
            hint_label(" Edit Cmds"),
            hint_separator(),
            hint_key("Tab"),
            hint_label(" Next"),
            hint_separator(),
            hint_key("Enter"),
            hint_label(" Focus"),
            hint_separator(),
            hint_key("1-9"),
            hint_label(" Bindings"),
        ],
        Mode::PaneFocused => vec![
            Span::raw(" "),
            hint_key("Esc"),
            hint_label(" Unfocus"),
            hint_separator(),
            hint_label("All keys forwarded to session"),
        ],
        Mode::SessionPicker => vec![
            Span::raw(" "),
            hint_key("\u{2191}\u{2193}/jk"),
            hint_label(" Navigate"),
            hint_separator(),
            hint_key("Enter"),
            hint_label(" Select"),
            hint_separator(),
            hint_key("a"),
            hint_label(" Add All"),
            hint_separator(),
            hint_key("z"),
            hint_label(" Scan Azlin"),
            hint_separator(),
            hint_key("Esc"),
            hint_label(" Cancel"),
        ],
        Mode::CommandEditor => vec![
            Span::raw(" "),
            hint_key("\u{2191}\u{2193}"),
            hint_label(" Navigate"),
            hint_separator(),
            hint_key("d"),
            hint_label(" Delete"),
            hint_separator(),
            hint_key("Esc"),
            hint_label(" Close"),
            hint_separator(),
            hint_label("Edit ~/.config/tmuch/config.toml to add bindings"),
        ],
    };

    let line = Line::from(spans);
    let bar = Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 30)));
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

    let version_tag = concat!("tmuch v", env!("CARGO_PKG_VERSION"), " ");
    let version_len = version_tag.len() as u16;

    // Left side spans
    let left_spans = vec![
        Span::styled(
            format!(" {} ", mode_str),
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::raw(" "),
        Span::styled(pane_info.clone(), Style::default().fg(Color::White)),
    ];

    // Compute padding for right-alignment
    let left_len = (mode_str.len() + 2) + 1 + pane_info.len();
    let padding = if area.width as usize > left_len + version_len as usize {
        area.width as usize - left_len - version_len as usize
    } else {
        1
    };

    let mut spans = left_spans;
    spans.push(Span::raw(" ".repeat(padding)));
    spans.push(Span::styled(
        version_tag,
        Style::default().fg(Color::DarkGray),
    ));

    let line = Line::from(spans);
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
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
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
            .title(Span::styled(" Sessions ", Style::default().fg(Color::Cyan)))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
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
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
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
            .title(Span::styled(
                " Command Bindings ",
                Style::default().fg(Color::Cyan),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(list, popup);
}
