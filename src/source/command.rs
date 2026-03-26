use super::{ContentSource, PaneSpec};
use anyhow::Result;
use std::time::{Duration, Instant};

/// Runs a command periodically and displays its output.
pub struct CommandSource {
    command: String,
    interval: Duration,
    last_run: Option<Instant>,
    latest_output: String,
    display_name: String,
}

impl CommandSource {
    pub fn new(command: String, interval_ms: u64) -> Self {
        // Derive a short display name from the command
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
        let result = std::process::Command::new("sh")
            .args(["-c", &self.command])
            .output();

        match result {
            Ok(output) => {
                self.latest_output = String::from_utf8_lossy(&output.stdout).to_string();
                if !output.stderr.is_empty() {
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

impl ContentSource for CommandSource {
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
        "cmd"
    }

    fn is_interactive(&self) -> bool {
        false
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::Command {
            command: self.command.clone(),
            interval_ms: self.interval.as_millis() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_source_captures_output() {
        let mut src = CommandSource::new("echo hello".to_string(), 60_000);
        let output = src.capture(80, 24).unwrap();
        assert!(output.contains("hello"), "output was: {}", output);
        assert_eq!(src.name(), "echo");
        assert_eq!(src.source_label(), "cmd");
        assert!(!src.is_interactive());
    }

    #[test]
    fn test_command_source_display_name_with_path() {
        let src = CommandSource::new("/usr/bin/ls -la".to_string(), 5000);
        assert_eq!(src.name(), "ls");
    }

    #[test]
    fn test_command_source_to_spec() {
        let src = CommandSource::new("date".to_string(), 3000);
        let spec = src.to_spec();
        match spec {
            PaneSpec::Command {
                command,
                interval_ms,
            } => {
                assert_eq!(command, "date");
                assert_eq!(interval_ms, 3000);
            }
            _ => panic!("expected Command spec"),
        }
    }

    #[test]
    fn test_command_source_send_keys_noop() {
        let mut src = CommandSource::new("echo".to_string(), 5000);
        assert!(src.send_keys("x").is_ok());
    }

    #[test]
    fn test_command_source_stderr_included() {
        let mut src = CommandSource::new("echo err >&2".to_string(), 60_000);
        let output = src.capture(80, 24).unwrap();
        assert!(output.contains("err"), "output was: {}", output);
    }

    #[test]
    fn test_command_source_no_refresh_before_interval() {
        let mut src = CommandSource::new("echo first".to_string(), 600_000);
        let _ = src.capture(80, 24).unwrap(); // first capture triggers refresh
        assert!(!src.should_refresh()); // should not need refresh yet
    }

    #[test]
    fn test_command_source_has_no_custom_render() {
        let src = CommandSource::new("echo".to_string(), 5000);
        assert!(!src.has_custom_render());
    }
}
