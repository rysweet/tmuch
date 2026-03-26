use super::{ContentSource, PaneSpec};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Sparkline, Widget};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const MAX_HISTORY: usize = 256;

/// A real-time sparkline chart that monitors a command's numeric output over time.
pub struct SparklineSource {
    command: String,
    interval: Duration,
    last_run: Option<Instant>,
    history: VecDeque<u64>,
    current_value: Option<f64>,
    display_name: String,
    error: Option<String>,
}

impl SparklineSource {
    pub fn new(command: String, interval_ms: u64) -> Self {
        let display_name = command
            .split_whitespace()
            .next()
            .unwrap_or(&command)
            .rsplit('/')
            .next()
            .unwrap_or(&command)
            .to_string();

        Self {
            command,
            interval: Duration::from_millis(interval_ms),
            last_run: None,
            history: VecDeque::with_capacity(MAX_HISTORY),
            current_value: None,
            display_name,
            error: None,
        }
    }

    fn should_refresh(&self) -> bool {
        match self.last_run {
            None => true,
            Some(t) => t.elapsed() >= self.interval,
        }
    }

    fn refresh(&mut self) {
        let result = std::process::Command::new("sh")
            .args(["-c", &self.command])
            .output();

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Parse first number from output
                if let Some(num) = extract_first_number(&stdout) {
                    self.current_value = Some(num);
                    // Scale to u64 for sparkline (multiply by 100 to keep 2 decimal places)
                    let scaled = (num * 100.0).max(0.0) as u64;
                    if self.history.len() >= MAX_HISTORY {
                        self.history.pop_front();
                    }
                    self.history.push_back(scaled);
                    self.error = None;
                } else {
                    self.error = Some(format!("No number in output: {}", stdout.trim()));
                }
            }
            Err(e) => {
                self.error = Some(format!("Error: {}", e));
            }
        }
        self.last_run = Some(Instant::now());
    }
}

/// Extract the first floating-point number from a string.
fn extract_first_number(s: &str) -> Option<f64> {
    for token in s.split(|c: char| !c.is_ascii_digit() && c != '.' && c != '-') {
        if let Ok(n) = token.parse::<f64>() {
            return Some(n);
        }
    }
    None
}

impl ContentSource for SparklineSource {
    fn capture(&mut self, _width: u16, _height: u16) -> Result<String> {
        if self.should_refresh() {
            self.refresh();
        }
        if let Some(ref err) = self.error {
            return Ok(format!("spark({}) error: {}", self.command, err));
        }
        let val_str = self
            .current_value
            .map(|v| format!("{:.2}", v))
            .unwrap_or_else(|| "?".to_string());
        Ok(format!(
            "{}: {} ({} pts)",
            self.display_name,
            val_str,
            self.history.len()
        ))
    }

    fn send_keys(&mut self, _keys: &str) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        &self.display_name
    }

    fn source_label(&self) -> &str {
        "spark"
    }

    fn is_interactive(&self) -> bool {
        false
    }

    fn to_spec(&self) -> PaneSpec {
        let mut config = toml::map::Map::new();
        config.insert(
            "command".to_string(),
            toml::Value::String(self.command.clone()),
        );
        config.insert(
            "interval_ms".to_string(),
            toml::Value::Integer(self.interval.as_millis() as i64),
        );
        PaneSpec::Plugin {
            plugin_name: "sparkline".to_string(),
            config: toml::Value::Table(config),
        }
    }

    fn has_custom_render(&self) -> bool {
        true
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 10 {
            return;
        }

        let chunks = Layout::vertical([
            Constraint::Length(1), // title
            Constraint::Length(1), // current value
            Constraint::Min(1),    // sparkline
            Constraint::Length(1), // status
        ])
        .split(area);

        // Title
        let title = Line::from(vec![Span::styled(
            format!("$ {}", self.command),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]);
        let title_para = Paragraph::new(title).alignment(Alignment::Left);
        Widget::render(title_para, chunks[0], buf);

        // Current value
        if let Some(ref err) = self.error {
            let err_line = Line::from(vec![Span::styled(
                err.clone(),
                Style::default().fg(Color::Red),
            )]);
            let err_para = Paragraph::new(err_line).alignment(Alignment::Left);
            Widget::render(err_para, chunks[1], buf);
        } else if let Some(val) = self.current_value {
            let val_line = Line::from(vec![
                Span::styled("Current: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:.2}", val),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            let val_para = Paragraph::new(val_line).alignment(Alignment::Left);
            Widget::render(val_para, chunks[1], buf);
        }

        // Sparkline chart
        if !self.history.is_empty() {
            // Take only as many points as fit in the width
            let display_width = chunks[2].width as usize;
            let data: Vec<u64> = if self.history.len() > display_width {
                self.history
                    .iter()
                    .skip(self.history.len() - display_width)
                    .copied()
                    .collect()
            } else {
                self.history.iter().copied().collect()
            };

            let sparkline = Sparkline::default()
                .data(&data)
                .style(Style::default().fg(Color::Green));
            Widget::render(sparkline, chunks[2], buf);
        }

        // Status line
        let status = Line::from(vec![Span::styled(
            format!(
                "{} samples | every {}ms",
                self.history.len(),
                self.interval.as_millis()
            ),
            Style::default().fg(Color::DarkGray),
        )]);
        let status_para = Paragraph::new(status).alignment(Alignment::Right);
        Widget::render(status_para, chunks[3], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_extract_first_number() {
        assert_eq!(extract_first_number("42"), Some(42.0));
        assert_eq!(extract_first_number("cpu: 3.15%"), Some(3.15));
        assert_eq!(extract_first_number("no numbers here"), None);
        assert_eq!(extract_first_number("  12.5 load"), Some(12.5));
        assert_eq!(extract_first_number("-7.3"), Some(-7.3));
    }

    #[test]
    fn test_sparkline_new() {
        let s = SparklineSource::new("echo 42".into(), 2000);
        assert_eq!(s.display_name, "echo");
        assert!(s.history.is_empty());
        assert!(s.current_value.is_none());
        assert!(s.should_refresh());
    }

    #[test]
    fn test_sparkline_metadata() {
        let s = SparklineSource::new("cat /proc/loadavg".into(), 2000);
        assert_eq!(s.name(), "cat");
        assert_eq!(s.source_label(), "spark");
        assert!(!s.is_interactive());
        assert!(s.has_custom_render());
    }

    #[test]
    fn test_sparkline_render_no_panic() {
        let s = SparklineSource::new("echo 1".into(), 60_000);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        s.render(area, &mut buf);
    }

    #[test]
    fn test_sparkline_render_with_data() {
        let mut s = SparklineSource::new("echo 1".into(), 60_000);
        s.current_value = Some(42.0);
        for i in 0..20 {
            s.history.push_back(i * 100);
        }
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        s.render(area, &mut buf);
    }

    #[test]
    fn test_sparkline_render_with_error() {
        let mut s = SparklineSource::new("bad".into(), 60_000);
        s.error = Some("command not found".into());
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        s.render(area, &mut buf);
    }

    #[test]
    fn test_sparkline_render_small_area() {
        let s = SparklineSource::new("x".into(), 60_000);
        let area = Rect::new(0, 0, 5, 2);
        let mut buf = Buffer::empty(area);
        s.render(area, &mut buf);
    }

    #[test]
    fn test_sparkline_to_spec() {
        let s = SparklineSource::new("cmd".into(), 1000);
        let spec = s.to_spec();
        match spec {
            PaneSpec::Plugin { plugin_name, .. } => assert_eq!(plugin_name, "sparkline"),
            _ => panic!("expected Plugin spec"),
        }
    }

    #[test]
    fn test_sparkline_send_keys_noop() {
        let mut s = SparklineSource::new("x".into(), 2000);
        assert!(s.send_keys("a").is_ok());
    }
}
