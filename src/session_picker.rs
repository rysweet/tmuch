use crate::azlin_integration::{self, AzlinConfig};
use crate::source::ssh_tmux::RemoteConfig;
use crate::tmux::{self, SessionInfo};
use anyhow::Result;
use azlin_ssh::SshPool;
use std::sync::Arc;

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
        pool: &Arc<SshPool>,
        rt: &tokio::runtime::Handle,
    ) -> Result<()> {
        // Local sessions first
        self.sessions = tmux::list_sessions()?;

        // Configured remote hosts
        for remote in remotes {
            let remote = remote.clone();
            let result = rt.block_on(crate::source::ssh_tmux::list_remote_sessions(pool, &remote));
            if let Ok(names) = result {
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
            let result = rt.block_on(azlin_integration::discover_remote_sessions(
                pool,
                azlin_config.resource_group.as_deref(),
            ));
            if let Ok(remote_sessions) = result {
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
