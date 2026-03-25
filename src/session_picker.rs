use crate::source::ssh_tmux::RemoteConfig;
use crate::tmux::{self, SessionInfo};
use anyhow::Result;

pub struct SessionPicker {
    pub sessions: Vec<SessionInfo>,
    pub selected: usize,
}

impl SessionPicker {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected: 0,
        }
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.sessions = tmux::list_sessions()?;
        if self.selected >= self.sessions.len() && !self.sessions.is_empty() {
            self.selected = self.sessions.len() - 1;
        }
        Ok(())
    }

    /// Refresh including remote sessions from configured hosts.
    pub fn refresh_with_remotes(
        &mut self,
        remotes: &[RemoteConfig],
        rt: &tokio::runtime::Handle,
    ) -> Result<()> {
        // Local sessions first
        self.sessions = tmux::list_sessions()?;

        // Remote sessions (fetch in parallel via tokio)
        for remote in remotes {
            let remote = remote.clone();
            let result = rt.block_on(crate::source::ssh_tmux::list_remote_sessions(&remote));
            match result {
                Ok(names) => {
                    for name in names {
                        self.sessions.push(SessionInfo {
                            name,
                            attached: false,
                            windows: 0,
                            host: Some(remote.name.clone()),
                        });
                    }
                }
                Err(_) => {
                    // Skip unreachable hosts silently in the picker
                }
            }
        }

        if self.selected >= self.sessions.len() && !self.sessions.is_empty() {
            self.selected = self.sessions.len() - 1;
        }
        Ok(())
    }

    pub fn select_next(&mut self) {
        if !self.sessions.is_empty() {
            self.selected = (self.selected + 1) % self.sessions.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.sessions.is_empty() {
            self.selected = if self.selected == 0 {
                self.sessions.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn confirm(&self) -> Option<&SessionInfo> {
        self.sessions.get(self.selected)
    }
}
