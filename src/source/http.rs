use super::{ContentSource, PaneSpec};
use anyhow::Result;
use std::time::{Duration, Instant};

/// Polls a URL via `curl` at a configurable interval and displays the response.
pub struct HttpSource {
    url: String,
    interval: Duration,
    last_run: Option<Instant>,
    latest_output: String,
    display_name: String,
}

impl HttpSource {
    pub fn new(url: String, interval_ms: u64) -> Self {
        // Derive display name from URL
        let display_name = url
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or(&url)
            .to_string();

        Self {
            url,
            interval: Duration::from_millis(interval_ms),
            last_run: None,
            latest_output: String::new(),
            display_name,
        }
    }

    fn should_refresh(&self) -> bool {
        match self.last_run {
            None => true,
            Some(t) => t.elapsed() >= self.interval,
        }
    }

    fn refresh(&mut self) {
        let result = std::process::Command::new("curl")
            .args(["-sS", "--max-time", "5", &self.url])
            .output();

        match result {
            Ok(output) => {
                self.latest_output = String::from_utf8_lossy(&output.stdout).to_string();
                if !output.status.success() && !output.stderr.is_empty() {
                    self.latest_output
                        .push_str(&String::from_utf8_lossy(&output.stderr));
                }
            }
            Err(e) => {
                self.latest_output = format!("Error: {}", e);
            }
        }
        self.last_run = Some(Instant::now());
    }
}

impl ContentSource for HttpSource {
    fn capture(&mut self, _width: u16, _height: u16) -> Result<String> {
        if self.should_refresh() {
            self.refresh();
        }
        Ok(self.latest_output.clone())
    }

    fn send_keys(&mut self, _keys: &str) -> Result<()> {
        Ok(()) // not interactive
    }

    fn name(&self) -> &str {
        &self.display_name
    }

    fn source_label(&self) -> &str {
        "http"
    }

    fn is_interactive(&self) -> bool {
        false
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::Http {
            url: self.url.clone(),
            interval_ms: self.interval.as_millis() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_display_name() {
        let s = HttpSource::new("http://example.com/api/health".into(), 5000);
        assert_eq!(s.display_name, "example.com");
        assert_eq!(s.name(), "example.com");
    }

    #[test]
    fn test_http_display_name_https() {
        let s = HttpSource::new("https://api.example.com/v1".into(), 5000);
        assert_eq!(s.display_name, "api.example.com");
    }

    #[test]
    fn test_http_metadata() {
        let s = HttpSource::new("http://localhost".into(), 3000);
        assert_eq!(s.source_label(), "http");
        assert!(!s.is_interactive());
        assert!(!s.has_custom_render());
    }

    #[test]
    fn test_http_to_spec() {
        let s = HttpSource::new("http://example.com".into(), 5000);
        let spec = s.to_spec();
        match spec {
            PaneSpec::Http { url, interval_ms } => {
                assert_eq!(url, "http://example.com");
                assert_eq!(interval_ms, 5000);
            }
            _ => panic!("expected Http spec"),
        }
    }

    #[test]
    fn test_http_send_keys_noop() {
        let mut s = HttpSource::new("http://x".into(), 5000);
        assert!(s.send_keys("a").is_ok());
    }

    #[test]
    fn test_http_should_refresh_initially() {
        let s = HttpSource::new("http://x".into(), 5000);
        assert!(s.should_refresh());
    }
}
