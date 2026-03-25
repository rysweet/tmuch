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
