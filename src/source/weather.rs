use super::{ContentSource, PaneSpec};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::time::{Duration, Instant};

/// A weather widget that fetches weather from wttr.in and renders a styled card.
pub struct WeatherSource {
    city: String,
    interval: Duration,
    last_run: Option<Instant>,
    temperature_c: Option<f64>,
    condition: String,
    humidity: String,
    wind_speed: String,
    feels_like: String,
    last_updated: String,
    error: Option<String>,
}

impl WeatherSource {
    pub fn new(city: String, interval_ms: u64) -> Self {
        Self {
            city,
            interval: Duration::from_millis(interval_ms),
            last_run: None,
            temperature_c: None,
            condition: String::new(),
            humidity: String::new(),
            wind_speed: String::new(),
            feels_like: String::new(),
            last_updated: String::new(),
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
        let url = format!("wttr.in/{}?format=j1", self.city);
        let result = std::process::Command::new("curl")
            .args(["-sS", "--max-time", "10", &url])
            .output();

        match result {
            Ok(output) => {
                let body = String::from_utf8_lossy(&output.stdout);
                self.parse_weather(&body);
                self.last_updated = chrono::Local::now().format("%H:%M:%S").to_string();
            }
            Err(e) => {
                self.error = Some(format!("curl error: {}", e));
            }
        }
        self.last_run = Some(Instant::now());
    }

    fn parse_weather(&mut self, json: &str) {
        // Parse the wttr.in JSON response
        let parsed: std::result::Result<serde_json::Value, _> = serde_json::from_str(json);
        match parsed {
            Ok(val) => {
                self.error = None;
                if let Some(current) = val
                    .get("current_condition")
                    .and_then(|c| c.as_array())
                    .and_then(|a| a.first())
                {
                    self.temperature_c = current
                        .get("temp_C")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok());
                    self.condition = current
                        .get("weatherDesc")
                        .and_then(|d| d.as_array())
                        .and_then(|a| a.first())
                        .and_then(|d| d.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    self.humidity = current
                        .get("humidity")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string();
                    self.wind_speed = current
                        .get("windspeedKmph")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string();
                    self.feels_like = current
                        .get("FeelsLikeC")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string();
                } else {
                    self.error = Some("No current_condition in response".to_string());
                }
            }
            Err(e) => {
                self.error = Some(format!("JSON parse error: {}", e));
            }
        }
    }

    fn temp_color(temp: f64) -> Color {
        if temp < 5.0 {
            Color::Blue
        } else if temp < 20.0 {
            Color::Green
        } else if temp < 30.0 {
            Color::Yellow
        } else {
            Color::Red
        }
    }

    fn weather_symbol(condition: &str) -> &'static str {
        let lower = condition.to_lowercase();
        if lower.contains("sun") || lower.contains("clear") {
            "☀"
        } else if lower.contains("cloud") || lower.contains("overcast") {
            "☁"
        } else if lower.contains("rain") || lower.contains("drizzle") || lower.contains("shower") {
            "🌧"
        } else if lower.contains("snow") || lower.contains("sleet") || lower.contains("ice") {
            "❄"
        } else if lower.contains("fog") || lower.contains("mist") {
            "🌫"
        } else if lower.contains("thunder") || lower.contains("storm") {
            "⛈"
        } else {
            "☁"
        }
    }
}

impl ContentSource for WeatherSource {
    fn capture(&mut self, _width: u16, _height: u16) -> Result<String> {
        if self.should_refresh() {
            self.refresh();
        }
        if let Some(ref err) = self.error {
            return Ok(format!("Weather error: {}", err));
        }
        let temp_str = self
            .temperature_c
            .map(|t| format!("{:.0}°C", t))
            .unwrap_or_else(|| "?".to_string());
        Ok(format!(
            "{} {} | {} | Humidity: {}% | Wind: {} km/h",
            Self::weather_symbol(&self.condition),
            temp_str,
            self.condition,
            self.humidity,
            self.wind_speed
        ))
    }

    fn send_keys(&mut self, _keys: &str) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        &self.city
    }

    fn source_label(&self) -> &str {
        "weather"
    }

    fn is_interactive(&self) -> bool {
        false
    }

    fn to_spec(&self) -> PaneSpec {
        let mut config = toml::map::Map::new();
        config.insert("city".to_string(), toml::Value::String(self.city.clone()));
        config.insert(
            "interval_ms".to_string(),
            toml::Value::Integer(self.interval.as_millis() as i64),
        );
        PaneSpec::Plugin {
            plugin_name: "weather".to_string(),
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

        let mid_y = area.height / 2;
        let mut y = area.y + mid_y.saturating_sub(4);

        // City name - bold cyan
        let city_line = Line::from(vec![Span::styled(
            self.city.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]);
        let city_para = Paragraph::new(city_line).alignment(Alignment::Center);
        Widget::render(city_para, Rect::new(area.x, y, area.width, 1), buf);
        y += 2;

        if let Some(ref err) = self.error {
            // Error display
            let err_line = Line::from(vec![Span::styled(
                err.clone(),
                Style::default().fg(Color::Red),
            )]);
            let err_para = Paragraph::new(err_line).alignment(Alignment::Center);
            Widget::render(err_para, Rect::new(area.x, y, area.width, 1), buf);
            return;
        }

        // Temperature - large colored text
        if let Some(temp) = self.temperature_c {
            let color = Self::temp_color(temp);
            let temp_str = format!("{:.0}°C", temp);
            let temp_line = Line::from(vec![Span::styled(
                temp_str,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )]);
            let temp_para = Paragraph::new(temp_line).alignment(Alignment::Center);
            Widget::render(temp_para, Rect::new(area.x, y, area.width, 1), buf);
            y += 1;
        }

        // Condition with symbol
        if !self.condition.is_empty() {
            let symbol = Self::weather_symbol(&self.condition);
            let cond_line = Line::from(vec![Span::styled(
                format!("{} {}", symbol, self.condition),
                Style::default().fg(Color::White),
            )]);
            let cond_para = Paragraph::new(cond_line).alignment(Alignment::Center);
            Widget::render(cond_para, Rect::new(area.x, y, area.width, 1), buf);
            y += 1;
        }

        // Feels like
        if !self.feels_like.is_empty() {
            let fl_line = Line::from(vec![Span::styled(
                format!("Feels like {}°C", self.feels_like),
                Style::default().fg(Color::DarkGray),
            )]);
            let fl_para = Paragraph::new(fl_line).alignment(Alignment::Center);
            Widget::render(fl_para, Rect::new(area.x, y, area.width, 1), buf);
            y += 2;
        }

        // Humidity and wind
        let detail_line = Line::from(vec![Span::styled(
            format!(
                "Humidity: {}%  Wind: {} km/h",
                self.humidity, self.wind_speed
            ),
            Style::default().fg(Color::DarkGray),
        )]);
        let detail_para = Paragraph::new(detail_line).alignment(Alignment::Center);
        if y < area.y + area.height {
            Widget::render(detail_para, Rect::new(area.x, y, area.width, 1), buf);
            y += 2;
        }

        // Last updated
        if !self.last_updated.is_empty() && y < area.y + area.height {
            let upd_line = Line::from(vec![Span::styled(
                format!("Updated {}", self.last_updated),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            )]);
            let upd_para = Paragraph::new(upd_line).alignment(Alignment::Center);
            Widget::render(upd_para, Rect::new(area.x, y, area.width, 1), buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_weather_new() {
        let w = WeatherSource::new("London".into(), 300_000);
        assert_eq!(w.city, "London");
        assert!(w.temperature_c.is_none());
        assert!(w.should_refresh()); // never run yet
    }

    #[test]
    fn test_weather_metadata() {
        let w = WeatherSource::new("Paris".into(), 60_000);
        assert_eq!(w.name(), "Paris");
        assert_eq!(w.source_label(), "weather");
        assert!(!w.is_interactive());
        assert!(w.has_custom_render());
    }

    #[test]
    fn test_weather_parse_valid_json() {
        let mut w = WeatherSource::new("Test".into(), 300_000);
        let json = r#"{
            "current_condition": [{
                "temp_C": "22",
                "weatherDesc": [{"value": "Sunny"}],
                "humidity": "55",
                "windspeedKmph": "12",
                "FeelsLikeC": "20"
            }]
        }"#;
        w.parse_weather(json);
        assert!(w.error.is_none());
        assert_eq!(w.temperature_c, Some(22.0));
        assert_eq!(w.condition, "Sunny");
        assert_eq!(w.humidity, "55");
        assert_eq!(w.wind_speed, "12");
        assert_eq!(w.feels_like, "20");
    }

    #[test]
    fn test_weather_parse_invalid_json() {
        let mut w = WeatherSource::new("Test".into(), 300_000);
        w.parse_weather("not json");
        assert!(w.error.is_some());
    }

    #[test]
    fn test_weather_parse_missing_condition() {
        let mut w = WeatherSource::new("Test".into(), 300_000);
        w.parse_weather(r#"{"other": "data"}"#);
        assert!(w.error.is_some());
    }

    #[test]
    fn test_temp_color() {
        assert_eq!(WeatherSource::temp_color(-5.0), Color::Blue);
        assert_eq!(WeatherSource::temp_color(10.0), Color::Green);
        assert_eq!(WeatherSource::temp_color(25.0), Color::Yellow);
        assert_eq!(WeatherSource::temp_color(35.0), Color::Red);
    }

    #[test]
    fn test_weather_symbol() {
        assert_eq!(WeatherSource::weather_symbol("Sunny"), "\u{2600}");
        assert_eq!(WeatherSource::weather_symbol("Cloudy"), "\u{2601}");
        assert_eq!(WeatherSource::weather_symbol("Light Rain"), "\u{1f327}");
        assert_eq!(WeatherSource::weather_symbol("Snow"), "\u{2744}");
        assert_eq!(WeatherSource::weather_symbol("Fog"), "\u{1f32b}");
        assert_eq!(WeatherSource::weather_symbol("Thunder"), "\u{26c8}");
        assert_eq!(WeatherSource::weather_symbol("Unknown"), "\u{2601}"); // default
    }

    #[test]
    fn test_weather_render_no_panic() {
        let w = WeatherSource::new("TestCity".into(), 300_000);
        let area = Rect::new(0, 0, 40, 15);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
    }

    #[test]
    fn test_weather_render_with_data() {
        let mut w = WeatherSource::new("London".into(), 300_000);
        w.temperature_c = Some(20.0);
        w.condition = "Sunny".into();
        w.humidity = "50".into();
        w.wind_speed = "10".into();
        w.feels_like = "18".into();
        w.last_updated = "12:00:00".into();
        let area = Rect::new(0, 0, 40, 15);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
    }

    #[test]
    fn test_weather_render_with_error() {
        let mut w = WeatherSource::new("Bad".into(), 300_000);
        w.error = Some("fetch failed".into());
        let area = Rect::new(0, 0, 40, 15);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
    }

    #[test]
    fn test_weather_render_small_area() {
        let w = WeatherSource::new("X".into(), 300_000);
        let area = Rect::new(0, 0, 5, 2);
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf);
    }

    #[test]
    fn test_weather_to_spec() {
        let w = WeatherSource::new("Seattle".into(), 60_000);
        let spec = w.to_spec();
        match spec {
            PaneSpec::Plugin { plugin_name, .. } => assert_eq!(plugin_name, "weather"),
            _ => panic!("expected Plugin spec"),
        }
    }

    #[test]
    fn test_weather_send_keys_noop() {
        let mut w = WeatherSource::new("X".into(), 300_000);
        assert!(w.send_keys("a").is_ok());
    }
}
