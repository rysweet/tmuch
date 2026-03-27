//! Pane creation and management operations for App.

use crate::app::App;
use crate::source::command::CommandSource;
use crate::source::local_tmux::LocalTmuxSource;
use crate::source::ssh_subprocess::RemoteConfig;
use crate::source::tail::TailSource;
use crate::source::PaneSpec;
use crate::tmux;
use anyhow::Result;

/// Add a pane from a PaneSpec.
pub fn add_from_spec(app: &mut App, spec: &PaneSpec) -> Result<()> {
    match spec {
        PaneSpec::LocalTmux {
            session,
            create_cmd,
        } => {
            if tmux::session_exists(session) {
                app.pane_manager
                    .add(Box::new(LocalTmuxSource::attach(session.clone())));
            } else if let Some(cmd) = create_cmd {
                let source = LocalTmuxSource::create(session.clone(), Some(cmd))?;
                app.pane_manager.add(Box::new(source));
            } else {
                let source = LocalTmuxSource::create(session.clone(), None)?;
                app.pane_manager.add(Box::new(source));
            }
        }
        PaneSpec::Command {
            command,
            interval_ms,
        } => {
            let source = CommandSource::new(command.clone(), *interval_ms);
            app.pane_manager.add(Box::new(source));
        }
        PaneSpec::Tail { path } => {
            let source = TailSource::new(path)?;
            app.pane_manager.add(Box::new(source));
        }
        PaneSpec::Http { url, interval_ms } => {
            let source = crate::source::http::HttpSource::new(url.clone(), *interval_ms);
            app.pane_manager.add(Box::new(source));
        }
        PaneSpec::Plugin {
            plugin_name,
            config,
        } => {
            if let Some(source) = app.plugin_registry.create(plugin_name, config.clone()) {
                app.pane_manager.add(source);
            } else {
                anyhow::bail!("Unknown plugin '{}'", plugin_name);
            }
        }
        PaneSpec::Remote {
            remote_name,
            session,
        } => {
            let remote = app
                .config
                .remote
                .iter()
                .find(|r| r.name == *remote_name)
                .ok_or_else(|| anyhow::anyhow!("Remote '{}' not found in config", remote_name))?
                .clone();
            let source =
                crate::source::ssh_subprocess::from_remote_config(&remote, session.clone());
            app.pane_manager.add(Box::new(source));
        }
    }
    Ok(())
}

/// Attach a remote tmux session by parsing user@host:session syntax.
pub fn attach_remote(app: &mut App, host_session: &str) -> Result<()> {
    let (user_host, session) = host_session
        .rsplit_once(':')
        .ok_or_else(|| anyhow::anyhow!("Remote format: [user@]host:session"))?;

    let (user, host) = if let Some((u, h)) = user_host.split_once('@') {
        (u.to_string(), h.to_string())
    } else {
        ("azureuser".to_string(), user_host.to_string())
    };

    let remote = app
        .config
        .remote
        .iter()
        .find(|r| r.host == host || r.name == host)
        .cloned()
        .unwrap_or(RemoteConfig {
            name: host.clone(),
            host: host.clone(),
            user,
            key: None,
            port: 22,
            poll_interval_ms: 500,
            bastion: None,
        });

    let source = crate::source::ssh_subprocess::from_remote_config(&remote, session.to_string());
    app.pane_manager.add(Box::new(source));
    Ok(())
}

/// Add a remote session pane by host name.
pub fn add_remote_session_pane(app: &mut App, host: &str, session_name: &str) {
    // Try configured remotes first, then discovered azlin remotes
    let remote = app
        .config
        .remote
        .iter()
        .find(|r| r.name == host)
        .cloned()
        .or_else(|| app.discovered_remotes.get(host).cloned());

    if let Some(remote) = remote {
        // For bastion VMs, the session_name might be a placeholder like "devo (bastion)".
        // In that case, try to connect to a default tmux session.
        let actual_session = if session_name.contains("(bastion)")
            || session_name.contains("(no sessions)")
            || session_name.contains("(unreachable)")
        {
            "azlin".to_string() // default tmux session name
        } else {
            session_name.to_string()
        };

        crate::dlog!("Connecting to {}:{}", host, actual_session);
        let source = crate::source::ssh_subprocess::from_remote_config(&remote, actual_session);
        app.pane_manager.add(Box::new(source));
    } else {
        crate::dlog!("No remote config found for host '{}'", host);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_attach_remote_parses_user_host_session() {
        // Test the parsing logic of attach_remote without actually connecting
        let input = "admin@myhost.com:mysession";
        let (user_host, session) = input.rsplit_once(':').unwrap();
        assert_eq!(session, "mysession");
        let (user, host) = user_host.split_once('@').unwrap();
        assert_eq!(user, "admin");
        assert_eq!(host, "myhost.com");
    }

    #[test]
    fn test_attach_remote_default_user() {
        let input = "myhost.com:session1";
        let (user_host, session) = input.rsplit_once(':').unwrap();
        assert_eq!(session, "session1");
        let (user, host) = if let Some((u, h)) = user_host.split_once('@') {
            (u.to_string(), h.to_string())
        } else {
            ("azureuser".to_string(), user_host.to_string())
        };
        assert_eq!(user, "azureuser");
        assert_eq!(host, "myhost.com");
    }

    #[test]
    fn test_attach_remote_missing_colon() {
        let input = "nocolon";
        assert!(input.rsplit_once(':').is_none());
    }
}
