use crate::app::App;
use crate::keys::Mode;
use crate::theme::{parse_border_type, parse_color};
use ansi_to_tui::IntoText;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
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

    // Draw session picker overlay if active
    if app.mode == Mode::SessionPicker {
        draw_session_picker(frame, app, main_area);
    }

    // Draw command editor overlay if active
    if app.mode == Mode::CommandEditor {
        draw_command_editor(frame, app, main_area);
    }

    // Draw app launcher overlay if active
    if app.mode == Mode::AppLauncher {
        draw_app_launcher(frame, app, main_area);
    }
}

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

fn draw_hints_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![Span::raw(" ")];

    match app.mode {
        Mode::Normal => {
            // Each group in a different color for quick visual scanning
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
            use crate::app::EditorInputMode;
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

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
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

    // Left side spans
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

    // Compute padding for right-alignment
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

fn draw_command_editor(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::EditorInputMode;

    let editor = match &app.command_editor {
        Some(e) => e,
        None => return,
    };

    let w = 50.min(area.width.saturating_sub(4));
    let entry_count = editor.entries.len().max(1); // at least 1 for "(no bindings)"
                                                   // +4 for title border + bottom border + hint line + input line
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
            // Show a prompt to press a key
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

            // Use a smaller popup for the key prompt
            let prompt_h = 3.min(popup.height);
            let prompt_y = popup.y + (popup.height.saturating_sub(prompt_h)) / 2;
            let prompt_area = Rect::new(popup.x, prompt_y, popup.width, prompt_h);
            frame.render_widget(Clear, prompt_area);
            frame.render_widget(prompt, prompt_area);
        }
        EditorInputMode::InputCommand => {
            // Show existing entries + an input line at bottom
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

            // Add separator and input line
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
            // Normal browse view
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

fn draw_app_launcher(frame: &mut Frame, app: &App, area: Rect) {
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
