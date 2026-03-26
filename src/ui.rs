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

        // Parse ANSI content
        let text = pane.content.as_bytes().into_text().unwrap_or_default();

        let paragraph = Paragraph::new(text).block(block);
        frame.render_widget(paragraph, *rect);
        let _ = inner; // inner used implicitly by block
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

fn hint_separator(app: &App) -> Span<'static> {
    Span::styled(
        " \u{2502} ",
        Style::default().fg(parse_color(&app.theme.hints_bar.separator)),
    )
}

fn hint_key(key: &'static str, app: &App) -> Span<'static> {
    Span::styled(
        key,
        Style::default()
            .fg(parse_color(&app.theme.hints_bar.key))
            .add_modifier(Modifier::BOLD),
    )
}

fn hint_label(label: &'static str, app: &App) -> Span<'static> {
    Span::styled(
        label,
        Style::default().fg(parse_color(&app.theme.hints_bar.label)),
    )
}

fn draw_hints_bar(frame: &mut Frame, app: &App, area: Rect) {
    let spans = match app.mode {
        Mode::Normal => {
            vec![
                Span::raw(" "),
                hint_key("q", app),
                hint_label(" Quit", app),
                hint_separator(app),
                hint_key("^A", app),
                hint_label(" Add", app),
                hint_separator(app),
                hint_key("^D", app),
                hint_label(" Drop", app),
                hint_separator(app),
                hint_key("^S", app),
                hint_label(" Sessions", app),
                hint_separator(app),
                hint_key("^Z", app),
                hint_label(" Azlin", app),
                hint_separator(app),
                hint_key("^E", app),
                hint_label(" Edit Cmds", app),
                hint_separator(app),
                hint_key("Tab", app),
                hint_label(" Next", app),
                hint_separator(app),
                hint_key("Enter", app),
                hint_label(" Focus", app),
                hint_separator(app),
                hint_key("^V/^H", app),
                hint_label(" Split", app),
                hint_separator(app),
                hint_key("F11", app),
                hint_label(" Max", app),
                hint_separator(app),
                hint_key("1-9", app),
                hint_label(" Bindings", app),
            ]
        }
        Mode::PaneFocused => vec![
            Span::raw(" "),
            hint_key("Esc", app),
            hint_label(" Unfocus", app),
            hint_separator(app),
            hint_label("All keys forwarded to session", app),
        ],
        Mode::SessionPicker => vec![
            Span::raw(" "),
            hint_key("\u{2191}\u{2193}/jk", app),
            hint_label(" Navigate", app),
            hint_separator(app),
            hint_key("Enter", app),
            hint_label(" Select", app),
            hint_separator(app),
            hint_key("a", app),
            hint_label(" Add All", app),
            hint_separator(app),
            hint_key("z", app),
            hint_label(" Scan Azlin", app),
            hint_separator(app),
            hint_key("Esc", app),
            hint_label(" Cancel", app),
        ],
        Mode::CommandEditor => vec![
            Span::raw(" "),
            hint_key("\u{2191}\u{2193}", app),
            hint_label(" Navigate", app),
            hint_separator(app),
            hint_key("d", app),
            hint_label(" Delete", app),
            hint_separator(app),
            hint_key("Esc", app),
            hint_label(" Close", app),
            hint_separator(app),
            hint_label("Edit ~/.config/tmuch/config.toml to add bindings", app),
        ],
    };

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

    let border_type = parse_border_type(&app.theme.border.style);
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
