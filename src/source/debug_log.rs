//! Debug log pane — captures tmuch's own log messages into a viewable pane.

use super::{ContentSource, PaneSpec};
use anyhow::Result;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

const MAX_LOG_LINES: usize = 1000;

/// Global log buffer shared between the `debug_log()` function and `DebugLogSource`.
static LOG_BUFFER: OnceLock<Arc<Mutex<VecDeque<String>>>> = OnceLock::new();

fn global_buffer() -> &'static Arc<Mutex<VecDeque<String>>> {
    LOG_BUFFER.get_or_init(|| Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES))))
}

/// Push a message into the global debug log buffer.
pub fn debug_log(msg: &str) {
    let buf = global_buffer();
    if let Ok(mut guard) = buf.lock() {
        if guard.len() >= MAX_LOG_LINES {
            guard.pop_front();
        }
        guard.push_back(msg.to_string());
    }
}

/// Convenience macro-style helper: formats and logs.
#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {
        $crate::source::debug_log::debug_log(&format!($($arg)*))
    };
}

pub struct DebugLogSource;

impl DebugLogSource {
    pub fn new() -> Self {
        // Ensure the buffer is initialized
        let _ = global_buffer();
        debug_log("debug pane opened");
        Self
    }
}

impl ContentSource for DebugLogSource {
    fn capture(&mut self, _width: u16, height: u16) -> Result<String> {
        let buf = global_buffer();
        let guard = buf.lock().unwrap_or_else(|e| e.into_inner());
        let n = height as usize;
        let lines: Vec<&String> = guard.iter().rev().take(n).collect::<Vec<_>>();
        // Reverse so newest is at the bottom
        let mut output = String::new();
        for line in lines.into_iter().rev() {
            output.push_str(line);
            output.push('\n');
        }
        Ok(output)
    }

    fn send_keys(&mut self, _keys: &str) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "debug"
    }

    fn source_label(&self) -> &str {
        "debug"
    }

    fn is_interactive(&self) -> bool {
        false
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::Plugin {
            plugin_name: "debug".into(),
            config: toml::Value::Table(toml::map::Map::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_log_and_capture() {
        debug_log("test message 1");
        debug_log("test message 2");
        let mut src = DebugLogSource::new();
        let output = src.capture(80, 24).unwrap();
        assert!(output.contains("test message 1"));
        assert!(output.contains("test message 2"));
    }

    #[test]
    fn test_debug_log_source_metadata() {
        let src = DebugLogSource::new();
        assert_eq!(src.name(), "debug");
        assert_eq!(src.source_label(), "debug");
        assert!(!src.is_interactive());
    }

    #[test]
    fn test_debug_log_buffer_limit() {
        // Push more than MAX_LOG_LINES
        for i in 0..1010 {
            debug_log(&format!("line {}", i));
        }
        let buf = global_buffer();
        let guard = buf.lock().unwrap();
        assert!(guard.len() <= MAX_LOG_LINES);
    }
}
