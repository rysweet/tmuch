use super::{ContentSource, PaneSpec};
use crate::tmux;
use anyhow::Result;

pub struct LocalTmuxSource {
    session_name: String,
    owned: bool,
    create_cmd: Option<String>,
}

impl LocalTmuxSource {
    pub fn attach(session_name: String) -> Self {
        Self {
            session_name,
            owned: false,
            create_cmd: None,
        }
    }

    pub fn create(session_name: String, command: Option<&str>) -> Result<Self> {
        tmux::create_session(&session_name, command)?;
        Ok(Self {
            create_cmd: command.map(|s| s.to_string()),
            session_name,
            owned: true,
        })
    }
}

impl ContentSource for LocalTmuxSource {
    fn capture(&mut self, width: u16, height: u16) -> Result<String> {
        tmux::capture_pane(&self.session_name, width, height)
    }

    fn send_keys(&mut self, keys: &str) -> Result<()> {
        tmux::send_keys(&self.session_name, keys)
    }

    fn name(&self) -> &str {
        &self.session_name
    }

    fn source_label(&self) -> &str {
        "local"
    }

    fn is_interactive(&self) -> bool {
        true
    }

    fn cleanup(&mut self) {
        if self.owned {
            let _ = tmux::kill_session(&self.session_name);
        }
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::LocalTmux {
            session: self.session_name.clone(),
            create_cmd: self.create_cmd.clone(),
        }
    }
}
