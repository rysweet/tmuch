use super::{ContentSource, PaneSpec};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget};

/// A clock widget pane that renders the current time using custom widget rendering.
pub struct ClockSource;

impl ContentSource for ClockSource {
    fn capture(&mut self, _width: u16, _height: u16) -> Result<String> {
        // Fallback text capture for non-widget paths
        let now = chrono::Local::now().format("%H:%M:%S").to_string();
        Ok(now)
    }

    fn send_keys(&mut self, _keys: &str) -> Result<()> {
        Ok(()) // not interactive
    }

    fn name(&self) -> &str {
        "clock"
    }

    fn source_label(&self) -> &str {
        "widget"
    }

    fn is_interactive(&self) -> bool {
        false
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::Plugin {
            plugin_name: "clock".to_string(),
            config: toml::Value::Table(toml::map::Map::new()),
        }
    }

    fn has_custom_render(&self) -> bool {
        true
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let now = chrono::Local::now().format("%H:%M:%S").to_string();
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();

        // Center vertically: put the time in the middle row
        let mid_y = area.height / 2;

        if area.height >= 3 && mid_y >= 1 {
            // Date line above time
            let date_text = Text::from(date);
            let date_para = Paragraph::new(date_text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray));
            let date_area = Rect::new(area.x, area.y + mid_y - 1, area.width, 1);
            Widget::render(date_para, date_area, buf);
        }

        // Time line
        let time_text = Text::from(now);
        let time_para = Paragraph::new(time_text)
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        let time_area = Rect::new(area.x, area.y + mid_y, area.width, 1);
        Widget::render(time_para, time_area, buf);
    }
}
