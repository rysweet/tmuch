use crate::app::App;
use crate::ipc::IpcCommand;
use crate::layout::SplitDirection;
use crate::source::command::CommandSource;
use crate::source::local_tmux::LocalTmuxSource;
use crate::source::PaneSpec;
use crate::tmux;

/// Handle an IPC command, returning a JSON response string.
pub fn handle_ipc(app: &mut App, cmd: IpcCommand) -> String {
    match cmd {
        IpcCommand::ListPanes => {
            let panes: Vec<serde_json::Value> = app
                .pane_manager
                .panes()
                .iter()
                .map(|(id, p)| {
                    serde_json::json!({
                        "id": id,
                        "name": p.name(),
                        "source": p.source_label(),
                    })
                })
                .collect();
            serde_json::json!({ "ok": true, "panes": panes }).to_string()
        }
        IpcCommand::AddPane(spec) => match app.add_from_spec(&spec) {
            Ok(()) => serde_json::json!({ "ok": true }).to_string(),
            Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }).to_string(),
        },
        IpcCommand::RemovePane(id) => {
            app.pane_manager.remove(id);
            serde_json::json!({ "ok": true }).to_string()
        }
        IpcCommand::FocusPane(id) => {
            app.pane_manager.focus_id(id);
            serde_json::json!({ "ok": true }).to_string()
        }
        IpcCommand::Split { direction, spec } => {
            let dir = if direction == "horizontal" {
                SplitDirection::Horizontal
            } else {
                SplitDirection::Vertical
            };
            let result = match &spec {
                PaneSpec::LocalTmux {
                    session,
                    create_cmd,
                } => {
                    if tmux::session_exists(session) {
                        Ok(Box::new(LocalTmuxSource::attach(session.clone()))
                            as Box<dyn crate::source::ContentSource>)
                    } else {
                        LocalTmuxSource::create(session.clone(), create_cmd.as_deref())
                            .map(|s| Box::new(s) as Box<dyn crate::source::ContentSource>)
                    }
                }
                PaneSpec::Command {
                    command,
                    interval_ms,
                } => Ok(Box::new(CommandSource::new(command.clone(), *interval_ms))
                    as Box<dyn crate::source::ContentSource>),
                PaneSpec::Plugin {
                    plugin_name,
                    config,
                } => app
                    .plugin_registry
                    .create(plugin_name, config.clone())
                    .ok_or_else(|| anyhow::anyhow!("Unknown plugin")),
                _ => Err(anyhow::anyhow!("Unsupported spec for split")),
            };
            match result {
                Ok(source) => {
                    app.pane_manager.split_focused(dir, source);
                    serde_json::json!({ "ok": true }).to_string()
                }
                Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }).to_string(),
            }
        }
        IpcCommand::Maximize(id) => {
            app.pane_manager.focus_id(id);
            app.pane_manager.toggle_maximize();
            serde_json::json!({ "ok": true }).to_string()
        }
        IpcCommand::SendKeys { id, keys } => {
            if let Some(pane) = app.pane_manager.get_mut(id) {
                let _ = pane.source.send_keys(&keys);
            }
            serde_json::json!({ "ok": true }).to_string()
        }
        IpcCommand::Quit => {
            app.should_quit = true;
            serde_json::json!({ "ok": true }).to_string()
        }
    }
}
