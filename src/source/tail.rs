use super::{ContentSource, PaneSpec};
use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const MAX_LINES: usize = 1000;

/// Tails a file using a background `tail -f` process.
pub struct TailSource {
    path: PathBuf,
    buffer: Arc<Mutex<VecDeque<String>>>,
    child: Option<Child>,
}

impl TailSource {
    pub fn new(path: &str) -> Result<Self> {
        let path = PathBuf::from(path);
        let buffer = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LINES)));

        let mut child = Command::new("tail")
            .args(["-n", "50", "-f"])
            .arg(&path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to spawn tail process")?;

        let stdout = child.stdout.take().context("No stdout from tail")?;
        let buf_clone = Arc::clone(&buffer);

        std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                let mut buf = buf_clone.lock().unwrap_or_else(|e| e.into_inner());
                if buf.len() >= MAX_LINES {
                    buf.pop_front();
                }
                buf.push_back(line);
            }
        });

        Ok(Self {
            path,
            buffer,
            child: Some(child),
        })
    }
}

impl ContentSource for TailSource {
    fn capture(&mut self, _width: u16, height: u16) -> Result<String> {
        let buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        let lines: Vec<&str> = buf
            .iter()
            .rev()
            .take(height as usize)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|s| s.as_str())
            .collect();
        Ok(lines.join("\n"))
    }

    fn send_keys(&mut self, _keys: &str) -> Result<()> {
        Ok(()) // not interactive
    }

    fn name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("tail")
    }

    fn source_label(&self) -> &str {
        "tail"
    }

    fn is_interactive(&self) -> bool {
        false
    }

    fn cleanup(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::Tail {
            path: self.path.to_string_lossy().to_string(),
        }
    }
}

impl Drop for TailSource {
    fn drop(&mut self) {
        self.cleanup();
    }
}
