pub mod clock;
pub mod command;
pub mod debug_log;
pub mod http;
pub mod local_tmux;
pub mod registry;
pub mod settings;
mod settings_render;
pub mod snake;
pub mod sparkline_monitor;
pub mod ssh_subprocess;
pub mod sysinfo;
pub mod tail;
pub mod weather;

use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// A content source that can provide terminal output for a pane.
pub trait ContentSource: Send {
    /// Capture current visible content, sized to fit the given dimensions.
    fn capture(&mut self, width: u16, height: u16) -> Result<String>;

    /// Send keystrokes to the source (for interactive sources).
    fn send_keys(&mut self, keys: &str) -> Result<()>;

    /// Display name for the pane title.
    fn name(&self) -> &str;

    /// Source type label (e.g., "local", "ssh:host", "cmd", "tail").
    fn source_label(&self) -> &str;

    /// Whether this source accepts keyboard input.
    fn is_interactive(&self) -> bool;

    /// Clean up resources when the pane is dropped.
    fn cleanup(&mut self) {}

    /// Serialize back to a layout spec for saving.
    fn to_spec(&self) -> PaneSpec;

    /// Whether this source renders custom widgets (instead of text capture).
    fn has_custom_render(&self) -> bool {
        false
    }

    /// Render custom widgets directly into the buffer.
    /// Only called if has_custom_render() returns true.
    fn render(&self, _area: Rect, _buf: &mut Buffer) {}
}

/// Specification for recreating a pane from a layout file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum PaneSpec {
    #[serde(rename = "local")]
    LocalTmux {
        session: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        create_cmd: Option<String>,
    },
    #[serde(rename = "command")]
    Command {
        command: String,
        #[serde(default = "default_interval")]
        interval_ms: u64,
    },
    #[serde(rename = "tail")]
    Tail { path: String },
    #[serde(rename = "remote")]
    Remote {
        remote_name: String,
        session: String,
    },
    #[serde(rename = "http")]
    Http {
        url: String,
        #[serde(default = "default_http_interval")]
        interval_ms: u64,
    },
    #[serde(rename = "plugin")]
    Plugin {
        plugin_name: String,
        #[serde(default = "default_plugin_config")]
        config: toml::Value,
    },
}

fn default_http_interval() -> u64 {
    5000
}

fn default_interval() -> u64 {
    5000
}

fn default_plugin_config() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

/// Parse a `-n` argument into a content source.
/// Supports prefixes: `watch:cmd:interval`, `tail:path`, `clock:`, or plain tmux command.
pub fn parse_new_arg(arg: &str) -> NewPaneRequest {
    if arg == "debug:" || arg == "debug" {
        return NewPaneRequest::DebugLog;
    }
    if arg == "settings:" || arg == "settings" {
        return NewPaneRequest::Settings;
    }
    if arg == "clock:" || arg == "clock" {
        return NewPaneRequest::Clock;
    }
    if arg == "snake:" || arg == "snake" {
        return NewPaneRequest::Snake;
    }
    if let Some(rest) = arg.strip_prefix("weather:") {
        // Format: weather:city or weather:city:interval_ms
        let parts: Vec<&str> = rest.rsplitn(2, ':').collect();
        if parts.len() == 2 {
            if let Ok(interval) = parts[0].parse::<u64>() {
                return NewPaneRequest::Weather {
                    city: parts[1].to_string(),
                    interval_ms: interval,
                };
            }
        }
        return NewPaneRequest::Weather {
            city: rest.to_string(),
            interval_ms: 300_000, // 5 minutes default
        };
    }
    if let Some(rest) = arg.strip_prefix("sysinfo:") {
        let interval_ms = if rest.is_empty() {
            2000
        } else {
            rest.parse::<u64>().unwrap_or(2000)
        };
        return NewPaneRequest::SysInfo { interval_ms };
    }
    if arg == "sysinfo" {
        return NewPaneRequest::SysInfo { interval_ms: 2000 };
    }
    if let Some(rest) = arg.strip_prefix("spark:") {
        // Format: spark:command:interval_ms
        let parts: Vec<&str> = rest.rsplitn(2, ':').collect();
        if parts.len() == 2 {
            if let Ok(interval) = parts[0].parse::<u64>() {
                return NewPaneRequest::Sparkline {
                    command: parts[1].to_string(),
                    interval_ms: interval,
                };
            }
        }
        return NewPaneRequest::Sparkline {
            command: rest.to_string(),
            interval_ms: 2000,
        };
    }
    if let Some(rest) = arg.strip_prefix("tail:") {
        NewPaneRequest::Tail {
            path: rest.to_string(),
        }
    } else if let Some(rest) = arg.strip_prefix("http:") {
        // Format: http:url or http:url:interval_ms
        let parts: Vec<&str> = rest.rsplitn(2, ':').collect();
        if parts.len() == 2 {
            if let Ok(interval) = parts[0].parse::<u64>() {
                return NewPaneRequest::Http {
                    url: parts[1].to_string(),
                    interval_ms: interval,
                };
            }
        }
        // URL may contain colons (http://...), so treat entire rest as URL
        NewPaneRequest::Http {
            url: rest.to_string(),
            interval_ms: 5000,
        }
    } else if let Some(rest) = arg.strip_prefix("watch:") {
        // Format: watch:command:interval_ms
        let parts: Vec<&str> = rest.rsplitn(2, ':').collect();
        if parts.len() == 2 {
            if let Ok(interval) = parts[0].parse::<u64>() {
                return NewPaneRequest::Command {
                    command: parts[1].to_string(),
                    interval_ms: interval,
                };
            }
        }
        NewPaneRequest::Command {
            command: rest.to_string(),
            interval_ms: 5000,
        }
    } else {
        NewPaneRequest::TmuxCommand {
            command: arg.to_string(),
        }
    }
}

pub enum NewPaneRequest {
    TmuxCommand { command: String },
    Command { command: String, interval_ms: u64 },
    Tail { path: String },
    Http { url: String, interval_ms: u64 },
    Clock,
    Weather { city: String, interval_ms: u64 },
    SysInfo { interval_ms: u64 },
    Snake,
    Sparkline { command: String, interval_ms: u64 },
    Settings,
    DebugLog,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clock() {
        assert!(matches!(parse_new_arg("clock"), NewPaneRequest::Clock));
        assert!(matches!(parse_new_arg("clock:"), NewPaneRequest::Clock));
    }

    #[test]
    fn test_parse_snake() {
        assert!(matches!(parse_new_arg("snake"), NewPaneRequest::Snake));
    }

    #[test]
    fn test_parse_weather_with_city() {
        match parse_new_arg("weather:London") {
            NewPaneRequest::Weather { city, interval_ms } => {
                assert_eq!(city, "London");
                assert_eq!(interval_ms, 300_000);
            }
            _ => panic!("expected Weather"),
        }
    }

    #[test]
    fn test_parse_weather_with_interval() {
        match parse_new_arg("weather:Paris:60000") {
            NewPaneRequest::Weather { city, interval_ms } => {
                assert_eq!(city, "Paris");
                assert_eq!(interval_ms, 60000);
            }
            _ => panic!("expected Weather"),
        }
    }

    #[test]
    fn test_parse_sysinfo() {
        match parse_new_arg("sysinfo") {
            NewPaneRequest::SysInfo { interval_ms } => assert_eq!(interval_ms, 2000),
            _ => panic!("expected SysInfo"),
        }
    }

    #[test]
    fn test_parse_sysinfo_with_interval() {
        match parse_new_arg("sysinfo:5000") {
            NewPaneRequest::SysInfo { interval_ms } => assert_eq!(interval_ms, 5000),
            _ => panic!("expected SysInfo"),
        }
    }

    #[test]
    fn test_parse_tail() {
        match parse_new_arg("tail:/var/log/syslog") {
            NewPaneRequest::Tail { path } => assert_eq!(path, "/var/log/syslog"),
            _ => panic!("expected Tail"),
        }
    }

    #[test]
    fn test_parse_watch() {
        match parse_new_arg("watch:df -h:3000") {
            NewPaneRequest::Command {
                command,
                interval_ms,
            } => {
                assert_eq!(command, "df -h");
                assert_eq!(interval_ms, 3000);
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn test_parse_http() {
        match parse_new_arg("http:example.com/api") {
            NewPaneRequest::Http { url, interval_ms } => {
                assert_eq!(url, "example.com/api");
                assert_eq!(interval_ms, 5000);
            }
            _ => panic!("expected Http"),
        }
    }

    #[test]
    fn test_parse_plain_command() {
        match parse_new_arg("htop") {
            NewPaneRequest::TmuxCommand { command } => assert_eq!(command, "htop"),
            _ => panic!("expected TmuxCommand"),
        }
    }

    #[test]
    fn test_parse_settings() {
        assert!(matches!(
            parse_new_arg("settings"),
            NewPaneRequest::Settings
        ));
    }

    #[test]
    fn test_parse_settings_colon() {
        assert!(matches!(
            parse_new_arg("settings:"),
            NewPaneRequest::Settings
        ));
    }

    #[test]
    fn test_parse_snake_colon() {
        assert!(matches!(parse_new_arg("snake:"), NewPaneRequest::Snake));
    }

    #[test]
    fn test_parse_sysinfo_colon() {
        match parse_new_arg("sysinfo:") {
            NewPaneRequest::SysInfo { interval_ms } => assert_eq!(interval_ms, 2000),
            _ => panic!("expected SysInfo"),
        }
    }

    #[test]
    fn test_parse_spark() {
        match parse_new_arg("spark:echo 1:500") {
            NewPaneRequest::Sparkline {
                command,
                interval_ms,
            } => {
                assert_eq!(command, "echo 1");
                assert_eq!(interval_ms, 500);
            }
            _ => panic!("expected Sparkline"),
        }
    }

    #[test]
    fn test_parse_spark_no_interval() {
        match parse_new_arg("spark:echo 1") {
            NewPaneRequest::Sparkline {
                command,
                interval_ms,
            } => {
                assert_eq!(command, "echo 1");
                assert_eq!(interval_ms, 2000);
            }
            _ => panic!("expected Sparkline"),
        }
    }

    #[test]
    fn test_parse_http_with_interval() {
        match parse_new_arg("http:localhost:3000") {
            NewPaneRequest::Http { url, interval_ms } => {
                assert_eq!(url, "localhost");
                assert_eq!(interval_ms, 3000);
            }
            _ => panic!("expected Http"),
        }
    }

    #[test]
    fn test_parse_watch_no_interval() {
        match parse_new_arg("watch:uptime") {
            NewPaneRequest::Command {
                command,
                interval_ms,
            } => {
                assert_eq!(command, "uptime");
                assert_eq!(interval_ms, 5000);
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn test_parse_debug() {
        assert!(matches!(parse_new_arg("debug"), NewPaneRequest::DebugLog));
        assert!(matches!(parse_new_arg("debug:"), NewPaneRequest::DebugLog));
    }
}
