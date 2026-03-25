pub mod command;
pub mod local_tmux;
pub mod tail;

use anyhow::Result;

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
}

fn default_interval() -> u64 {
    5000
}

/// Parse a `-n` argument into a content source.
/// Supports prefixes: `watch:cmd:interval`, `tail:path`, or plain tmux command.
pub fn parse_new_arg(arg: &str) -> NewPaneRequest {
    if let Some(rest) = arg.strip_prefix("tail:") {
        NewPaneRequest::Tail {
            path: rest.to_string(),
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
}
