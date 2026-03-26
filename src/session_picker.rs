use crate::azlin_integration::{self, AzlinConfig};
use crate::source::ssh_subprocess::{self, RemoteConfig};
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
        self.clamp_selected();
        Ok(())
    }

    /// Refresh including remote sessions from configured hosts and azlin VMs.
    pub fn refresh_with_remotes(
        &mut self,
        remotes: &[RemoteConfig],
        azlin_config: &AzlinConfig,
    ) -> Result<()> {
        // Local sessions first
        self.sessions = tmux::list_sessions()?;

        // Configured remote hosts (via SSH subprocess)
        for remote in remotes {
            if let Ok(names) = ssh_subprocess::list_remote_sessions(remote) {
                for name in names {
                    self.sessions.push(SessionInfo {
                        name,
                        attached: false,
                        windows: 0,
                        host: Some(remote.name.clone()),
                    });
                }
            }
        }

        // Azlin VM discovery (if enabled)
        if azlin_config.enabled {
            if let Ok(remote_sessions) = azlin_integration::discover_remote_sessions_sync(
                azlin_config.resource_group.as_deref(),
            ) {
                self.sessions.extend(remote_sessions);
            }
        }

        self.clamp_selected();
        Ok(())
    }

    fn clamp_selected(&mut self) {
        if self.selected >= self.sessions.len() && !self.sessions.is_empty() {
            self.selected = self.sessions.len() - 1;
        }
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
