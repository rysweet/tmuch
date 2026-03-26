use crate::app::App;
use crate::editor_state::EditorInputMode;
use crate::theme::parse_border_type;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

pub fn draw_session_picker(frame: &mut Frame, app: &App, area: Rect) {
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

    let border_type = parse_border_type(&app.theme.border.style);
    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(" Sessions ", Style::default().fg(Color::Cyan)))
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(list, popup);
}

pub fn draw_command_editor(frame: &mut Frame, app: &App, area: Rect) {
    let editor = match &app.command_editor {
        Some(e) => e,
        None => return,
    };

    let w = 50.min(area.width.saturating_sub(4));
    let entry_count = editor.entries.len().max(1);
    let h = ((entry_count as u16) + 5)
        .min(area.height.saturating_sub(4))
        .max(8);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);

    frame.render_widget(Clear, popup);

    let border_type = parse_border_type(&app.theme.border.style);

    match editor.input_mode {
        EditorInputMode::InputKey => {
            let block = Block::default()
                .title(Span::styled(
                    " Add Binding ",
                    Style::default().fg(Color::Green),
                ))
                .borders(Borders::ALL)
                .border_type(border_type)
                .border_style(Style::default().fg(Color::Green));

            let prompt = Paragraph::new(Line::from(vec![Span::styled(
                "Press a key (0-9):",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]))
            .block(block);

            let prompt_h = 3.min(popup.height);
            let prompt_y = popup.y + (popup.height.saturating_sub(prompt_h)) / 2;
            let prompt_area = Rect::new(popup.x, prompt_y, popup.width, prompt_h);
            frame.render_widget(Clear, prompt_area);
            frame.render_widget(prompt, prompt_area);
        }
        EditorInputMode::InputCommand => {
            let key_label = editor
                .pending_key
                .map(|k| k.to_string())
                .unwrap_or_default();

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
                items.push(
                    ListItem::new("  (no bindings)").style(Style::default().fg(Color::DarkGray)),
                );
            }

            items.push(ListItem::new(""));
            let input_line = format!("  {}: {}\u{2588}", key_label, editor.input_buffer);
            items.push(
                ListItem::new(input_line).style(
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            );

            let list = List::new(items).block(
                Block::default()
                    .title(Span::styled(
                        " Command Bindings ",
                        Style::default().fg(Color::Cyan),
                    ))
                    .borders(Borders::ALL)
                    .border_type(border_type)
                    .border_style(Style::default().fg(Color::Green)),
            );

            frame.render_widget(list, popup);
        }
        EditorInputMode::Browse => {
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
                items.push(
                    ListItem::new("  (no bindings)").style(Style::default().fg(Color::DarkGray)),
                );
            }

            let list = List::new(items).block(
                Block::default()
                    .title(Span::styled(
                        " Command Bindings ",
                        Style::default().fg(Color::Cyan),
                    ))
                    .borders(Borders::ALL)
                    .border_type(border_type)
                    .border_style(Style::default().fg(Color::Cyan)),
            );

            frame.render_widget(list, popup);
        }
    }
}

pub fn draw_app_launcher(frame: &mut Frame, app: &App, area: Rect) {
    let launcher = match &app.app_launcher {
        Some(l) => l,
        None => return,
    };

    let border_type = parse_border_type(&app.theme.border.style);
    let w = 60.min(area.width.saturating_sub(4));
    let h = ((launcher.apps.len() as u16) + 3)
        .min(area.height.saturating_sub(4))
        .max(6);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);

    frame.render_widget(Clear, popup);

    let items: Vec<ListItem> = launcher
        .apps
        .iter()
        .enumerate()
        .map(|(i, (name, desc, _usage))| {
            let prefix = if i == launcher.selected {
                "\u{25b6} "
            } else {
                "  "
            };
            let style = if i == launcher.selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(*name, style),
                Span::styled(
                    format!("  {}", desc),
                    Style::default().fg(Color::Rgb(100, 100, 100)),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(Line::from(Span::styled(
                " Apps ",
                Style::default().fg(Color::LightCyan),
            )))
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(Color::LightCyan)),
    );

    frame.render_widget(list, popup);
}
